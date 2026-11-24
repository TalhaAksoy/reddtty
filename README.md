# ReddTTY

<p align="center">
  <img src="assets/reddtty-logo.png" alt="ReddTTY logo" width="220">
</p>

[![Rust](https://img.shields.io/badge/rust-1.88%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A fast, keyboard-first Reddit reader for the terminal, written in Rust with
[Ratatui](https://ratatui.rs). It supports feeds, post detail, nested comments,
galleries, inline images, search, sorting and multiple content sources.

> Not affiliated with Reddit. “ReddTTY” is a working name.

## Features

- Feed, post detail, recursive comments, gallery and image viewer
- Vim-style navigation, mouse support, responsive three-pane layout
- Demo data offline; Reddit OAuth; opt-in Old Reddit and Redlib HTML providers
- Local bookmarks with offline snapshots and read/unread state
- Local filters: hide read posts, keywords, subreddits, authors and domains
- Comment navigation: root/parent jumps, collapse all, OP-only, next unread
- Source health screen with latency, errors, retry and manual source switching
- Command palette (`:` or `Ctrl+P`)
- Cursor pagination, search, Top time ranges and sort menus
- Instant, restart-free provider/config switching
- Kitty, iTerm2, Sixel and Unicode half-block image rendering
- Dark, light and Catppuccin themes (Latte, Frappé, Macchiato, Mocha)
- Configurable favorites, recents, layout and rate limits
- No write actions, telemetry or credential logging

## Install

Build from source:

```bash
git clone https://github.com/TalhaAksoy/reddtty.git
cd reddtty
cargo build --release
./target/release/reddtty --demo
```

Install the binary:

```bash
cargo install --path .
```

or:

```bash
make install        # /usr/local/bin/reddtty
```

## Ready-made configs

On first launch with no config, ReddTTY asks you to choose a starter config.
The same picker is available later from **Settings** (`S`). Changing provider
applies immediately: the new provider is built in-process, navigation is reset
safely, and favorites, recents, theme, bookmarks and filters are preserved.

The [`configs/`](configs/) directory contains tested provider configs, including
a fully offline demo and Redlib instances. Install one with an interactive picker:

```bash
make configs
```

Or choose directly:

```bash
make configs CONFIG=redlib-hbubli.toml
make configs CONFIG=2
make configs-list
```

The Makefile creates `${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/`, backs up an
existing config as `config.toml.bckp_<unixtimestamp>`, and installs the selected
file with mode `0600`.

## Run

```bash
reddtty --demo
reddtty --provider redlib --redlib-instance https://redlib.example
reddtty --subreddit rust --sort new
reddtty --diagnostics
```

## Keys

| Key | Action |
|---|---|
| `j` / `k` | Move / scroll |
| `Enter` | Open post or selected item |
| `Tab` | Switch focus pane |
| `s` | Sort |
| `t` | Top time range |
| `g` | Go to subreddit |
| `/` | Search |
| `f` | Add favorite; confirm before removing |
| `b` / `B` | Bookmark post / open bookmarks |
| `m` | Toggle read state |
| `y` / `Y` | Copy permalink (comment when comments are focused) |
| `[` / `]` | Previous / next top-level comment |
| `p` | Jump to parent comment |
| `z` | Collapse or expand all threads |
| `O` | Show only the original poster's comments |
| `n` | Next unread comment |
| `:` / `Ctrl+P` | Command palette |
| `S` | Settings: config, theme, source health, filters |
| `T` | Cycle themes |
| `v` | Image viewer / gallery |
| `o` | Open in browser |
| `?` | Help |
| `q` | Quit |

Full reference: [`docs/KEYBINDINGS.md`](docs/KEYBINDINGS.md).

## Configuration

Default config location:

```text
${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml
```

Windows: `%APPDATA%\reddtty\config.toml`.

Local bookmarks, read state and filters live next to it in `library.toml`. They
are provider-independent, never written to Reddit, and survive source changes.

Settings from older `threadline` installs are migrated on first run without
overwriting an existing ReddTTY config. If no config exists at all, the first-run
picker installs a starter config and backs up any existing file before replacement. Environment variables use the
`REDDTTY_*` prefix; legacy `THREADLINE_*` names are accepted only as fallback.

Example:

```toml
favorites = []

[general]
theme = "dark"              # dark | light | latte | frappe | macchiato | mocha
default_sort = "hot"

[provider]
type = "redlib"

[provider.html]
redlib_instance = "https://redlib.example"
allow_network = true
```

## Content sources

| Source | Status |
|---|---|
| Embedded demo | Offline, always available |
| Reddit OAuth | Only pre-approved existing apps; public API is sunsetting |
| Old Reddit HTML | Opt-in, subject to Reddit access rules; not officially verified |
| Redlib HTML | Opt-in, user-selected instance; third-party and not an official API |

HTML access is **disabled by default**. Set `provider.html.allow_network = true`
only after checking the source’s terms and permissions. ReddTTY does not bypass
login walls, CAPTCHAs or rate limits, and never auto-rotates instances.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
```

- Real-terminal verification: [`scripts/pty_verify.py`](scripts/pty_verify.py)
- Architecture and internals: [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md)
- API access and limitations: [`docs/API_ACCESS.md`](docs/API_ACCESS.md)
- Test evidence: [`docs/TEST_REPORT.md`](docs/TEST_REPORT.md)
- Feature roadmap: [`ROADMAP.md`](ROADMAP.md)

## License

MIT
