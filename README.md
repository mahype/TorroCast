# TorroCast

A podcast player with a portable core and a terminal UI.

**Status: v0.1 — find and read.** Search podcasts, browse charts and categories, open
a podcast, read show notes and chapters. No playback yet; that is v0.2.

## Run it

```
cargo run --release
```

Needs Rust 1.88 or newer and a terminal of at least 80 × 24. Linux, macOS and Windows.
Type to search, `enter` opens, `esc` goes back, `?` lists every key. The interface
speaks German or English, following `LANG`. An address typed into the search field is
opened as a feed.

## What v0.1 does

- **Search** Apple's podcast directory, optionally fyyd as well (Settings). Results of
  several directories arrive as they come and are folded into one list.
- **Charts** by country and **categories** with their own charts.
- **Podcast:** description, categories, website, support link, the episode list with
  date, length and marks for chapters and transcripts; filter with `/`, reverse with `o`.
- **Episode:** show notes laid out for the terminal with numbered links (`1`–`9` open
  them), and chapters from all three places they can live: the feed (Podlove), the
  `podcast:chapters` file, and the ID3 tag at the head of the MP3 — read with two small
  range requests, never by downloading the episode.
- Settings are kept in `config.toml` in the platform's config directory.

Not in v0.1: playback, subscriptions, the synced library folder, cover art, Podcast
Index (it will need your own free key).

## Layout

| Crate | What it is |
|---|---|
| `torrocast-net` | HTTP behind a trait that tests replace |
| `torrocast-directory` | Directories: Apple, fyyd, and merging their answers |
| `torrocast-feed` | Feed parsing, chapters, show notes — pure, no network |
| `torrocast-core` | Commands in, events out; settings. No user interface |
| `torrocast-tui` | The terminal interface (binary `torrocast`) |

The user interface contains no logic another front end would need too.

## Idea

- Search podcasts (Apple by default; Podcast Index and fyyd can be added), browse
  podcast details, episodes, show notes and chapter marks.
- A modern TUI that runs on Linux, macOS and Windows.
- All logic lives in a platform-independent Rust core, so native front ends
  (SwiftUI on macOS, a Windows app) can be added on top later.
- User data (subscriptions, playback positions, queue) lives in a **folder of your
  choice** – Dropbox, Syncthing, Nextcloud, a NAS, a git repo – so it is backed up
  and shared between devices without any server.

## Documents

The planning documents are written in German.

| Document | Content |
|---|---|
| [docs/architektur.md](docs/architektur.md) | Architecture summary, decisions, open questions |
| [docs/funktionssammlung.md](docs/funktionssammlung.md) | Feature list by release (v0.1, v0.2, later) |
| [docs/oberflaeche.md](docs/oberflaeche.md) | TUI concept with mockups, following the TorroMail TUI and the Torro design language |
| [docs/research/01-podcast-verzeichnisse.md](docs/research/01-podcast-verzeichnisse.md) | Where podcast catalog data comes from: APIs, terms, limits |
| [docs/research/02-clients-und-funktionsumfang.md](docs/research/02-clients-und-funktionsumfang.md) | Survey of existing clients, chapter formats, sync standards |
| [docs/research/03-technik-rust-oder-go.md](docs/research/03-technik-rust-oder-go.md) | Rust vs. Go, TUI frameworks, cross-platform, audio, FFI |
| [docs/research/04-datenablage-und-sync.md](docs/research/04-datenablage-und-sync.md) | Design of the synced library folder |
| [docs/research/fixtures/](docs/research/fixtures/) | Recorded API responses, usable as test fixtures |

## Roadmap

- **v0.1** – search, podcast detail, episode list, show notes, chapters (no playback) — done
- **v0.2** – subscriptions, library folder sync, playback, queue, downloads
- **later** – transcripts, per-podcast settings, gpodder-compatible server sync, native GUIs
