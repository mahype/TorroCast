# Funktionssammlung

> **Stand 19.09.2026:** v0.1 und v0.2 sind umgesetzt, dazu aus „Später“: mehrere Playlists, Schlaf-Timer,
> Cover, Medientasten (Linux), Podcast Index. Die genaue Liste des Erledigten und Offenen: [offen.md](offen.md).
> Aus v0.2 fehlen noch die Episodenzustände „archiviert“ und „Favorit“.

Abgeleitet aus der Auswertung bestehender Clients
([research/02](research/02-clients-und-funktionsumfang.md)). Jede Funktion ist einer
Schicht zugeordnet: **Core** (plattformunabhängige Bibliothek) oder **UI**
(TUI, später native Oberflächen). Die UI enthält keine Fachlogik; alles, was eine
zweite Oberfläche ebenfalls bräuchte, liegt im Core.

## v0.1 – Entdecken und Ansehen (ohne Wiedergabe)

Ziel: Podcasts finden und alles über sie lesen können.

### Suche und Entdecken

| Funktion | Schicht | Anmerkung |
|---|---|---|
| Podcast-Suche nach Begriff | Core | über `DirectoryProvider`; Apple ist Hauptquelle und immer aktiv |
| Weitere Quellen zuschalten | Core + UI | Podcast Index (eigener Key des Nutzers), fyyd; in den Einstellungen |
| Ergebnisse mehrerer Verzeichnisse zusammenführen und de-duplizieren | Core | nach `podcast:guid`, iTunes-ID, normalisierter Feed-URL |
| Ergebnisse treffen gestaffelt ein | Core + UI | ein langsames Verzeichnis blockiert nie |
| Top-Charts nach Land | Core | Apple; Feed-URLs per Batch-Lookup |
| Trending | Core | nur mit zugeschaltetem Podcast Index |
| Kategorien durchblättern | Core | Apple-Genres als kanonische Taxonomie |
| Feed-URL direkt öffnen | Core | unabhängig von allen Verzeichnissen |
| Suche beim Tippen | UI | 600 ms Verzögerung, ab 3 Zeichen, `enter` sofort; hält das Apple-Limit ein |
| Herkunftsanzeige pro Treffer | UI | welches Verzeichnis den Treffer lieferte |

### Podcast-Detail

| Funktion | Schicht | Anmerkung |
|---|---|---|
| Feed abrufen und parsen | Core | RSS 2.0, iTunes-Tags, `content:encoded`, Podcasting-2.0-Namespace |
| HTTP-Cache (ETag, Last-Modified) | Core | schont die Hoster |
| Titel, Autor, Beschreibung, Kategorien, Sprache, Website | Core + UI | |
| Episodenzahl, letzte Veröffentlichung, Explicit-Kennzeichen | Core + UI | |
| Unterstützungs-Links (`podcast:funding`) | Core + UI | im Browser öffnen |
| Personen (`podcast:person`) | Core + UI | Hosts und Gäste |
| Cover-Bild | UI | über Terminal-Grafikprotokolle, mit Block-Zeichen als Rückfall |

### Episodenliste

| Funktion | Schicht | Anmerkung |
|---|---|---|
| Liste mit Datum, Dauer, Staffel/Folge | Core + UI | |
| Kennzeichen für Kapitel und Transkript | Core + UI | |
| Textfilter in der Liste (`/`) | UI | |
| Sortierung (neueste/älteste zuerst) | UI | |
| Paginierung großer Feeds | Core | Feeds mit über 1000 Episoden |

### Episoden-Detail

| Funktion | Schicht | Anmerkung |
|---|---|---|
| Shownotes aus HTML lesbar aufbereiten | Core | Absätze, Listen, nummerierte Links; der Core liefert ein neutrales Dokumentmodell, die UI zeichnet es |
| Links öffnen | UI | |
| Zeitmarken in Shownotes erkennen | Core | ab v0.2 anspringbar |
| Kapitel aus Podlove Simple Chapters | Core | direkt aus dem Feed |
| Kapitel aus `podcast:chapters` (JSON) | Core | externe Datei, mit Bildern und Links |
| Eingebettete Kapitel (ID3 in MP3, MP4) | Core | erst beim Öffnen der Episode, per Teilabruf, mit Größenlimit |
| Kapitelquellen zusammenführen | Core | dokumentierte Regel, ein normalisierter `Chapter`-Typ |
| Kapitel-Links und -Bilder | UI | |

### Bedienoberfläche (TUI)

| Funktion | Anmerkung |
|---|---|
| Rahmen wie TorroMail: Markenleiste, Menü links, Panels, Tastenzeile | siehe [oberflaeche.md](oberflaeche.md) |
| Hineingehen Suche → Podcast → Folge, `esc` zurück | Pfad im Panel-Titel |
| Vim-Tasten (`j/k`, `gg/G`, `h/l`, `/`) und Pfeiltasten | |
| Tastenhilfe-Leiste unten, vollständige Hilfe mit `?` | |
| Maus: Klicken, Scrollen | |
| Farbschemata, passt sich dem Terminal an | |
| Konfiguration als TOML | Tasten, Verzeichnisse, Land, Sprache |
| Statuszeile mit Fehlern und laufenden Abrufen | |
| Hinweis bei zu kleinem Fenster mit aktueller und benötigter Größe (wie btop) | Mindestgröße 80 × 24 |

## v0.2 – Abos, Wiedergabe, Warteschlange

| Funktion | Schicht | Anmerkung |
|---|---|---|
| Abonnieren und Abbestellen | Core | |
| **Bibliotheks-Ordner frei wählbar** (Dropbox, Syncthing, NAS …) | Core | siehe [research/04](research/04-datenablage-und-sync.md); ein Codepfad, auch ohne Sync |
| Abgleich mit anderen Geräten über den Ordner | Core | Journal pro Gerät, konfliktfrei |
| OPML-Import und -Export | Core | toleranter Import |
| Episodenstatus: neu, angefangen, gespielt, archiviert, Favorit | Core | |
| Wiedergabe: Streaming und lokale Datei | Core | hinter `Player`-Trait |
| Geschwindigkeit mit Tonhöhenkorrektur | Core | |
| Springen vor/zurück, Intervalle einstellbar | Core | |
| Kapitel-Navigation, Sprung aus der Kapitelliste | Core + UI | |
| Zeitmarken in Shownotes anspringen | Core + UI | |
| Position merken und fortsetzen | Core | |
| „Als Nächstes“ (Up Next wie Pocket Casts): ans Ende, an den Anfang, jetzt spielen – aus jeder Folgenliste per Taste | Core + UI | intern eine Playlist mit fester Kennung |
| „Als Nächstes“ umsortieren, entfernen, leeren; automatisch weiter mit der nächsten Folge | Core + UI | |
| Folgensuche (nicht nur Podcasts) | Core | Apple `entity=podcastEpisode`; Folge direkt einreihbar |
| Kapitel vor/zurück, nächste Folge, Stopp als eigene Tasten und Knöpfe | Core + UI | |
| Pegelanzeige im Player | Core + UI | der Core liefert Pegelwerte aus der laufenden Wiedergabe |
| Großer Player mit Kapitelliste und Shownotes | UI | |
| Feeds aktualisieren (manuell und periodisch), Neu-Zähler | Core | |
| Downloads, fortsetzbar | Core | Download-Ordner getrennt vom Bibliotheks-Ordner |
| Medientasten und Systemintegration | Core | MPRIS (Linux), Now Playing (macOS), SMTC (Windows) |
| Schlaf-Timer | Core | |
| Player in der Menüspalte, in jeder Ansicht sichtbar | UI | siehe [oberflaeche.md](oberflaeche.md) |

## Später

- **Mehrere Playlists:** benannte Listen, per Taste füllen, ganz oder teilweise in „Als Nächstes“ legen; später Listen nach Regeln (Smart Playlists).
- Einstellungen pro Podcast: Geschwindigkeit, Intro/Outro überspringen, automatisch in die Warteschlange.
- Auto-Download-Regeln, Speicherlimit, automatisches Aufräumen.
- Stille kürzen, Lautstärke-Anhebung für Sprache.
- **Transkripte** (`podcast:transcript`: VTT, SRT, JSON) mit Mitlaufanzeige und Suche. Auf dem Linux-Desktop bietet das bisher kein Client.
- Eingangskorb-Prinzip für neue Episoden (Castro/AntennaPod), Tags und Ordner, intelligente Filter.
- Statistiken, Lesezeichen.
- Sync mit gpodder-kompatiblen Servern (oPodSync, Nextcloud) als zweites Backend neben dem Ordner.
- Headless-Kommandos für cron (`torrocast refresh`, `torrocast sync`), JSON-Statusausgabe für Statusleisten.
- Podcasting 2.0, zweite Reihe: Podroll, Soundbites, Ort, Live-Episoden; Value4Value zuletzt.
- Offline-Suchindex aus dem Podcast-Index-Datenbankabzug.
- Native Oberflächen für macOS und Windows auf demselben Core.

## Bewusst nicht geplant

- Eigener Server oder Benutzerkonten.
- Spotify- oder YouTube-Anbindung (liefern keine RSS-Feeds).
- Bezahlte Verzeichnisdienste als Standard (nur mit eigenem Key des Nutzers denkbar).
