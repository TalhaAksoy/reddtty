# Architecture

ReddTTY is split into a small binary and a testable library. The binary owns
process/terminal lifecycle and the async loop; everything else lives in
`reddtty::*` so integration tests can drive the same code.

## Module map

```text
main.rs ── clap CLI, logging, terminal init/restore, input thread, ticker, event loop
  │
  ├── cli.rs
  ├── app/          application state and policy
  │     feed.rs     feed view: posts, heights, scroll, cursor pagination
  │     post.rs     detail view: body cache, comment tree flattening/navigation
  │     gallery.rs  gallery index
  │     modal.rs    sort/range/text/bookmark/filter/source/palette state
  │     sidebar.rs  navigation sections and favorites/recents
  │     health.rs   in-memory request latency/status/success metrics
  │     hit.rs      hit-map/pane rectangles produced by rendering
  │
  ├── ui/           immediate-mode rendering only
  │     layout.rs   responsive rectangles (no widgets)
  │     theme.rs    one central palette for dark/light/Catppuccin
  │     screens/    feed, post, gallery, overlays
  │     widgets/    post cards, comments, sidebar, top/status bars, sort tabs
  │
  ├── providers/    ContentProvider trait + provider selection
  │     demo.rs     embedded synthetic fixtures (offline)
  │     reddit.rs   documented OAuth Data API client
  │     html/       opt-in public-HTML providers
  │       client.rs shared HTTP transport, size/content-type/origin guards
  │       parser.rs shared safe HTML helpers and gate detection
  │       old_reddit.rs
  │       redlib.rs
  │
  ├── media/        downloader, decoder/encoder, bounded LRU cache, manager
  ├── models/       provider-independent Post, Comment, Feed, Subreddit types
  ├── config/       TOML settings, validation, platform paths
  ├── library.rs    local bookmarks (post snapshots), read state, filters
  └── utils/        sanitisation, wrapping, Markdown, time, browser, clipboard
```

Dependency direction is deliberately one-way:

```text
UI → App state → Provider trait / models / media
```

No widget performs HTTP, parses JSON, or spawns downloads. No provider knows
about Ratatui.

## First-run setup and Settings

- If the default config path is missing, `src/setup.rs` runs before App
  construction, shows the embedded presets from `configs/`, and installs the
  selected one through `config::presets::install`. The normal startup path then
  reloads the file. Esc skips installation and uses demo data for that session.
- `S` opens the Settings modal inside the running app. Theme changes apply
  immediately through the Settings theme picker: `j/k` previews live, `Enter`
  persists, `Esc`/outside click restores the previous theme. `T` cycles all six
  themes directly. Changing the config replaces only the provider section,
  preserves favorites/recents/theme/images/UI/bookmarks/filters, backs up the old
  file, and rebuilds the provider in-process. The navigation stack is replaced
  with a fresh feed, so stale view/request ids are ignored without a restart.
- Presets are embedded with `include_str!`, so `cargo install` users do not need
  a repository checkout.

## Local library, filters and source health

`library.toml` sits next to `config.toml`. It stores:

- bookmark snapshots (`SavedPost` includes the full `Post`), so opening a saved
  post still shows its body when the provider is unavailable;
- read state for posts and comments (prefixed internally by separate sets);
- local filter rules: keywords, subreddits, authors, domains and hide-read.

`app::App` loads the file at construction and writes it atomically after every
mutation (mode `0600` on Unix). Filters are applied when a provider page arrives
and are also pruned from the visible feed immediately; if a page is fully
filtered, pagination continues until a visible post or the end of the listing is
reached. None of this data is sent to a provider.

`app::health::SourceHealth` records each completed request's status, error and
latency (last 12 samples). It is memory-only and rendered by the Source screen.
“Test now” simply refreshes the current view through the normal request path, so
the same metrics are updated without a separate probe.

## Runtime and event flow

```text
 keyboard/mouse ──(std thread: crossterm::event::read)──┐
 resize/paste                                           │
                                                        ▼
                                             AppEvent (unbounded mpsc)
 250 ms ticker ─────────────────────────────────────────┤
 provider/comment/image task results ───────────────────┤
                                                        ▼
                                                   App::handle_event
                                                        │
                                               mutate App state
                                                        │
                                              prepare() + render()
                                                        │
                                              ratatui buffer diff
```

- The **input thread** is a blocking `crossterm::event::read` loop. It forwards
  key, mouse, resize and paste events. Keeping it off the async runtime avoids
  blocking a Tokio worker on terminal input.
- The **ticker** is a Tokio interval (250 ms). It drives spinners, image-pending
  animation and toast expiry. Idle redraws are suppressed: `App::animating()`
  decides whether a tick sets the dirty flag.
- **Provider requests** are short-lived Tokio tasks. Each result is an `AppEvent`
  carrying request identity.
- **Image work**: downloads are async; decode/resize/protocol encoding happen in
  `spawn_blocking` so the UI thread never blocks. At most three image jobs run
  concurrently.
- The terminal image-protocol query runs **after** entering the alternate screen
  but **before** the input thread starts, so query responses cannot be stolen by
  the event reader. A splash is drawn first; the query is capped at 300 ms per
  stage, then falls back to Unicode half-blocks.
- **Multiplexers skip the query.** When `TMUX`/`STY`/`TERM` indicates tmux or
  screen, `Auto` and `halfblocks` use `Picker::halfblocks()` directly. The query
  previously consumed the first real keypress in tmux (and passthrough support
  is configuration-dependent). Explicitly forced `kitty`/`iterm2`/`sixel`
  protocols still query and encode that protocol, with passthrough left to the
  user's tmux configuration.

### Provider selection and HTML parsing

Provider construction happens at startup and again on every in-app config
change through `providers::build_provider`:

```text
provider.type = auto      → Reddit OAuth if credentials exist, else demo
provider.type = demo      → DemoProvider
provider.type = reddit    → RedditProvider or a visible failing provider
provider.type = old-reddit→ OldRedditProvider or a visible failing provider
provider.type = redlib    → RedlibProvider for the configured instance, or a
                            visible failing provider
```

An explicitly selected provider never falls back to a different source. Missing
credentials, a missing/unsafe Redlib instance, or `provider.html.allow_network =
false` produces a `FailingProvider` that returns a typed `ProviderError`; the UI
shows that error and the top bar names the selected source. A provider switch
clears the in-flight request start map and resets navigation to a new view id, so
a stale async result cannot be attributed to the new source.

HTML request flow (off Old Reddit or a user-selected Redlib instance):

```text
App request (FeedRequest/Post/Comments)
  → provider builds a source URL (sort path, top range, search query)
  → HtmlClient checks same-origin + allow_network
  → GET with ReddTTY User-Agent and Accept: text/html
  → status/content-type/size checks
  → provider-specific parser (scraper + static CSS selectors)
  → normalise into Post/Comment/Subreddit models
  → App applies the result only if view + request ids still match
```

Key rules:

- Parsers never panic on missing fields; absent title/author/score/time become
  explicit fallback values. A missing expected container is
  `HtmlStructureChanged`, not an empty feed.
- Login/CAPTCHA/block pages are detected only after the normal listing/comment
  container is missing, because Old Reddit includes a login form in ordinary
  page headers.
- Old Reddit and Redlib have separate parsers; Redlib markup is not treated as a
  variant of Old Reddit.
- Comment trees are rebuilt recursively. Parents are assigned only from the
  actual nested listing; the parser never infers a parent from a flat list.
- **Redlib routes are not Old Reddit routes.** Redlib's front page has no
  `/new` route; ReddTTY uses `/r/popular/<sort>` for Home/Popular and
  `/r/<sub>/<sort>` for subreddits (no trailing slash). Old Reddit keeps
  `/new/` and `/r/<sub>/<sort>/`.
- `after` is the source's own next-page URL, verified to stay on the configured
  origin. There is no numeric page guessing.
- Media URLs are resolved to absolute HTTP(S) and still pass the image
  downloader's host/size checks.

### Configuration migration and environment compatibility

- Default config is `${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml`
  (Windows: `%APPDATA%\reddtty\config.toml`). Cache/logs use
  `reddtty` under the platform cache root.
- On first run without an explicit `--config`, if the new config is absent and
  a legacy `threadline` config exists (`~/.config/threadline` or the old macOS
  Application Support location), it is copied to the new directory with mode
  `0600`. The old file is left in place; an existing new config is never
  overwritten; a second run is a no-op.
- Environment lookup prefers `REDDTTY_*`; the old `THREADLINE_*` names are a
  fallback only. Credentials are never logged, and legacy user-agent strings are
  normalized to the ReddTTY brand.

### Request identity and stale results

Every screen instance has a `ViewId`; every request has a monotonic `u64` id.

```rust
AppEvent::FeedLoaded { view: ViewId, request: u64, append: bool, result }
AppEvent::CommentsLoaded { view: ViewId, request: u64, result }
...
```

The handler finds the view by id and compares the request id with the view's
active request. Consequences:

- Changing sort/subreddit invalidates the old feed request; a late response is
  ignored.
- Popping a screen makes its results disappear (view not found).
- Background results for a feed still on the stack are applied without touching a
  newer screen.
- Pagination has its own request slot, so an append cannot overwrite a refresh.
- A same-cursor or duplicate-free append stops pagination to prevent request
  storms.

## App state

```rust
struct App {
    provider: Arc<dyn ContentProvider>,
    settings, config_path,
    nav: Vec<View>,                 // stack; feed state survives detail/gallery
    focus: Focus,                   // Sidebar | Feed | PostBody | Comments
    sidebar: SidebarState,
    modal: Option<Modal>,
    notifications: VecDeque<Notification>,
    images: ImageManager,
    hit_map: HitMap,                // rebuilt every render
    revealed: HashSet<String>,      // session-only NSFW/spoiler reveal
    ...
}

enum View {
    Feed(FeedView),                 // posts, heights, selected, offset, cursors
    Post(PostView),                 // post, comment tree, flattened rows
    Gallery(GalleryView),
}
```

Navigation is a stack, not a route enum, because preserving scroll position and
loaded pages is a first-class requirement. `Esc` pops; `Enter` pushes.

## Feed scrolling with variable card heights

Cards have different heights (title wrapping, body preview, optional image).
ReddTTY computes each card's height once per width/compact mode change and
caches `Vec<u16>` heights. Prefix sums give card start positions.

- The scroll offset is expressed in content lines but **snapped to card
  boundaries**. This avoids partially drawing a card over the feed header or
  status bar (Ratatui widgets cannot be clipped to an arbitrary parent rect
  without off-screen buffers).
- Only cards intersecting the viewport are rendered. A 500-post feed does not
  re-layout or re-render off-screen cards.
- `ensure_selected_visible` moves the viewport to the selected card's start when
  needed.
- Wheel scrolling moves one card at a time; keyboard `j`/`k` moves one card;
  PageUp/PageDown move a fraction of the viewport.
- A custom one-column scrollbar is drawn from `(offset, total, viewport)`.

Pagination is triggered when the selection is within three cards of the end or
the viewport is near the bottom, guard-railed by `loading_more`, `more_request`,
an existing cursor, and computed heights. This prevents the “heights unknown at
first load” bug that would otherwise fetch every page immediately.

## Comment tree

Comments are stored as a recursive tree (`Comment.replies`), never as a flat
list. For rendering, the tree is flattened into `CommentRow`s:

- depth (capped visually at 8) and a rail prefix (`│ ` / `├─ `),
- author, score, time, moderator/OP badges,
- deleted/removed state,
- collapsed flag and hidden descendant count,
- pre-wrapped body lines,
- “more replies” stub with child ids.

Collapsing inserts the comment id into a `HashSet`; the flattening pass skips its
children. Rows and heights are rebuilt in one pass and the selected id is
preserved when possible. “Load more” calls the provider
(`/api/morechildren` live, synthetic replies in demo), splices loaded comments
into the original tree position, and rebuilds rows without blocking the UI.

Comment sorting is provider-driven; the demo sorts locally, live mode passes the
documented sort parameter to Reddit.

## Rendering and mouse hit-testing

Rendering is immediate mode. During each frame the screens also build:

- `HitMap`: rectangles for sidebar entries, cards, sort tabs, comment rows,
  modal items, gallery arrows and retry buttons (later registrations win, so
  modal items sit above the modal backdrop).
- role panes (sidebar/feed/body/comments/gallery) used for wheel-scroll routing.

Clicks are resolved against the last frame's hit map. A same-target click within
450 ms is a double-click. Mouse wheel over a pane routes to that pane's scroll
function. Every mouse action has a keyboard equivalent.

## Responsive layout

`ui/layout.rs` is pure math and contains no widgets:

| Full terminal width | Layout |
|---|---|
| ≥ 130 | sidebar + feed + right panel |
| 76–139 | sidebar + feed |
| < 76 | single feed column |
| very small | compact headers, clipped cards, compact status hints |

The right panel and sidebar also yield space if the feed would become too narrow.
Resizing invalidates cached heights, body line caches and comment rows; the next
frame recomputes them.

Additional measured rules:

- **Status hints** are not gated by a fixed column count. Each view builds a
  full, medium, compact and micro hint list; the app measures rendered width and
  picks the richest list that fits. This keeps `q Quit` (and `? Help`) reachable
  at 30–80 columns instead of letting a near-fit line truncate the end.
- **Compact cards** turn on below 24 visible rows. Metadata/title keep one blank
  line; body and footer gaps collapse so two cards remain usable at 60×20 and
  80×24.
- **Feed header meta** (post count, “more”, Top range) is right-aligned and shown
  only if it fits beside the title. The description row is hidden below 96
  columns or 12 rows.
- **Card clipping**: a partially visible card renders only through its last
  complete content row; body/image lines never overwrite the bottom border.
- **Modals** clear the body pane behind them and notifications are confined to
  the feed pane, so side panels and unrelated panes do not bleed through.
- **Narrow chrome**: below 52 columns the `? Help` affordance yields space, but
  the `● DEMO`/`● LIVE` badge is retained whenever it fits. A truncated help
  modal reports how many shortcuts are hidden rather than silently dropping
  them.

## Image pipeline

```text
request(original_url, fetch_url, w, h)
  key = (original_url, w, h)
  if cache hit / pending / failed → return
  spawn task:
    semaphore(3)
    download (http(s) or file under allowed cache root)
    spawn_blocking:
      guess format (PNG/JPEG/GIF/WebP only)
      apply decode limits (8000px, 256 MiB allocation)
      thumbnail huge images to 4096px
      picker.new_protocol(..., Resize::Scale)  // up/downscale, preserve aspect
    send ImageReady { key, result }
  UI event inserts protocol into bounded LRU cache
```

- `Resize::Scale` is used so images fill the viewer area as much as their aspect
  ratio allows, including upscaling small fixtures. The terminal protocol is
  encoded off-thread.
- Cache keys include the cell size, so a feed thumbnail and a full-screen viewer
  encode separate protocols. Eviction is byte-budget based (estimated), oldest
  first.
- Pending downloads are deduplicated per key; failed keys are retained until
  refresh (`r`) clears failures.
- The downloader rejects unsafe schemes/hosts, redirects to private hosts, files
  outside the demo cache root, oversized bodies and non-image formats.
- Rendering checks `ImageStatus` and draws one of: protocol, spinner, failure
  message with “open externally”, or disabled notice.

## Terminal lifecycle and safety

- `ratatui::try_init()` enters the alternate screen and installs its panic hook.
  ReddTTY installs a hook before that which disables mouse capture, shows the
  cursor and logs the panic; Ratatui's hook restores raw mode/alternate screen.
- A `SessionGuard` disables mouse capture, shows the cursor and calls
  `try_restore()` on normal exit.
- `Terminal::clear()` is avoided in the event loop because it queries cursor
  position, which races with the input thread. View transitions use
  `Terminal::resize(current_area)`, which clears the viewport and resets the back
  buffer without reading stdin.

## Security model

- All untrusted text goes through `utils::text::sanitize` before display: ANSI
  CSI/OSC/DCS sequences, C0/C1 controls and bidi overrides are removed.
- Browser and image URLs are validated with `url::Url`; private/loopback/
  link-local hosts are rejected. No shell is used to open URLs.
- JSON and image bodies have byte limits and timeouts.
- Provider errors and image errors are sanitised before rendering or logging.
- Tokens are never printed or logged; diagnostics redacts credential values.
- Config writes use a temp file and atomic rename, with `0600` on Unix (`remove+rename` on Windows).

## Testing strategy

- Pure state machines (`FeedView`, `PostView`, modals, providers, utils) are
  unit-tested without a terminal.
- `ui::render` is driven with Ratatui `TestBackend` at 20×6 through 180×50,
  including modals, nested comments, hidden content and mouse hit maps.
- The live provider and downloader are integration-tested against a local
  OAuth/HTTP mock server, including failure modes and timeouts.
- The HTML providers are tested with synthetic Old Reddit/Redlib fixtures for
  posts, galleries, pagination, sorts, nested/deleted comments, missing fields,
  login walls and changed structure, plus a local HTTP mock for content types,
  403/429, origin checks and the network gate. Live access is still unverified.
- The release binary is driven through a real PTY by `scripts/pty_verify.py`
  (isolated tmux socket, 36 captures / 66 assertions): feed, detail, body
  scroll, comment collapse/expand, gallery, sort, goto, search, quit and a
  responsive sweep. This catches clipping and key-loss bugs that `TestBackend`
  cannot.
- `scripts/protocol_probe.py` runs the binary in a raw PTY with each forced image
  protocol and scans the emitted escape sequences. This proves the encoder path,
  not that a GUI terminal painted it.
- `examples/stress.rs` renders 500 posts, 1,000 nested comments and 100 real
  images and prints frame/input/collapse timings plus RSS. It is a release-mode
  example, not part of CI.
- Real terminal graphics protocols and live Reddit approval cannot be verified in
  CI; those gaps are called out in `docs/TEST_REPORT.md` rather than being
  presented as tested. Pixel rendering is only claimed where a screenshot or
  direct visual inspection exists.

## Deliberate trade-offs

- Card-boundary scrolling instead of arbitrary pixel-perfect line scrolling for
  cards, to avoid off-screen buffer copies and header overpainting.
- Memory-only caches; simpler and avoids persisting content that may be deleted.
- Requests are not cancelled when a screen is popped (they are ignored); this is
  simpler than cancellation tokens and bounded by per-view requests.
- No interactive OAuth browser flow in v1; refresh tokens are supplied externally
  so token storage stays out of scope.
- HTML network access is off by default. This trades convenience for an explicit
  acknowledgement that the user checked the source's terms; it is not a
  technical access-control mechanism.
