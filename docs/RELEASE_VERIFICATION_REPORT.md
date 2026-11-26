# ReddTTY release verification report

Date: **2026-11-26**  
Commit under verification: `d0eed8a` plus this verification sprint's
uncommitted changes.  
Environment: macOS 27.0.1, Apple M1, Rust 1.100.0, release profile, tmux PTY,
TestBackend for render assertions.

## Baseline and final results

Previous report: 368 tests, 0 failed, clean fmt/Clippy, release build,
68 PTY captures / 114 assertions, 4/4 protocol probes.

Final this sprint:

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets --all-features -- -D warnings` | pass |
| `cargo test --locked` | **381 passed, 0 failed, 0 ignored** |
| `cargo test --locked --all-features` | pass |
| `cargo build --release --locked` | pass |
| `cargo build --release --locked --example stress` | pass |
| `cargo build --release --locked --example bookmarks_bench` | pass |
| `python3 scripts/pty_verify.py` | 68 captures / 114 assertions, pass |
| `python3 scripts/process_verify.py` | 15 real-process assertions, pass |
| `python3 scripts/recovery_verify.py` | 25 executable recovery assertions, pass |
| `python3 scripts/protocol_probe.py` | 4/4 byte-level protocols |
| `git diff --check` | pass |

Suite split: 293 library/unit, 2 binary/setup, 15 HTML provider, 13
provider/image, 57 UI TestBackend.

## Independent reviewer findings and resolutions

The sprint ran three read-only reviewers (persistence, providers/async, UI).
Confirmed P0/P1 findings from the prior audit cycle were already addressed in
this branch; this sprint added executable verification and fixed the remaining
P1/P2 items below.

| Finding | Severity | Resolution |
|---|---|---|
| Retry-After not enforced for manual retry | P1 | Per-feed cooldown; numeric and HTTP-date parsing; `R` blocked with visible remaining time; tests |
| Notifications could cover modal footer | P2 | Modal feedback renders in the status-bar row; footer remains visible; 80×24 buffer test |
| Real multi-process persistence unverified | P0 verification | `scripts/process_verify.py` launches two release binaries against one library |
| Recovery unverified through the binary | P0 verification | `scripts/recovery_verify.py` corrupts files and checks bytes/backups/recovery actions |
| Real tall-card fixture missing | P1 verification | Real `card_layout` test with long Markdown/Unicode + image at 80×12; card exceeds viewport and scrolls |
| No bookmark/read-history benchmark | P1 verification | `examples/bookmarks_bench.rs` for 1k/10k bookmarks and 50k read-comment IDs |
| Missing/truncated library during save could erase unchanged session data | P1 data loss | Full in-memory state preserved when disk is missing/empty; two regression tests |
| Clicking outside source picker bypassed cancellation | P1 | Modal-close path cancels pending switch/probe |
| Refresh could race an append request | P1 | `wants_more` gated on refresh/request/error/cooldown |
| Redlib blocked page could look like an empty listing | P1 | gate detection + real listing-container check |
| Two-cursor duplicate cycle could loop | P1 | bounded duplicate-page cap |
| Line scrolling snapped back during prepare() | P0 | selected card snaps only when fully outside; prepare-preservation test |
| Modal wheel changed state without repaint / typed into bookmark search | P1 | dirty repaint; search mode ignores wheel |
| Settings/theme picker could select clipped rows | P1 | scroll offset + visible-row window |
| NO_COLOR splash/setup emitted RGB | P1 | terminal defaults in splash and setup |
| Bookmark return consumed on Gallery→Post | P1 | restore only when the bookmarked Post is popped |
| Muted/semantic contrast on selected rows | P1 | selected_bg added to contrast enforcement/tests |
| Post body indicator hid the last line | P1 | body viewport reserves the indicator row |
| CI Windows job unexecuted | High verification gap | Job configured; still not executed in this environment |

No new critical data-loss defect was found in the final review pass.

## Real multi-process results

`scripts/process_verify.py` uses two actual `target/release/reddtty` processes
through tmux, sharing one isolated `--config` directory. It asserts persisted
TOML, not in-memory state.

| Scenario | Result |
|---|---|
| Two processes add different bookmarks | PASS — both IDs persisted |
| One removes while another adds | PASS — removed ID not resurrected, added ID present |
| Concurrent read state | PASS — both read IDs persisted |
| Concurrent keyword filters | PASS — both filters persisted |
| Concurrent theme/config writes | PASS — valid TOML, revision advanced, one winner |
| Restart after concurrent writes | PASS — both bookmarks still present |
| External lock held by another process | PASS — visible timeout, no bookmark written |
| Lock holder killed with SIGKILL | PASS — OS releases lock; next save succeeds |

The lock timeout is bounded at 2 seconds. Lock files are never deleted, so a
crashed process cannot leave an unbreakable stale lock.

## Recovery and disk-failure results

`scripts/recovery_verify.py` uses the release binary and byte comparisons.

| Scenario | Result |
|---|---|
| Invalid config TOML | PASS — original bytes preserved, `.corrupt-*` backup created, save blocked |
| Truncated config | PASS |
| Non-UTF8 config | PASS — backup created from raw bytes |
| Invalid library TOML | PASS — original preserved, bookmark save blocked |
| Non-UTF8 library | PASS |
| Exit while using temporary defaults | PASS — config bytes unchanged |
| Theme change during recovery | PASS — blocked, bytes unchanged |
| Explicit create-new configuration | PASS — valid TOML written, original backup remains |
| Explicit restore newest backup | PASS — restored bytes match backup exactly |
| Read-only directory write failure | PASS — visible “Could not save”, no false “Saved”, no file created |

## Journey A–H

| Journey | Result | Evidence | Limitations |
|---|---|---|---|
| A New user | PARTIAL | setup render tests at 80×24/30×10; real-process restart verifies persisted state | first-run picker not re-driven end-to-end in PTY |
| B Custom Redlib | PARTIAL | validation/consent/probe app tests | no deterministic mock-Redlib PTY journey |
| C Failed provider | PARTIAL | switch failure/cancel/stale tests | persistence-failure PTY not injected |
| D Bookmark lifecycle | PARTIAL | real-process add/remove/restart; search/delete/return tests | no PTY search after restart |
| E Gallery | PASS (interaction) | PTY captures 07/08/08a/08b; uppercase/info/copy assertions | pixel rendering not visually verified |
| F Failure recovery | PARTIAL | executable corrupt/write-failure; provider/HTML/rate tests | oversized/403/429 not driven through the binary |
| G Concurrent processes | PASS | 15 real-process assertions | same-machine, same-filesystem only |
| H Long feed card | PARTIAL | real `card_layout` long Markdown/Unicode/image test; line scroll tests | no PTY capture of a real tall card |

No unexecuted journey is marked PASS.

## Performance measurements

Environment: Apple M1, macOS 27.0.1, release, 140×38 tmux for startup/idle and
160×45 TestBackend for stress.

### Startup and idle

7 runs with `--demo --no-images`, measured until the Home screen:

- median **26.6 ms**
- samples: 45.9, 26.6, 26.1, 26.3, 27.4, 26.0, 26.9 ms
- idle CPU: **0.0%** across four 5-second samples
- idle RSS: **~11.6 MB**

### Stress harness

500 posts, 1,000 comment nodes, 100 images, 1,200 feed frames:

| Metric | Value |
|---|---:|
| Feed frame avg | 0.333 ms |
| Feed frame p95 | 0.422 ms |
| Feed first frame | 8.06 ms |
| Comments frame p95 | 0.257 ms |
| Collapse/expand avg | 1.246 ms |
| 100 images total | 140.5 ms |
| Peak RSS after images | 51.6 MB |

### Bookmark/read-history benchmark

`examples/bookmarks_bench.rs`:

| Dataset | Save | Load | Search |
|---|---:|---:|---:|
| 1,000 bookmarks | 39.2 ms | 14.1 ms | 0.22 ms |
| 10,000 bookmarks | 941.6 ms | 80.7 ms | 0.97 ms |
| 50,000 read-comment IDs | 10.8 ms save | 14.3 ms load | 34.1 ms mutation |

10,000-bookmark save is synchronous and noticeable (~0.9 s). It is documented
as a future scalability item; no speculative storage redesign was introduced.
Read history remains unbounded; no silent retention limit was added.

## Platform verification matrix

| Platform / area | Status |
|---|---|
| macOS ARM64 | Tested locally |
| Linux | CI configured; not executed in this sprint |
| Windows compile/tests | CI `windows-check` configured; **not executed** |
| Windows persistence/ACL | Not verified; experimental |
| Unix atomic rename | Tested |
| Windows MoveFileEx replace | Code-reviewed, not runtime-verified |
| Kitty/iTerm2/Sixel/halfblocks | Byte-level 4/4 |
| GUI pixel rendering | Not verified |
| NO_COLOR RGB output | Tested |
| tmux PTY | Tested |

## Image protocol verification matrix

| Protocol | Byte-level | Real terminal interaction | GUI pixels |
|---|---|---|---|
| Kitty | PASS | Not verified | Not verified |
| iTerm2 | PASS | Not verified | Not verified |
| Sixel | PASS | Not verified | Not verified |
| Halfblocks | PASS | PTY interaction/capture | Not visually inspected |

Byte probes are not visual proof. See `docs/MANUAL_TERMINAL_TEST_CHECKLIST.md`.

## Remaining limitations

- Windows runtime and crash-durability behavior is unverified.
- GUI image rendering is unverified.
- Journey A/B/C/D/F/H are partial; the missing steps are recorded above.
- 10,000-bookmark saves take ~0.9 s on the test machine.
- Read history is unbounded; no automatic retention policy.
- Config conflicts are rejected rather than merged; the user must reopen.
- Notification feedback replaces status-bar hints while a modal is open.
- Real tall-card PTY capture is missing; the render/layout path is tested.

## Release recommendation

- **Linux/macOS beta:** **BETA READY** for `v0.2.0-beta.3`.
- **Windows:** **NOT READY** (experimental; compile job configured but not run).
- **Stable candidate:** **NOT READY** until Windows runtime, GUI image rendering,
  and the remaining E2E journeys are verified.

No commit, tag, version bump or release was performed by this sprint.
