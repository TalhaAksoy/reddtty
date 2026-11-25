# Test report

Date: **2026-11-25**

Build: debug + release, Rust 1.100.0 (2026-11-09, Homebrew), macOS 27.0.1
(aarch64, Apple M1). Terminal used for PTY work: tmux 3.x with `tmux-256color`;
outer terminals on the machine: Ghostty 1.4.0 and iTerm2 3.7.7.

This report separates three kinds of evidence:

1. **Automated** — commands in this repository that pass or fail reproducibly.
2. **Manual/PTY** — a real terminal was driven and its screen was captured.
3. **Documentation-only** — external facts (for example Reddit policy dates)
   that were read, not executed.

Nothing below is presented as verified beyond its category.

## Commands run

```bash
cargo fmt --all -- --check                 # pass
cargo clippy --all-targets --all-features -- -D warnings   # pass, no warnings
cargo test                                 # 300 passed, 0 failed, 0 ignored
cargo build --release                      # pass
python3 scripts/pty_verify.py              # 66 captures, 109 assertions, all pass
python3 scripts/protocol_probe.py          # 4/4 protocols emit expected bytes
cargo build --release --example stress
/usr/bin/time -l ./target/release/examples/stress   # numbers below
```

## 1. Automated tests

| Suite | Tests | Passed | Failed | Ignored |
|---|---:|---:|---:|---:|
| Library unit tests (`src/**`) | 230 | 230 | 0 | 0 |
| HTML provider integration (`tests/html_providers.rs`) | 11 | 11 | 0 | 0 |
| OAuth provider/image integration (`tests/providers.rs`) | 13 | 13 | 0 | 0 |
| UI integration (`tests/ui.rs`, Ratatui `TestBackend`) | 46 | 46 | 0 | 0 |
| Doc tests | 0 | 0 | 0 | 0 |
| **Total** | **300** | **300** | **0** | **0** |

### What the unit tests cover

- Text: ANSI/control/bidi sanitisation, display-width wrapping, grapheme-safe
  long words, blank lines, ellipsis, CJK width, count humanisation, subreddit
  normalisation, Markdown stripping.
- Markdown: headings, bold/italic, inline code, fenced code, links, bullets,
  escape injection removal, width limits.
- Time: relative boundaries and future clock skew.
- Browser: `http(s)`-only, loopback/private/link-local rejection.
- Models: post kinds/preview images/permalink, hidden content logic, comment
  descendant counts, recursive sorting, more-node ordering/splicing.
- Config: defaults, partial configs, malformed fallback, clamping, invalid
  default feed/sort repair, favorites, recent de-duplication, save/load
  round-trip, credential precedence, protocol aliases, provider selection,
  HTML-network opt-in default, Redlib instance URL validation and the
  XDG-style `~/.config/reddtty` default on macOS/Linux.
- ReddTTY migration: legacy settings are copied only when no new config exists;
  existing new configs are never overwritten; the old file is not deleted;
  migration is idempotent; broken/unreadable legacy data is handled without
  panic; destination failures leave the legacy file intact; favorites/theme
  survive; `REDDTTY_*` environment variables beat legacy `THREADLINE_*` names;
  config fallback and legacy user-agent normalization are covered.
- Demo provider: ≥30 posts and all post types, genuinely different sort orders,
  Top time windows, cursor pagination without duplicates, scoped search, nested
  comments, comment sorts, `morechildren` simulation, unknown subreddit errors.
- Reddit JSON mapping: text/link/video/gallery/removed posts, nested comments,
  `morechildren` forest building, HTML entities, paths/query parameters,
  capability rules.
- Feed state: height recalculation, selection visibility, page/home/end,
  duplicate filtering, same-cursor stop, refresh selection preservation,
  pagination waiting for heights, visible range bounds.
- Post/comment state: indentation/depth, collapse and hidden reply count,
  load-more outcome, splicing loaded replies, body cache/scroll, comment
  selection scrolling.
- Modals: sort/range choices and cycling, text editing by character index,
  search scope defaults/toggle, clamping.
- App: back preserves feed selection, sort/range modals, text inputs capture
  `q`/`j` and do not quit, Esc closes only the modal, focus cycling, theme
  toggle, reveal, stale request rejection, all provider error categories in the
  feed, append-error pagination stop.
- Images: PNG/JPEG decode, unsupported format rejection, tiny and oversized
  images, disabled fallback, LRU eviction and memory budget, local-file limits,
  unsafe URL rejection.
- HTML providers: synthetic Old Reddit/Redlib fixtures for text/image/gallery/
  link/video/removed posts, NSFW/spoiler flags, promoted-post skipping, next-page
  links, nested/deleted comments, missing title/author/score/time, login walls,
  changed structure and absent fields without panic. Provider construction tests
  assert explicit selections never silently become demo and that switching the
  selected kind builds a different provider.
- HTML transport: `ProviderStatus` mapping for access denied, login required,
  rate limit, structure change, parse error and unsupported feature.

### Provider/integration tests (local mock HTTP server, no external network)

- OAuth token success and listing success; access token parsed from the token
  response; missing token is a parse error.
- OAuth request shape asserted directly: `Basic` client credentials on the token
  endpoint, `grant_type=client_credentials`, `scope=read`, and the configured
  `User-Agent`. Refresh-token grant sends `grant_type=refresh_token` and
  `refresh_token`, with no `scope`.
- Feed request carries `Authorization: Bearer …`, `Accept: application/json`,
  `raw_json=1` and pagination `after`.
- 401, 403, 404, 429 (with `Retry-After`), 500 mapping.
- Token-endpoint 401 mapping.
- Invalid JSON.
- Single token refresh after a 401, then success.
- Request timeout via a short configured client timeout.
- Comments, nested replies, `morechildren` and subreddit-about mapping.
- Search uses `q`, `type=link`, `sort`, `restrict_sr=1`, `t`.
- Rate-limit cooldown suppresses a second HTTP request after a 429.
- Provider configuration validation (missing credentials, bad `User-Agent`).
- Image downloader: HTTP success, byte-limit rejection, missing image, plus
  local-file limits and unsafe-URL rejection in unit tests.

### HTML provider integration (`tests/html_providers.rs`, local mock server)

- Old Reddit feed: six fixture posts, pagination cursor reused as the source's
  own next URL, honest `User-Agent`, `Accept: text/html`, and a normal page that
  contains a login form in the header (not misdetected as a wall).
- Sort URLs actually change: `/r/rust/`, `/new/`, `/rising/`,
  `/controversial/`, `/top/?t=week`; search uses `/search` with `q`,
  `sort=relevance` and no unsupported parameters. A mock response test also
  proves a sort switch returns the new page's content, not just a new label.
- Old Reddit comments: three-level nesting, deleted comment retained, parent ids
  assigned only from nested listings, `limit=500&sort=top` request.
- Redlib feed/comments: independent parser structure, image/gallery variants,
  nested and deleted comments.
- Network gate: `allow_network = false` fails before any socket request.
- 401 maps to login required, 403 to access denied, and 429 (with
  `Retry-After: 17`) to a rate-limit error.
- A Redlib “Reddit blocked this request (403 Forbidden)” error page returned
  with HTTP 404 is mapped to a clean access-denied error; raw HTML is not dumped
  into the UI.
- Redlib sort routes are asserted separately: Home uses `/r/popular/<sort>`
  (Redlib has no `/new` front-page route), subreddits use `/r/<name>/<sort>`,
  and Old Reddit keeps its trailing-slash routes.
- Redlib's bare `<footer><a accesskey="N" href="?after=...">NEXT</a></footer>`
  next-page link is recognized; pagination is exercised against the mock server.
- Login page and changed HTML are errors, never empty feeds.
- Non-HTML content type is rejected.
- Invalid pagination URL is rejected without a request.

### UI tests (`TestBackend`)

- Rendering at 30×10, 60×20, 80×24, 100×30, 140×40 and 180×50 without panic;
  additionally 20×6 in the size sweep.
- Status bar at 30×10, 60×20, 80×24, 100×30 and 120×36 always keeps a `Quit`
  hint and never ends in a truncation marker.
- Post and gallery status bars keep the `q Quit` hint.
- Removing an existing favorite opens a confirmation popup; `Esc`/`n` keeps it
  and `Enter`/`y` removes it. Adding a favorite remains immediate.
- The sidebar always shows a `FAVORITES` heading; when empty it shows an
  `f to add` hint instead of hiding the section.
- First-run/setup presets embed every file in `configs/`, parse as settings, and
  install with a timestamped backup. The Settings modal and config picker are
  rendered in `TestBackend`; installing a preset is covered by an app test.
- The Redlib provider label includes the configured host (for example
  `REDLIB · redlib.hbubli.cc`) so the active source is visible in the top bar.
- While pagination is in flight, the feed bottom shows an animated spinner and
  `Loading more posts…`, so a slow network does not look like a frozen app.
- All six themes (dark, light and Catppuccin Latte/Frappé/Macchiato/Mocha)
  render without panic; theme cycling and config serialization are covered.
- The Theme picker previews with `j/k`, commits with `Enter`, and restores the
  original theme on `Esc`/outside click. Preview and persistence are app-tested;
  the six-row picker is rendered in `TestBackend`.
- Selecting a config rebuilds the provider in-process: the new top-bar label,
  reset navigation stack, preserved favorites/theme, persisted provider choice
  and rejection of old view/request results are all asserted.
- `library.toml` round-trips bookmark post snapshots, read posts/comments and
  filter rules; malformed files warn and start empty. Bookmark toggling,
  bookmark-list opening, read/unread toggling, hide-read pruning and offline
  snapshots are app-tested.
- Local filters match keywords, subreddits, authors, domains and read state;
  invalid expressions are rejected. Filtered pages are dropped before reaching
  the feed, hide-read prunes immediately, and an all-filtered page keeps
  paginating instead of stalling. The Filters modal adds/removes/clears rules
  and persists them.
- Comment navigation is covered for next/previous root, parent jump, collapse
  all/expand all, OP-only filtering (including hidden non-OP parents), next
  unread and comment-vs-post permalink targets.
- Source health records successes/failures, latency samples, last error,
  success rate and reset-on-provider-switch. The Settings → Source screen and
  its Retry/Change-source actions are app- and render-tested.
- The command palette filters by label and keywords, moves selection, runs
  actions, tolerates typing `q` without quitting, and toggles with `Ctrl+P`.
- Image protocol `Auto` detects iTerm2 from `TERM_PROGRAM`, `LC_TERMINAL` or
  `ITERM_SESSION_ID` and uses Unicode half-blocks there. Both iTerm2's Kitty
  placeholder path and its native inline-image path were blank or unstable in
  the full-screen repaint loop, while half-blocks rendered reliably. The helper
  is unit-tested; `--diagnostics` reports the effective protocol. Users can
  still force `iterm2`/`kitty` explicitly.
- Redlib live-feed media markup (`div.post_media_content` /
  `a.post_media_image`) is parsed as an image, and `video.post_media_video`
  contributes a video URL plus its poster image. This was the missing selector
  behind blank image cards on Redlib feeds; a dedicated regression test covers
  the real markup shape.
- Gallery zoom/pan is covered by gallery-state unit tests and a UI test:
  100/150/200/300/400% steps, clamped pan, reset, navigation resets the view,
  thumbnail clicks, and the cached crop key. The image pipeline tests crop
  geometry and the bounded raw-source byte cache (so panning does not re-download
  the same image), and the viewer requests the next image and visible thumbnails
  ahead of time. The PTY run verifies the thumbnail strip, zoom and pan on screen.
- Post detail layout is unit-tested for content-sized panes (short bodies give
  more room to comments), the 60% cap, the minimum-height path and the optional
  wide-screen community panel. A UI test renders the community name, title,
  description, suggested communities and actions. The post watch state receives
  subreddit updates through the same `SubredditLoaded` event as feeds.
- The info drawer snapshots community/member/post data and renders on narrow
  layouts. Settings image-protocol and appearance actions are app-tested; image
  protocol changes clear pending encoded work. Help search/context ordering,
  palette text aliases, quick mute/undo, breadcrumbs, last-feed restoration and
  the eight themes are covered by unit/UI tests.
- Directional focus is app-tested: `←` moves feed → sidebar (showing a hidden
  sidebar on wide terminals and explaining the 76-column limit on narrow ones),
  `→` moves sidebar → feed, and post detail moves comments ↔ body. PTY captures
  verify the real status-bar hint changes.
- Link actions are covered for post comments URLs, validated subreddit URLs,
  selected-post/gallery image URLs, invalid-subreddit rejection and the demo
  image guard. Clipboard remains the shared cross-platform pbcopy/xclip path.
- The help screen is categorised into Global/Feed/Sidebar/Post Detail/Image
  Viewer/Clipboard & Links/Sort & Range/Text Input/Bookmarks/Local Filters/
  Source Health/Command Palette/Mouse. Layout math is unit-tested for one/two columns and clamped
  scrolling; a UI test verifies early sections are visible and `End` reveals the
  later sections. Narrow terminals keep a `more · Esc close` footer.
- Top bar shows `OLD REDDIT`/`REDLIB` instead of `DEMO`, includes
  `Access denied`/`HTML changed`/`Login required` statuses, and keeps the
  provider badge at 60 columns when space is tight.
- Long provider/sort error messages wrap on the error screen instead of being
  truncated into an unreadable single line.
- Partial cards never paint body/image content over the bottom border.
- Compact card mode fits two cards at 60×20/80×24 (short-title fixture; the
  height threshold itself was not weakened).
- Narrow feed keeps the post count and “more” hint.
- Sidebar appears at medium width, right “About” panel only at wide width.
- Feed cards, title/score/sort tabs and responsive metadata.
- Post detail with nested comments (`├─` rails), comment header and sort label.
- Comment list at 60×20, 80×24 and 120×40.
- Gallery header, position (`1 / N`) and controls.
- Help and search modals fit small screens; search scope row present. At 30×10
  the help footer explicitly reports how many shortcuts are hidden, and the
  `DEMO` badge remains visible.
- Empty feed error state with a Retry control.
- NSFW hidden note without body leakage.
- Sort modal lists supported sorts and the Top range row.
- Initial async feed request actually loads posts and clears loading state.
- Mouse: double-click card opens post; wheel scrolls; sidebar click navigates;
  sort-tab click changes sort; outside-modal click closes; comment double-click
  collapses; gallery arrow advances.
- Keyboard: goto normalises `r/rust` and opens the feed; search opens a search
  feed; Top range switches sort to Top; sort menu applies New; `q` sets quit;
  `default_sort` config is honoured.

## 2. Manual/PTY verification

Harness: [`scripts/pty_verify.py`](../scripts/pty_verify.py). It starts the
**release binary** in an isolated tmux socket (`-L reddtty-verify`), sends
real keystrokes, captures real screen contents and asserts on what a user would
see. Latest result:

```json
{"ok": true, "captures": 66, "assertions": 109}
```

Captures and the machine-readable summary are in
[`docs/verification/`](verification/). This is not a unit test: the checks run
against the compiled terminal application and its alternate-screen output.

### Scenarios driven through the real PTY

| # | Scenario | Result |
|---|---|---|
| 1 | `--demo` starts at Home; brand, title, `DEMO`, cards and sort tabs render; no stuck loading state | **Pass** |
| 1b | `←` focuses the sidebar and `→` returns to the feed; status-bar hints change accordingly | **Pass** |
| 2 | `Enter` opens post detail; title, author, body, `COMMENTS`, `Sort: Best`, `ABOUT` and `SUGGESTED COMMUNITIES` visible | **Pass** |
| 3 | `←` focuses the post body, `→` returns to comments, and body scroll shows the `↕` indicator | **Pass** |
| 4 | `c` collapses a comment; `▶` marker and hidden-reply count appear | **Pass** |
| 5 | `c` expands again; hidden marker disappears | **Pass** |
| 6 | `Esc` returns to the feed with the prior context | **Pass** |
| 7 | Gallery post opens; `1 / 2`, controls and the thumbnail strip are visible | **Pass** |
| 8 | `→` advances to `2 / 2` | **Pass** |
| 8a | `+` zooms to 150%, `Shift+→` pans to `x+10`, and the header/footer report both | **Pass** |
| 8b | Gallery `i` explains that demo images are local fixtures instead of copying a fake external URL | **Pass** |
| 9 | Sort menu opens and shows the Top time-range row | **Pass** |
| 10 | `g` → `rust` opens r/rust with the expected title/metadata | **Pass** |
| 11 | `/` → `terminal` opens a search feed titled `Search: terminal` | **Pass** |
| 12 | `q` restores the terminal; shell resumes and prints `EXIT:0` | **Pass** |
| 13 | Responsive sweep 60×20, 80×24, 100×30, 130×36, 160×45: feed renders, detail opens, help opens, status bar keeps `Quit`, quit is clean | **Pass** |
| 14 | 30×10: feed renders with the `DEMO` badge; help opens and reports hidden shortcuts; quit is clean | **Pass** |
| 15 | `b` shows `★ saved`, `m` shows `Marked as read.`, `B` opens the bookmarks list and `Enter` reopens the saved post | **Pass** |
| 16 | Comment `z` collapse-all/expand-all, `n` next unread, `O` OP-only and `y` copy-permalink toasts are visible | **Pass** |
| 17 | Settings → Source health shows `DEMO`, `Test now`, `Change source`, and `r` keeps the screen healthy | **Pass** |
| 18 | Settings → Filters adds `keyword  black hole`; the feed reports `1 post(s) hidden by local filters` and the matching card disappears | **Pass** |
| 19 | `:` opens Commands; typing `mocha` and Enter applies and persists `Theme: Catppuccin Mocha` | **Pass** |
| 20 | Settings → Change config installs Demo in-process and shows `Provider applied immediately` without restarting | **Pass** |
| 21 | `i` opens the community-info drawer with `ABOUT` and `SUGGESTED COMMUNITIES` at 140 columns | **Pass** |
| 22 | Help `/` search for `zoom` shows the image-viewer shortcuts and hides feed shortcuts | **Pass** |
| 23 | Settings → Images lists the protocol picker including `Halfblocks` | **Pass** |
| 24 | Settings → Appearance shows relative timestamps and motion controls | **Pass** |
| 25 | `M` mutes the selected subreddit and `u` undoes it with visible toasts | **Pass** |

Visual inspection of the captures (not only assertions) found and drove the
fixes listed in [`UX_REVIEW.md`](UX_REVIEW.md).

### Image protocol encoder probe

[`scripts/protocol_probe.py`](../scripts/protocol_probe.py) forces each protocol
in a raw PTY (no tmux) and scans the byte stream:

| Forced protocol | Evidence | Result |
|---|---|---|
| Kitty | 33 `ESC_G` APC sequences | **Pass** (encoded) |
| iTerm2 | 1 `ESC]1337;File=` sequence | **Pass** (encoded) |
| Sixel | 1 `ESC P9;1;0q…` DCS sequence | **Pass** (encoded) |
| Half-blocks | 110 `▀`/`▄` cells | **Pass** (encoded; visually verified) |

This proves the **encoder/render path**, not that a GUI terminal painted the
image. The script and raw scan are in `scripts/` and
`docs/verification/protocol-probe.json`.

### GUI terminal detection (not pixel verification)

- **Ghostty 1.4.0**: launched with the release binary; log reports
  `terminal image capabilities probed protocol=Kitty`.
- **iTerm2 3.7.7**: launched with the release binary. `Auto` detects iTerm2
  from the environment and selects Unicode half-blocks; the log reports
  `protocol=Halfblocks`. Kitty placeholders and the native inline-image path
  both left blank image regions in the real TUI, while half-blocks rendered.
- No window/display screenshot could be captured because macOS Screen Recording
  permission is unavailable to this environment (`screencapture` returned
  “could not create image from window/display”). Therefore **pixel rendering in
  Ghostty and iTerm2 is not verified**; only detection and encoded output are.
- Kitty and WezTerm binaries are not installed on this machine.

### Terminal compatibility matrix

| Terminal | Evidence | Status |
|---|---|---|
| tmux + half-blocks | Real PTY, 58 captures, visual inspection | **Verified** |
| Ghostty 1.4.0 | Detection only (Kitty), no screenshot | Detection verified; pixels **unverified** |
| iTerm2 3.7.7 | Auto selects half-blocks; log and working image verified | **Verified via half-blocks**; native protocols blank |
| Kitty | Encoder bytes only; binary absent | Encoder verified; terminal **unverified** |
| WezTerm | Encoder bytes only; binary absent | Encoder verified; terminal **unverified** |
| foot / Konsole / Alacritty / xterm-like | Not run | **Unverified** |
| Windows Terminal / Windows | Not run | **Unverified** |

Inside tmux, `Auto` intentionally does **not** run the graphics query and uses
Unicode half-blocks. This avoids tmux passthrough differences and the
first-keypress loss that the query caused. Forced `kitty`/`iterm2`/`sixel`
still run the query and encode that protocol; whether tmux passes it through
depends on the user's configuration.

## 3. Stress measurements (real runs, not estimates)

Harness: [`examples/stress.rs`](../examples/stress.rs). Command:

```bash
cargo build --release --example stress
/usr/bin/time -l ./target/release/examples/stress
```

Workload: **500 posts**, **1,000 nested comment nodes** (100 roots × depth 10),
**100 real local PNG images**, 1,200 feed frames, 400 comment frames and 60
collapse/expand rebuilds at 160×45. Rendering uses `TestBackend`; event handling
and layout are the real application code.

| Measurement | Result |
|---|---:|
| Feed first frame (500 posts) | 3.6–7.1 ms across runs |
| Feed frame average / p95 / max | 0.305 / 0.422 / 1.436 ms |
| Key handling average / p95 / max | 0.0001 / 0.0001 / 0.0013 ms |
| 1,000-comment first frame | 1.35 ms |
| Comment frame average / p95 / max | 0.209 / 0.222 / 0.398 ms |
| Collapse/expand average / p95 / max (tree rebuild included) | 1.216 / 1.443 / 1.759 ms |
| 100 image encodes at 80×12 (half-blocks) | 139.9 ms total, 1.4 ms/image |
| Image failures | 0 |
| Image cache after run | 100 entries, 3,276,800 bytes |
| RSS before / after feed / after comments / after images | 6.6 / 11.8 / 14.0 / 26.8 MiB |
| Peak RSS (`/usr/bin/time -l`) | 27,443,200 bytes (~26.2 MiB) |
| Process wall / user / sys time | 1.11 s / 0.92 s / 0.06 s |

Interpretation: at the required 500/1000/100 scale, the measured frame budget is
far below one 60 Hz frame (16.7 ms), including tree collapse. The figures are
from a release build on an Apple M1 with `TestBackend`, so they exclude terminal
I/O and GPU painting; they do **not** prove identical latency in every terminal.
The encoder probe and PTY pass cover the terminal-facing path separately.

## 4. Documentation-only evidence

- Reddit's migration timeline was read from primary pages and corroborating
  reporting; see [`API_ACCESS.md`](API_ACCESS.md) for sources and verification
  status. Key dates: new public API requests closed **31 Oct 2026**; access
  removal begins **12 Jan 2027**; remaining public access ends **March 2027**;
  RSS ended **13 Nov 2026**. The Reddit announcement page itself returned
  403/empty to automated fetching, so that one date is corroborated rather than
  directly fetched.
- No live Reddit request was made. There are no approved credentials, and new
  public access is closed. Provider behaviour is verified only against the local
  mock server.
- No scraping, cookie extraction or access-control bypass was implemented.

## HTML provider status

- **Old Reddit: fixture-verified, not live-verified.** Parsers, URL
  construction, pagination reuse, comments, gates and error mapping pass against
  synthetic HTML and a local mock server. A manual live attempt on 2026-11-23
  returned Reddit's CAPTCHA/access page (`Access denied`), so no claim of live
  Old Reddit support is made.
- **Redlib: fixture-verified plus manual third-party smoke tests.** On
  2026-11-23, `https://redlib.zanity.net` passed a full manual run with the
  release binary: Home (25 posts), New (different 23-post listing), Rising, Top
  with All-time, r/burdurland, post detail, and 539 comments including nested
  replies. `https://redlib.hbubli.cc` also passed a full manual TUI run: Home
  25 posts, different New/Rising listings, Top with All-time, r/burdurland,
  post detail, comments with nested replies, and pagination (25 → 50 → 75 posts
  as the end was reached). `https://safereddit.com` loaded Home and some
  comments, but New, Rising
  and some comment pages returned Reddit access/rate-limit errors. Other checked
  instances returned 429, Cloudflare 403, a bot challenge, or 403 text.
  These are third-party instances, not endorsements, not stability guarantees,
  and not part of CI. A blocked sort shows `Access denied` with an explanation;
  ReddTTY does not substitute Hot, rotate instances or bypass access controls.
- **Fixtures are synthetic.** They model the documented Old Reddit `thing`/
  `sitetable`/`title`/`author`/`comments`/`score` classes and Redlib `post`/
  `post_title`/`post_author`/`post_subreddit`/`post_score`/`post_comments`/
  `thread`/`comment` classes. This proves the parser logic, not that today's live
  sites emit exactly these documents.
- **No load-more-comments HTML flow.** ReddTTY does not call Reddit's
  `/api/morechildren` for an HTML provider and does not invent a mechanism. A
  page's own nested replies are parsed; unsupported expansion is visible as a
  normal error rather than a fake success.

### Additional Redlib instance checks (2026-11-23)

None of these passed the full feed/sort/comment test, so no configs were added:

| Instance | Observed result |
|---|---|
| `redlib.catsarch.com` | HTTP 429 |
| `redlib-1.privadency.com` | bot/CAPTCHA page |
| `redlib.nadeko.net` | HTTP 418 bot check |
| `redlib.privadency.com` | bot/CAPTCHA page |
| `redlib.reallyaweso.me` | redirects to `redlib.blueberry.reallyaweso.me`; Hot was partial, New/Rising returned access-denied |
| `reddit.nerdvpn.de` | HTTP/2 protocol error / empty reply |
| `red.artemislena.eu` | **Passed full TUI test**; added as `redlib-artemislena.toml` |
| `redlib.tiekoetter.com` | Home loaded; New/Rising/Top returned empty responses |
| `redlib.utilibre.org` | **Passed full TUI test**; added as `redlib-utilibre.toml` |

The working configs are `redlib-artemislena.toml`, `redlib-hbubli.toml`,
`redlib-utilibre.toml` and `redlib-zanity.toml`. A wider pass over the public Redlib instance list on
2026-11-24 found mostly DNS/connectivity failures, Cloudflare/Anubis challenges,
5xx responses or placeholder pages; only `red.artemislena.eu` passed the full
feed/sort/comment check.

## Known limitations

1. Live Reddit mode is not verified against Reddit; new public access is closed
   and remaining access expires in March 2027. Mock-server behaviour is verified.
2. Pixel output in Kitty/iTerm2/Sixel is still not screenshot-verified in this
   environment (Screen Recording permission is unavailable). Detection and
   encoded sequences are proven; half-blocks are visually verified. `Auto`
   uses half-blocks on iTerm2 because both native/Kitty paths blanked there.
3. Third-party Redlib instances can see requests and IP addresses; ReddTTY
   does not rotate instances or verify their policies.
4. Sixel residue may persist after closing the viewer in some terminals.
5. Very narrow layouts truncate aggressively; help lists only a subset and now
   says how many entries are hidden.
6. Feed wheel scrolling moves one card at a time by design.
7. Reddit `morechildren` may report more replies than it returns; the remaining
   count is not re-stubbed after a load.
8. Removed posts hide comments entirely, even if Reddit might still expose them.
9. No proactive 100-QPM token bucket; rate limiting is header/429-driven.
10. Full-viewport clearing on route transitions may produce a brief flicker in
    some terminals.
11. Windows and Linux were not run in this environment.
12. Demo comment timestamps and scores are synthetic and sometimes look odd; the
    app labels demo mode, including at narrow widths after the badge fix.
13. The stress harness uses `TestBackend`, so terminal I/O latency is not part of
    the frame measurements.
14. Old Reddit HTML was not live-verified; one Redlib instance was manually
    smoke-tested. Redlib sort pages depend on the instance's Reddit backend;
    a third-party instance can block individual sorts even while its cached home
    page works.
15. HTML providers do not support `Best` listings; it is offered only for
    search (`relevance`). Redlib's actual support depends on the instance and is
    not live-verified.
16. Bookmarks store a post snapshot and its metadata, not the full comment
    thread. Comments still require the source, while the post body remains
    readable offline.
17. Source-health metrics are session-only and are reset when the provider is
    switched; they are not persisted across launches.
18. Local filters apply to feed pages. Changing a filter refetches the current
    feed; unchanged pages already loaded from a provider are not archived.
19. The last **feed/sort** is restored, not the last open post or gallery.
20. ASCII/legacy glyph fallback is not implemented; Unicode box drawing and
    icons are still required. `--no-color`, mono/high-contrast themes and
    reduced motion are available.
21. The custom self-hosted Redlib URL workflow still requires editing the config
    or CLI; there is no dedicated in-app custom-URL form yet.

## Next steps

1. If Reddit grants written/commercial access or an official successor API,
   verify live mode end-to-end and add interactive OAuth with a loopback callback
   and OS keychain storage. Do not assume public access will return.
2. If a source's terms and permissions are established, repeat the opt-in live
   smoke test periodically and save redacted real HTML fixtures to detect drift.
   Keep the distinction between one smoke test and supported live access.
3. Capture real pixel screenshots on Kitty, Ghostty, iTerm2, WezTerm, foot and
   Konsole from an environment with Screen Recording permission; add
   protocol-specific residue checks.
4. Add an optional scrollable help modal for very small terminals (the current
   footer is honest, but does not let the user reach the hidden rows).
5. Preserve remaining `morechildren` counts with a follow-up stub.
6. Add a short-TTL persistent metadata/image cache with explicit deletion
   semantics for removed posts.
7. Add a proactive token bucket if live access ever becomes available again.
8. Run the automated suite on Windows and Linux CI, and repeat the PTY pass on
   those platforms.
