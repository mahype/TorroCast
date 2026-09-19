# Offene Punkte

Arbeitsliste. Erledigtes wird abgehakt und bleibt stehen, damit der Weg nachvollziehbar ist.

**Stand 19.09.2026, abends:** Die ursprüngliche Liste ist abgearbeitet. Was unten noch offen ist, wurde
bewusst zurückgestellt – weil es eine Entscheidung von dir braucht, Hardware, die hier nicht zur Hand
ist (macOS, Windows), oder einen echten Schlüssel zum Testen.

- [x] Schlaf-Timer (`t`: 15, 30, 45, 60 Minuten, Ende der Folge)
- [x] Bibliotheks-Ordner in den Einstellungen wählen, vorhandene Daten ziehen mit um
- [x] Podcast Index als zuschaltbare Quelle mit eigenem Schlüssel des Nutzers (Suche; mit echtem Schlüssel noch ungetestet – nur die Ablehnung eines falschen ist gegen den Dienst geprüft)
- [ ] Podcast Index: Trending als eigener Reiter; Schlüssel im Schlüsselbund des Systems statt in der `config.toml`
- [x] Downloads: Folgen laden (`D`), von der Platte abspielen, Download-Ansicht, löschen
- [ ] Downloads: laufenden Download abbrechen; automatisch laden und aufräumen; Download-Ordner in der Oberfläche wählen (bisher `download_dir` in der `config.toml`)
- [x] Cover-Bilder im Podcast-Kopf und im großen Player (Kitty, Sixel, iTerm2 nach Terminal, sonst Halbblöcke; abschaltbar; Platten-Cache). In tmux als Halbblöcke geprüft – **Sixel in foot und Kitty-Grafik noch von niemandem angesehen**
- [x] Medientasten unter Linux (MPRIS): Medientasten, Klangmenü, Sperrbildschirm sehen und steuern die Wiedergabe – gegen den Session-Bus geprüft
- [x] Fernsteuerung: `torrocast daemon` (ohne Fenster), `status` (JSON), `ctl` – Unix-Socket, siehe `fernsteuerung.md`; Grundlage des Omarchy-Plugins
- [ ] Fernsteuerung: die TUI als Fenster auf ein dauerhaft laufendes TorroCast (heute endet die Wiedergabe mit der TUI); Windows (Named Pipe); Tempo über die Übergabe Daemon→TUI hinweg behalten
- [ ] Medientasten unter macOS (braucht einen Run-Loop auf dem Main-Thread) und Windows (braucht ein verstecktes Fenster)
- [x] Mehrere Playlists: anlegen (`N`), Folgen von überall hineinlegen (`L`), ganz in „Als Nächstes“ legen (`a`/`A`), löschen – im Bibliotheks-Ordner gespeichert und damit geräteübergreifend
- [ ] Playlists: umbenennen, umsortieren; Playlists nach Regeln (Smart Playlists)
- [x] Bibliotheks-Ordner: verdichtete Schnappschüsse – ab 128 KiB faltet ein Gerät beim Start seine Journale zu einem Schnappschuss
- [ ] Bibliotheks-Ordner: Ordner beobachten statt alle 30 s nachsehen; kopierte Geräte-Kennung erkennen; Einstellungen und Favoriten im Ordner
- [x] Opus: mit `--features opus` (nutzt das libopus des Systems) oder `opus-bundled` (baut es mit, braucht cmake) – mit einer echten Opus-Folge geprüft
- [x] Weite Sprünge warten nicht mehr auf den Download, sondern holen die Stelle per Teilabruf
- [ ] HE-AAC (nur über die patentbehaftete FDK-Bibliothek zu haben; bewusst zurückgestellt)
- [x] Maus: Ein Klick wählt eine Zeile, ein zweiter öffnet sie; Reiter sind anklickbar
- [x] Verschachtelte Listen in Shownotes behalten ihre Einrückung
- [x] OPML-Import und -Export (`I` und `E` bei den Abos)
- [ ] Episodenzustände „archiviert“ und „Favorit“; Statistiken; Transkripte
- [ ] Private Feeds mit Token in der URL: beim Abonnieren warnen, „nur auf diesem Gerät“ anbieten
- [ ] FFI-Schicht (`torrocast-ffi`, UniFFI) und native Oberflächen

- [x] Fortschrittsbalken fester Breite und Cover vor jedem Eintrag in den Folgenlisten
- [ ] Fortschrittsbalken auch in der Folgenliste eines Podcasts (einzeilig; dort fehlt er noch)

## Noch von niemandem mit eigenen Augen oder Ohren geprüft

- **Ton:** Alle Wiedergabetests liefen stumm. Wie der Zeitstrecker bei 1,5× bis 3× klingt, hat noch niemand gehört.
- **Cover als echtes Bild** (Sixel in foot, Kitty-Grafik): nur die Halbblock-Variante wurde gesehen.
- **Podcast Index mit echtem Schlüssel:** nur die Ablehnung eines falschen ist gegen den Dienst geprüft.
- **macOS und Windows:** Die Tests laufen dort in der CI; bedient hat die TUI dort noch niemand.
