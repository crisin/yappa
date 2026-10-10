//! Turns the transport's running totals into what the debug panel shows: rates over the
//! last interval next to the totals.

use engine_protocol::NetworkStats;
use transport::NetStats;

/// `previous` is the snapshot `seconds` ago, or `None` for the first one of a connection.
pub fn network_stats(previous: Option<&NetStats>, now: &NetStats, seconds: f32) -> NetworkStats {
    // After a rejoin the totals start again; a "previous" above "now" is from the old link.
    let zero = NetStats::default();
    let before = previous
        .filter(|p| {
            p.packets_sent <= now.packets_sent && p.samples_received <= now.samples_received
        })
        .unwrap_or(&zero);

    let received = now.packets_received.saturating_sub(before.packets_received);
    let lost = now.packets_lost.saturating_sub(before.packets_lost);
    let samples = now.samples_received.saturating_sub(before.samples_received);
    let concealed = now
        .concealed_samples
        .saturating_sub(before.concealed_samples);
    let bytes = now.bytes_sent.saturating_sub(before.bytes_sent);

    NetworkStats {
        rtt_ms: now.rtt_ms as f32,
        send_codec: now.send_codec.clone(),
        send_kbps_recent: if seconds > 0.0 {
            bytes as f32 * 8.0 / 1000.0 / seconds
        } else {
            0.0
        },
        packets_sent: saturate(now.packets_sent),
        uplink_loss_percent: (now.uplink_fraction_lost * 100.0) as f32,
        packets_received: saturate(now.packets_received),
        packets_lost: saturate(now.packets_lost),
        loss_percent_recent: percent(lost, received + lost),
        jitter_ms: now.jitter_ms as f32,
        jitter_buffer_ms: if now.jitter_buffer_emitted > 0 {
            (now.jitter_buffer_delay_s / now.jitter_buffer_emitted as f64 * 1000.0) as f32
        } else {
            0.0
        },
        concealed_percent_recent: percent(concealed, samples),
        concealed_percent_total: percent(now.concealed_samples, now.samples_received),
    }
}

fn percent(part: u64, whole: u64) -> f32 {
    if whole == 0 {
        0.0
    } else {
        (part as f64 / whole as f64 * 100.0) as f32
    }
}

fn saturate(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totals(seconds: u64) -> NetStats {
        NetStats {
            rtt_ms: 12.0,
            send_codec: "audio/opus".into(),
            packets_sent: 50 * seconds,
            bytes_sent: 8_000 * seconds,
            packets_received: 45 * seconds,
            packets_lost: 5 * seconds,
            jitter_buffer_delay_s: 1920.0 * seconds as f64,
            jitter_buffer_emitted: 48_000 * seconds,
            samples_received: 48_000 * seconds,
            concealed_samples: 480 * seconds,
            ..NetStats::default()
        }
    }

    #[test]
    fn rates_cover_the_interval_and_totals_the_connection() {
        let mut now = totals(10);
        now.packets_lost += 45; // a bad last second: 50 of 95 lost
        now.concealed_samples += 24_000 - 480;
        let stats = network_stats(Some(&totals(9)), &now, 1.0);
        assert_eq!(stats.send_kbps_recent, 64.0);
        assert!((stats.loss_percent_recent - 52.63).abs() < 0.01);
        assert!((stats.concealed_percent_recent - 50.0).abs() < 0.01);
        assert!((stats.concealed_percent_total - 5.9).abs() < 0.01);
        assert_eq!(stats.jitter_buffer_ms, 40.0);
        assert_eq!(stats.packets_received, 450);
    }

    #[test]
    fn the_first_snapshot_and_a_fresh_link_count_from_zero() {
        let first = network_stats(None, &totals(1), 1.0);
        assert_eq!(first.send_kbps_recent, 64.0);
        assert_eq!(first.loss_percent_recent, 10.0);
        let after_rejoin = network_stats(Some(&totals(500)), &totals(1), 1.0);
        assert_eq!(after_rejoin, first);
    }

    #[test]
    fn nothing_received_is_zero_not_nan() {
        let stats = network_stats(None, &NetStats::default(), 1.0);
        assert_eq!(stats.loss_percent_recent, 0.0);
        assert_eq!(stats.concealed_percent_total, 0.0);
        assert_eq!(stats.jitter_buffer_ms, 0.0);
    }
}
