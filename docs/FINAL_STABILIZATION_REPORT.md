# Final stabilization and independent audit report

Date: **2026-11-26**  
Repository: ReddTTY working tree (uncommitted stabilization changes)  
Environment: macOS 27.0.1, Apple M1, Rust 1.100.0, release profile,
TestBackend plus tmux PTY.

## Executive summary

Starting state for this pass: 341 passing tests, clean fmt/Clippy, release
build, 68 PTY captures / 114 assertions, four protocol probes.

Four independent read-only audits were run (persistence, providers/async,
TUI/accessibility, tests/performance). Their confirmed findings were validated
against source and addressed where feasible:

- Multi-process library lost updates: added an OS file lock plus delta merge
  against a loaded baseline. App now uses the merged save path.
- Config multi-process lost updates: added a revision CAS under the same lock.
- Corrupt config: explicit recovery state, blocked saves, original preservation,
  backup/restore/create-new UI.
- Silent 500-bookmark truncation: removed; 1,001-bookmark round-trip test.
- Windows replacement: removed the copy-over fallback; std rename uses
  `MoveFileExW(MOVEFILE_REPLACE_EXISTING)`. Parent-directory fsync added on Unix.
- Legacy library migration, non-UTF8 recovery, unreadable-file backups.
- Provider switch: probe sort clamping, cancellation, stale-result protection.
- Real line scrolling for oversized feed cards (`J`/`K`, wheel, page movement),
  with off-screen card-buffer clipping.
- Settings cursor restoration, confirm hitboxes/labels, NO_COLOR guards,
  selected-row contrast against selected/surface backgrounds.
- Bookmark search Ctrl+U, return from post/gallery, modal wheel routing.
- Clippy/MSRV blocker fixed (`rust-version` 1.89), Windows CI compile/test job,
  docs included in release archives.

Final automated state: **368 passed, 0 failed, 0 ignored**; fmt and Clippy clean;
release build clean; PTY 68/114; protocol probe 4/4.

Verdict: **BETA READY**, not STABLE CANDIDATE. The remaining blockers for RC are
Windows runtime verification, visual image-protocol verification, and PTY
journeys that are still only partially automated (real two-process lock,
restart persistence, failure-state journeys).

## Previous findings verification

| Finding | Previous status | Current status | Evidence |
|---|---|---|---|
| False success after failed save | Fixed | Verified | rollback tests; `save_library() -> bool` |
| Corrupt library overwritten | Fixed | Verified | corrupt/non-UTF8 preservation tests |
| Corrupt config overwritten | Partial | Fixed | `SettingsLoad`, blocked saves, recovery UI tests |
| Atomic replacement | Partial (Windows copy) | Fixed code, Windows unverified | `fs::rename` + parent fsync; Windows CI job added |
| Multi-process lost updates | Deferred | Fixed for library; config conflict | `FileLock`, `save_merged`, revision CAS tests |
| Bookmark truncation | Unresolved | Fixed | 1,001-bookmark test |
| Unbounded HTML | Fixed | Verified chunked | chunked/oversized tests |
| Modal notifications | Fixed | Verified | render test |
| Gallery Shift keys | Fixed | Verified | uppercase test |
| Redlib quick start | Fixed | Verified | `--allow-network` test |
| First-run 80×24 | Fixed | Verified | setup render tests |
| Custom Redlib | Fixed | Verified | consent/probe tests |
| Provider switch transaction | Fixed | Improved | probe sort clamp, cancel/stale tests |
| Source Health | Fixed | Verified | independent probe tests |
| Theme contrast | Fixed | Extended | selected/surface contrast test |
| NO_COLOR | Fixed | Guarded | no-RGB + no_color commit tests |
| Image/body layout | Fixed | Verified | layout tests |
| Pagination recovery | Fixed | Extended | storm/retry/duplicate cursor tests |
| Feed line scrolling | Deferred | Implemented | line-scroll + partial-render tests |
| Bookmark management | Fixed | Extended | search/delete/return tests |
| Settings cursor | Partial | Fixed | submenu cursor test |
| Info drawer actions | Fixed | Verified | action tests |
| Domain mute self posts | Fixed | Verified | external-domain tests |
| OP/read markers | Fixed | Verified | OP badge render test |
| Last view restore | Fixed | Extended | top range/selected id round-trip |

## Independent audit results

| ID | Severity | File/line | Finding | Resolution | Regression test |
|---|---|---|---|---|---|
| P-01 | High | app/mod.rs save_library | App still called non-merging `save` | switched to `save_merged`; missing-file baseline set | two-App merge test |
| P-02 | High | settings.rs migrate | legacy library not migrated | copy `library.toml` create-new with 0600 | copy test |
| P-03 | Med-High | settings.rs read error | non-UTF8 config had no backup | backup on read error; recovery allowed | non-UTF8 test |
| P-04 | Med-High | app recovery | exit persist could overwrite restored backup | `skip_exit_persist` + corrupt guard | restored-backup test |
| P-05 | Med | open_post_object | rollback snapshot taken after mutation | snapshot before mark-read | covered by save-failure path |
| P-06 | Med | provider switch | backup failure swallowed | abort switch on backup error | source failure test path |
| P-07 | Med | newest_recovery_backup | corrupt backups invisible | include `.corrupt-`, mtime, validity filter | recovery UI tests |
| P-08 | Med | serde defaults | typo keys silently dropped | unknown-top-level-key warnings | parse warning path |
| P-09 | Med | read history | unbounded read sets | documented limitation; no silent loss | — |
| P-10 | Med-Low | library read error | no backup/reset | backup on read error; read-only state | non-UTF8 library test |
| P-11 | Low-Med | toggle sidebar/last view | save result ignored | revert on failure | existing rollback tests |
| P-12 | Low-Med | exit | library/last view not saved | `finalize_on_exit` | restore test |
| P-13 | Low | persistence | no dir fsync; Windows crash semantics | Unix dir fsync; Windows unverified | platform matrix |
| P-14 | Low | backups/temp | TOCTOU/hygiene | unique create-new, 0600, lock | permissions tests |
| PRV-01 | High | feed wants_more | error retry storm possible | `more_error` guard restored | storm/retry tests |
| PRV-02 | High | probe_source/switch | `best` unsupported by HTML | probe sort clamped | probe-sort test |
| PRV-03 | High | stale probe | old provider health contamination | clear probe/pending on reload | late-probe test |
| PRV-04 | High | switch cancel | late success applied after Esc | cancel pending on Esc; conditional modal close | cancel test |
| PRV-05 | Med | empty listing | 200/empty reported healthy | message distinguishes valid-empty | source probe tests |
| PRV-07 | Low | request_starts | early-return leak | moved request id after guards in pagination | compile/test |
| PRV-08 | Low | redirect policy | same-host scheme/port downgrade | compare scheme+host+port | mock redirect tests |
| PRV-09 | Low | HTML limits | chunked/gzip untested | chunked oversized test added | html_providers |
| PRV-10 | Low | pagination | duplicate-only fresh cursor stopped | keep fresh cursor | duplicate/repeated tests |
| F1 | High | modal q | q quit from non-text modals | removed global modal quit | help-search path |
| F2 | High | settings render | recovery row clipped | height 19 when corrupt | UI render test |
| F3 | Med | theme contrast | selected/surface pairs failed | enforce + test against selected/surface/alt/code | contrast test |
| F4 | Med | NO_COLOR | settings could re-enable color/images | guards + tests | no_color test |
| F5 | Med | page move | PgDn no-op on tall card | page by lines | feed test |
| F6 | Med | sidebar focus | page/home/end leaked to feed | focus guards | — |
| F7 | Med | confirm modal | no hitboxes, wrong verb | ConfirmYes/No + action verb | UI hitbox test |
| F8 | Med | settings cursor | submenu reset | preserved cursor, fresh open resets | submenu test |
| F11 | Med | modal wheel | ignored | routed to modal list movement | — |
| F12 | Low-Med | gallery wheel | scrolled hidden feed | gallery fallback step | — |
| F13 | Low-Med | bookmark return/search | lost from post/gallery; no Ctrl+U | return on any view; Ctrl+U | bookmark flow test |
| F15 | Low | body indicator | overwrote last line | reserve indicator row | layout/render |
| F19 | Low-Med | source picker | custom source no active marker | “Current source” row | custom picker test |
| REL-01 | Blocker | Clippy | MSRV/lock API/args | rust-version 1.89; lint fixes | Clippy clean |
| CI-01 | High | workflow | no Windows job | added windows-check compile/test | configured, not executed |
| SEC-01 | High | browser.rs | cmd metacharacter injection | rundll32 FileProtocolHandler | code review |
| PERF-01 | High | docs/stress | stale RSS/docs | updated measured values | stress run |
| PERF-02 | Med | card_layout | full-body wrap per frame | bounded prefix before wrap | stress rerun |

## Data integrity

- **Concurrent library:** `FileLock` (OS advisory lock, 2 s timeout) + baseline
  delta merge for bookmarks, read state and filters. Two stale `Library`
  sessions and two `App` sessions preserve both bookmarks; removals do not
  resurrect deleted entries.
- **Concurrent config:** revision CAS under lock; a stale session receives a
  clear “another ReddTTY process” error and does not overwrite.
- **Corrupt library:** original bytes preserved, unique `.corrupt-*` backup,
  saving disabled until repaired/removed.
- **Corrupt config:** original bytes preserved, saving blocked, explicit
  recovery UI (temporary defaults, newest valid backup, create new, quit).
- **Windows replacement:** `fs::rename` uses `MoveFileExW` with replace-existing;
  copy-over fallback removed. Windows runtime/crash semantics are **not
  verified**; a Windows CI compile/test job was added but not executed here.
- **Bookmarks:** no 500-item truncation; 1,001 survive save/load.
- **Persistence failures:** rollback paths verified for bookmarks, filters,
  favorites, theme, image protocol, read state and settings.
- **Residual:** read-history sets are unbounded; parent-directory fsync is
  Unix-only; concurrent config changes are rejected rather than merged.

## UX verification

Verified by tests/render: modal notifications, Gallery Shift keys, responsive
picker at 80×24/30×10, custom Redlib consent, provider-switch feedback, Source
Health, image/body reservation, comments on short terminals, OP badge, read
marker, retry hitboxes, wheel focus, bookmark search/return, offline comment
labels, selected-row contrast, NO_COLOR no-RGB output, effective protocol.

Implemented in this pass: true line scrolling (`J`/`K`), page-by-lines, partial
card clipping via off-screen buffers, settings cursor restoration, confirm
hitboxes/labels, modal wheel routing, gallery wheel isolation, body-indicator
row reservation, current-source picker row.

Not fully verified: notification placement over modal footers, very small
terminal content policy beyond a minimum-size message, visual selected-row
appearance in every terminal, image/body order consistency.

## Performance

Measured with `./target/release/examples/stress` on Apple M1, macOS 27.0.1,
release profile, 160×45 TestBackend:

| Metric | Before this pass | Final | Change |
|---|---:|---:|---:|
| Feed frame avg (500 posts, 1,200 frames) | 0.324 ms | 0.328 ms | noise |
| Feed frame p95 | 0.447 ms | 0.338 ms | improved |
| Feed first frame | 3.08 ms | 4.79 ms | noise/startup |
| Comments frame p95 (1,000 nodes) | 0.266 ms | 0.272 ms | noise |
| Collapse/expand avg | 1.288 ms | 1.286 ms | noise |
| 100 images total | 155.9 ms | 148.0 ms | improved |
| Peak RSS after images | 52.3 MB | 51.3 MB | improved |

No claim is made for cold startup or idle CPU in this report beyond the stress
harness; those were not measured with a stable method here.

## End-to-end verification

| Journey | Result | Assertions | Evidence | Limitations |
|---|---|---:|---|---|
| A New user | PARTIAL | 114 total PTY | setup/render tests + PTY | no full restart loop |
| B Custom Redlib | PARTIAL | app tests | validation/consent/probe tests | no local mock PTY server |
| C Failed provider | PARTIAL | app tests | failure/cancel tests | no PTY save-failure injection |
| D Bookmark lifecycle | PARTIAL | app + UI | search/delete/return tests | no PTY restart |
| E Gallery | PASS (PTY) | PTY | 08/08a/08b captures | visual pixels not verified |
| F Failure recovery | PARTIAL | unit/integration | corrupt/oversize/retry tests | no PTY disk-failure journey |
| G Concurrent processes | PARTIAL | app/library tests | merge/CAS/lock timeout | not two real processes |
| H Long feed card | PARTIAL | unit/UI | line scroll + partial render | test injects heights; no real tall card fixture |

PTY suite: **68 captures / 114 assertions**, pass.

## Automated tests

- Baseline at start of this pass: 341 passed.
- Final: **368 passed, 0 failed, 0 ignored**.
  - 284 library/unit
  - 2 binary/setup
  - 14 HTML provider
  - 13 provider/image
  - 55 UI TestBackend
- New regression coverage: library merge, config revision conflict, corrupt/
  non-UTF8 recovery, 1,001 bookmarks, lock timeout, stale probe, cancelled
  switch, probe sort clamp, chunked HTML limit, duplicate cursor, line scroll,
  partial card rendering, settings cursor, NO_COLOR guards, confirm hitboxes,
  recovery row rendering.

## Platform compatibility

| Platform / area | Status |
|---|---|
| macOS ARM64 build/test | Tested locally |
| Linux build/test | CI configured; not executed in this session |
| Windows compile/tests | CI job configured; **not executed** |
| Windows persistence semantics | Not verified |
| Unix atomic rename | Tested |
| Windows MoveFileEx replace | Code-reviewed, not runtime-verified |
| Kitty/iTerm2/Sixel/half-block bytes | Byte-level verified |
| Visual image rendering | Not verified |
| NO_COLOR RGB output | Tested (no RGB cells) |
| Long-session memory | Stress harness only |

## Remaining risks

- Windows file replacement/locking/ACL behavior is unverified; only compilation
  is configured in CI.
- Read history and comment read sets are unbounded.
- Concurrent config changes are rejected, not merged; users must reopen.
- Notification placement can overlap a modal footer on short terminals.
- Real tall-card fixtures are not exercised; line-scroll tests inject heights.
- PTY journeys do not yet cover restart persistence, real two-process locking,
  disk-failure injection or live resize.
- Pixel image protocols are byte-verified only.
- No measured cold-start or idle-CPU baseline.

## Changed files (important)

- `src/persistence.rs`: file lock, rename-only replacement, dir fsync, unknown-key helper.
- `src/library.rs`: baseline delta merge, merged save, no bookmark truncation, non-UTF8 backup.
- `src/config/settings.rs`: `SettingsLoad`, revision CAS, recovery writes, legacy library copy, unknown-key warnings.
- `src/app/mod.rs`: recovery state/UI, probe generation/cancel, probe sort clamp, settings cursor, exit finalize, NO_COLOR guards, line-scroll keys, sidebar guards.
- `src/app/feed.rs`: line scrolling, page-by-lines, duplicate cursor handling.
- `src/ui/widgets/post_card.rs`: buffer rendering + bounded body/title wrapping.
- `src/ui/screens/feed.rs`: partial card clipping, reserved status row.
- `src/ui/screens/overlays.rs`: recovery/settings/confirm/source picker rendering.
- `src/ui/theme.rs`: selected/surface contrast enforcement.
- `src/ui/mod.rs`: minimum-size message, notification/modal layout.
- `src/utils/browser.rs`: Windows `rundll32` launch.
- `.github/workflows/build.yml`: Windows check job; docs in archives.
- `Cargo.toml`: MSRV 1.89.
- `Makefile`: `make verify`.
- Tests/docs as listed in the diff.

## Release recommendation

Recommended version: **v0.2.0-beta.1**.

Reason: the critical data-integrity and async-state issues found by the
independent audits are fixed and covered, but Windows runtime behavior, visual
image rendering, and several end-to-end journeys remain unverified. It is not
honest to call this `v0.2.0-rc.1` or `STABLE CANDIDATE` until those gates are
executed on real platforms.

No commit, tag, push, package or release was performed.
