# Mittesten

Für alle aus der Gang, die yAPPA ausprobieren. Du brauchst Windows 10 oder 11, ein Headset
und zwei Dinge von Chris: den **Installer** und deine persönliche **Einladung** (eine lange
Textzeile, die mit `yappa1.` beginnt).

## Loslegen

1. Installer starten. Windows warnt vor einer unbekannten App: „Weitere Informationen" →
   „Trotzdem ausführen". Das liegt daran, dass die App noch nicht signiert ist.
2. yAPPA öffnen. Links **Mikrofon** und **Ausgabe** wählen. Wenn du sprichst, bewegt sich
   der Balken unter dem Mikrofon – das geht schon, bevor du verbunden bist.
3. Die Einladung komplett in das Feld **Einladung** einfügen und auf **Beitreten** klicken.
4. Du siehst, wer da ist. Wer spricht, bekommt einen farbigen Rand.

Die Einladung merkt sich yAPPA. Beim nächsten Mal reicht ein Klick auf „Beitreten".

**Bitte Headset benutzen.** Mit Lautsprechern hören die anderen sich selbst als Echo – eine
Echounterdrückung gibt es noch nicht.

## Was die Knöpfe tun

| Knopf | Wirkung |
| --- | --- |
| Mikro an / aus | Dein Mikrofon stummschalten. Der Balken zeigt weiter an, dass es funktioniert. |
| Hören / Taub | Alle anderen stumm für dich. |
| Regler bei einer Person | Diese Person für dich lauter oder leiser machen. |
| Hörbar / Stumm für mich | Diese eine Person nur für dich stummschalten. |
| Verlassen | Raum verlassen. |
| Debug | Blendet rechts die Messwerte ein (siehe unten). |

## Wenn etwas komisch klingt

Genau dafür testen wir. Bitte so melden, dann können wir es nachvollziehen:

1. **Debug** einschalten und kurz schauen, ob etwas orange ist – z. B. „Paketverlust" oder
   „Ersetzte Samples". Das gern dazuschreiben.
2. Im Debug-Bereich auf **Diagnose-Datei speichern** klicken. Die Datei landet im Ordner
   „Downloads" (der genaue Pfad wird angezeigt).
3. Die Datei an Chris schicken, zusammen mit: **Was** ist passiert (Knacken, Aussetzer,
   Roboterstimme, jemand war weg) und **ungefähr wann** (Uhrzeit reicht).

In der Datei stehen Programmversion, deine Einstellungen, die Gerätenamen und ein
Protokoll mit Messwerten pro Sekunde. Gespräche werden nicht aufgezeichnet, und deine
Einladung steht nicht drin.

## Bekannte Lücken

- Kein Push-to-Talk und keine Rauschunterdrückung: das Mikrofon ist immer offen.
- Geräte müssen auf 48000 Hz stehen (Windows-Soundeinstellungen → Gerät → Format). Meldet
  yAPPA „kein 48-kHz-Modus", dort umstellen.
- Ein Raum, keine Channels, kein Text-Chat.
- Updates kommen noch von Hand: neuer Installer, drüberinstallieren.
- Dieselbe Einladung nicht auf zwei Rechnern gleichzeitig benutzen – der zuerst verbundene
  fliegt raus.
