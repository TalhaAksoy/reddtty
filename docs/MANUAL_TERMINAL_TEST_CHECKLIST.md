# ReddTTY manual terminal verification checklist

Use a release binary and an isolated config directory, e.g.:

```bash
./target/release/reddtty --demo --config /tmp/reddtty-manual/config.toml
```

Do not use your real config or library for destructive checks.

## Image protocol checklist

Repeat for each terminal: Kitty, iTerm2, Ghostty, Terminal.app, Konsole,
Alacritty, tmux (and Windows Terminal if testing Windows).

- [ ] `--diagnostics` reports the expected configured/detected/effective protocol.
- [ ] Feed image previews appear inside card borders, not over text.
- [ ] Post detail image + body text both remain readable.
- [ ] Gallery shows the image, thumbnails and position counter.
- [ ] `+` / `-` zoom changes the image; `0` resets.
- [ ] Shift+arrows pan; image stays clipped to its pane.
- [ ] Resize the terminal; image redraws without stale pixels or overflow.
- [ ] Open a modal over an image; modal background is clean.
- [ ] Navigate away; old image pixels are cleared.
- [ ] Quit; alternate screen and image state are cleaned up.
- [ ] Disable images in Settings; no image panes are reserved incorrectly.
- [ ] Re-enable images; Auto returns to the detected protocol.
- [ ] `NO_COLOR=1` disables image colours and keeps terminal default colours.
- [ ] Record terminal name, version, OS, protocol and result.

## Reading and input checklist

- [ ] 30×10, 60×20, 80×24, 100×30, 130×36, 160×45 render without panic.
- [ ] `j/k`, `J/K`, PgUp/PgDn, Home/End behave predictably.
- [ ] A card taller than the viewport can be scrolled to its bottom border.
- [ ] Mouse wheel over comments focuses comments; Enter acts on that pane.
- [ ] Mouse wheel in Gallery does not move the hidden feed.
- [ ] Gallery `i` opens info; `I`/`y` copy image URL.
- [ ] Info drawer actions run; `Esc` returns correctly.
- [ ] Settings submenu `Esc` returns to the same row.
- [ ] Confirm dialogs: click Yes/No and keyboard y/n both work.
- [ ] Bookmark search: `/`, Ctrl+U, Backspace, Enter, Esc.
- [ ] Opening a bookmark and returning restores search/selection.

## Persistence and recovery checklist

- [ ] Two terminals share one config directory; add different bookmarks.
- [ ] Quit both; both bookmarks exist in `library.toml`.
- [ ] Delete one in each; neither deleted bookmark returns.
- [ ] Hold `library.lock` externally; save shows a timeout and writes nothing.
- [ ] Kill a lock holder; next save succeeds.
- [ ] Corrupt `config.toml`; start; original bytes remain; recovery screen works.
- [ ] Restore newest backup; restart; restored settings load.
- [ ] Create new config; restart; valid config loads.
- [ ] Corrupt `library.toml`; start; original bytes remain; saves blocked.
- [ ] Read-only config directory; save failure is visible and no false success.

## Release artifact checklist

- [ ] Archive contains binary, README, LICENSE, configs and docs.
- [ ] `sha256sum -c` / `shasum -a 256 -c` passes.
- [ ] `--help`, `--version`, `--diagnostics` work from the extracted archive.
- [ ] Diagnostics contains no tokens, passwords or client secrets.
