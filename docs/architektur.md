# Architektur

> **Stand 19.09.2026:** Der hier beschriebene Aufbau ist umgesetzt, bis auf die FFI-Schicht und die nativen
> Oberflächen. Was gebaut ist und was offen bleibt, steht in [offen.md](offen.md).

Zusammenfassung der Entscheidungen aus der Recherche. Begründungen und Quellen stehen
in den verlinkten Dokumenten unter [`research/`](research/).

## Grundsatz

TorroCast besteht aus einem **plattformunabhängigen Core** und austauschbaren
**Oberflächen**. Die erste Oberfläche ist eine TUI. Native Oberflächen für macOS
(SwiftUI) und Windows können später auf denselben Core gesetzt werden.

```
┌──────────────┐  ┌──────────────┐  ┌──────────────┐
│  TUI         │  │  macOS-App   │  │ Windows-App  │   Oberflächen: nur Darstellung
│  (ratatui)   │  │  (SwiftUI)   │  │  (C#/WinUI)  │   und Eingabe, keine Fachlogik
└──────┬───────┘  └──────┬───────┘  └──────┬───────┘
       │ Rust-API        │ UniFFI          │ UniFFI (C#)
┌──────┴─────────────────┴─────────────────┴───────┐
│  torrocast-core: Commands rein, Events raus      │
├───────────┬──────────┬──────────┬────────┬───────┤
│ directory │   feed   │  player  │ store  │library│
│ Apple, PI,│ RSS, P2.0│ Decoder, │ SQLite │ Ordner│
│ fyyd      │ Kapitel  │ Stretch  │ lokal  │ -Sync │
└───────────┴──────────┴──────────┴────────┴───────┘
```

Regel: Was eine zweite Oberfläche ebenfalls bräuchte, gehört in den Core. Die TUI
bekommt fertige, neutrale Daten (z. B. Shownotes als Dokumentmodell, nicht als HTML)
und zeichnet sie nur.

## Sprache: Rust

Begründung in [research/03](research/03-technik-rust-oder-go.md). Kurz:

- Audio ist das Herz eines Podcast-Players, und nur Rust erfüllt die Anforderungen
  (AAC/M4A, Streaming mit Seek, Geschwindigkeit mit Tonhöhenkorrektur, Medientasten
  auf allen Systemen) ohne ffmpeg-Unterprozess oder zwingendes libmpv.
- Der Core lässt sich über UniFFI an Swift und C# anbinden; die Rust↔Swift-Brücke ist
  aus TorroMail bekannt. Go bettet seine komplette Runtime in die Host-App ein und
  lässt sich nicht wieder entladen.
- Cover-Bilder im Terminal, Snapshot-Tests und Kapitel-Bibliotheken sind reifer.

Go wäre nur für eine reine TUI ohne AAC, ohne Tonhöhenkorrektur und ohne native GUIs
im Vorteil.

## TUI auf allen Betriebssystemen

**Ja, dieselbe TUI läuft unter Linux, macOS, Windows, BSD, WSL und über SSH.**
ratatui + crossterm unterstützen alle. Einschränkungen unter Windows:

- Tastenbelegung ohne exotische Modifier-Kombinationen entwerfen.
- Cover-Bilder: Rückfall auf Block-Zeichen einplanen.
- Medientasten brauchen ein verstecktes Fenster (souvlaki); unter macOS muss die
  TUI-Schleife vom Main-Thread weg.

Die nativen GUIs sind damit eine Komfortfrage, keine Voraussetzung für andere
Betriebssysteme.

## Workspace

```
TorroCast/
├── Cargo.toml                  # [workspace], gemeinsame Versionen
└── crates/
    ├── torrocast-net/          # HTTP hinter einem Trait, den Tests ersetzen
    ├── torrocast-directory/    # DirectoryProvider-Trait; Apple, Podcast Index, fyyd; Zusammenführung
    ├── torrocast-feed/         # RSS + podcast:-Namespace, Kapitel (Podlove, JSON, ID3), Shownotes
    ├── torrocast-library/      # Bibliotheks-Ordner: Journale pro Gerät, HLC, Merge, Schnappschüsse
    ├── torrocast-player/       # Streaming, Decoder (symphonia), Zeitstrecker (WSOLA), Ausgabe (cpal)
    ├── torrocast-media/        # Medientasten des Desktops; MPRIS unter Linux
    ├── torrocast-core/         # Fassade: Commands rein, Events raus; Wiedergabe, „Als Nächstes“, Downloads
    └── torrocast-tui/          # ratatui-App (Binary `torrocast`)
```

Noch nicht vorhanden: `torrocast-ffi` (UniFFI-Schicht über `torrocast-core`) und `apps/` für die
nativen Oberflächen. Ein lokales SQLite (`torrocast-store`) hat sich bisher erübrigt: Feeds werden
pro Sitzung im Speicher gehalten, alles Dauerhafte liegt im Bibliotheks-Ordner, Downloads und
Cover in eigenen Ordnern.

`torrocast-core` stellt eine Command/Event-Schnittstelle bereit (Worker-Threads und Kanäle, kein
async-Runtime), die TUI und FFI identisch nutzen. Ein späterer Daemon-Modus (TUI und GUI an derselben
Wiedergabe-Instanz) bleibt damit eine reine Ergänzung.

## Podcast-Verzeichnisse

Details in [research/01](research/01-podcast-verzeichnisse.md).

**Entschieden (19.09.2026):** Apple ist die Hauptquelle und ohne Einrichtung aktiv.
Weitere Quellen schaltet der Nutzer in den Einstellungen dazu. TorroCast liefert
**keinen** Podcast-Index-Key mit; wer Podcast Index nutzen will, trägt seinen eigenen
kostenlosen Key ein. Damit entfällt die Frage, ob ein eingebetteter Key gegen die
Nutzungsbedingungen verstößt.

| Rolle | Quelle | Einrichtung |
|---|---|---|
| Hauptquelle, immer aktiv | Apple iTunes Search + Charts + Genres | keine |
| Zuschaltbar | Podcast Index | eigener Key und Secret des Nutzers (im OS-Schlüsselbund) |
| Zuschaltbar | fyyd (deutschsprachig) | keine, ein Schalter |
| Immer | OPML-Import, Feed-URL direkt | – |

Alle hinter einem `DirectoryProvider`-Trait; neue Quellen sind damit eine Ergänzung,
kein Umbau. Sind mehrere aktiv, werden Ergebnisse parallel geholt, de-dupliziert
(`podcast:guid` → iTunes-ID → normalisierte Feed-URL) und gestaffelt an die Oberfläche
gereicht. Verzeichnisse dienen nur der Entdeckung; abonnierte Podcasts leben
ausschließlich vom RSS-Feed.

Folgen von „Apple zuerst":

- Apple erlaubt rund 20 Anfragen pro Minute. Die Suche startet deshalb nicht bei
  jedem Tastendruck, sondern nach einer kurzen Pause, und Ergebnisse werden
  zwischengespeichert.
- Die Apple-Suche liefert weder Beschreibung noch Sprache. Beides kommt beim Öffnen
  eines Podcasts aus dem Feed selbst.
- Trending, Personen-Suche und der Hinweis „hat Kapitel/Transkript" schon in der
  Trefferliste gibt es nur mit Podcast Index.

## Datenablage

Details in [research/04](research/04-datenablage-und-sync.md).

Zwei getrennte Welten:

| | Lokal, nie synchronisiert | Bibliotheks-Ordner, frei wählbar |
|---|---|---|
| Inhalt | SQLite (Feed-Cache, Episoden, Suchindex), Cover, Downloads, gerätelokale Einstellungen | Abos, Episodenstatus, Positionen, Warteschlange, Favoriten, geräteübergreifende Einstellungen |
| Format | binär, intern | JSON/JSONL, dokumentiert, versioniert |
| Größe | MB bis GB | unter 1–2 MB |
| Verlust | harmlos, rekonstruierbar | das eigentliche Nutzerdatum |

**Entschieden (19.09.2026):** Der Ansatz „ein Unterverzeichnis pro Gerät" ist
bestätigt.

**Im Bibliotheks-Ordner liegt keine SQLite-Datei.** Eine Live-Datenbank in Dropbox,
Syncthing oder iCloud wird laut SQLite-Dokumentation und Praxisberichten (Zotero)
früher oder später beschädigt.

Stattdessen schreibt **jedes Gerät nur in sein eigenes Unterverzeichnis**
(`devices/<geräte-id>/`): ein Journal, an das nur angehängt wird, und ein
verdichteter Schnappschuss. Weil nie zwei Geräte dieselbe Datei schreiben, können
Sync-Dienste keine Konfliktkopien erzeugen. Der Gesamtzustand ist die Vereinigung
aller Geräte-Dateien; pro Wert gewinnt der jüngste Eintrag (Hybrid Logical Clock).

Der Ordner funktioniert damit in Dropbox, Syncthing, Nextcloud, iCloud Drive,
OneDrive, auf einem NAS oder in einem git-Repo. Auch ohne Sync läuft die App über
dasselbe Format – ein Codepfad, kein Sondermodus.

Nicht in den Ordner gehören: Audio-Downloads (eigener, separat wählbarer Ordner),
Cover, Datenbank, Logs, Zugangsdaten (OS-Schlüsselbund).

Server-Sync (gpodder-kompatibel: oPodSync, Nextcloud) kommt später als zweites
Backend hinter demselben `SyncBackend`-Trait.

## Wiedergabe

**Umgesetzt als eigene Pipeline**, ohne mpv: symphonia dekodiert (MP3, AAC, Vorbis, FLAC; Opus
über libopus als Feature), eine eigene Streaming-Quelle lädt in eine temporäre Datei und bedient
weite Sprünge per Teilabruf, ein eigener WSOLA-Zeitstrecker in reinem Rust ändert das Tempo bei
gleicher Tonhöhe, cpal gibt aus. Abweichungen von der Empfehlung der Recherche:

- Kein SoundTouch (LGPL, C++): der eigene Zeitstrecker hat keine Abhängigkeit und keine Lizenzfrage.
- Kein `stream-download`-Crate (braucht ein async-Runtime): die eigene Quelle ist rund 200 Zeilen.
- Pegelwerte für die Anzeige fallen in der Pipeline nebenbei ab.

HE-AAC fehlt weiterhin; das gäbe es nur über die patentbehaftete FDK-Bibliothek.

## Entscheidungen

| # | Frage | Stand |
|---|---|---|
| 1 | Podcast-Index-Key | **Entschieden:** kein Key im Projekt, der Nutzer trägt seinen eigenen ein. Umgesetzt. |
| 2 | Wiedergabe: eigene Pipeline oder mpv? | **Umgesetzt:** eigene Pipeline, siehe oben. |
| 3 | Widersprüchliche Positionen zweier Geräte | Umgesetzt: die jüngste gewinnt. Das Angebot „auf dem anderen Gerät warst du weiter“ fehlt noch. |
| 4 | OPML-Export im Bibliotheks-Ordner | OPML-Import und -Export sind gebaut (von Hand, `I`/`E` bei den Abos). Ein automatischer Export in den Ordner fehlt. |
| 5 | Private Feeds mit Token in der URL | Offen: Sie landen derzeit im Klartext im Ordner. |
| 6 | macOS-Pfade | Umgesetzt: `~/Library/Application Support/TorroCast`. |
| 7 | Verschlüsselung des Bibliotheks-Ordners | Nicht umgesetzt, wie empfohlen. |
| 8 | Lizenz und Sichtbarkeit des Repos | **Entschieden (19.09.2026):** Das Repo bleibt vorerst privat. Der Code steht unter `MIT OR Apache-2.0`. |
| 9 | Verhalten bei Stopp (`x`) | **Entschieden (19.09.2026):** Stelle merken, Player schließen – so umgesetzt. |
