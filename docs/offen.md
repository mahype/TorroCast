# Offene Punkte

Arbeitsliste. Erledigtes wird abgehakt und bleibt stehen, damit der Weg nachvollziehbar ist.

- [x] Schlaf-Timer (`t`: 15, 30, 45, 60 Minuten, Ende der Folge)
- [x] Bibliotheks-Ordner in den Einstellungen wählen, vorhandene Daten ziehen mit um
- [x] Podcast Index als zuschaltbare Quelle mit eigenem Schlüssel des Nutzers (Suche; mit echtem Schlüssel noch ungetestet – nur die Ablehnung eines falschen ist gegen den Dienst geprüft)
- [ ] Podcast Index: Trending als eigener Reiter; Schlüssel im Schlüsselbund des Systems statt in der `config.toml`
- [x] Downloads: Folgen laden (`D`), von der Platte abspielen, Download-Ansicht, löschen
- [ ] Downloads: laufenden Download abbrechen; automatisch laden und aufräumen; Download-Ordner in der Oberfläche wählen (bisher `download_dir` in der `config.toml`)
- [x] Cover-Bilder im Podcast-Kopf und im großen Player (Kitty, Sixel, iTerm2 nach Terminal, sonst Halbblöcke; abschaltbar; Platten-Cache). In tmux als Halbblöcke geprüft – **Sixel in foot und Kitty-Grafik noch von niemandem angesehen**
- [x] Medientasten unter Linux (MPRIS): Medientasten, Klangmenü, Sperrbildschirm sehen und steuern die Wiedergabe – gegen den Session-Bus geprüft
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
