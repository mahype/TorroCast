# TorroCast

A podcast player with a portable core and a terminal UI.

**Status: planning.** This repository currently holds research and design documents.
There is no code yet.

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

- **v0.1** – search, podcast detail, episode list, show notes, chapters (no playback)
- **v0.2** – subscriptions, library folder sync, playback, queue, downloads
- **later** – transcripts, per-podcast settings, gpodder-compatible server sync, native GUIs
