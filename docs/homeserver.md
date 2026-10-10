# Homeserver einrichten (Stufe 1)

LiveKit auf dem Mini-PC zu Hause, erreichbar für die Gang über die FRITZ!Box. Diese
Anleitung ist für genau diesen Aufbau geschrieben: FRITZ!Box 6660 Cable, Mini-PC mit
Windows, darauf eine Linux-VM.

> **Stand 2026-10-10:** An echter Hardware noch nicht durchgespielt. Lokal geprüft sind die
> LiveKit-Konfiguration, die Schlüssel aus der `.env` und der Beitritt mit ausgestellten
> Tokens. Nicht geprüft: Caddy mit Let's-Encrypt-Zertifikat, die FRITZ!Box-Freigaben und
> der Zugriff von außen. Menünamen der FRITZ!Box können je nach FRITZ!OS-Version leicht
> abweichen. Was beim ersten Durchlauf hakt, bitte hier nachtragen.

## Was am Ende läuft

```text
Gang (Internet) ──► FRITZ!Box ──► Linux-VM auf dem Mini-PC
                     4 Freigaben    ├─ Caddy     TCP 80, 443   TLS, Zertifikat automatisch
                                    └─ LiveKit   TCP 7881, UDP 7882   Sprache
```

Die Clients verbinden sich mit `wss://<deine-adresse>` (Anmeldung über Caddy) und schicken
die Sprache danach direkt per UDP an LiveKit. Eine Control-Plane gibt es noch nicht: wer
mitreden darf, bekommt von dir ein Token (Schritt 8).

## 1. Vorab prüfen: Hast du eine öffentliche IPv4-Adresse?

Das ist der Punkt, an dem das Vorhaben scheitern kann — also zuerst klären. Kabelanschlüsse
laufen oft mit **DS-Lite**: dann teilst du dir die IPv4-Adresse mit anderen Kunden, und
Portfreigaben für IPv4 funktionieren nicht.

1. FRITZ!Box-Oberfläche (`http://fritz.box`) → **Internet → Online-Monitor**.
2. Steht dort **„DS-Lite"** oder „IPv4 über DS-Lite-Tunnel", hast du keine eigene IPv4.
3. Gegenprobe: Die dort angezeigte IPv4-Adresse muss dieselbe sein, die eine Seite wie
   `https://ifconfig.me` anzeigt. Beginnt sie mit `100.64.` bis `100.127.`, ist es
   ebenfalls keine öffentliche Adresse.

**Bei DS-Lite:** Beim Anbieter nach „Dual Stack" oder einer „öffentlichen IPv4-Adresse"
fragen (bei Kabelanbietern oft auf Zuruf möglich). Geht das nicht, ist der Homeserver für
diesen Anschluss raus und wir nehmen einen VPS (Stufe 2) — dann hier nicht weitermachen.

## 2. Die Linux-VM

LiveKit soll auf Linux laufen, nicht in Docker Desktop unter Windows: dort geht die Sprache
durch eine zusätzliche Netzwerkschicht, und Docker Desktop läuft nur, solange jemand
angemeldet ist.

- **Virtualisierung:** Hyper-V (Windows Pro) oder VirtualBox (geht auch mit Windows Home).
- **System:** Debian 12 oder Ubuntu Server 24.04, ohne Desktop. 2 CPU-Kerne, 2 GB RAM,
  16 GB Platte reichen.
- **Netzwerk: Netzwerkbrücke** (VirtualBox: „Netzwerkbrücke", Hyper-V: „Externer Switch").
  Die VM muss eine eigene Adresse aus deinem Heimnetz bekommen, wie ein eigenes Gerät. Mit
  „NAT" funktioniert es nicht.
- **Feste Adresse:** FRITZ!Box → **Heimnetz → Netzwerk** → die VM bearbeiten → „Diesem
  Netzwerkgerät immer die gleiche IPv4-Adresse zuweisen".
- **Dauerbetrieb:** Am Mini-PC Energiesparen/Standby abschalten und die VM so einstellen,
  dass sie mit Windows automatisch startet.

## 3. Docker in der VM

```bash
curl -fsSL https://get.docker.com | sudo sh
sudo usermod -aG docker $USER      # danach einmal ab- und wieder anmelden
```

LiveKit möchte größere Netzwerkpuffer, sonst warnt es beim Start:

```bash
echo "net.core.rmem_max=5000000" | sudo tee /etc/sysctl.d/90-livekit.conf
echo "net.core.wmem_max=5000000" | sudo tee -a /etc/sysctl.d/90-livekit.conf
sudo sysctl --system
```

## 4. Adresse und Freigaben in der FRITZ!Box

**MyFRITZ!-Konto:** **Internet → MyFRITZ!-Konto** einrichten, falls noch nicht geschehen. Du
bekommst eine Adresse der Form `xxxxxxxx.myfritz.net`, die immer auf deinen Anschluss zeigt.

**Freigaben:** **Internet → Freigaben → Portfreigaben → Gerät für Freigaben hinzufügen** →
die VM auswählen → vier Freigaben anlegen:

| Art | Protokoll | Port (außen = innen) | Wofür |
| --- | --- | --- | --- |
| MyFRITZ!-Freigabe (HTTPS-Server) | TCP | 443 | Anmeldung der Clients (`wss://`) |
| Portfreigabe | TCP | 80 | Let's Encrypt stellt darüber das Zertifikat aus |
| Portfreigabe | TCP | 7881 | Sprache über TCP, falls bei jemandem UDP gesperrt ist |
| Portfreigabe | UDP | 7882 | Sprache — der Normalfall |

- Bei jeder Freigabe „Internetzugriff über IPv4 und IPv6" wählen, wenn es angeboten wird.
- „Selbstständige Portfreigaben für dieses Gerät erlauben" bleibt **aus**.
- Die MyFRITZ!-Freigabe zeigt nach dem Speichern eine eigene Adresse für die VM an, in der
  Form `<vm-name>.xxxxxxxx.myfritz.net`. **Diese Adresse ist ab jetzt deine `YAPPA_DOMAIN`.**
  Die Adresse der FRITZ!Box selbst (`xxxxxxxx.myfritz.net`) zeigt bei IPv6 auf die Box
  statt auf die VM; damit kann die Zertifikatsausstellung scheitern.

Port 7880 wird **nicht** freigegeben — den spricht nur Caddy innerhalb der VM an.

## 5. Dateien und Schlüssel auf die VM

Das Repo liegt noch nicht auf einem Server, also die vier Dateien aus `infra/livekit/`
in einen Ordner auf der VM kopieren (z. B. mit `scp` oder WinSCP nach `~/yappa/`):

```text
home.yml   livekit.home.yaml   Caddyfile   .env.example
```

Dann auf der VM:

```bash
cd ~/yappa
docker run --rm livekit/livekit-server:v1.13.8 generate-keys   # gibt API Key + Secret aus
cp .env.example .env
nano .env        # die beiden Werte und YAPPA_DOMAIN eintragen
chmod 600 .env
```

Die `.env` enthält das Geheimnis des Servers. Sie gehört nicht ins Repo (ist per
`.gitignore` ausgeschlossen) und nicht in einen Chat.

## 6. Starten und prüfen

```bash
docker compose -f home.yml up -d
docker compose -f home.yml logs livekit | tail -20
docker compose -f home.yml logs caddy | tail -20
```

- **LiveKit:** Die Zeile `starting LiveKit server` muss bei `nodeIP` deine öffentliche
  IPv4-Adresse zeigen (dieselbe wie im Online-Monitor). Der Server braucht nach dem Start
  einige Sekunden, bis er Verbindungen annimmt.
- **Caddy:** Im Log steht nach kurzer Zeit `certificate obtained successfully`.
- **Von außen:** Am Handy WLAN ausschalten und `https://<YAPPA_DOMAIN>` im Browser öffnen.
  Erscheint `OK` ohne Zertifikatswarnung, steht die Anmeldung.

## 7. Erster Test

Auf dem Entwicklungs-PC dieselbe `.env` nach `infra/livekit/.env` legen (damit die
Einladungen mit dem Schlüssel des Servers signiert werden), dann:

```bash
cargo xtask livekit token chris     # gibt die Einladung aus: eine Zeile, beginnt mit yappa1.
cargo xtask dev                     # die App starten, Einladung einfügen, "Beitreten"
```

Steht unten „Verbunden", funktioniert die Anmeldung. Für einen zweiten Teilnehmer eine
zweite Einladung ausstellen und z. B. auf einem Laptop am Handy-Hotspot beitreten: sobald
ihr euch seht und hört, läuft die Sprache über deinen Anschluss. Im Debug-Bereich der App
stehen Laufzeit, Paketverlust und Jitterbuffer.

Ohne Fenster geht es auch, z. B. um nur den Server zu prüfen:
`cargo run --release -p yappa-desktop --example headless -- --invite <einladung>`.

Aus dem eigenen Heimnetz heraus geht die Verbindung einmal zur FRITZ!Box und wieder zurück.
Das klappt normalerweise; wenn nicht, zuerst von außen (Hotspot) testen, bevor du am Server
suchst.

## 8. Die Gang einladen

Jede Person bekommt zwei Dinge und die Anleitung [mittesten.md](mittesten.md):

1. Den Installer: `cargo xtask build` legt ihn unter `target/release/bundle/nsis/` ab
   (Windows warnt beim ersten Start, weil er nicht signiert ist).
2. Eine eigene Einladung: `cargo xtask livekit token <name> --days 30`. Der Name ist der,
   unter dem die Person im Raum erscheint; jede Einladung nur einmal vergeben.

Eine Einladung ist der Zugang: wer sie hat, kommt bis zum Ablaufdatum in den Raum `gang`. Einzeln
zurückziehen lässt es sich nicht — nur alle auf einmal, indem du auf dem Server neue
Schlüssel erzeugst (Schritt 5) und neu startest. Die Control-Plane (S6) ersetzt das später
durch Einladungslinks.

## Betrieb

| Was | Befehl (auf der VM, im Ordner `~/yappa`) |
| --- | --- |
| Status | `docker compose -f home.yml ps` |
| Logs mitlesen | `docker compose -f home.yml logs -f livekit` |
| Neu starten | `docker compose -f home.yml restart` |
| Stoppen | `docker compose -f home.yml down` |
| LiveKit aktualisieren | Version in `home.yml` ändern, dann `docker compose -f home.yml pull && docker compose -f home.yml up -d` — nicht während jemand spricht |

Beide Container starten nach einem Neustart der VM von selbst (`restart: unless-stopped`).

## Wenn es nicht geht

| Symptom | Wahrscheinliche Ursache | Prüfen |
| --- | --- | --- |
| Caddy bekommt kein Zertifikat | Port 80 kommt nicht an, oder die Adresse zeigt per IPv6 auf die FRITZ!Box | Freigabe TCP 80; `YAPPA_DOMAIN` ist die Adresse der **VM**, nicht der Box |
| `https://…` von außen nicht erreichbar | DS-Lite, oder Freigabe 443 fehlt | Schritt 1 und 4 |
| App: „could not connect … 401 Unauthorized" | Einladung mit anderen Schlüsseln signiert oder abgelaufen | `.env` auf Server und Entwicklungs-PC müssen gleich sein |
| App: „… 500" direkt nach dem Serverstart | Server noch nicht bereit | ein paar Sekunden warten |
| Jemand fliegt immer wieder raus | dieselbe Einladung läuft auf zwei Rechnern | pro Person und Rechner eine eigene Einladung |
| Beitritt klappt, aber niemand hört sich | UDP 7882 kommt nicht an | Freigabe UDP 7882; `nodeIP` im LiveKit-Log ist die öffentliche Adresse |
| Ein Einzelner hört nichts, die anderen schon | sein Netz sperrt UDP | Freigabe TCP 7881 muss stehen |
| Geht von außen, aber nicht aus dem eigenen WLAN | Rückweg über die FRITZ!Box | über Hotspot gegenprüfen |
| LiveKit-Log: `nodeIP` ist eine `192.168.…`-Adresse | öffentliche Adresse nicht ermittelt | hat die VM Internet? `use_external_ip: true` in `livekit.home.yaml` |

## Was offen ist

- Kein TURN: wer hinter einer Firewall sitzt, die nur Port 443 durchlässt, kommt nicht
  rein. Kommt mit Stufe 2 (eigene Subdomain und Zertifikat nötig).
- Keine Überwachung: fällt der Mini-PC aus, merkt es niemand außer den Leuten im Call.
- Die VM selbst braucht Updates (`sudo apt update && sudo apt upgrade`).
