# Stability and UX improvement report

Date: **2026-11-26**  
Scope: ReddTTY 0.1.1 working tree (post-`v0.1.1` UI work on `main`)  
Method: static inspection, targeted regression tests, `TestBackend` renders,
local mock HTTP servers, the release binary through a real PTY, and the existing
protocol probe. No live Reddit account, write action or public-instance
availability claim is involved.

## Executive summary

The starting point was a feature-rich early beta with 302 passing tests. Several
user-visible problems were confirmed and fixed: false success toasts after failed
disk writes, corrupt-file overwrite risk, non-atomic file replacement, unbounded
HTML buffering, hidden modal notifications, swallowed Gallery uppercase keys,
a non-working documented Redlib quick start, first-run overflow at 80×24, theme
contrast failures, image/body layout starvation, unrecoverable pagination,
unlabelled offline bookmarks, and several misleading docs.

The final state is **BETA READY**. The core read-only flows are usable and the
highest-risk data-integrity paths are protected, but feed line-scrolling inside
an oversized card, multi-process lost-update protection, and full module
decomposition remain deferred. Real pixel image rendering and Windows were not
verified in this environment.

## Issue tracking

Statuses: Fixed / Already fixed / Partially fixed / Not reproducible / Deferred /
Blocked / Requires manual verification.

| ID | Finding | Severity | Status | Root cause | Fix | Tests |
|---|---|---|---|---|---|---|
| 1 | Success toast after failed library/config save | P0 | Fixed | `save_library()` returned nothing; callers always notified success | Typed `save_library() -> bool` + rollback helper; config saves return bool and revert | failed bookmark/filter/settings tests |
| 2 | Corrupt library overwritten | P0 | Fixed | Parse failure returned an empty default and later saves replaced the file | `LibraryLoad` distinguishes missing/read/parse; corrupt files are backed up and saves disabled | corrupt-library preservation test |
| 3 | Corrupt config overwritten | P0 | Partially fixed | Parse failure returned defaults | Original bytes are preserved and copied to a unique `.corrupt-*` backup before defaults are used | corrupt-config test |
| 4 | Fixed temp filename / delete-before-rename | P0 | Fixed | `path.with_extension("toml.tmp")`; Windows removed the destination first | Shared `persistence` module with unique sibling temp files, flush+sync, atomic rename on Unix, copy-over on Windows | replace/permissions/backup tests |
| 5 | Multi-process lost updates | P1 | Deferred | No inter-process lock or revision check | Unique temp files prevent torn files, not last-writer-wins | documented limitation |
| 6 | Unbounded HTML response buffering | P1 | Fixed | `response.bytes().await` before the size check | Chunked `read_bounded` with separate error-body bound | oversized success/error tests |
| 7 | Modal notifications invisible | High | Fixed | Notifications rendered before the modal backdrop | Notifications render last; modal-aware area | UI regression test |
| 8 | Gallery uppercase shortcuts swallowed | High | Fixed | Any `SHIFT` event returned early | Only Shift+arrows pan; other keys fall through | uppercase gallery test |
| 9 | Documented Redlib quick start failed | High | Fixed | `allow_network` stayed false | `--allow-network` CLI flag, docs and tests; no implicit persistence | CLI parse test |
| 10 | First-run picker overflowed 80×24 | High | Fixed | Fixed row height and no scroll offset | Scrollable/compact picker, custom-instance row, Home/End/PgUp/PgDn | setup 80×24 and 30×10 tests |
| 11 | No in-app custom Redlib URL | High | Fixed | Only presets/TOML | Custom form → validation → explicit consent → bounded probe → save | custom-instance flow tests |
| 12 | Provider switch replaced a working source before verification | High | Fixed | Saved config and reloaded immediately | Candidate provider is probed first; failure keeps old source/config/nav | success/failure switch tests |
| 13 | “Test now” refreshed the feed | High | Fixed | Action reused `refresh_current()` | Independent bounded probe event; visible feed untouched | probe success/failure tests |
| 14 | Theme contrast failures | High | Fixed | Fixed palettes with low-contrast muted/accent values | Runtime contrast enforcement + WCAG-style quantitative tests | all-8-theme contrast test |
| 15 | `NO_COLOR` still emitted RGB | High | Fixed | Mapped to the RGB monochrome theme | Session-only `Theme::no_color()` with terminal defaults and reverse selection | no-RGB buffer test |
| 16 | Image could consume the whole post body | High | Fixed | Image height used all remaining rows | Reserve ≥3 body rows; shrink/skip preview; comments keep rows on short terminals | layout tests at 5 sizes |
| 17 | Pagination failure unrecoverable | High | Fixed | Append error set `has_more=false` | `more_error` pauses auto-pagination, keeps cursor, `R`/inline retry resumes deliberately | storm + retry tests |
| 18 | Feed line scrolling inside oversized cards | Medium | Partially fixed | Renderer paints whole cards; offset snaps to card starts | Not changed: opening the post exposes the full body. Deferred to avoid an unsafe rendering rewrite | documented limitation |
| 19 | Bookmark list lacked search/paging/confirmation | Medium | Fixed | Single `selected` index, immediate delete | Case-insensitive search, PgUp/PgDn/Home/End, confirm delete, state restored after reading | bookmark flow test + render test |
| 20 | Offline bookmarks implied comments existed | Medium | Fixed | Generic “N comments” label | Labels comments as “not stored”; empty state distinguishes no match | render test |
| 21 | Info drawer actions were decorative | Medium | Fixed | Modal only handled close | Advertised keys execute; drawer scrolls; footer explains immediate execution | info-action tests |
| 22 | Domain mute on self/text posts | Medium | Fixed | Fell back to subreddit name as a domain | `Post::external_domain()` rejects self/Reddit/gallery/missing domains with guidance | domain-mute tests |
| 23 | OP-only mode had no state or honest empty text | Medium | Fixed | `op_only` never read by UI | Header badge + “No comments by u/…” state | OP-only render test |
| 24 | Comment read state was invisible | Medium | Fixed | No UI use of `read_comments` | `✓` marker on read comments | existing read-state flow |
| 25 | Last view restored only feed/sort | Medium | Fixed | Only two fields persisted | Top range and selected post id persisted with backward-compatible defaults | round-trip test |
| 26 | Destructive clear operations had no confirmation | Medium | Fixed | Immediate mutation | Confirm dialogs for filters and read history; cancel restores UI | clear-confirmation tests |
| 27 | Configured vs effective image protocol | Medium | Fixed | Settings displayed the configured value | Settings/picker show configured, detected and effective values; Auto recovers after disable | protocol recovery test |
| 28 | Retry hitboxes misaligned / wheel focus | Medium | Fixed | Fixed offsets; wheel did not move focus | Hitbox derived from rendered retry line; wheel sets pane focus | compile/render coverage |
| 29 | Gallery `i`/`I` inconsistent and docs reversed | Medium | Fixed | Gallery swapped semantics | `i` info, `I`/`y` copy URL on every screen; docs/help/status updated | gallery uppercase test |
| 30 | Docs claimed a non-existent `--images` flag | Medium | Fixed | Stale message | Points to Settings → Images / `images.enabled` | static |
| 31 | README missing `r`/`x`/`h`; Home/End wrong; six-theme text | Medium | Fixed | Stale docs | Updated README, KEYBINDINGS, TEST_REPORT | static |
| 32 | CI wording overclaimed “every push” | Low | Fixed | Stale description | Documented `main`, PRs, tags, manual dispatch; archive contents corrected | static |
| 33 | Command-palette invalid-command description | Low | Fixed | Omitted highlighted-action behavior | README clarified highlighted action wins | static |
| 34 | Oversized app/overlay modules | Low | Deferred | `app/mod.rs` and `overlays.rs` carry many responsibilities | Behavioural fixes were prioritised; extraction would be a separate low-risk refactor | not attempted |
| 35 | Performance claims / measurements | Low | Deferred | No profiling this pass | No speculative optimisation and no unmeasured claims | not measured |
| 36 | Windows replacement semantics | Low | Requires manual verification | No Windows host here | Copy-over fallback avoids delete-first; crash atomicity not claimed | CI only |
| 37 | Kitty/Sixel/iTerm2 visual output | Medium | Requires manual verification | No GUI capture permission | Protocol bytes verified; half-blocks visually verified | protocol probe |

## Technical changes

- **Persistence**: new `src/persistence.rs`; atomic/private writes, unique temp
  names, sync, corrupt backups, restrictive permissions. Library and settings
  use it.
- **Data model**: `LibraryLoad`, `last_top_range`, `last_selected_post`,
  `external_domain()`, pagination `more_error`.
- **App state**: probe/pending-switch state, bookmark return state, custom-form
  return state, session no-colour flag, read-comment rendering.
- **Events**: `ProbeFinished`, `SwitchProbeFinished`.
- **CLI/config**: `--allow-network`; first-run `SetupChoice`; custom Redlib
  consent flow.
- **UI**: notifications after modals, scrollable source pickers, bookmark
  search, info drawer actions/scroll, contrast enforcement, effective protocol
  display, OP badge, retry rows, corrected hints.
- **Providers**: bounded HTML streaming.
- **Docs**: README, KEYBINDINGS, TEST_REPORT and this report.

## Validation

Baseline before changes:

- 302 tests passed (231 library, 11 HTML, 13 provider/image, 47 UI).
- `cargo fmt --check` and Clippy clean.
- Release build clean.
- PTY: 68 captures / 114 assertions.

Final:

- **368 tests passed, 0 failed, 0 ignored**
  (284 library, 2 binary, 14 HTML, 13 provider/image, 55 UI).
- `cargo fmt --all -- --check`: pass.
- `cargo clippy --all-targets --all-features -- -D warnings`: pass.
- `cargo build --release --locked`: pass.
- `python3 scripts/pty_verify.py`: 68 captures, 114 assertions, pass.
- `python3 scripts/protocol_probe.py`: Kitty, iTerm2, Sixel, half-blocks all
  emitted expected bytes.
- `git diff --check`: pass.

No performance numbers are reported because no profiling was performed.

## Before / after evidence

- 80×24 setup and source picker now keep the selected row and the custom
  instance visible; tests assert both.
- A corrupt `library.toml` and `config.toml` keep their exact original bytes and
  gain a unique `.corrupt-*` copy.
- A failed bookmark/filter/settings write rolls back the in-memory change and
  shows an error instead of “Saved”.
- Gallery uppercase `I` copies while lowercase `i` opens info.
- Image+text post detail retains at least three body rows at 60×18 through
  130×36.
- A failed append keeps the cursor and shows “press R to retry”.

## Remaining risks and unverified areas

- **Feed line scrolling inside a card taller than the viewport** is not
  implemented; users can open the post to read the full body. This is the main
  remaining reading-flow limitation.
- **Concurrent ReddTTY processes** can still overwrite each other’s latest
  library/config changes (last writer wins). Files are never torn or truncated.
- **Corrupt config** is preserved and backed up, but the app continues with
  defaults and may replace it after an explicit user save. The library has a
  stricter read-only corrupt state.
- **Windows** file replacement and crash durability are not verified locally.
- **Pixel image protocols** were byte-verified, not visually verified in a GUI
  terminal.
- **Public Redlib instances** can rate-limit or disappear; a successful probe is
  not a permanent availability guarantee.
- No account login, OAuth flow or Reddit write action was added.
- No speculative performance optimisation was applied.

## Release readiness

**BETA READY.** Critical persistence and corruption paths are protected, the
documented quick starts work, modal feedback is visible, Gallery keys and theme
contrast are fixed, provider changes are transactional, and the main reading
flows are regression-tested. It is not declared STABLE because oversized-card
scrolling, multi-process coordination and Windows/visual image verification
remain open.
