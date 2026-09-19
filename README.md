# TorroCast

A podcast player with a portable core and a terminal UI.

**Status: v0.2 — find, read, listen, keep.** Search podcasts and single episodes, browse
charts and categories, read show notes and chapters, and play: streaming, tempo at the
same pitch, chapter jumps, and an Up Next queue after the model of Pocket Casts.
Subscriptions, Up Next and playback positions live in a library folder that a sync
service can carry between devices.

## Run it

```
cargo run --release
```

Needs Rust 1.88 or newer and a terminal of at least 80 × 24. Linux (with the ALSA
headers, `alsa-lib` / `libasound2-dev`), macOS and Windows. Type to search, `enter`
opens, `esc` goes back, `?` lists every key. The interface speaks German or English,
following `LANG`. An address typed into the search field is opened as a feed.

`TORROCAST_OUTPUT=muted` plays without touching the sound card.

## What it does

- **Search** Apple's podcast directory for shows or, with `e`, for single episodes;
  optionally fyyd and Podcast Index as well (Settings; Podcast Index wants your own free
  key, TorroCast ships none). Results of several directories arrive as they come and are
  folded into one list.
- **Charts** by country and **categories** with their own charts.
- **Podcast:** description, categories, website, support link, the episode list with
  date, length and marks for chapters and transcripts; filter with `/`, reverse with `o`.
- **Episode:** show notes laid out for the terminal with numbered links (`1`–`9` open
  them), and chapters from all three places they can live: the feed (Podlove), the
  `podcast:chapters` file, and the ID3 tag at the head of the MP3 — read with two small
  range requests, never by downloading the episode.
- **Playback:** `p` plays the selected episode, streaming while it downloads. `space`
  pauses, `,` `.` jump by chapter, `b` `f` by 30 seconds, `-` `+` change the tempo without
  changing the pitch, `n` goes to the next episode, `x` stops and remembers the place,
  `t` sets a sleep timer.
  The player sits under the menu on every screen, with a level meter and clickable
  buttons; `0` opens the large player with the chapter list.
- **Up Next:** `a` puts an episode at the end, `A` at the front — from any episode list,
  search results included. When an episode ends the next one starts. `J` `K` reorder,
  `d` removes, `C` empties.
- **Subscriptions:** `s` on a podcast subscribes; they are listed under their own menu entry.
- **New episodes:** what the subscriptions published in the last two weeks and is still
  unheard, newest first, across all feeds — fetched at the start, every half hour and with
  `r`; playable and queueable from the list.
- **Library folder:** subscriptions, Up Next and the place in every episode are written
  to a folder — one journal per device, so Dropbox, Syncthing or a NAS can share it
  without ever producing a conflict. Settings → library folder moves it to where it is
  synced, data included; see [docs/bibliothek-format.md](docs/bibliothek-format.md). What was
  playing is first in line after a restart and resumes where it stopped.
- Settings are kept in `config.toml` in the platform's config directory.

Not yet: downloads, cover art, Opus and HE-AAC audio.

## Layout

| Crate | What it is |
|---|---|
| `torrocast-net` | HTTP behind a trait that tests replace |
| `torrocast-directory` | Directories: Apple, fyyd, and merging their answers |
| `torrocast-feed` | Feed parsing, chapters, show notes — pure, no network |
| `torrocast-library` | The library folder: per-device journals, merged without conflicts |
| `torrocast-player` | Streaming, decoding, tempo at the same pitch (WSOLA), the sound card |
| `torrocast-core` | Commands in, events out; playback, Up Next and the library's upkeep; settings. No user interface |
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
- **v0.2** – playback, Up Next, subscriptions, library folder — done; downloads still open
- **later** – transcripts, per-podcast settings, gpodder-compatible server sync, native GUIs
