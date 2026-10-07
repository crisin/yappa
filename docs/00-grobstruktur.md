# yAPPA — Grobstruktur v0

Oct 1, 2026 · @crisin

> yAPPA: Appa aus Avatar, und yappen ist, was wir da tun. Selbstgehosteter Gaming-Voice-Chat als Discord-/Steam-Voice-Ersatz. Fokus: Sprachqualität und Robustheit. Dieses Dokument ist die Planungsgrundlage, die später an Claude Code übergeben wird; es wird iterativ vertieft.

## Ziel und Kernfeatures

yAPPA ersetzt Discord und Steam-Voice für eine feste Gruppe von bis zu 20 Leuten; Sprachqualität und Ausfallsicherheit stehen über allem anderen. Die Einordnung in der Tabelle ist per Dropdown änderbar, so sortieren wir die Features gemeinsam.

| Feature | Einordnung | Anmerkung |
| --- | --- | --- |
| Voice-Channels bis 20 Teilnehmer, Opus 48 kHz | Fix (v1) | Kernstück des Projekts |
| Windows-Client auf Tauri 2 | Fix (v1) | macOS baut aus demselben Code |
| Push-to-Talk global, auch mit Spiel im Vordergrund | Fix (v1) | braucht einen nativen Hotkey-Hook, nicht den Webview |
| Voice Activation mit einstellbarer Schwelle | Fix (v1) |  |
| Noise Reduction (RNNoise-Klasse) | Fix (v1) |  |
| EQ, Noise Gate und Kompressor pro Eingang | Fix (v1) |  |
| Lautstärke und Mute pro User auf Empfangsseite | Fix (v1) |  |
| Reconnect, Opus FEC/RED, TURN-Fallback | Fix (v1) | Robustheit ist der Grund für das Projekt |
| Stimmeffekte (Pitch, Robot, Reverb) | Später | sitzt in derselben DSP-Kette wie EQ und Gate |
| VST3/CLAP-Hosting für eigene Effekte | Später | Spike nötig, siehe Audio-Engine |
| Text-Chat | Später | fällt über die Control-Plane fast gratis ab |
| Screenshare und Video | Offen | LiveKit kann es, Client-Aufwand unklar |
| Ende-zu-Ende-Verschlüsselung | Offen | LiveKit-SDKs bringen E2EE mit |
| macOS-Client offiziell unterstützt | Offen | Build läuft mit, Audio-Pipeline muss auf Mac getestet werden |
| Mobile-Apps | Nicht-Ziel |  |
| Öffentliche Instanz, Multi-Tenant, Federation | Nicht-Ziel | eine Gruppe, eine Instanz |
| Discord-Parität bei Community-Features | Nicht-Ziel | Rollen-Zoo, Bots, Nitro-Kram |
| App-Audio streamen (Reason, Ableton) als eigener music-Track | Später | stereo, hohe Bitrate, keine Sprachverarbeitung; Zuhören und Produzieren, kein latenzfreies Jammen; Windows zuerst |
| Soundboard: Sounds P2P synchronisiert, Wiedergabe über den Stream des Klickenden | Später | eigener soundboard-Track, pro User einzeln regelbar |
| Lobby-Radio, wenn man allein im Channel ist | Später | rein lokal im Client, nichts wird gestreamt |
| Einstellungen mit Einfach- und Profi-Ansicht | Fix (v1) | beide schreiben dieselben Settings aus engine-protocol |
| Minimalistische UI, Themes über Design-Tokens | Fix (v1) | ein Theme ist eine Datei, kein Code; Nicht-ITler-tauglich |
| Admin in der Client-App, Web nur Bootstrap und Notfall | Fix (v1) | Server, Channels, Einladungen, Rollen, Kick |
| Diagnose-Panel und Diagnose-Bundle-Export | Fix (v1) | RTT, Jitter, Paketverlust, Pegel pro Stufe; Zip zum Teilen im Channel |
| Auto-Update mit Kanälen nightly, beta, stable und Mindestversion | Fix (v1) | Builds aus CI landen in Minuten bei der Gang |

## Architektur-Überblick

Drei Ebenen mit klarer Zuständigkeit: die Desktop-App (Tauri) ist die eigentliche Client-Anwendung mit Login, Channels, Voice und Settings und macht die gesamte Audioverarbeitung nativ in Rust, die Control-Plane auf Railway verwaltet Identität und Channels und hat im Web nur eine Bootstrap- und Notfall-Admin-Seite, die eigentliche Verwaltung sitzt in der Desktop-App; die Media-Plane (LiveKit) routet die Sprachpakete auf eigener Hardware.

&#91;embedded content: Architektur · 3 Ebenen, Sprachpfad nur zwischen Client und Media-Plane\]

Railway bekommt nur Login, Channel-Verwaltung und Token-Ausgabe; der Opus-Stream geht per UDP direkt zum eigenen LiveKit, TURN springt ein, wenn UDP blockiert ist.

**Warum diese Schnitte**

- Audio nativ in Rust statt im Webview: Web Audio in WebView2/WKWebView taugt nicht für VST-Hosting, globales Push-to-Talk und harte Latenzkontrolle.
- LiveKit statt eigenem WebRTC-Stack: SFU, Opus, FEC/RED, TURN und Reconnect sind gelöst; wir bauen nur davor (DSP) und dahinter (Mixer).
- Control-Plane auf Railway, weil dort nichts Echtzeitkritisches liegt: ein laufender Call überlebt einen Railway-Ausfall, solange die Token-Laufzeit reicht.

**Client-Shell: Tauri 2 bleibt, aber austauschbar**

Tauri bleibt die Empfehlung, weil die Engine ohnehin in Rust lebt und Tauri die dünnste Hülle darum ist; die Alternativen kosten entweder eine zweite Laufzeit oder UI-Tempo.

| Shell | Windows-Reife | Engine-Anbindung | UI-Tempo | PTT, Tray, Updater | Größe und RAM |
| --- | --- | --- | --- | --- | --- |
| Tauri 2 | gut: WebView2 ist in Windows 11 enthalten, MSI/NSIS-Bundling | in-process, Rust-Commands und Events | hoch: React oder Svelte | als Plugins vorhanden | klein |
| Electron | sehr gut: Chromium gebündelt, rendert überall gleich | FFI über napi-rs oder Sidecar-Prozess, Node dazwischen | hoch: React | eingebaut | groß, auf Gaming-PCs egal |
| Native Rust-GUI (egui, iced, Slint) | gut, keine Webview-Abhängigkeit | direkt, ein Prozess, eine Sprache | niedrig: wenige Komponenten, kein CSS | selbst bauen | am kleinsten |

Absicherung gegen jede dieser Wahlen: Die Engine bekommt von Anfang an eine nachrichtenbasierte API (Commands rein, Events und Pegel raus) und enthält keinen Tauri-Code. So kann sie später als eigener Prozess laufen (Crash-Isolation gegen die UI, Plugin-Host-Isolation), und die Shell ist ohne Engine-Umbau austauschbar.

## Audio-Engine im Client

Die Engine läuft als eigener Echtzeit-Thread im Tauri-Backend; der Webview sieht nur Pegel, Schalter und Settings. Alle Bausteine sind Rust-Crates, nichts davon läuft im Browser-Audio-Stack.

**Sendekette (Mikro → Netz)**

1. Capture mit cpal: WASAPI auf Windows, CoreAudio auf macOS, 48 kHz mono, 10-ms-Blöcke.
2. Hochpass (ca. 80 Hz) und Noise Reduction: DeepFilterNet als Default auf Gaming-PCs, RNNoise (nnnoiseless) als leichter Fallback; beide hinter einem `Denoiser`-Trait.
3. Noise Gate und Voice Activation: Energie plus Sprach-Wahrscheinlichkeit aus dem Denoiser, einstellbare Schwelle, Hold-Zeit und Hysterese; Push-to-Talk übersteuert die VAD.
4. Parametrischer EQ (3 bis 5 Biquad-Bänder) und Kompressor/Limiter, damit alle User gleich laut sitzen.
5. Effekt-Slot: eigene Effekte (Pitch-Shift, Robot, Reverb), später CLAP/VST3-Plugins über denselben Slot.
6. Übergabe an LiveKit per `NativeAudioSource::capture_frame`; Publish mit `red: true`, `dtx: false` und fester Bitrate (64 bis 96 kbps). Opus-Encoding und FEC macht das SDK.

**Empfangskette (Netz → Kopfhörer)**

1. Pro Remote-User ein dekodierter PCM-Stream aus dem SDK.
2. Pro User: Lautstärke, Mute, optional eigener EQ oder Spaß-Effekt.
3. Mixer in Rust (Summierung, Limiter); räumliches Audio wäre später an dieser Stelle möglich.
4. Ausgabe mit cpal auf das gewählte Gerät.

**Quellen und Tracks**

Musik-Streaming, Soundboard und Lobby-Radio sind kein drittes, viertes und fünftes Feature, sondern ein Modell: Die Engine kennt Quellen und veröffentlicht pro Teilnehmer bis zu drei Tracks mit eigenem Profil. Empfänger haben pro User und Track einen eigenen Regler.

| Track | Quelle | Profil | Verarbeitung |
| --- | --- | --- | --- |
| voice | Mikrofon | mono, 64 bis 96 kbps, FEC und RED, kein DTX | komplette Sendekette: Denoiser, Gate, VAD/PTT, EQ, Effekte |
| music | Anwendung (Reason, Ableton) oder Ausgabe-Loopback | stereo, 128 bis 256 kbps, LiveKit-Musik-Preset, größerer Jitterbuffer | keine Sprachverarbeitung, nur Limiter |
| soundboard | Datei aus der lokalen Sound-Bibliothek | mono oder stereo, 96 kbps, DTX an | kein Denoiser, kein Gate, nur Limiter |

- App-Audio: unter Windows über die Process-Loopback-API von WASAPI (nur das gewählte Programm), Windows zuerst; macOS bekommt, was ohne Zusatzaufwand geht (ScreenCaptureKit ab macOS 13 oder ein virtuelles Gerät wie BlackHole), der Rest wird später gezielt nachgezogen. Ziel ist Zuhören und gemeinsames Produzieren; latenzfreies Jammen mit Instrumenten (unter 30 ms, Jamulus-Klasse) ist eine andere Problemklasse und ausdrücklich kein Ziel.
- Soundboard: Wiedergabe immer beim klickenden User als eigener Track (entschieden), damit andere ihn einzeln leiser drehen oder stummschalten können; der Klickende hört den Sound lokal mit. Die Sound-Bibliothek synchronisiert sich per LiveKit-Byte-Streams direkt zwischen den Clients (Hash-adressiert, lokaler Cache), ohne Server-Speicher und ohne Railway. Hotkeys laufen über denselben globalen Hook wie PTT.
- Lobby-Radio: rein lokal im Client (Stream-URL oder Playlist), spielt nur, wenn man allein im Channel ist, und blendet beim ersten Join aus. Nichts wird gestreamt. Soll das Radio später für alle hörbar sein, ist es ein music-Track eines Users und kein neues Feature.

**Bausteine**

| Baustein | Crate | Lizenz | Anmerkung |
| --- | --- | --- | --- |
| Audio-I/O | cpal | Apache-2.0 | WASAPI, CoreAudio; ASIO optional |
| Noise Reduction | [DeepFilterNet (libDF)](https://github.com/Rikorose/DeepFilterNet), nnnoiseless | MIT/Apache | DeepFilterNet für Qualität, RNNoise für wenig CPU |
| Transport | [livekit 0.9.x](https://docs.rs/livekit/latest/livekit/options/struct.TrackPublishOptions.html) | Apache-2.0 | [NativeAudioSource](https://docs.rs/livekit/latest/livekit/webrtc/audio_source/native/struct.NativeAudioSource.html), RED/DTX-Optionen, Windows und macOS |
| Lock-free Puffer | rtrb | MIT/Apache | keine Locks, keine Allokation im Audio-Thread |
| CLAP-Hosting | [clack-host](https://github.com/prokopyl/clack) | MIT/Apache | feature-complete, API noch in Bewegung |
| VST3-Hosting | vst3-sys | GPLv3 oder proprietär (VST3-SDK) | Lizenz muss zur Projektlizenz passen, CLAP zuerst |

**Latenzbudget (Zielwerte, Mund zu Ohr)**

Capture 10 ms, DSP unter 5 ms, Opus-Frame 20 ms, Netz innerhalb Deutschlands 5 bis 15 ms, Jitterbuffer 20 bis 60 ms, Ausgabe 10 ms: Ziel sind 70 bis 120 ms. Zum Vergleich: Discord nennt keinen Mund-zu-Ohr-Wert, die In-App-Anzeige ist nur der Ping zum Voice-Server; Vergleichsseiten geben 50 bis 100 ms ohne Messmethode an, nach unserer Rechnung (20-ms-Opus-Frames plus Jitterbuffer) liegt Discord innerhalb Europas eher bei 100 bis 150 ms. Unser Ziel ist also mindestens gleichauf, und Spike S1 misst es per Loopback-Klick über zwei Clients. Spike S1 und S2 messen die echten Werte; jedes spätere Feature wird gegen dieses Budget gemessen.

**Gaming-PC-Optimierung**

- Dedizierter Audio-Thread mit erhöhter Priorität (Windows MMCSS „Pro Audio“, macOS Realtime-Policy), keine Allokationen, keine Locks.
- DeepFilterNet ist das rechenintensivste Glied; Gaming-CPUs tragen es, die GPU bleibt dem Spiel.
- Qualität schlägt Sparsamkeit: höhere Bitrate, größeres Denoiser-Modell, kein DTX.

**Echo-Cancellation**

Annahme: Headsets. AEC über das libwebrtc Audio Processing Module, das das SDK mitbringt, bleibt eine Option für Lautsprecher-Nutzer; ob wir es brauchen, klärt Spike S1.

**Plugins (CLAP/VST3)**

- CLAP zuerst: offenes Format unter MIT, clack-host ist feature-complete; VST3 danach, falls die SDK-Lizenz zur Projektlizenz passt.
- Plugins laufen zuerst in-process, ein Plugin-Crash reißt die Engine mit. Robuste Variante für Phase 3: eigener Host-Prozess, Audio über Shared Memory.
- Plugin-GUIs brauchen ein natives Fenster-Handle; Tauri kann Fenster liefern, der Aufwand ist unklar und bekommt einen eigenen Spike.

**Plattform-Abstraktion**

Windows ist die Referenz, macOS bekommt, was mitläuft. Damit sich Teile später sauber nachziehen lassen, liegt alles Plattformspezifische hinter Traits in der Engine, eine Implementierung pro OS, per `cfg` gewählt:

| Trait | Windows | macOS | Wenn die Implementierung fehlt |
| --- | --- | --- | --- |
| `AudioDevice` (Capture, Ausgabe) | WASAPI über cpal | CoreAudio über cpal | Pflicht, ohne sie kein Build |
| `AppCapture` (App-Audio) | WASAPI Process-Loopback | ScreenCaptureKit oder virtuelles Gerät | Feature in der UI ausgeblendet |
| `GlobalHotkey` (PTT, Soundboard) | Raw Input oder RegisterHotKey | CGEventTap | Hotkey nur bei App-Fokus |
| `RealtimeThread` | MMCSS „Pro Audio“ | Thread-Time-Constraint-Policy | normale Priorität, Warnung im Log |

Regel: Eine fehlende Plattform-Implementierung blendet das Feature aus oder degradiert es, sie bricht nie den Build und nie den Call. Jede Implementierung ist ein eigenes Modul, das Claude Code isoliert nachziehen kann.

**Hauptrisiko und Fallback**

Das Rust-SDK ist weniger erprobt als das JS-SDK. Spike S1 klärt Custom-Source, RED/FEC-Verhalten und Reconnect. Fallback: JS-SDK im Webview mit AudioWorklet-DSP; VST und native Effekte wären dann erst über ein virtuelles Audio-Gerät erreichbar.

## Media-Plane

Ein einzelner LiveKit-Server reicht für 20 Leute; wir bauen ihn in drei Stufen aus, immer mit derselben `livekit.yaml` und nur anderem Host.

| Stufe | Wo | Zweck | Voraussetzung |
| --- | --- | --- | --- |
| 0 Dev | Docker auf dem Dev-Rechner, `livekit-server --dev` | Engine-Entwicklung, LAN-Tests | nichts, Dev-Keys sind fest |
| 1 Home | Homeserver, Docker Compose mit LiveKit und Caddy (TLS) | erste echte Abende mit der Gruppe | öffentliche IPv4 oder IPv6 am Anschluss plus Port-Forwarding; bei DS-Lite/CGNAT nur Stufe 2 oder ein WireGuard-Hop über den VPS |
| 2 VPS | VPS in Deutschland (2 vCPU, 4 GB reichen) | Dauerbetrieb, immer erreichbar | Domain, TLS über Caddy und Let's Encrypt |
| 3 Eigene Hardware | eigener Rechner mit stabilem Anschluss, VPS bleibt Fallback | Langfristziel | Monitoring und Failover über die Control-Plane |

**Ports und Netz** ([LiveKit-Deployment-Doku](https://docs.livekit.io/transport/self-hosting/deployment/))

- 7880 TCP: API und Signalisierung, nach außen hinter Caddy auf 443
- 7881 TCP: ICE über TCP, Fallback bei UDP-Block
- 50000 bis 60000 UDP: Media, der Normalfall
- TURN/TLS auf 5349 mit eigener Subdomain und eigenem Zertifikat, TURN/UDP auf 443 für Netze, die nur 443 durchlassen
- `rtc.use_external_ip: true`, damit der Server seine öffentliche IP per STUN kennt; in Docker mit Host-Netzwerk

**Konfiguration, in allen Stufen gleich**

- Ein API-Key-Paar pro Server; das Secret kennt nur die Control-Plane, die damit Tokens signiert.
- Webhooks an `https://<railway-api>/voice/webhook`, signiert mit demselben Key, für Presence.
- Räume werden beim ersten Join automatisch angelegt; die Control-Plane gibt Tokens nur für existierende Channels aus. So braucht ein Join kein lebendes Railway.
- Redis nur für Multi-Node; ein Node läuft ohne.
- `prometheus_port: 6789` für Metriken, Grafana kommt in Phase 2.

**Robustheit**

- Failover: Jeder Channel ist in der Control-Plane einem Server zugeordnet. Fällt der primäre Server aus, setzt sie die Zuordnung auf den Fallback um, und die Clients verbinden sich über das Presence-Event neu.
- Reconnect: Das SDK nimmt eine unterbrochene Session mit demselben Token wieder auf; Spike S1 testet das mit gezogenem Kabel und WLAN-Wechsel.
- Lasttest: `lk load-test` aus der LiveKit-CLI simuliert 20 Publisher, bevor die Gruppe draufgeht.
- Kein Cluster: LiveKit-Räume hängen an einem Node, solange kein Redis läuft. Für 20 Leute ist Single-Node plus Failover einfacher und robuster als ein Cluster.

## Control-Plane

Dein NestJS-Prisma-Setup auf Railway bleibt das Hirn, aber nie der Flaschenhals: ein laufender Call überlebt einen Railway-Ausfall, weil die Tokens am Client liegen und LiveKit allein weiterroutet.

**Module**

| Modul | Status | Aufgabe |
| --- | --- | --- |
| AuthModule | existiert | Login, JWT, Refresh; der Guard schützt alles weiter unten |
| VoiceModule | neu | `POST /voice/token` (Room-Grant), `GET /voice/servers` (Serverliste mit Health), `POST /voice/webhook` (participant\_joined/left) |
| ChannelModule | neu | Server, Channels, Mitgliedschaften; die Channel-ID ist der LiveKit-Raumname |
| PresenceModule | neu | wer sitzt wo, aus Webhooks und Client-Heartbeat; Push an die Clients per WebSocket-Gateway |
| Admin | neu, klein | Verwaltung sitzt in der Desktop-App hinter der Admin-Rolle; im Web nur eine winzige Bootstrap- und Notfall-Seite (ersten Server anlegen, Kill-Switch, Mindestversion), siehe Abschnitt Client-UI, Admin und Betrieb |
| TextModule | später | Nachrichten pro Channel über denselben Gateway |

**Prisma-Skizze** (User existiert bereits)

```prisma
model VoiceServer {
  id         String    @id @default(cuid())
  name       String
  wsUrl      String
  apiKey     String
  apiSecret  String    // verschlüsselt abgelegt
  isPrimary  Boolean   @default(false)
  healthy    Boolean   @default(false)
  lastSeenAt DateTime?
  channels   Channel[]
}

model Channel {
  id          String       @id @default(cuid())
  name        String
  position    Int
  serverId    String
  server      VoiceServer  @relation(fields: [serverId], references: [id])
  memberships Membership[]
}

model Membership {
  userId    String
  channelId String
  role      String  @default("member")
  @@id([userId, channelId])
}
```

**Token-Logik**

- Identity ist die User-ID, Name der Anzeigename; Grant: `roomJoin`, `room = channelId`, `canPublish`, `canSubscribe`.
- Lange Laufzeit (ein Spielabend, etwa 12 h), damit Reconnects und Railway-Ausfälle den Call nicht beenden. Rechte-Entzug wirkt dann erst beim nächsten Join oder sofort über `RoomService.removeParticipant`.
- Signiert mit dem Secret des Servers, dem der Channel gerade zugeordnet ist; beim Failover wechselt die Zuordnung und die Clients holen neue Tokens.

**Verhalten bei API-Ausfall**

Der Client hält Channel-Liste und letzte Tokens im Cache, bleibt im Call und versucht die API im Hintergrund wieder zu erreichen. Presence veraltet dann, Sprache läuft weiter.

Entschieden: Die Control-Plane lebt als `apps/api` im Monorepo. lyrics-helper (deine Railway-App) bleibt getrennt und bekommt ihre Arbeitsaufträge ebenfalls über den App-Hub.

## Client-UI, Admin und Betrieb

Die App wird von einer festen Gang benutzt, darunter Nicht-ITler: installieren, Einladungslink klicken, reden. Alles Weitere ist optional und sitzt hinter „Profi“, und ein neuer Build muss in Minuten bei allen sein, weil wir während der Entwicklung ständig ausliefern.

**UI-Prinzipien**

- Minimal: ein Fenster; Channel-Liste links, wer gerade spricht in der Mitte, Mikro- und PTT-Status unten. Was sich nicht auf den ersten Blick erklärt, landet nicht in der Einfach-Ansicht.
- Themes über Design-Tokens (`packages/ui-tokens`, JSON → CSS-Variablen): Farben, Radien, Abstände, Schrift. Ein Theme ist eine Datei, kein Code. So bleibt der UI-Stack frei wählbar (Spike S5), und die Tokens laufen in deinen anderen Apps mit.
- Einstellungen in zwei Ansichten: „Einfach“ (Gerät, PTT-Taste, Lautstärke, Theme) und „Profi“ (DSP-Kette mit Live-Pegeln, Bitrate, Jitterbuffer, Effekte, Diagnose). Beide schreiben dasselbe Settings-Schema aus `engine-protocol`.
- Erststart: Wizard mit Mikrofon-Test und Gerätewahl, Login per Einladungslink, sinnvolle Defaults (VAD an, Denoiser an, Auto-Gain). Keine Konfigurationsdateien für Nutzer.

**Admin**

- Verwaltung (Server, Channels, Einladungen, Rollen, Kick) sitzt in der Desktop-App hinter der Admin-Rolle, mit demselben UI-Stack und derselben Auth.
- Die Web-Seite in `apps/api` bleibt bewusst winzig und ist von Anfang an voll funktionsfähig für zwei Fälle: Bootstrap (ersten Server und Admin anlegen, bevor es einen Client gibt) und Notfall (Kill-Switch für eine kaputte Client-Version, Mindestversion setzen, Failover auslösen), wenn gerade kein Client läuft.

**Analytics, Debugging, Troubleshooting**

| Ebene | Was | Womit |
| --- | --- | --- |
| Client | Verbindung: RTT, Jitter, Paketverlust, Bitrate, aktueller Server; Pipeline: Pegel und CPU pro Stufe; Geräteliste | LiveKit-Stats aus dem SDK, Engine-Events; Diagnose-Bundle (Logs, Settings, Stats der letzten 10 Minuten) als Zip, teilbar im Channel |
| Media-Plane | Teilnehmer, Paketverlust, Bandbreite, CPU pro Raum | LiveKit-Prometheus → Grafana in `infra/observability`, ab Stufe 2 |
| Control-Plane | Requests, Fehler, Webhook-Ausfälle, Presence-Verlauf | strukturierte NestJS-Logs auf Railway, Ereignis-Tabelle in Prisma, Ansicht im Admin |
| Ende-zu-Ende | „Wer hat wann wen nicht gehört?“ | Event-Log pro Session: Join, Leave, Reconnect, Server-Wechsel, Update; im Admin filterbar |

**App-Updates: schnell zur Gang**

Während der Entwicklung ist der Update-Weg das wichtigste Werkzeug, kein Release-Ritual: Jeder grüne Build auf `main` wird ein `nightly`, und die Gang hat ihn beim nächsten App-Start.

| Kanal | Was landet dort | Wer |
| --- | --- | --- |
| nightly | jeder grüne CI-Build auf `main`, automatisch signiert und veröffentlicht | wer experimentieren will und Bugs jagt |
| beta | per Tag aus `main`, nach einem Abend ohne Ärger | die ganze Gang |
| stable | manuell befördert aus beta | Default für Nicht-ITler |

- Mechanik: `tauri-plugin-updater` mit signierten Artefakten (Tauri-Minisign-Key, kein teures Zertifikat nötig; Windows SmartScreen meckert einmal, für die Gang okay) und einem Update-Manifest pro Kanal auf GitHub Releases (Repo ist öffentlich, Rolling-Tag \`nightly\` plus \`v\*\`-Tags); Mindestversion und Kill-Switch kommen aus `apps/api`; Kanalwechsel in den Profi-Einstellungen.
- Ablauf im Client: Prüfung beim Start und stündlich, Download im Hintergrund, Installation nie während eines Calls, sondern beim nächsten Start oder per Klick mit „Was ist neu“ aus dem Changelog.
- Kompatibilität: Client meldet App- und Protokollversion beim Login; die Control-Plane hält eine Mindestversion und antwortet N-1-kompatibel. Darunter: Zwangs-Update statt Join.
- Rollback: der vorherige Installer bleibt lokal; der Kill-Switch per Remote-Config aus dem Notfall-Admin zieht eine Version aus dem Verkehr.
- Server: LiveKit-Version gepinnt, Update im Wartungsfenster per `docker compose pull`; Control-Plane per Railway-Deploy mit `prisma migrate deploy`, nur additive Migrationen, damit ältere Clients weiterlaufen.
- Ziel vom Merge bis zur Gang: unter 20 Minuten (CI-Build mit Cargo-Cache, Windows-Installer, Manifest-Update).

**Austauschbarkeit: experimentieren ohne Umbau**

| Naht | Vertrag | Austauschbar |
| --- | --- | --- |
| UI ↔ Engine | `engine-protocol`: typisierte Commands und Events, versioniert, TS-Typen generiert | UI-Stack und Shell (Tauri → anderes) |
| Engine ↔ Transport | `Transport`-Trait | LiveKit → anderer SFU oder P2P |
| Engine ↔ DSP | `Denoiser`-, `Effect`-, `Source`-Traits | DeepFilterNet ↔ RNNoise, Effekte, Plugins |
| Engine ↔ OS | Plattform-Traits, eigene Crates pro OS | Windows- und macOS-Teile einzeln |
| Client ↔ Control-Plane | DTOs in `packages/shared`, OpenAPI | NestJS → anderes Backend |
| Control-Plane ↔ Media | LiveKit-Server-API und Webhooks | Server-Instanzen, Hosting-Stufen |
| Experimente | Feature-Flags lokal im Client plus Remote-Config | pro User oder Channel ein- und ausschaltbar |

Regel für jede Naht: Der Vertrag liegt in einem eigenen Paket oder Crate, hat Tests, und keine Seite importiert die andere direkt.

## Monorepo und Dev-Setup

Ein Repo, drei Welten (Rust, Node, Infra), ein Befehl pro Welt; alles läuft identisch auf Windows 11 und macOS, weil kein Skript Bash voraussetzt.

```text
yappa/
├── apps/
│   ├── desktop/              # Tauri 2 Shell: Fenster, Tray, Hotkeys, Updater; UI-Stack nach Spike S5
│   │   ├── src/              # UI, spricht nur engine-protocol und packages/shared
│   │   └── src-tauri/        # Rust-Shell, bindet crates/audio-engine ein
│   └── api/                  # NestJS Control-Plane + winzige Bootstrap-/Notfall-Admin-Seite
├── crates/
│   ├── engine-protocol/      # Commands, Events, Settings-Schema; versioniert; TS-Typen daraus generiert (ts-rs)
│   ├── dsp/                  # reine DSP-Bausteine: Biquad-EQ, Gate, Kompressor, Effekte; kein I/O
│   ├── audio-engine/         # Quellen, Pipeline, Mixer, Ausgabe, Plattform-Traits; kennt weder Tauri noch LiveKit
│   ├── transport/            # Transport-Trait + LiveKit-Implementierung, Reconnect, Publish-Optionen
│   ├── platform-windows/     # WASAPI Process-Loopback, Raw-Input-Hotkeys, MMCSS
│   ├── platform-macos/       # ScreenCaptureKit, CGEventTap, Realtime-Policy; wächst nach Bedarf
│   └── plugin-host/          # CLAP/VST3, Phase 3
├── packages/
│   ├── shared/               # API-DTOs für Client und Control-Plane
│   └── ui-tokens/            # Design-Tokens und Themes als JSON → CSS-Variablen, stack-unabhängig
├── infra/
│   ├── livekit/              # docker-compose für dev, home, vps; livekit.yaml-Vorlagen; Caddyfile
│   ├── observability/        # Prometheus, Grafana, Loki; ab Stufe 2
│   └── scripts/              # Node-Skripte, cross-platform
├── docs/                     # ADRs, Spike-Ergebnisse, dieses Dokument als Markdown
├── Cargo.toml                # Rust-Workspace
├── package.json              # npm-Workspaces
├── .gitattributes            # * text=auto eol=lf, ab Commit 1
└── CLAUDE.md
```

Die Schichtung ist die Regel, nicht nur die Ordnerstruktur: `dsp` kennt kein I/O, `audio-engine` kennt kein Netz, `transport` kennt keine DSP. So bleibt jedes Crate einzeln testbar und ein Austausch (anderer Denoiser, anderes SDK) bleibt lokal.

**Werkzeuge**

| Welt | Werkzeug | Anmerkung |
| --- | --- | --- |
| Rust | rustup stable, Cargo-Workspace, clippy, rustfmt | `CARGO_TARGET_DIR` zentral, damit Worktrees den Build-Cache teilen |
| Node | Node 20+, npm-Workspaces | wie im App-Hub; pnpm hatte dort Rechteprobleme |
| Tauri | Tauri 2 CLI; Windows: WebView2 (in Win 11 enthalten), MSVC Build Tools; macOS: Xcode CLT | `tauri-plugin-global-shortcut` für PTT, Spike S3 prüft Vollbild-Spiele |
| Infra | Docker Desktop, LiveKit-CLI `lk` | Compose-Dateien für dev, home, vps |
| Skripte | Node-Skripte in `infra/scripts`, aufgerufen über npm-Scripts | kein Bash-only, kein PowerShell-only |

**Befehle im Root**

```text
npm run dev            # Tauri dev: Vite plus cargo, Hot-Reload für UI, Neustart für Rust
npm run dev:livekit    # docker compose -f infra/livekit/dev.yml up, mit Dev-Keys
npm run dev:api        # NestJS lokal gegen das lokale LiveKit
npm run check          # cargo clippy, cargo test, tsc, eslint
npm run test:audio     # Offline-Pipeline-Tests (WAV rein, WAV raus), ohne Audiogeräte
npm run loadtest       # lk load-test mit 20 Publishern gegen den Dev-Server
```

**Lokaler Dev-Modus des Clients**

- `YAPPA_DEV=1`: Client verbindet gegen `ws://localhost:7880` mit den Dev-Keys, Login über Mock-User, keine Control-Plane nötig.
- Zwei Client-Instanzen auf einem PC (eigene Profile) plus Loopback, um die eigene Kette zu hören.
- Die Audio-Engine läuft auch als CLI-Binary ohne Tauri-Fenster, für Messungen und Tests.

**Tests**

- `dsp`: Unit-Tests mit synthetischen Signalen, Frequenzgang-Snapshots für EQ und Gate.
- `audio-engine`: deterministische Offline-Pipeline WAV → WAV, läuft headless in CI und in Claude-Code-Worktrees.
- `transport`: Integrationstest gegen LiveKit in Docker, nur in CI auf Linux.
- Jeder Test loggt Latenz pro Block und CPU-Zeit, damit Regressionen auffallen.

**CI und Releases**

- GitHub Actions Matrix `windows-latest` und `macos-latest`: check, test, `tauri build`; LiveKit-Integration auf `ubuntu-latest` mit Docker.
- `tauri-action` baut Installer (MSI/NSIS, DMG); Auto-Updater über `tauri-plugin-updater` ab Phase 2.

## App-Hub-Evaluation

Ja, der App-Hub passt als Entwicklungs-Cockpit für yAPPA, aber erst nach zwei kleinen Fixes; zusammen etwa ein halber Tag, danach prüft Spike S0 die Pipeline einmal komplett mit dem leeren yAPPA-Repo.

**Was gut passt**

- Die Board-Pipeline (idea → plan → build → claude → review → done) deckt Spikes, Kern und Qualitätsphase ab.
- Der Debate-Workflow (Label `debate`: Critic, Advocate, Judge) ist genau das „hinterfragen statt Pflaster“ für neue Anforderungen, und er läuft automatisch beim Anlegen.
- Worktree pro Task plus Review-Lane: nichts schreibt direkt auf main.
- Das `context`-Feld in `.apphub.md` und die CLAUDE.md fließen in den Runner-Prompt; das ist der Übergabekanal für dieses Dokument.
- Markdown als Source of Truth passt zu `docs/` und ADRs im yAPPA-Repo.

**Befunde** (Stand des Checkouts auf dem Windows-Rechner)

| Befund | Auswirkung auf yAPPA | Fix | Schwere |
| --- | --- | --- | --- |
| `git diff --stat` zeigt 265 geänderte Dateien mit identisch 33.121 Zeilen rein und raus: komplettes CRLF/LF-Flapping zwischen Mac-Commit und Windows-Checkout | In jedem Worktree gelten alle Dateien als geändert; Claude-Code-Commits würden Komplett-Rewrites enthalten, die Review-Lane wird unlesbar | `.gitattributes` mit `* text=auto eol=lf`, `git add --renormalize .`, einmal committen; vorher mit `git diff -w --stat` prüfen, ob unter dem Rauschen echte Änderungen stecken (z. B. `templates/sveltekit-web`) | Blocker |
| `git-worktree.ts` spiegelt nur `node_modules` in Worktrees, kein Cargo-Cache | Jeder Rust-Task baut von null: Minuten pro Task, Gigabytes pro Worktree | Runner setzt `CARGO_TARGET_DIR=<repo>/.cargo-target` in der Spawn-Env (Cargo lockt selbst); alternativ Junction auf `target/` wie bei `node_modules` | Blocker |
| Kein Tauri/Rust-Template; `postCreate` ist ein einzelner Shell-String, das KMP-Template ruft `./setup.sh` (Bash, „for macOS“) | Ein yAPPA-Template braucht `npm install` plus `cargo fetch` auf beiden Systemen | Template `tauri-rust-monorepo` mit `postCreate: node setup.mjs`; kurzfristig reicht es, das Repo unter `projects/` anzulegen und per `.apphub.md` zu registrieren | Nötig |
| `projects/` ist leer, es existiert nur ein Dogfooding-Worktree: der Hub hat noch nie ein externes Projekt durch die Pipeline gefahren | Reibung ist wahrscheinlich, Umfang unbekannt | Spike S0: ein Backlog-Item einmal durch Board, Worktree, Build, Review und Merge fahren | Nötig |
| Projekte müssen unter `App-Hub/projects/` liegen (Scanner, `resolveScope`) | yAPPA lebt unter `D:\Projekte\dev\App-Hub\projects\yappa`, eigenes Git, vom Hub ignoriert | Registrierung externer Pfade, weil lyrics-helper außerhalb liegt; bis dahin eine Junction von projects/ auf das Repo | Nötig |
| Runner fährt genau einen Task gleichzeitig (`currentProcess`), Concurrency ist v2 Phase 2A | Spikes laufen seriell, für einen Entwickler in Ordnung | Keiner für den Start | Nice-to-have |
| Hub-Autostart gibt es nur für macOS (launchd) | Auf Windows `npm run dev` von Hand starten | Task-Scheduler-Eintrag, irgendwann | Nice-to-have |

**Falls S0 mehr als einen Tag frisst:** yAPPA ohne Hub starten (Claude Code direkt im Repo, `TASKS.md` von Hand) und den Hub später nachziehen. Die Projektstruktur ändert sich dadurch nicht, `.apphub.md` ist nur ein Marker.

## Projekt-Vorgehen

Erst Spikes, die die Architektur-Risiken abräumen und mit S1 früh einen Proof of Concept liefern, dann ein spielbarer Kern, dann Qualität; das Projekt-Setup (S0) läuft daneben und blockiert den PoC nicht; neue Anforderungen gehen durch eine feste Intake-Frage statt direkt ins Backlog.

&#91;embedded content: Roadmap · 4 Phasen, 3 Gates\]

Fällt Spike S1 durch, greift der Fallback aus dem Audio-Engine-Abschnitt, bevor Phase 1 beginnt; Phase 2 endet erst, wenn die Gruppe vier Wochen ohne Ausfall gespielt hat.

**Spikes** (je 1 bis 3 Tage, Ergebnis als Markdown in `docs/spikes/`)

- S0: App-Hub-Fixes, yAPPA-Repo anlegen, ein Item durch Board, Worktree, Build, Review und Merge fahren. Erster `nightly`-Build aus CI an zwei Rechner der Gang.
- S1, der Proof of Concept: zwei Clients hören sich über das lokale LiveKit, Engine als CLI ohne UI (cpal → NativeAudioSource), RED und Bitrate, Reconnect mit gezogenem Kabel. Startet in einem Scratch-Crate und wartet nicht auf S0. Messung: Latenz per Loopback-Klick und Verhalten bei Paketverlust.
- S2: DSP-Kette offline: WAV → DeepFilterNet → Gate → EQ → WAV. Messung: Latenz und CPU pro Block.
- S3: Tauri: globaler PTT-Hotkey bei Vollbild-Spiel unter Windows, Tray, Autostart.
- S4, optional: clack-host lädt ein CLAP-Plugin und schleift Audio durch.
- S5: UI-Stack-Analyse über deine Apps (lyrics-helper, App-Hub, yAPPA): Kriterien sind Minimalismus, Theming über Tokens, Tauri-Tauglichkeit, Wiederverwendung. Ergebnis als ADR; entscheidet das Framework für Phase 1.

**Intake für neue Anforderungen**

1. Jede Anforderung wird als Board-Item in `idea` angelegt, mit Label `debate`; Critic, Advocate und Judge laufen automatisch, das Urteil landet als Notiz am Item.
2. Vor `plan` müssen drei Fragen beantwortet sein: Welches Problem beim Zocken löst das? Welche Ebene ändert sich, und bleibt die Trennung Client, Control, Media sauber? Was wird dafür bewusst nicht gebaut?
3. Architektur-relevante Entscheidungen bekommen ein ADR in `docs/adr/` (Kontext, Entscheidung, Konsequenzen, eine Seite). Dieses Dokument wird ADR-000.
4. Kein Feature, das in die Sendekette eingreift, ohne Latenz- und CPU-Messung davor und danach.

**Definition of Done pro Item**

- `npm run check` grün auf Windows und macOS (CI)
- Review-Lane im Hub durchlaufen, kein Direkt-Merge
- Eine Zeile im Changelog, bei Architekturänderung ein ADR

**Übergabe an Claude Code**

- Dieses Dokument wird als `docs/00-grobstruktur.md` ins Repo exportiert; `CLAUDE.md` verweist darauf und trägt die Schichtregeln aus dem Monorepo-Abschnitt.
- Das `context`-Feld in `.apphub.md` bekommt die Kurzfassung (Ebenen, Crates, Befehle), weil der Runner es in jeden Prompt packt.

Die fertigen Aufträge für Claude Code (App-Hub-Fixes, Monorepo-Skelett, Proof of Concept) liegen im Tab Prompts für Claude Code.

## Offene Entscheidungen

Zehn Entscheidungen sind offen; die ersten drei brauchen wir vor Spike S0, der Rest klärt sich in den Spikes. Jede Entscheidung bekommt beim Umstellen auf „Entschieden“ ein ADR.

| Entscheidung | Optionen | Tendenz | Status |
| --- | --- | --- | --- |
| Projektname | yAPPA | yAPPA: Appa aus Avatar, und yappen ist, was wir da tun | Entschieden |
| UI-Framework der Desktop-App | React (lyrics-helper), Svelte 5 (App-Hub) oder etwas Drittes, einheitlich für alle deine Apps | erst Spike S5 (UI-Stack-Analyse über deine Apps); das Tokens-Paket macht die Wahl reversibel | Offen |
| Wo lebt die Control-Plane | VoiceModule in der bestehenden Web-App oder eigenes `apps/api` im Monorepo | apps/api im Monorepo; lyrics-helper bleibt eine eigene Railway-App | Entschieden |
| Denoiser-Default | DeepFilterNet (Qualität) oder RNNoise (wenig CPU) | DeepFilterNet, RNNoise bleibt als Schalter | Offen |
| Plugin-Format zuerst | CLAP oder VST3 | CLAP (MIT, clack-host); VST3 nur, wenn die Lizenz passt | Offen |
| Lizenz von yAPPA | MIT/Apache oder GPL | hängt an VST3; ohne VST3 MIT/Apache | Offen |
| Paketmanager | npm oder pnpm | npm, Konsistenz mit dem App-Hub | Offen |
| Primärer Server | VPS primär, Hardware später; oder Hardware primär mit VPS-Fallback | VPS zuerst, Umzug sobald die Hardware steht | Offen |
| Echo-Cancellation | Headset-Annahme ohne AEC oder AEC als Option | Spike S1 entscheidet | Offen |
| PTT-Mechanik | tauri-plugin-global-shortcut oder Raw-Input-Hook | Spike S3 entscheidet | Offen |
| Client-Shell | Tauri 2, Electron oder native Rust-GUI (egui, iced, Slint) | Tauri 2; Engine ohne Tauri-Code, damit die Shell austauschbar bleibt | Offen |
| Repo öffentlich oder privat | öffentlich (Nightly über GitHub Releases); privat mit separatem öffentlichem Releases-Repo; privat mit Auslieferung über apps/api | öffentlich, Nightly über GitHub Releases; Bedingung: keine Secrets im Repo, Signing-Key nur in CI-Secrets | Entschieden |

## Quellen

- [LiveKit Rust SDK: TrackPublishOptions (livekit 0.9.3)](https://docs.rs/livekit/latest/livekit/options/struct.TrackPublishOptions.html)
- [LiveKit Rust SDK: NativeAudioSource](https://docs.rs/livekit/latest/livekit/webrtc/audio_source/native/struct.NativeAudioSource.html)
- [livekit/rust-sdks auf GitHub](https://github.com/livekit/rust-sdks)
- [LiveKit: Deploying LiveKit (Ports, TURN, Konfiguration)](https://docs.livekit.io/transport/self-hosting/deployment/)
- [Railway: LiveKit-Template, „Railway routes no inbound UDP“](https://railway.com/deploy/livekit-server)
- [DeepFilterNet (libDF, Rust, MIT/Apache)](https://github.com/Rikorose/DeepFilterNet)
- [clack: CLAP-Host und -Plugins in Rust](https://github.com/prokopyl/clack)
- App-Hub: `CLAUDE.md`, `packages/hub/src/lib/server/{templates,git-worktree,claude-runner,coder-backends}.ts`, `git diff --stat` im Checkout unter `D:\Projekte\dev\App-Hub`

* [Discord vs TeamSpeak vs Mumble 2026 (Latenzangaben ohne Messmethode)](https://tech-insider.org/discord-vs-teamspeak-vs-mumble-2026/)
