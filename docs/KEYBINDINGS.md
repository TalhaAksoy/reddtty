# Keyboard and mouse reference

ReddTTY is keyboard-first. Every action below is a real binding in the
current build; if a key is not here and not in the in-app `?` help, it is not
wired up.

`Shift+Tab` is reported as BackTab by many terminals. In the two-pane post
detail it behaves as the reverse of `Tab` (both toggle the two panes).

## Global

| Key | Action |
|---|---|
| `Ctrl+C` | Quit immediately |
| `q` | Quit from any screen except a modal text field |
| `?` | Open keyboard help |
| `:` or `Ctrl+P` | Open the command palette |
| `S` | Open Settings |
| `T` | Cycle dark, light, Catppuccin, high-contrast and mono themes (persisted) |
| Terminal resize | Layout recalculates automatically; no restart required |

Mouse wheel, clicks and double-clicks are described at the end of this file.

## Feed screen

| Key | Action |
|---|---|
| `j` / `↓` | Select next post |
| `k` / `↑` | Select previous post |
| `PageDown` | Move down by roughly half a viewport of cards |
| `PageUp` | Move up by roughly half a viewport of cards |
| `Home` | First post |
| `End` | Last loaded post (also asks for the next page when appropriate) |
| `Enter` | Open the selected post |
| `Esc` | Go back one feed in the navigation stack |
| `s` | Open feed sort menu |
| `t` | Open the Top time-range menu and switch to Top on apply |
| `g` | Open “Go to subreddit” |
| `/` | Open search |
| `r` | Refresh the first page (keeps selection when possible) |
| `R` | Retry a failed “load more” request without refreshing |
| `o` | Open the post/link in the system browser |
| `v` | Open the gallery/image viewer |
| `x` | Reveal an NSFW or spoiler post for this session |
| `f` | Favorite the current/selected subreddit |
| `b` | Bookmark the selected post with a local offline snapshot |
| `B` | Open the local bookmarks list |
| `m` | Mark the selected post read/unread |
| `y` | Copy the selected post's permalink |
| `i` | Open the community-info drawer |
| `I` | Copy the selected post's image-preview URL |
| `M` / `A` / `D` | Quick-mute subreddit / author / domain |
| `u` | Undo the last quick mute |
| `Y` | Open the comments page in the browser |
| `G` | Open the selected/current subreddit in the browser |
| `h` | Show/hide the sidebar (persisted) |
| `Tab` / `Shift+Tab` | Switch focus between sidebar and feed |
| `←` | Focus the sidebar (shows it when the terminal is wide enough) |
| `q` | Quit |

### While the sidebar pane has focus

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move through sidebar sections |
| `Enter` | Open the selected Home/Popular/subreddit feed |
| `→` | Return focus to the feed |
| `Esc` | Return focus to the feed |
| `Tab` / `Shift+Tab` | Return focus to the feed |
| `f` | Add favorite, or ask before removing an existing favorite |
| `h` | Hide the sidebar |
| `G` | Open the selected subreddit in the browser |
| `s`, `t`, `g`, `/`, `r`, `o`, `v`, `x` | Same as feed actions |

## Post detail

The detail screen has two panes: **post body** and **comments**. `Tab` switches
between them. Comments are focused by default after opening a post.

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch post body ↔ comments |
| `←` | Focus the post body |
| `→` | Focus the comments |
| `j` / `↓` | Scroll body down, or select next comment |
| `k` / `↑` | Scroll body up, or select previous comment |
| `PageDown` / `PageUp` | Page the focused pane |
| `Home` / `End` | Start/end of the focused pane; in comments, first/last loaded comment |
| `Enter` (comments) | Collapse/expand a comment, or load a “more replies” node |
| `c` (comments) | Collapse/expand the selected comment |
| `[` / `]` | Previous / next top-level comment |
| `p` | Jump to the parent comment |
| `z` | Collapse all threads, or expand all when collapsed |
| `O` | Show only comments written by the original poster |
| `n` | Mark the current comment read and jump to the next unread one |
| `y` | Copy the selected comment permalink (post permalink from the body pane) |
| `i` | Open the community-info drawer |
| `I` | Copy the post's image-preview URL |
| `M` / `A` / `D` | Quick-mute subreddit / author / domain |
| `u` | Undo the last quick mute |
| `Y` | Open that permalink in the browser |
| `G` | Open this post's subreddit in the browser |
| `b` / `B` | Bookmark this post / open bookmarks |
| `m` | Toggle this post's read state |
| `Enter` (body) | Open the image/gallery viewer |
| `s` | Comment sort menu |
| `v` | Open the image/gallery viewer |
| `o` | Open the post/link in the browser |
| `x` | Reveal NSFW/spoiler content for this session |
| `r` | Refresh post and comments |
| `g` | Go to subreddit |
| `/` | Search |
| `f` | Favorite this post's subreddit |
| `h` | Toggle sidebar preference |
| `T` | Toggle theme |
| `Esc` | Back to the feed (scroll position preserved) |
| `q` | Quit |

Removed posts hide their comments with an explanatory placeholder.

## Image / gallery viewer

| Key | Action |
|---|---|
| `←` / `h`, `PageUp` | Previous image |
| `→` / `l`, `PageDown` | Next image |
| `Home` / `End` | First / last image |
| `+` / `=` | Zoom in (100% → 150% → 200% → 300% → 400%) |
| `-` | Zoom out |
| `Shift+←/→/↑/↓` | Pan a zoomed image |
| `0` | Reset zoom and pan |
| Click a thumbnail | Jump to that image |
| `Esc` | Close viewer (returns to feed or detail) |
| `o` | Open the current image externally |
| `y` / `I` | Copy the current image URL |
| `i` | Open the community-info drawer |
| `Y` | Open the post's comments page in the browser |
| `G` | Open the post's subreddit in the browser |
| `r` | Request a cleared redraw of the image |
| `T` | Toggle theme |
| `?` | Help |
| `q` | Quit |

## Sort menu

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move selection |
| `Enter` | Apply the selected sort, or open the Top range menu from the range row |
| `h` / `l`, `←` / `→` | Cycle the Top time range while the range row is selected |
| `Esc` | Close without applying |

Comment sort menu shows Best, Top, New, Controversial and Old. Feed sort menus
show only sorts supported by the active provider/feed, plus a “Top time range”
row.

## Top time-range menu

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move through Past hour, Today, This week, This month, This year, All time |
| `Enter` | Apply: switches the feed to Top with this window |
| `Esc` | Cancel |

## Go to subreddit / Search

| Key | Action |
|---|---|
| Character keys | Insert text (never interpreted as commands) |
| `Backspace` / `Delete` | Edit text |
| `←` / `→`, `Home` / `End` | Move the input cursor |
| `Ctrl+U` | Clear input |
| `Tab` | Search scope: current subreddit ↔ all Reddit (search only) |
| `Enter` | Open the subreddit / submit the search |
| `Esc` | Cancel |
| Paste | Insert sanitized pasted text |

Accepted subreddit forms: `rust`, `r/rust`, `/r/rust/`,
`https://www.reddit.com/r/rust`, `rust.json`. Names are normalised; invalid
names show an inline error and do not trigger a request.


## Settings

`S` opens the Settings modal.

| Key | Action |
|---|---|
| `j` / `k` | Move between Settings rows |
| `Enter` | Open Change config / Theme / Images / Appearance / Source health / Filters |
| `Esc` | Close Settings |
| `j` / `k` in Change config | Choose a bundled config preset |
| `Enter` in Change config | Install the selected config (existing config is backed up) |
| `Esc` in Change config | Return to Settings |
| `j` / `k` in Theme | Preview the highlighted theme live |
| `Enter` in Theme | Apply and persist the previewed theme |
| `Esc` in Theme | Revert to the theme that was active when the list opened |
| `j` / `k` in Images | Move through Auto / Kitty / iTerm2 / Sixel / Halfblocks / Disabled |
| `Enter` in Images | Apply the image protocol immediately |
| `j` / `k` in Appearance | Move between timestamp and motion rows |
| `Enter` in Appearance | Toggle relative/absolute time or reduced/full motion |

Changing the config replaces only the provider settings; favorites, recents,
theme, image, bookmarks, filters and UI preferences are preserved. The candidate
provider is probed first; only a successful probe replaces the active source and
resets navigation safely. A failed probe keeps the previous source and config.
Theme changes apply immediately and are persisted.

`--no-color` (or the standard `NO_COLOR` environment variable) forces the
strict terminal-default colours (and disables image colours) for the session
without overwriting the saved theme.

## Bookmarks and read state

`b` saves the selected post together with an offline snapshot in
`library.toml` next to the config. `B` opens the saved list.

| Key | Action |
|---|---|
| `j` / `k` | Move through saved posts |
| `Enter` | Open the selected saved post (works from the local snapshot if the source is down) |
| `d` / `Delete` | Remove the selected bookmark (confirmation required) |
| `/` | Search title, author and subreddit |
| `Home` / `End`, `PgUp` / `PgDn` | Jump / page through the list |
| `Esc` | Close the list |

Offline snapshots store the post, not its comments. The list labels recorded
comment counts as “not stored offline” instead of pretending comments exist.

`m` toggles read/unread. Unread cards show `● unread`; saved cards show
`★ saved`. Bookmarking does not perform a Reddit write.

## Clipboard and browser links

| Action | Feed | Post detail | Image viewer |
|---|---|---|---|
| Copy post/comment permalink | `y` | `y` | — |
| Copy current image URL | `I` | `I` (post image preview) | `y` / `I` |
| Open comments/permalink | `Y` | `Y` | `Y` |
| Open current subreddit | `G` | `G` | `G` |
| Open post/image externally | `o` | `o` | `o` |

Clipboard support uses `pbcopy` on macOS, `clip` on Windows, and
`wl-copy`/`xclip`/`xsel` on Linux. Demo image URLs are local fixtures, so
ReddTTY explains that there is no external URL to copy instead of claiming one.

## Info drawer

`i` opens a community drawer that works on narrow terminals where the wide
right panel is hidden. It shows the community name, title, member counts,
description, post details, suggested communities and actions.

| Key | Action |
|---|---|
| `i` (feed/post/gallery) | Open the info drawer |
| `j` / `k`, `PgUp` / `PgDn` | Scroll the drawer |
| `g` / `G` / `f` / `r` / `Y` / `b` | Run the advertised action immediately |
| `Esc` / `i` | Close |
| Click outside | Close |

## Local filters

Open **Settings → Filters**. Filters are local and provider-independent.

| Key | Action |
|---|---|
| `j` / `k` | Move between rules |
| `Enter` | Toggle “Hide read posts” on the first row |
| `a` | Add a filter expression |
| `d` / `Delete` | Remove the selected rule |
| `C` | Clear all filters |
| `Esc` | Back to Settings |

Quick mute from a selected post:

| Key | Action |
|---|---|
| `M` | Mute the post's subreddit |
| `A` | Mute the post's author |
| `D` | Mute the post's external link domain (self/text/gallery posts explain why this is unavailable) |
| `u` | Undo the last quick mute |

Accepted expressions:

```text
keyword:ai          block posts whose title/body contains “ai”
sub:politics        mute r/politics locally
author:someone      mute a username
domain:example.com  mute links to a domain
bare-text           keyword filter
```

Filter changes persist immediately, prune the visible feed, and refetch it.
When a page is fully hidden, ReddTTY keeps paginating instead of getting stuck.

## Source health

Open **Settings → Source health**.

| Key | Action |
|---|---|
| `j` / `k` | Move between actions |
| `Enter` or `r` | Test now: run an independent bounded probe (does not replace the feed) |
| `Enter` or `c` on Change source | Open the config picker |
| `Esc` | Back to Settings |

The screen shows the active provider/host, demo vs live mode, last status,
last success/attempt, last and average latency, request/success rate, last error,
bookmark/filter counts and the session cache policy.

## Command palette

Open with `:` or `Ctrl+P`.

| Key | Action |
|---|---|
| Character keys | Filter commands by label or keyword |
| `↑` / `↓`, `Tab` | Move selection |
| `Backspace` | Delete the last query character |
| `Ctrl+U` | Clear the query |
| `Enter` | Run the selected command or text command |
| `Esc` | Close |

Text commands:

```text
sort new
theme mocha
theme high-contrast
theme mono
source
bookmarks
filter add keyword:ai
filter clear
mute sub politics
mute author someone
mute domain example.com
unmute sub politics
images halfblocks
open comments
copy url
info
settings
refresh
sidebar
quit
```

## Favorite removal confirmation

Pressing `f` on an existing favorite opens a confirmation instead of removing it
immediately.

| Key | Action |
|---|---|
| `Enter` / `y` | Remove from favorites |
| `Esc` / `n` | Cancel and keep the favorite |
| Click outside the modal | Cancel |

Adding a new favorite is still immediate.

## Help modal

The help screen is a categorised reference rather than a flat list. The current
screen's section is shown first. Sections are: `GLOBAL`, `FEED`, `SIDEBAR`,
`POST DETAIL`, `IMAGE VIEWER`, `CLIPBOARD & LINKS`, `SORT & RANGE`,
`TEXT INPUT`, `BOOKMARKS`, `LOCAL FILTERS`, `SOURCE HEALTH`,
`COMMAND PALETTE` and `MOUSE`.

| Key | Action |
|---|---|
| `j` / `k`, `↓` / `↑` | Scroll one row |
| `PageDown` / `PageUp` | Scroll one page |
| `Home` / `End` | First / last row |
| Mouse wheel | Scroll the reference |
| `/` | Search shortcuts |
| `Esc` | Clear search, then close |
| `?` | Close when not searching |
| Click outside | Close |

At wide terminals entries are shown in two columns; the footer reports the
visible range and how many rows remain. Narrow terminals use one column and a
scrollable list.

## Mouse

Mouse support is enabled by default (`ui.mouse_enabled = true`).

| Gesture | Action |
|---|---|
| Click a post card | Select it and focus the feed |
| Double-click a post card | Open the post (within 450 ms on the same card) |
| Click a sidebar entry | Open that feed |
| Click a sort tab | Apply that sort |
| Click a comment | Select it and focus comments |
| Double-click a comment | Collapse/expand it, or load a “more replies” node |
| Wheel over feed/sidebar | Move selection one card/entry at a time |
| Wheel over comments | Scroll the comment list |
| Wheel over post body | Scroll the post body |
| Wheel over gallery | Previous/next image |
| Click gallery arrows | Previous/next image |
| Click outside a modal | Close it |
| Click a sort/range item | Select and apply it |
| Click a bookmark | Open that saved post |
| Click a filter row | Select/toggle that local rule |
| Click a command | Run that command |

Mouse support supplements the keyboard; it never replaces it.
