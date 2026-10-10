//! The receive mix: one ring per remote speaker, summed in the output callback.
//!
//! Two halves. [`MixerRt`] lives in the real-time callback and only touches `rtrb` rings and
//! atomics. [`MixerControl`] is everything else: handing rings to speakers, volume, mute,
//! levels. The number of speakers is fixed up front so the callback never allocates.

use rtrb::{Consumer, Producer, RingBuffer};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use transport::{FrameSink, BLOCK};

/// Speakers mixed at once. A full channel is 19 others.
pub const SLOTS: usize = 24;
/// Audio that must be queued before a speaker starts (again): at least 20 ms, and always
/// one network frame more than the device asks for in one go — a cushion that is smaller
/// than the callback runs dry on every callback.
pub const PREFILL: usize = 2 * BLOCK;
/// More than this queued means the speaker's clock runs ahead of our output device, or
/// playout stalled: the callback jumps forward to the prefill level. 100 ms.
pub const HIGH_WATER: usize = 10 * BLOCK;
/// Ring size per speaker: 200 ms. A frame that does not fit is dropped by the sink.
const RING: usize = 20 * BLOCK;

#[derive(Default)]
struct Slot {
    /// Linear gain as `f32` bits.
    gain: AtomicU32,
    muted: AtomicBool,
    /// Set when the slot gets a new speaker: the callback discards what the last one left.
    flush: AtomicBool,
    /// RMS of the last received frame as `f32` bits, 0..1. Written by the sink.
    level: AtomicU32,
}

#[derive(Default)]
struct Shared {
    slots: [Slot; SLOTS],
    deafened: AtomicBool,
    underruns: AtomicU32,
    skips: AtomicU32,
    dropped_frames: AtomicU32,
}

/// Volume and mute for one remote person, remembered across their reconnects.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct PeerMix {
    pub gain: f32,
    pub muted: bool,
}

impl Default for PeerMix {
    fn default() -> Self {
        Self {
            gain: 1.0,
            muted: false,
        }
    }
}

struct Routing {
    free: Vec<(usize, Producer<i16>)>,
    /// Who currently speaks through which slot.
    active: HashMap<String, usize>,
    peers: HashMap<String, PeerMix>,
}

pub struct MixerControl {
    shared: Arc<Shared>,
    routing: Mutex<Routing>,
}

pub struct MixerRt {
    shared: Arc<Shared>,
    rings: Vec<Consumer<i16>>,
    playing: [bool; SLOTS],
}

pub fn mixer() -> (Arc<MixerControl>, MixerRt) {
    let shared = Arc::new(Shared::default());
    let mut free = Vec::with_capacity(SLOTS);
    let mut rings = Vec::with_capacity(SLOTS);
    for index in 0..SLOTS {
        let (producer, consumer) = RingBuffer::new(RING);
        shared.slots[index].gain.store(1f32.to_bits(), Relaxed);
        free.push((index, producer));
        rings.push(consumer);
    }
    free.reverse(); // hand out slot 0 first
    let control = Arc::new(MixerControl {
        shared: shared.clone(),
        routing: Mutex::new(Routing {
            free,
            active: HashMap::new(),
            peers: HashMap::new(),
        }),
    });
    let rt = MixerRt {
        shared,
        rings,
        playing: [false; SLOTS],
    };
    (control, rt)
}

impl MixerRt {
    /// Adds every speaker into `out` (mono, -1..1), which must come in zeroed, and clamps
    /// the sum. Real-time safe: no allocation, no lock, no system call.
    pub fn render(&mut self, out: &mut [f32]) {
        let wanted = out.len();
        let deafened = self.shared.deafened.load(Relaxed);
        for (index, ring) in self.rings.iter_mut().enumerate() {
            let slot = &self.shared.slots[index];
            let playing = &mut self.playing[index];
            if slot.flush.swap(false, Relaxed) {
                discard(ring, ring.slots());
                *playing = false;
            }
            let mut queued = ring.slots();
            let cushion = PREFILL.max(wanted + BLOCK);
            if !*playing {
                if queued < cushion {
                    continue;
                }
                *playing = true;
            }
            if queued > HIGH_WATER.max(2 * cushion) {
                discard(ring, queued - cushion);
                queued = ring.slots();
                self.shared.skips.fetch_add(1, Relaxed);
            }
            let take = queued.min(wanted);
            if let Ok(chunk) = ring.read_chunk(take) {
                if !deafened && !slot.muted.load(Relaxed) {
                    let gain = f32::from_bits(slot.gain.load(Relaxed)) / f32::from(i16::MAX);
                    let (head, tail) = chunk.as_slices();
                    for (o, s) in out.iter_mut().zip(head.iter().chain(tail)) {
                        *o += f32::from(*s) * gain;
                    }
                }
                chunk.commit_all();
            }
            if take < wanted {
                *playing = false;
                self.shared.underruns.fetch_add(1, Relaxed);
            }
        }
        for o in out.iter_mut() {
            *o = o.clamp(-1.0, 1.0);
        }
    }
}

fn discard(ring: &mut Consumer<i16>, n: usize) {
    if let Ok(chunk) = ring.read_chunk(n.min(ring.slots())) {
        chunk.commit_all();
    }
}

impl MixerControl {
    /// A sink for one remote speaker's frames, or `None` when all slots are taken. Dropping
    /// the sink frees the slot.
    pub fn speaker(self: &Arc<Self>, identity: &str) -> Option<SpeakerSink> {
        let mut routing = self.routing.lock().unwrap();
        let (index, producer) = routing.free.pop()?;
        let mix = routing.peers.get(identity).copied().unwrap_or_default();
        let slot = &self.shared.slots[index];
        slot.gain.store(mix.gain.to_bits(), Relaxed);
        slot.muted.store(mix.muted, Relaxed);
        slot.level.store(0, Relaxed);
        slot.flush.store(true, Relaxed);
        routing.active.insert(identity.to_string(), index);
        Some(SpeakerSink {
            mixer: self.clone(),
            identity: identity.to_string(),
            slot: index,
            producer: Some(producer),
        })
    }

    /// Changes one person's volume or mute — now, and for the next time they speak.
    pub fn set_peer(&self, identity: &str, change: impl FnOnce(&mut PeerMix)) {
        let mut routing = self.routing.lock().unwrap();
        let mix = routing.peers.entry(identity.to_string()).or_default();
        change(mix);
        let mix = *mix;
        if let Some(&index) = routing.active.get(identity) {
            self.shared.slots[index]
                .gain
                .store(mix.gain.to_bits(), Relaxed);
            self.shared.slots[index].muted.store(mix.muted, Relaxed);
        }
    }

    pub fn set_deafened(&self, deafened: bool) {
        self.shared.deafened.store(deafened, Relaxed);
    }

    /// Current level of everyone speaking, as (identity, RMS 0..1).
    pub fn levels(&self) -> Vec<(String, f32)> {
        let routing = self.routing.lock().unwrap();
        routing
            .active
            .iter()
            .map(|(identity, &index)| {
                let level = f32::from_bits(self.shared.slots[index].level.load(Relaxed));
                (identity.clone(), level)
            })
            .collect()
    }

    /// Times a speaker's ring ran dry while playing.
    pub fn underruns(&self) -> u32 {
        self.shared.underruns.load(Relaxed)
    }

    /// Times the callback jumped forward because a ring was over the high-water mark.
    pub fn skips(&self) -> u32 {
        self.shared.skips.load(Relaxed)
    }

    /// Frames a sink could not queue because its ring was full.
    pub fn dropped_frames(&self) -> u32 {
        self.shared.dropped_frames.load(Relaxed)
    }
}

/// The producer side of one slot, owned by whoever delivers that speaker's audio.
pub struct SpeakerSink {
    mixer: Arc<MixerControl>,
    identity: String,
    slot: usize,
    producer: Option<Producer<i16>>,
}

impl FrameSink for SpeakerSink {
    fn frame(&mut self, samples: &[i16]) {
        let shared = &self.mixer.shared;
        let energy: f64 = samples.iter().map(|s| f64::from(*s).powi(2)).sum();
        let rms = (energy / samples.len().max(1) as f64).sqrt() / f64::from(i16::MAX);
        shared.slots[self.slot]
            .level
            .store((rms as f32).to_bits(), Relaxed);
        let producer = self.producer.as_mut().expect("present until drop");
        if producer.push_entire_slice(samples).is_err() {
            shared.dropped_frames.fetch_add(1, Relaxed);
        }
    }
}

impl Drop for SpeakerSink {
    fn drop(&mut self) {
        let mut routing = self.mixer.routing.lock().unwrap();
        // The same person may already speak through a newer slot (resubscribe).
        if routing.active.get(&self.identity) == Some(&self.slot) {
            routing.active.remove(&self.identity);
        }
        self.mixer.shared.slots[self.slot].level.store(0, Relaxed);
        if let Some(producer) = self.producer.take() {
            routing.free.push((self.slot, producer));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const HALF: i16 = i16::MAX / 2;

    fn render(rt: &mut MixerRt, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        rt.render(&mut out);
        out
    }

    #[test]
    fn a_speaker_starts_after_the_prefill_and_not_before() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK); // consumes the flush of a fresh slot
        anna.frame(&[HALF; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "10 ms queued: wait"
        );
        anna.frame(&[HALF; BLOCK]);
        let out = render(&mut rt, BLOCK);
        assert!(out.iter().all(|s| (*s - 0.5).abs() < 1e-3));
        assert_eq!(control.underruns(), 0);
    }

    #[test]
    fn speakers_are_summed_with_their_gain_and_the_sum_is_clamped() {
        let (control, mut rt) = mixer();
        control.set_peer("ben", |m| m.gain = 0.5);
        let mut anna = control.speaker("anna").unwrap();
        let mut ben = control.speaker("ben").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..2 {
            anna.frame(&[HALF; BLOCK]);
            ben.frame(&[HALF; BLOCK]);
        }
        assert!(render(&mut rt, BLOCK)
            .iter()
            .all(|s| (*s - 0.75).abs() < 1e-3));
        control.set_peer("ben", |m| m.gain = 4.0);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 1.0), "clamped");
    }

    #[test]
    fn peer_mute_and_deafen_silence_but_keep_draining() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..4 {
            anna.frame(&[HALF; BLOCK]);
        }
        control.set_peer("anna", |m| m.muted = true);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        control.set_peer("anna", |m| m.muted = false);
        control.set_deafened(true);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        control.set_deafened(false);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s > 0.4));
    }

    #[test]
    fn running_dry_counts_once_and_waits_for_a_new_prefill() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..3 {
            anna.frame(&[HALF; BLOCK]);
        }
        render(&mut rt, BLOCK); // playing now, two blocks left
        let out = render(&mut rt, 3 * BLOCK);
        assert!(out[..2 * BLOCK].iter().all(|s| *s > 0.4));
        assert!(out[2 * BLOCK..].iter().all(|s| *s == 0.0));
        assert_eq!(control.underruns(), 1);
        anna.frame(&[HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s == 0.0));
        assert_eq!(control.underruns(), 1, "not playing: no second underrun");
    }

    #[test]
    fn a_backlog_over_the_high_water_mark_is_skipped() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        for _ in 0..12 {
            anna.frame(&[HALF; BLOCK]);
        }
        render(&mut rt, BLOCK);
        assert_eq!(control.skips(), 1);
        // It jumped to the cushion (two blocks) and played one: one block is left.
        render(&mut rt, BLOCK);
        assert_eq!(control.underruns(), 0);
        render(&mut rt, BLOCK);
        assert_eq!(control.underruns(), 1);
    }

    /// A device that asks for more than the 20 ms prefill at a time (the Focusrite here:
    /// 1056 frames) must not run dry on every callback.
    #[test]
    fn a_callback_larger_than_the_prefill_does_not_underrun() {
        const CALLBACK: usize = 1056;
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, CALLBACK);
        let mut heard = 0;
        // 300 callbacks ≈ 6.6 s; network frames arrive every 480 samples of that time.
        for callback in 0..300 {
            let (before, after) = (
                callback * CALLBACK / BLOCK,
                (callback + 1) * CALLBACK / BLOCK,
            );
            for _ in before..after {
                anna.frame(&[HALF; BLOCK]);
            }
            heard += render(&mut rt, CALLBACK)
                .iter()
                .filter(|s| **s > 0.4)
                .count();
        }
        assert_eq!(control.underruns(), 0);
        assert_eq!(control.skips(), 0);
        assert!(
            heard > 290 * CALLBACK,
            "plays almost from the start: {heard}"
        );
    }

    #[test]
    fn a_full_ring_drops_the_frame_and_counts_it() {
        let (control, _rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        for _ in 0..21 {
            anna.frame(&[HALF; BLOCK]);
        }
        assert_eq!(control.dropped_frames(), 1);
    }

    #[test]
    fn a_reused_slot_does_not_play_the_previous_speaker() {
        let (control, mut rt) = mixer();
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[HALF; BLOCK]); // below the prefill: stays in the ring
        drop(anna);
        let mut ben = control.speaker("ben").unwrap();
        ben.frame(&[-HALF; BLOCK]);
        assert!(
            render(&mut rt, BLOCK).iter().all(|s| *s == 0.0),
            "flushed, ben not prefilled"
        );
        ben.frame(&[-HALF; BLOCK]);
        ben.frame(&[-HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK).iter().all(|s| *s < -0.4), "only ben");
    }

    #[test]
    fn slots_come_back_and_levels_follow_the_frames() {
        let (control, _rt) = mixer();
        let sinks: Vec<_> = (0..SLOTS)
            .map(|i| control.speaker(&format!("p{i}")).unwrap())
            .collect();
        assert!(control.speaker("one-too-many").is_none());
        drop(sinks);
        let mut anna = control.speaker("anna").unwrap();
        anna.frame(&[HALF; BLOCK]);
        let levels = control.levels();
        assert_eq!(levels.len(), 1);
        assert_eq!(levels[0].0, "anna");
        assert!((levels[0].1 - 0.5).abs() < 1e-3);
        drop(anna);
        assert!(control.levels().is_empty());
    }

    #[test]
    fn volume_survives_a_resubscribe() {
        let (control, mut rt) = mixer();
        control.set_peer("anna", |m| m.gain = 0.5);
        drop(control.speaker("anna").unwrap());
        let mut anna = control.speaker("anna").unwrap();
        render(&mut rt, BLOCK);
        anna.frame(&[HALF; BLOCK]);
        anna.frame(&[HALF; BLOCK]);
        assert!(render(&mut rt, BLOCK)
            .iter()
            .all(|s| (*s - 0.25).abs() < 1e-3));
    }

    proptest! {
        /// Whatever arrives in whatever rhythm: the output stays in range and nothing panics.
        #[test]
        fn output_stays_in_range(
            steps in prop::collection::vec((0usize..4, 1usize..2000, any::<i16>(), 0f32..8.0), 1..60)
        ) {
            let (control, mut rt) = mixer();
            let mut sinks = vec![control.speaker("a").unwrap(), control.speaker("b").unwrap()];
            for (frames, wanted, sample, gain) in steps {
                control.set_peer("a", |m| m.gain = gain);
                for sink in &mut sinks {
                    for _ in 0..frames {
                        sink.frame(&[sample; BLOCK]);
                    }
                }
                let out = render(&mut rt, wanted);
                prop_assert!(out.iter().all(|s| (-1.0..=1.0).contains(s)));
            }
        }
    }
}
