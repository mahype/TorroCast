# Bestehende Podcast-Clients: Funktionsumfang, Kapitelmarken, Sync-Standards

Stand: 19.09.2026. Release- und Commit-Daten stammen aus der GitHub-REST-API dieses
Tages. Markierungen: **[V]** = am Stichtag gegen die Quelle geprüft, **[K]** =
Hintergrundwissen, nicht erneut verifiziert, **[unverifiziert]** = konnte nicht
bestätigt werden. Die GitHub-Code-Suche war während der Recherche eingeschränkt;
deshalb beruhen einige Angaben zu kleineren Clients nur auf deren READMEs.

---

## Teil A – Terminal-Podcast-Clients

### Vergleichstabelle

| Client | Sprache / UI-Lib | Playback | Directory-Suche | Chapters | Sync | Letztes Release / Commit | Zustand |
|---|---|---|---|---|---|---|---|
| [podliner](https://github.com/timkicker/podliner) | C# / Terminal.Gui | extern: LibVLC, mpv (IPC), ffplay, Media Foundation | im README nicht erwähnt [unverifiziert] | ja: podcast:chapters JSON + ID3 CHAP, eigener Tab, `,`/`.` springen; liest Chapters remote per HTTP Range | gpodder.net API v2 + Nextcloud gPodder-Sync | v2.0.1, 2026-09-14 | aktiv, 172 Stars |
| [hullcaster](https://github.com/gilcu3/hullcaster) | Rust / ratatui | eingebaut (rodio) + externer Player | keine gefunden [unverifiziert] | keine gefunden [unverifiziert] | gpodder API | v0.3.0, 2026-04-06 / Commit 2026-06-07 | aktiv, 27 Stars |
| [castero](https://github.com/xgi/castero) | Python / curses | libVLC- oder libmpv-Bindings | nicht im README [unverifiziert] | nein | nein (nur OPML) | v0.9.5, 2022-04-03 | verwaist, 684 Stars |
| [shellcaster](https://github.com/jeff-hughes/shellcaster) | Rust | externes Kommando (Default VLC) | nein | nein | nein (OPML) | v2.0.1, 2022-03-03 | README: "no longer maintained" |
| [lqdev/podcast-tui](https://github.com/lqdev/podcast-tui) | Rust | rodio, externer Fallback | PodcastIndex (`:discover`, `:trending`), eigener API-Key nötig | nein | Sync auf MP3-Player-Geräte, ListenBrainz; kein gpodder | v1.15.0, 2026-06-05 | aktiv, 1 Star |
| [gocaster](https://github.com/amurru/gocaster) | Go / Bubble Tea + Lipgloss | libmpv (go-mpv) | unklar | nicht erwähnt | keiner | keine Releases / 2026-07-25 | jung, 4 Stars; hexagonale Architektur, MPRIS über DBus |
| [curlypod](https://github.com/hitensaxena/curlypod) | Rust / ratatui | headless mpv über JSON-IPC | PodcastIndex | ja, via mpv | keiner | einzelner Push 2026-06-16 | 0 Stars, Ideenquelle |
| [picast](https://github.com/pierdom/picast) | Python | mpv | PodcastIndex | nicht verifiziert | nein | 2026-06-16 | 0 Stars |
| [podboat/newsboat](https://github.com/newsboat/newsboat) | C++ + Rust / ncurses+STFL [K] | externes `player`-Kommando | nein | nein | nur newsboat-seitiger Feed-Sync | r2.44 / 2026-09-19 | sehr aktiv, 3.9k Stars; nur Download-Queue-Manager |
| [termusic](https://github.com/tramhao/termusic) (Podcast-Modus) | Rust / tuirealm+ratatui [K] | symphonia, mpv oder GStreamer per Cargo-Feature | iTunes Search API | nicht verifiziert | nein | v0.13.2, 2026-05-06 | aktiv, 2.2k Stars |

Reine Downloader/CLI: [pcd](https://github.com/kvannotten/pcd) (Go, 2023),
[greg](https://github.com/manolomartinez/greg) (Python, 2021),
[castget](https://github.com/mlj/castget) (C), [podfox](https://github.com/brtmr/podfox)
(Python, 2021), [poddl](https://github.com/freshe/poddl) (C++),
[gPodder `gpo`](https://github.com/gpodder/gpodder) (3.11.5, kein Playback [K]).

### Layout- und UX-Notizen

- **podliner**: Pane-Kette Feeds → Episoden → Shownotes → Chapters (mit `h`/`l`), Player-Bar; vim-Keys (`j/k`, `gg/G`, `/`); Colon-Kommandos (`:add`, `:queue`, `:sync`, `:sleep 30m`, `:feed speed 1.5`); MPRIS, gpodder-Credentials im OS-Keyring, Undo für destruktive Aktionen, Per-Feed-Speed und Auto-Download, Sleep-Timer, Maus. Schwächen: .NET-Runtime, kein eingebautes Audio.
- **hullcaster**: Panels Podcasts/Episoden/Queue/Unread, `Tab` wechselt, Keybinding-Leiste unten; `Space` Play/Pause, `e` enqueue, Ctrl+Up/Down Queue umsortieren; TOML-Konfiguration; headless `hullcaster sync` für cron.
- **castero**: fünf umschaltbare Layouts (`1`–`5`), Spalten Feed/Episode/Metadaten + Queue; `]`/`[` Speed, `f`/`b` Seek; SQLite.
- **shellcaster**: zwei Panes + Details; Zahlentasten togglen Played/Downloaded-Filter.
- **lqdev/podcast-tui**: Emacs-artig (Buffers, Minibuffer mit Completion, Keybinding-Presets default/vim/emacs).
- **termusic Podcast-Modus**: Feed-Liste mit „(neu/gesamt)“, Episodenliste mit `[D]` = heruntergeladen, durchgestrichen = gespielt. Code ist GPLv3 (Lizenz beachten, falls Code übernommen würde).

### Musik-TUIs als UX-Inspiration

| App | UI-Lib | Audio-Backend | MPRIS | Cover-Art | Ideen |
|---|---|---|---|---|---|
| [spotify-player](https://github.com/aome510/spotify-player) (v0.25.1, 2026-09-09) | ratatui | librespot; rodio default | MPRIS (Linux), OS-Events macOS/Windows | ratatui-image: Kitty/iTerm2/Sixel, Fallback Block-Zeichen | Daemon-Modus, Popups, Command Palette, Playback-Bar |
| [ncspot](https://github.com/hrkfdn/ncspot) (v1.4.0, 2026-08-21) | cursive | librespot-playback | MPRIS via zbus + UNIX-Socket mit JSON-State | `cover`-Feature | F-Key-Screens, vim-Keys, `:`-Kommandozeile |
| termusic | tuirealm [K] | symphonia / mpv / GStreamer | MPRIS über dbus | Kitty, iTerm2; Sixel optional | Client/Server-Split (`termusic-server` + TUI über gRPC) |
| [kew](https://github.com/ravachol/kew) (v4.3.4, 2026-09-12) | C, eigen | miniaudio [unverifiziert] | Linux/macOS | Sixel-Cover (chafa) | großes Cover + Visualizer, Themes aus Albumfarben |
| [musikcube](https://github.com/clangen/musikcube) (3.0.5) | C++ / curses | eigene Engine, ffmpeg-Decode | [unverifiziert] | keine | `hotkeys.json`, Kommandoleiste, eingebauter Streaming-Server |

### Erkenntnisse

1. Die etablierten Clients sind verwaist (castero 2022, shellcaster aufgegeben, pcd 2023, greg 2021). Aktiv: podliner (C#), hullcaster (Rust), termusic-Podcast-Modus. **Es gibt keinen aktiven Rust-TUI-Client mit Directory-Suche, Chapters und Sync zusammen – das ist die Lücke für TorroCast.**
2. Zwei Playback-Muster: (a) mpv über IPC/libmpv (podliner, curlypod, gocaster, picast) – Chapters, Speed, Skip-Silence quasi gratis; (b) reines Rust mit rodio/symphonia (hullcaster, podcast-tui, termusic, spotify-player) – keine Laufzeitabhängigkeit, aber Chapters, Speed und Streaming-Seek selbst bauen.
3. Directory-Suche: PodcastIndex (API-Key nötig) oder iTunes Search (ohne Key). Kein TUI nutzt das gpodder.net-Directory.
4. Sync: nur podliner und hullcaster (gpodder); nur podliner zusätzlich Nextcloud.
5. Chapters erstklassig nur in podliner. Inline-Cover-Art hat keiner der Podcast-Clients verifiziert; Standardweg in Rust ist `ratatui-image`.
6. Das ncspot-Muster (MPRIS + JSON-State-Socket) ist attraktiv für Statusbars und Scripting.

---

## Teil B – Funktionsumfang führender GUI-Clients

### Statuschecks

- **Pocket Casts [V]**: Open Source sind nur die Mobile-Apps ([android](https://github.com/Automattic/pocket-casts-android), ios), MPL-2.0. Sync-Backend und Web-Player nicht offen [K]. v8.0 (Nov 2025) ersetzte Filter durch Smart/Manual Playlists.
- **Podverse [V]**: lebt, mitten im Rewrite; AGPL-3.0.
- **Castro [V]**: seit Jan 2024 bei Bluck Apps; iPad + Geräte-Sync Aug 2025.
- **Overcast [V]**: Transkripte seit 2026.04, serverseitig erzeugt, Tap-to-Seek.
- **Apple Podcasts [V]**: Auto-Transkripte seit iOS 17.4; iOS 26 Enhance Dialogue, 0.5–3x; iOS 26.2 automatisch generierte Chapters, Timed Links.
- **FOSS-Versionen [V]**: Kasts 26.04.0 (2026-04-16), GNOME Podcasts 25.3 (2025-10-17, Chapters neu), AntennaPod 3.12.1 (2026-09-09).

### Konsolidierte Feature-Matrix

| Bereich | Befund |
|---|---|
| Discovery/Suche | AntennaPod: iTunes, fyyd, gpodder.net, Podcast Index [V]. Kasts: Podcast Index [V]. gPodder: gpodder.net + URL-Präfixe `yt:`/`sc:` [V]. Podverse/Podcast Guru/Fountain/TrueFans: Podcast Index. Pocket Casts/Overcast/Castro/Apple: eigene Server-Directories [K]. |
| Abo-Organisation | Pocket Casts Folders/Smart Folders [V]; Castro Folders [V]; AntennaPod Tags [K]; Overcast Playlists, Apple Stations [K]; gPodder Sections [K]. |
| Episodenliste, Filter, States | AntennaPod: Inbox (neu), gespielt, Favoriten, History, Suche in Shownotes. Castro: Inbox→Queue-Triage [V]. gPodder: played/unplayed, downloaded, archived, Regex-Filter [V]. Pocket Casts: Archive, Starred, In-Progress, Smart Playlists. Resume-Position: überall. |
| Queue / Up Next | AntennaPod, Kasts, Castro, Pocket Casts (synchronisiert), Fountain. gPodder: keine [V]. |
| Downloads, Auto-Download, Cleanup | AntennaPod: pro Feed, WLAN/Mobil-Regeln, Speicherlimit, Smart Deletion [V]. gPodder: 4 Auto-Download-Modi, Cleanup nach 1–30 Tagen [V]. Kasts: fortsetzbare Downloads. Pocket Casts: Auto-Archive [K]. |
| Streaming | überall außer gPodder. |
| Playback | Variable Speed überall. Silence-Trimming: Overcast Smart Speed, Castro (Plus), Pocket Casts [V], AntennaPod [K]. Voice/Volume Boost: Overcast, Castro, Pocket Casts, Apple Enhance Dialogue [V]. Sleep-Timer: AntennaPod, Kasts [V], Rest [K]. Per-Podcast-Settings: Castro (Speed, Auto-Queue, Filter, Skip Intro) [V], AntennaPod (Speed, Skip Intro/Ending), Apple [V]. |
| Chapters | AntennaPod: ID3, PSC, VorbisComment, podcast:chapters JSON [V]. Kasts: PSC + ID3 (nur lokale Dateien). GNOME Podcasts: seit 25.3. Castro: Chapters abwählen/überspringen (Plus) [V]. Pocket Casts: Chapter-Deselect [K]. Apple: embedded + auto-generiert [V]. Fountain: Chapter-Artwork [V]. gPodder: nativ keine [K]. |
| Show Notes | HTML überall; klickbare Timestamps: Kasts, Apple [V], AntennaPod/Overcast/Pocket Casts [K]. |
| Transkripte | podcast:transcript: AntennaPod (SRT/VTT/JSON), Pocket Casts [V], Podverse, Podcast Guru, Fountain, TrueFans. Auto-generiert: Apple, Overcast, Fountain [V]. Keine: Kasts, GNOME Podcasts, gPodder. |
| Positions-Sync | Offen: AntennaPod, Kasts (gpodder API / Nextcloud) [V]; gPodder (v. a. Abos). Proprietär: Pocket Casts, Overcast, Castro (Plus), Apple (iCloud), Podverse (Premium), Fountain. GNOME Podcasts: kein Sync [K]. |
| OPML | Import+Export: AntennaPod, gPodder [V]. Nur Import: GNOME Podcasts [V]. Apple: keines [K]. |
| Notifications, Media Keys, MPRIS | AntennaPod: Notification, Widget, Headset [V]. Kasts: MPRIS2 [K], Suspend-Inhibit [V]. GNOME Podcasts: MPRIS [K]. |
| Statistiken | AntennaPod; Pocket Casts (inkl. Jahresrückblick) [K]; Desktop-Linux-Clients: keine [K]. |
| Extras | Pocket Casts: Bookmarks (Plus), Datei-Upload. Castro: Sideloading. Fountain: Clips, Nostr. |

### Podcasting-2.0-Support (Selbstauskünfte der Entwickler)

Quelle: [apps.json der Podcast-Index-Web-UI](https://raw.githubusercontent.com/Podcastindex-org/web-ui/master/server/data/apps.json).
Einträge sind möglicherweise veraltet.

- AntennaPod: Search, Funding, Chapters, Social Interact, Transcript
- Pocket Casts: Alternate Enclosure, Chapters, Podping, Transcript, Funding, Podroll, Remote Item
- Castro: Transcript · Apple: Transcript, Chapters, Txt · Kasts/gPodder: nur Search
- Podverse, Podcast Guru, Fountain, TrueFans: nahezu der volle Stack (Value/Boostagrams, Live Item, Person, Soundbite, Location, Podroll, GUID, Medium …)

Fazit: Nur **Transcript, Chapters, Funding** erreichen Mainstream-Clients.
Tag-Definitionen: [podcast-namespace 1.0.md](https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/1.0.md).

### Pflichtumfang und Differenzierer

- **Pflichtumfang**: Abo per URL + Directory-Suche, Episodenliste mit neu/gespielt + Resume, Streaming + Download, Auto-Download, variable Speed, Skip vor/zurück, Sleep-Timer, HTML-Shownotes, OPML-Import, OS-Media-Controls, Queue; eingebettete Chapters inzwischen faktisch auch.
- **Verbreitet, nicht universell**: Per-Podcast-Settings, Silence-Trimming/Voice Boost, Cleanup-Regeln, Favoriten, Geräte-Sync, klickbare Timestamps, podcast:transcript/chapters/funding.
- **Differenzierer**: Auto-Transkripte, Audio-DSP, Inbox-Triage, Smart Playlists, offene Sync-Protokolle, Statistiken, Bookmarks/Clips, Chapter-Vorauswahl, voller P2.0-Stack.
- **Lücke auf Desktop-Linux**: Kein Linux-GUI-Client bietet nach Befundlage Transkripte, Silence-Trimming, Statistiken oder P2.0-Tags jenseits von Suche und Chapters.

---

## Teil C – Kapitelmarken im Detail

### Die drei Mechanismen

1. **Podlove Simple Chapters** ([Spec](https://podlove.org/simple-chapters/)): Namespace `http://podlove.org/simple-chapters`. `<psc:chapters version="1.2">` im `<item>`; `<psc:chapter>` mit `start` (NPT: `HH:MM:SS.mmm`, `MM:SS`, `SS`), `title` (beide Pflicht), `href`, `image` optional. Keine Endzeit, kein Hidden-Flag.
2. **podcast:chapters** ([JSON-Spec 1.2.0](https://github.com/Podcastindex-org/podcast-namespace/blob/main/docs/examples/chapters/jsonChapters.md)): `<podcast:chapters url type="application/json+chapters"/>`. Chapter: `startTime` (float s) Pflicht; optional `title`, `img`, `url`, `toc` (default true; false = „stilles“ Chapter, z. B. nur Bildwechsel), `endTime`, `location`. Die Datei kann sich nach Veröffentlichung ändern → HTTP-Caching mit Revalidierung.
3. **Eingebettet in der Mediendatei**:
   - **ID3v2 CHAP/CTOC** ([Mirror der Spec](https://mutagen-specs.readthedocs.io/en/latest/id3/id3v2-chapters-1.0.html)): CHAP = Element-ID, Start/Ende in ms, Sub-Frames TIT2/TIT3/WXXX/APIC. CTOC = Flags + Kind-IDs.
   - **MP4/M4A**: Nero `moov.udta.chpl` (keine formale Spec, Referenz: FFmpeg [`mov_read_chpl`](https://github.com/FFmpeg/FFmpeg/blob/master/libavformat/mov.c)) und QuickTime-Chapter-Track (`tref`→`chap` verweist auf Text-Track).
   - **Vorbis Comments** ([Xiph Chapter Extension](https://wiki.xiph.org/Chapter_Extension)): `CHAPTERxxx=HH:MM:SS.mmm`, `CHAPTERxxxNAME=`; in Ogg/Opus und FLAC.

### Priorisierung und Merge in realen Clients

- **AntennaPod** ([ChapterUtils.java](https://github.com/AntennaPod/AntennaPod/blob/develop/ui/chapters/src/main/java/de/danoeh/antennapod/ui/chapters/ChapterUtils.java)): lädt alle drei, `merge(merge(PSC, embedded), JSON)`. Regeln: unterschiedliche Länge → längere Liste gewinnt; gleiche Länge → indexweise, bei Startzeit-Abweichung > 1000 ms gewinnt die Liste mit mehr ausgefüllten Feldern; sonst werden leere Felder gegenseitig aufgefüllt. Effektive Priorität bei Gleichstand: podcast:chapters > embedded > PSC. M4A: nur Nero `chpl`.
- **Pocket Casts iOS** ([ChapterManager.swift](https://github.com/Automattic/pocket-casts-ios/blob/trunk/podcasts/ChapterManager.swift)): kein Merge; Reihenfolge embedded → podcastIndex JSON → Podlove → servergeneriert. Code-Kommentar: „Prioritize embedded chapters, given for some shows it will take into account dynamic ads“.
- **Kasts**: zuerst ID3 CHAP via TagLib (nur lokale Datei), sonst PSC; kein podcast:chapters JSON.
- **gPodder**: PSC geparst, podcast:chapters nur als URL; kein Embedded-Parsing gefunden.

### Eingebettete Kapitel lesen, ohne die ganze Datei zu laden

- **AntennaPod** macht es ohne Range-Requests: normales GET, sequentiell lesen, Verbindung früh schließen. **podliner** nutzt laut README HTTP Range.
- **Empfohlene Strategie**:
  - MP3: ~64 KB vorab holen, `ID3` prüfen, syncsafe 28-Bit-Größe lesen, exakt den Tag nachladen. APIC (Cover und Chapter-Bilder) können den Tag auf mehrere MB treiben → inkrementell parsen, APIC-Payload überspringen, Größenlimit setzen.
  - MP4: Top-Level-Atome über Header ablaufen; bei `moov` hinter `mdat` über Atomgrößen dorthin springen.
  - Ogg/Opus/FLAC: Comment-Header in den ersten Pages/Metadatenblöcken.
  - Redirect-Präfixe (podtrac, chartable, op3): Range-Header bei jedem Hop erneut senden; `200` statt `206` behandeln. Verhalten konkreter Präfixe nicht getestet.
  - **Dynamic Ad Insertion**: jede Anfrage kann eine anders zusammengesetzte Datei liefern; eingebettete Chapters stimmen nur für genau die Datei, aus der sie kommen. Ideal: Chapters aus derselben Response parsen, die abgespielt wird.
  - **IAB** ([Podcast Measurement Guidelines v2.2](https://iabtechlab.com/wp-content/uploads/2024/02/PodcastMeasurement_v2.2_pc.pdf)): Ein reiner Tag-Fetch sollte bei konformen Hostern nicht als Download zählen; bei nicht-konformen eventuell doch → **lazy laden** (beim Öffnen der Episode), nie für ganze Feeds. Eindeutiger User-Agent, Enclosure-URL nicht verändern.

### Bibliotheken

**Rust**

| Crate | Chapters? |
|---|---|
| [`id3`](https://docs.rs/id3/latest/id3/frame/) 1.17.1 | **Ja**: `frame::Chapter`, `frame::TableOfContents`. |
| `lofty` 0.25.3 | **Nur ID3v2, erst seit 0.25.0**. Keine MP4-Chapters gefunden. |
| [`mp4ameta`](https://docs.rs/mp4ameta) 0.13.0 | **Ja, beide**: `chapter_list` (chpl) und `chapter_track`; braucht `Read+Seek` → remote nur mit Range-gestütztem Reader. |
| `symphonia` 0.6.1 | **Teilweise**: `FormatReader::chapters()`, gefüllt aus ID3v2 CHAP, Vorbis CHAPTERxxx, MKV. **Keine isomp4-Chapters.** |
| [`chapters`](https://github.com/rssblue/chapters) 0.4.2 | PC2.0-JSON, ID3v2, Shownotes-Timestamps; kein PSC; seit 2023-10 ungepflegt. |
| `rss` 2.1.2 | kein typisiertes psc/podcast; beides über die generische `extensions()`-Map erreichbar. |
| `feed-rs` 2.4.0 | Nein (kein podcast/psc, keine generische Extension-Map gefunden). |

JSON-Chapters (serde) und PSC (quick-xml) sind einfach selbst zu schreiben.

**Go**

- [`n10v/id3v2`](https://github.com/n10v/id3v2/blob/master/chapter_frame.go): CHAP ja; WXXX/APIC in Chapters nicht exponiert, **kein CTOC**.
- `dhowden/tag`: **Nein**. `abema/go-mp4`: keine `chpl`/`chap`-Box-Typen.
- `mmcdole/gofeed`: keine typisierten Structs, aber `Item.Extensions["psc"]` / `["podcast"]` als Baum – brauchbar.

**Fallbacks**: `ffprobe -show_chapters -of json <url>`. libmpv: Property
`chapter-list` – wenn Playback über mpv läuft, kommen eingebettete Chapters des
tatsächlich gespielten Streams gratis.

**Design-Empfehlung**: Ein normalisierter Typ
`Chapter { start, end?, title?, url?, image?, hidden (toc=false), source }`; Quellen
lazy laden; Merge-Policy explizit festlegen. Für Rust: `id3` + `mp4ameta` +
handgeschriebene Parser für Vorbis/JSON/PSC ergibt die beste Abdeckung.

---

## Teil D – Sync-Standards zwischen Clients

### gpodder.net API – [Doku](https://gpoddernet.readthedocs.io/en/latest/api/)

- Synct: Abos (pro Gerät, Deltas mit `since`), Episode Actions (`download`, `delete`, `play` mit `started`/`position`/`total`, `new`), Geräte, generischer Settings-Store.
- Auth: HTTP Basic → `sessionid`-Cookie; kein OAuth.
- Schwächen: Episoden-Identität über Media-URL + Feed-URL, kein GUID → bricht bei Tracking-Präfixen und Feed-Umzug; kein Queue-Sync; kein „played“-Flag im Kern; Zuverlässigkeit – AntennaPod-Doku: Server „often overloaded“, empfiehlt Self-Hosting ([Quelle](https://antennapod.org/documentation/general/synchronization)).

### Selbst gehostete Server

| Projekt | Sprache | Umfang | Zustand |
|---|---|---|---|
| [oPodSync](https://github.com/kd2org/opodsync) | PHP + SQLite | Auth, Abos, Episode Actions, Geräte + Nextcloud-Endpoints | aktiv, Commit 2026-09-11 |
| [Nextcloud GPodder Sync](https://github.com/thrillfall/nextcloud-gpodder) | PHP | eigene Minimalvariante, keine Geräte, optionales `guid` | sehr aktiv, v3.18.0 2026-09-18 |
| [PodFetch](https://github.com/SamTV12345/PodFetch) | Rust | „GPodder integration“, Umfang unverifiziert | aktiv, v5.2.3 |
| [goPodder](https://github.com/cbrgm/gopodder) | Go | Abos, Episode Actions, Geräte | aktiv, v1.2.5 |
| PinePods | Go-Sidecar in Rust-App | Abos + Episode Actions | aktiv, 0.9.0 |

Clients mit gpodder-API: AntennaPod, Kasts, gPodder, PinePods, Cardo, Repod,
GNOME Podcasts; im TUI-Bereich podliner und hullcaster.

### Open Podcast API ([openpodcastapi.org](https://openpodcastapi.org/))

- Ziel: gpodder-Nachfolger mit Abos, Position, played/favorite, Queue inkl. Reihenfolge; GUID-basierte Identität. Mitwirkende aus AntennaPod, Kasts, nextcloud-gpodder, Funkwhale.
- Status: „All specifications are currently 'in progress'“. Auf `main` nur **Subscriptions**; Episodes-Spec seit 2024-06 als offener PR. Kein ausgelieferter Client gefunden.
- **Noch nicht nutzbar.**

### Fazit Sync

**Die gpodder-API ist der einzige De-facto-Standard** (mehrere unabhängige Clients,
mindestens fünf gepflegte Server). Später unterstützen: konfigurierbare Server-URL
(gpodder.net nicht hart verdrahten), zusätzlich die Nextcloud-Variante. Den Core so
entwerfen, dass ein Open-Podcast-API-Backend später passt: Sync-Abstraktion,
Feed-GUID + Episode-GUID + Änderungszeitstempel pro Feld lokal speichern. Open
Podcast API beobachten, nicht implementieren. Pocket Casts, Apple, Overcast:
proprietär, überspringen.

Das ist **zusätzlich** zur Ordner-Synchronisation gedacht, siehe
[04-datenablage-und-sync.md](04-datenablage-und-sync.md).

---

## Nicht verifizierte Punkte

- Directory-Suche, Chapters, MPRIS in castero, hullcaster, gocaster (nur READMEs); UI-Crates von shellcaster, lqdev/podcast-tui, termusic.
- GNOME Podcasts: Queue, Suche, Speed, MPRIS, Chapter-Quellformat. Kasts: MPRIS, OPML, Filter.
- Podcast Guru, Fountain, TrueFans: über apps.json hinaus dünne Quellenlage.
- Verhalten konkreter Tracking-Präfixe bei Range-Requests nicht getestet.
- PodFetch-API-Umfang; gpodder.net-Uptime (nur eine eigene Probe + Drittberichte).
