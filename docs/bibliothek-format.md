# Der Bibliotheks-Ordner: was umgesetzt ist

Der Entwurf steht in [research/04-datenablage-und-sync.md](research/04-datenablage-und-sync.md).
Dieses Dokument beschreibt den Stand im Code (`crates/torrocast-library`) und wo er
vom Entwurf noch abweicht.

## Wo der Ordner liegt

Ohne Einstellung im Datenverzeichnis der Plattform:

| System | Ort |
|---|---|
| Linux | `$XDG_DATA_HOME/torrocast/library`, sonst `~/.local/share/torrocast/library` |
| macOS | `~/Library/Application Support/TorroCast/library` |
| Windows | `%APPDATA%\TorroCast\library` |

Um ihn zu teilen und mitzusichern: **Einstellungen → Bibliotheks-Ordner → `enter`**, den
neuen Ort eintippen (`~` steht für das Home-Verzeichnis), `enter`. Was dieses Gerät
geschrieben hat, zieht mit um; liegt am neuen Ort schon eine Bibliothek eines anderen
Geräts, wird beides zusammengeführt. Der alte Ordner bleibt als Sicherung liegen. Auf
jedem weiteren Gerät denselben synchronisierten Ordner wählen.

In der `config.toml` steht der Ort als `library_dir`.

## Aufbau

```
library/
├── format.json
└── devices/
    ├── 83a5dd7cd384/journal-000001.jsonl     schreibt nur dieses Gerät
    └── c41d07b8e2f0/journal-000001.jsonl     schreibt nur jenes
```

- **Kein Gerät schreibt je in die Datei eines anderen.** Deshalb kann ein Sync-Dienst
  keine Konfliktkopien erzeugen.
- Die Geräte-Kennung entsteht beim ersten Start und steht als `device_id` in der
  `config.toml` des Geräts – nicht im Ordner.
- Ein Journal wird bei 256 KiB abgeschlossen und ein neues begonnen.
- `format.json` nennt die Format-Version. Verlangt ein Ordner eine neuere Version zum
  Lesen, öffnet TorroCast ihn nicht und arbeitet nur für die Sitzung; verlangt er sie nur
  zum Schreiben, wird gelesen, aber nichts geschrieben.

## Eine Zeile

```json
{"v":1,"id":"83a5dd7cd384:2","hlc":"2026-09-19T17:06:48.566Z-0000-83a5dd7cd384","t":"Subscribed","podcast":"9dfbd227-…","feed_url":"https://…","title":"Lage der Nation"}
```

| Feld | Bedeutung |
|---|---|
| `v` | Version der Zeile |
| `id` | `<gerät>:<laufende nummer>` |
| `hlc` | Zeitstempel: Uhrzeit, Zähler, Gerät. Als Text sortierbar; die spätere Änderung gewinnt. |
| `t` | Art der Änderung |

Arten: `DeviceRegistered`, `Subscribed`, `Unsubscribed`, `PlaybackUpdated`
(`episode`, `position_ms`, `duration_ms`, `played`), `QueueItemSet` (`playlist`,
`episode`, `sort`, `item`), `QueueItemRemoved`, `PlaylistSet` (`playlist`, `name`),
`PlaylistRemoved`. Unbekannte Arten und Felder werden
überlesen und bleiben in der Datei des Geräts, das sie schrieb, unangetastet.

- **Podcast-Kennung:** die `podcast:guid` des Feeds; fehlt sie, die UUIDv5 aus der
  Feed-Adresse nach der Podcasting-2.0-Spezifikation – auf jedem Gerät dieselbe.
- **Folgen-Kennung:** `ep1:` + UUIDv5 aus Feed-Adresse und GUID der Folge (ohne GUID aus
  der Audio-Adresse).
- **„Als Nächstes“** ist die Playlist `up-next`. Jeder Eintrag trägt eine Sortierzahl;
  ein neuer Eintrag bekommt die Mitte zwischen seinen Nachbarn, damit niemand sonst
  umziehen muss. Der Eintrag enthält alles, um die Folge abzuspielen, ohne ihren Feed zu
  kennen. Die gerade laufende Folge steht immer an erster Stelle der gespeicherten
  Liste; nach einem Neustart liegt sie deshalb auf Platz 1.
- **Hörposition:** wird bei Pause, Stopp, Folgenwechsel und Programmende geschrieben,
  während der Wiedergabe einmal pro Minute. Bei „gehört“ steht `played: true`.

## Was Sync-Dienste hinterlassen

Gelesen wird jede Datei `*.jsonl` im Verzeichnis eines Geräts – auch eine „conflicted
copy“; dieselbe Änderung zweimal anzuwenden ändert nichts. Temporäre Dateien (Name
beginnt mit `.` oder `~`) und alles andere werden übergangen. Eine letzte Zeile ohne
Zeilenende gilt als noch unterwegs, eine beschädigte Zeile wird übersprungen.

Andere Geräte werden alle 30 Sekunden eingelesen.

## Noch nicht umgesetzt

- Verdichtete Schnappschüsse und das Löschen alter Journale. Eine Stunde Hören sind
  etwa 60 Zeilen; das wird erst nach langer Zeit relevant.
- Beobachten des Ordners statt Nachsehen im Takt.
- Erkennen einer kopierten Geräte-Kennung (geklontes Home-Verzeichnis).
- Einstellungen und Favoriten im Ordner.
