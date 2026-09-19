# Rust oder Go für Core und TUI

Stand: 19.09.2026. Versionen und Daten stammen aus der crates.io-API, GitHub-Releases,
proxy.golang.org und offiziellen Dokumentationen. Nicht Belegbares ist als
**UNVERIFIZIERT** markiert und am Ende gesammelt.

## Kurzfassung

**Empfehlung: Rust (ratatui + crossterm) für Core und TUI.** Drei Punkte geben den
Ausschlag, in dieser Reihenfolge:

1. **Audio.** Rust deckt alle Anforderungen mit gepflegten Crates ab: MP3, AAC-LC/M4A,
   Vorbis, FLAC nativ in symphonia; Opus über einen Adapter; HTTP-Streaming mit Seek;
   Geschwindigkeit mit Tonhöhenkorrektur; Medientasten auf allen drei Betriebssystemen.
   Genau dieser Stack läuft bereits in termusic und pdcst. Go hat keinen gepflegten
   MP3-Decoder, keinen M4A/HE-AAC-Pfad, keine etablierte Time-Stretch-Bibliothek und
   nichts für macOS Now Playing oder Windows SMTC.
2. **Core mit nativen GUIs teilen.** UniFFI deckt Swift offiziell ab und C# über
   NordSecurity (async, Callbacks). Go bietet nur gomobile (ObjC/Java, enge Typgrenzen,
   ein Panic beendet den Prozess) oder rohes c-shared mit nicht entladbarer Runtime im
   Host. Die Rust↔Swift-Brücke ist aus TorroMail bereits bekannt.
3. **Cover-Bilder im Terminal.** ratatui-image ist ausgereift (Kitty, Sixel, iTerm2,
   Halfblocks, tmux); in Go gibt es kein vergleichbar reifes Gegenstück.

Go punktet beim „modernen Look ohne Aufwand“ (Lip Gloss, automatisches
Farb-Downsampling) und seit oto v3.5.0 bei cgo-freier Cross-Kompilierung inklusive
Audioausgabe. Diese Vorteile tragen aber nur für eine reine TUI mit MP3/FLAC/Vorbis;
sobald AAC/M4A, Time-Stretching oder eine GUI-Anbindung dazukommen, fallen sie weg.

Zwei Befunde, die gängigen Annahmen widersprechen:

- **symphonia kann Opus weiterhin nicht nativ** (0.6.1, Issue #8 offen). Workaround: `symphonia-adapter-libopus`.
- **feed-rs 2.4.0 parst den `podcast:`-Namespace nicht**; das liegt nur im unveröffentlichten 3.0.0-dev-Zweig.

---

## 1. TUI-Frameworks

### Rust

- **ratatui 0.30.2** (2026-06-19). 0.30 brachte die Aufteilung in `ratatui-core`, `ratatui-widgets` und separate Backend-Crates, `no_std`, `ratatui::run()`, MSRV 1.86. https://ratatui.rs/highlights/v030/
- **crossterm 0.29.0** (2025-04-05) – seit ca. 17 Monaten kein Release, master aber aktiv. Laut ratatui-Doku die Standardwahl. https://ratatui.rs/concepts/backends/comparison/

| Crate | Version | Datum | Hinweis |
|---|---|---|---|
| tui-textarea (rhysd) | 0.7.0 | 2024-10 | faktisch verwaist |
| **ratatui-textarea** | 0.9.2 | 2026-06-12 | Fork der ratatui-Org, von gitui genutzt |
| tui-input | 0.15.4 | 2026-08-10 | von hullcaster genutzt |
| **ratatui-image** | 11.1.0 | 2026-09-17 | |
| tachyonfx | 0.25.2 | 2026-09-06 | Effekte, jetzt in der ratatui-Org |
| tui-scrollview / tui-widgets | 0.6.7 / 0.7.11 | 2026 | |
| tui-tree-widget | 0.24.1 | 2026-08-09 | |
| throbber-widgets-tui | 0.11.1 | 2026-06-19 | |
| tui-logger | 0.18.3 | 2026-07-04 | |
| tui-markdown | 0.3.9 | 2026-07 | |
| insta | 1.48.0 | 2026-06-11 | |

- **ratatui-image**: Sixel, Kitty (inkl. Unicode-Placeholders), iTerm2, Halfblocks als Rückfall; erkennt Protokoll und Font-Pixelgröße automatisch; tmux-Passthrough vorhanden. Laut README problematisch: Alacritty, Konsole, Warp. https://github.com/ratatui/ratatui-image
- Alternativen: cursive 0.21.1 (langsame Releases), iocraft 0.9.1 (React-artig, klein), r3bl_tui (geringe Verbreitung).
- **Maus**: crossterm liefert Press/Release/Drag/Move/Scroll; ratatui hat kein eingebautes Hit-Testing – die App rechnet Koordinaten selbst auf Bereiche um.
- **Async**: ratatui ist synchron; empfohlen tokio + crossterm `EventStream`. Offizielle Templates: https://github.com/ratatui/templates
- **Tests**: offizielles Rezept `TestBackend` + `insta::assert_snapshot!`; Farben werden nicht mitgeprüft. https://ratatui.rs/recipes/testing/snapshots/

### Go

- **Bubble Tea v2 ist stabil**: v2.0.0 am 2026-02-24, aktuell v2.0.9. Neu: Cell-Diffing-Renderer, Kitty-Keyboard-Erweiterungen mit Rückfall, deklaratives `tea.View`, eingebautes Farb-Downsampling. https://github.com/charmbracelet/bubbletea/releases/tag/v2.0.0
- Lip Gloss v2.0.6, Bubbles v2.2.1, glamour v2.0.1.
- **Bilder**: keine offizielle Komponente (Issue #163 seit 2021 offen). `blacktop/go-termimg` meldet Layout- und Redraw-Probleme mit v2. Grundproblem: der Cell-Diffing-Renderer kollidiert mit Bild-Escapes.
- **teatest**: weiterhin experimentell.

### Bewertung

- Moderner Look: Bubble Tea/Lip Gloss liefern mit weniger Aufwand ein poliertes Ergebnis; ratatui erreicht dasselbe Niveau (yazi, television, spotify-player), verlangt aber mehr eigene Gestaltung. tachyonfx hat kein Go-Gegenstück.
- Bilder: klarer Vorteil Rust. Tests: Vorteil Rust.
- Bestehende Podcast-TUIs: Rust hat hullcaster, pdcst, termusic. In Go nur Hobbyprojekte.

---

## 2. Läuft die TUI auch auf anderen Betriebssystemen?

**Ja. Ein Quellstand für Linux, macOS, Windows, BSD, WSL und SSH ist mit beiden Stacks
machbar.** Einschränkungen:

- **Windows-Eingabe**: crossterm nutzt die WinAPI-Konsole → Windows liefert Press- und Release-Events, die App muss auf `KeyEventKind::Press` filtern.
- **Kitty-Keyboard-Protokoll unter Windows**: Windows Terminal hat es erst in 1.25 Preview (2026-03-05); crossterms `PushKeyboardEnhancementFlags` schlägt unter Windows grundsätzlich fehl (Issue #1022). Modifier kommen trotzdem über WinAPI. **Folge: Tastenbelegung ohne exotische Modifier-Kombinationen entwerfen.**
- **Bilder in Windows Terminal**: Sixel stabil seit v1.22 (2025-02). crossterm `window_size()` liefert dort aber 0 Pixel (#1021) → Halfblocks-Rückfall einplanen.
- **Farben/Unicode**: Bubble Tea rechnet Farben automatisch herunter; bei ratatui muss die App das selbst lösen (UNVERIFIZIERT). Offene Issues zu doppelt breiten Glyphen (#2526).
- **BSD**: nominell unterstützt, keine Praxisberichte gefunden (UNVERIFIZIERT).
- **SSH/tmux**: Bild-Escapes gehen über SSH; tmux braucht `allow-passthrough on`.

### Paketierung

- **dist (cargo-dist)**: aktiv gepflegt. Installer: shell, powershell, npm, Homebrew, msi. **Nicht** unterstützt: winget, scoop, AUR, deb, rpm.
- **GoReleaser v2.18.2** baut seit v2.5 auch Rust (cargo-zigbuild) und deckt Homebrew, winget, scoop, AUR, deb/rpm/apk, Snap ab – das einzige Werkzeug, das nahezu alle Kanäle abdeckt. https://goreleaser.com/customization/builds/rust/
- Rust-spezifisch: cargo-binstall, cargo-deb, cargo-generate-rpm; PKGBUILD für AUR von Hand.
- **Flatpak ungeeignet**: Flathub nimmt Konsolen-Software nicht an.

---

## 3. Audio

### Rust

- **rodio 0.22.2**: hoher API-Umbau zwischen Versionen. `set_speed` **verändert die Tonhöhe** – kein Time-Stretcher. Kein Silence-Trimmer.
- **cpal 0.18.2** (2026-08-16): native PipeWire- und PulseAudio-Hosts neu, erhalten noch viele Fixes. rodio 0.22.2 hängt noch an cpal 0.17.
- **symphonia 0.6.1** (2026-08-13; MPL-2.0): neu u. a. **Kapitel** (ID3v2, Matroska, Vorbis-Comments).

| Codec | Status |
|---|---|
| MP3 | ausgezeichnet |
| AAC-LC | sehr gut |
| HE-AAC / v2 | **nicht implementiert** |
| Vorbis, FLAC, WAV | ausgezeichnet |
| ALAC | sehr gut |
| **Opus** | **nicht implementiert** (Issue #8 offen) |
| MP4-Container | sehr gut |

- **Opus-Workaround**: `symphonia-adapter-libopus` 0.3.0 (bündelt libopus; 0.3.x für symphonia 0.6). termusic nutzt es. Achtung: rodio hängt noch an symphonia 0.5.5 – Versionsschere.
- **HE-AAC**: nur über `symphonia-adapter-fdk-aac` (FDK-Lizenz patentbehaftet). Die meisten Podcasts sind AAC-LC; HE-AAC kommt bei niedrigen Bitraten vor.
- **stream-download 0.24.4**: HTTP-Range-Seek, liefert `Read + Seek` für symphonia. termusic und pdcst nutzen es so. https://github.com/aschey/stream-download-rs
- **Geschwindigkeit mit Tonhöhenerhalt**:

| Crate | Version | Lizenz | Hinweis |
|---|---|---|---|
| `soundtouch` | 0.5.4 | LGPL-2.1 | bündelt SoundTouch, von termusic genutzt; für Sprache bewährt |
| `signalsmith-stretch` | 0.1.3 | MIT | gebündeltes C++ |
| `wsola` | 0.1.0 | MIT | reines Rust, aus pdcst, sehr neu |
| `rubberband` | 0.1.5 | **GPL-2.0+** | brandneu |

  rubato ist reines Resampling.
- **Stille kürzen**: kein Crate; eigene Stufe vor dem Stretcher (RMS über 10–20 ms, Schwelle ca. −45 dB, Hysterese, kurze Crossfades) – Empfehlung, keine recherchierte Implementierung.
- **Alternative Backends**: `libmpv2` 6.0.0 (mpv deckt alle Codecs inkl. HE-AAC ab und korrigiert die Tonhöhe automatisch) – aber mpv ist standardmäßig GPLv2+, und Bündelung auf macOS/Windows ist schwer. `gstreamer` 0.25.3 – auf Linux trivial, auf macOS/Windows schwerste Bündelungslast.
- Referenz: termusic hat alle drei Backends hinter einer Abstraktion. https://github.com/tramhao/termusic · pdcst: https://github.com/jhheider/pdcst

### Go

- faiface/beep aufgegeben → **gopxl/beep v2.1.1**: WAV, MP3, Vorbis, FLAC – **kein AAC, kein Opus**; Geschwindigkeit verändert die Tonhöhe.
- **oto v3.5.0** (2026-09-05): Linux/BSD jetzt cgo-frei.
- Decoder: hajimehoshi/go-mp3 **archiviert**; AAC nur über ein 11-Sterne-Projekt ohne MP4-Demux. cliamp, der populärste Go-TUI-Player, verlangt ein `ffmpeg`-Binary für AAC/ALAC/Opus.
- Time-Stretch in Go: keine etablierte Bibliothek.
- **Der einzige realistische Go-Weg ist libmpv** – mit denselben GPL- und Bündelungsfolgen.

### Betriebssystem-Integration

- **souvlaki 0.8.3**: MPRIS + macOS Now Playing + Windows SMTC. Fallstricke für eine TUI: **Windows braucht ein verstecktes Fenster mit Message-Pump** (so macht es spotify-player); **macOS braucht einen Run-Loop auf dem Main-Thread** → die TUI-Schleife muss auf einen anderen Thread. https://github.com/Sinono3/souvlaki
- Go: MPRIS über godbus machbar; für macOS/Windows nichts Etabliertes.

**Urteil: Rust hat die klar stärkere Audio-Geschichte.**

---

## 4. Core mit nativen GUIs teilen

### Rust

- **UniFFI 0.32.1**: Swift, Kotlin, Python voll; `async fn` → Swift async/await, Callback-Interfaces. https://mozilla.github.io/uniffi-rs/latest/
- **uniffi-bindgen-cs** (NordSecurity): folgt UniFFI **0.31**, nicht 0.32. **Folge: UniFFI auf 0.31.x festnageln, solange C# ein Ziel ist.** https://github.com/NordSecurity/uniffi-bindgen-cs/releases
- Weitere: swift-bridge, cbindgen, csbindgen (roh, kein Objekt- oder Async-Modell).
- Rust-eigene GUI-Optionen: Tauri 2 stabil; Slint 1.18 am produktionsreifsten (Lizenzmodell beachten); egui, iced, Dioxus.

### Go

- c-shared/c-archive: volle Go-Runtime im Host; **kein Entladen möglich** (golang/go#11100 seit 2015 offen); unter Windows nur MinGW/clang, **kein MSVC**.
- gomobile bind: nur ObjC/Java-Bindings, enge Typgrenzen. Kein C#/Windows.

### Daemon-Architektur (mpd-Modell)

- Dafür: sprachneutral, TUI und GUI hängen an derselben Wiedergabe-Instanz, Absturzisolation.
- Dagegen: Lebenszyklus, Versionierung, Serialisierung, Paketierung. Im Mac App Store ist ein gebündelter Helper wegen Sandboxing fummelig; FFI im selben Prozess ist dort deutlich einfacher.
- **Empfehlung: Core als Bibliothek mit sauberer Command/Event-Schnittstelle entwerfen.** Ein optionaler Daemon-Modus lässt sich später darüberlegen, ohne die Architektur festzulegen.

---

## 5. Feeds und Ökosystem

- **`rss` 2.1.2** (2026-09-10): iTunes eingebaut, generische `ExtensionMap` liefert alle `podcast:`-Tags untypisiert. **Praktischer Weg: `rss` + eigene typisierte Schicht.**
- **gofeed v1.4.2**: gleichwertig. → **Feed-Parsing ist ein Patt.**
- quick-xml 0.42; **reqwest 0.13.5** (rustls mit aws-lc als Default → C-Toolchain nötig, relevant für Cross-Builds); rusqlite 0.40.2 (`bundled`).
- **Kapitel – Vorteil Rust**: `id3` (CHAP/CTOC), `mp4ameta` (beide MP4-Varianten), symphonia 0.6. In Go: bogem/id3v2 ruht seit 2023, dhowden/tag ohne Kapitel.
- Shownotes: html2text, ammonia, htmd, tui-markdown. Go ist hier bequemer (html-to-markdown + glamour).
- `opml`-Crate ruht (Format trivial; selbst über quick-xml).

---

## 6. Build und Cross-Kompilierung

- **Go**: mit `CGO_ENABLED=0` trivial – seit oto v3.5.0 inklusive Audioausgabe. Sobald cgo nötig wird (libmpv, Medientasten auf macOS/Windows, c-shared für GUIs), ist der Vorteil weg.
- **Rust**: cargo-zigbuild, cargo-xwin aktiv. macOS-Cross von Linux ist wegen der Xcode-Lizenz eine Grauzone → **native GitHub-Actions-Runner-Matrix**, wie bei TorroMail. cpal unter Linux braucht `libasound2-dev`.
- **Wie viel vom Go-Vorteil bleibt?** Für eine reine TUI mit MP3/FLAC/Vorbis: vollständig. Für die tatsächlichen Anforderungen (AAC/M4A, Time-Stretch, Medientasten, spätere GUI-Anbindung): nahezu nichts.

---

## 7. Risiken des Rust-Wegs, die ein früher Spike klären sollte

1. rodio-Umbauten und die Versionsschere rodio (symphonia 0.5) ↔ Opus-Adapter 0.3 (symphonia 0.6). Optionen: rodio + Adapter 0.2 festnageln **oder** symphonia 0.6 + cpal direkt ansteuern (eigene Pipeline; volle Kontrolle über Stretcher, Stille-Kürzung, Kapitel). Letzteres ist für einen Player mit eigener Signalkette vermutlich der sauberere Weg.
2. Stretch-Qualität bei 2,5–3x für Sprache: SoundTouch, Signalsmith und `wsola` hörend vergleichen.
3. souvlaki-Threading: Main-Thread-Run-Loop auf macOS, verstecktes Fenster auf Windows.
4. HE-AAC-Lücke: akzeptieren, FDK-Adapter als optionales Feature oder optionales libmpv-Backend.
5. Windows: Tastenbelegung ohne exotische Modifier, Halfblocks-Rückfall für Cover.

## 8. Empfohlene Crates

| Bereich | Crates |
|---|---|
| TUI | ratatui 0.30, crossterm 0.29 (`event-stream`), ratatui-image, ratatui-textarea oder tui-input, tui-scrollview, throbber-widgets-tui, tachyonfx (sparsam), tui-logger, color-eyre |
| Async/Infra | tokio, tracing, thiserror, serde, serde_json |
| HTTP | reqwest 0.13 (rustls), reqwest-middleware (Retry) |
| Feeds | rss 2.1 + eigene Podcasting-2.0-Schicht über `ExtensionMap`; quick-xml für OPML und PSC |
| Verzeichnis | eigene Clients für Apple, Podcast Index, fyyd |
| Store | rusqlite 0.40 (`bundled`), rusqlite_migration |
| Audio | symphonia 0.6, symphonia-adapter-libopus 0.3, cpal 0.18 (oder rodio nach Spike), stream-download 0.24, soundtouch 0.5 (Alternative signalsmith-stretch), rubato 5 |
| Kapitel/Tags | id3, mp4ameta, symphonia-Kapitel |
| Shownotes | ammonia, html2text oder htmd |
| OS-Integration | souvlaki |
| Bibliotheks-Ordner | notify 8; Pfade als eigene reine Funktion (wie TorroMail) |
| FFI | uniffi **0.31.x** (festgenagelt wegen C#), später uniffi-bindgen-cs |
| Tests | insta + `TestBackend`, wiremock für HTTP, proptest für die Merge-Logik |
| Release | GoReleaser (Rust-Build; Homebrew, winget, scoop, AUR, deb, rpm) |

## 9. Nicht verifizierte Punkte

- Stretch-Qualität bei 2,5–3x für Sprache; Produktionsreife neuer Pure-Rust-Opus-Decoder.
- Ob das nächste rodio-Release auf symphonia 0.6 geht.
- Ob Bubble Tea v2 das Kitty-Keyboard-Protokoll mit Windows Terminal 1.25 aushandelt; Kitty-Grafik in Windows Terminal.
- ratatui ohne automatisches Farb-Downsampling (Allgemeinwissen, nicht neu geprüft).
- BSD-Praxisberichte für beide Stacks.
- Binärgrößen (nur Sekundärquellen: Rust-TUI ca. 3–8 MB, Go ca. 8–20 MB).
