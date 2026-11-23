# UX review — real-terminal pass

Date: **2026-11-23**  
Method: the release binary was driven through a real PTY (tmux, 35+ captures) and
screens were inspected at 30×10, 60×20, 80×24, 100×30, 130×36 and 160×45. This
review distinguishes **observed** behaviour from **inferred** or **untested**
behaviour. Captures are in [`docs/verification/`](verification/).

Severity scale:

- **P0** — data loss, security issue, crash, or the app cannot be used at all.
- **P1** — a normal task is blocked or the UI actively misleads the user.
- **P2** — friction, density, discoverability or polish.

## Verdict

No P0 issues were found in the exercised paths. The issues that blocked normal
use (clipped quit hint, card content on borders, swallowed first key, unusable
help at tiny sizes) were fixed during this pass and now have regression tests.

## Fixed during this pass

| Severity | Issue | Evidence | Fix |
|---|---|---|---|
| P1 | At 80 columns the status bar clipped the line and dropped `q`/`?`, so quit was undiscoverable. | `80x24 status bar: missing 'Quit'` | Status hints are selected by measured width (full → medium → compact → micro). |
| P1 | Partially visible cards painted body/image content over their bottom border. | `╰─After using another OS for years, I moved my whole…` at 60×20 | Card content is clipped above the bottom border. |
| P1 | Under tmux, the image-capability query consumed the first real keypress. | `x` was not logged; later keys worked; `--no-images` avoided it. | Multiplexers skip the query and start with half-blocks; forced protocols still query. |
| P1 | Help/notification overlays left unrelated feed columns visible behind the modal. | 60×20 sort/help captures | The body is cleared behind a modal; notifications are confined to the feed pane. |
| P2 | Narrow feed wasted a row on a truncated description. | 60×20 feed capture | Description row is hidden below 96 columns or 12 rows. |
| P2 | `--demo` startup toast covered the first card. | startup capture | The redundant toast was removed. |
| P2 | At 30×10 help showed 4 of 22 shortcuts with no indication that the rest were hidden. | `help-30x10.txt` | Footer reports `18 more · Esc close`. |
| P2 | Below 52 columns the `● DEMO` badge disappeared, so demo content looked live. | 30×10 capture | The badge is kept whenever it fits; `? Help` is what yields space. |
| P2 | Post and Gallery status bars omitted the `q` quit hint that actually works. | detail/gallery captures | `q Quit` is present in full, medium, compact and micro variants where it fits. |
| P2 | At 80 columns the feed hid its loaded post count and “more” hint. | 80×24 capture | Title and right-aligned meta are measured; meta is shown when it fits. |

## Remaining findings

### P0

None observed.

### P1

1. **Live Reddit access is unavailable to new users.** This is a product-level
   blocker, not a rendering defect: public API requests closed on 31 October
   2026 and remaining access ends in March 2027. The app is honest about it and
   falls back to demo mode, but “read real Reddit” cannot be delivered without
   pre-existing approved credentials. See [`API_ACCESS.md`](API_ACCESS.md).
2. **Kitty/Sixel/iTerm2 pixel output is not visually verified.** Detection and
   encoded bytes were verified, but no GUI screenshot could be captured (macOS
   Screen Recording permission). On those terminals an image could still render
   incorrectly. Half-blocks are visually verified. Treat this as a known
   verification gap, not as proven support.

### P2

1. **30×10 is survivable but minimal.** One card is visible; help shows four
   shortcuts plus the “18 more” notice. There is no scrolling inside help.
2. **60×20 post detail is cramped.** Roughly five body lines are visible before
   scrolling; comments require switching focus. No layout bug, but it is a
   reading-only size.
3. **Toasts still overlap feed content.** They no longer cross panes and expire
   quickly, but a notification temporarily covers part of the selected card.
4. **Image fidelity under tmux is half-block only.** `Auto` chooses correctness
   and responsiveness over resolution. Forced graphics require configured
   passthrough and are not the default.
5. **Wheel scrolling advances one card at a time.** Intentional (variable card
   heights and clipped-safe rendering), but it feels coarse in long feeds.
6. **`morechildren` count drift.** Reddit may report more replies than it
   returns; the leftover count is not re-stubbed.
7. **Removed posts hide comments entirely.** Conservative and safe, but it can
   hide context that Reddit still exposes.
8. **No proactive 100-QPM bucket.** Rate limiting is driven by headers and 429s;
   a burst can still hit the limit once before backing off.
9. **No interactive OAuth browser flow or keychain storage.** Refresh tokens are
   supplied externally. This is moot for most users because new access is closed,
   but it remains a limitation for existing approved apps.
10. **Windows/Linux were not run in this environment.** Rendering is terminal-
    agnostic, but platform integration (clipboard, browser, terminal restore)
    is only verified on macOS.

## What looked good

- Feed density improved: 60×20 and 80×24 show two usable cards instead of one.
- Cards keep metadata, title, body excerpt and score/comments readable at 80
  columns; borders and scrollbar are intact.
- Comment tree rails, collapse markers and hidden-reply counts are legible at
  60 columns.
- Gallery position (`1 / 2`, `2 / 2`), controls and close behaviour are clear.
- Sort, range, goto and search modals are readable at 80+ columns and now sit on
  a cleared background.
- Demo/live state is visible even at 30 columns after the badge fix.
- Quit restores the terminal cleanly in every captured size (shell prompt and
  `EXIT:0` observed).
