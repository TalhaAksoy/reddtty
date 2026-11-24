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
| `S` | Open Settings |
| `T` | Cycle dark, light and Catppuccin themes (persisted) |
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
| `o` | Open the post/link in the system browser |
| `v` | Open the gallery/image viewer |
| `x` | Reveal an NSFW or spoiler post for this session |
| `f` | Favorite the current/selected subreddit |
| `h` | Show/hide the sidebar (persisted) |
| `Tab` / `Shift+Tab` | Switch focus between sidebar and feed |
| `q` | Quit |

### While the sidebar pane has focus

| Key | Action |
|---|---|
| `j` / `↓`, `k` / `↑` | Move through sidebar sections |
| `Enter` | Open the selected Home/Popular/subreddit feed |
| `Esc` | Return focus to the feed |
| `Tab` / `Shift+Tab` | Return focus to the feed |
| `f` | Add favorite, or ask before removing an existing favorite |
| `h` | Hide the sidebar |
| `s`, `t`, `g`, `/`, `r`, `o`, `v`, `x` | Same as feed actions |

## Post detail

The detail screen has two panes: **post body** and **comments**. `Tab` switches
between them. Comments are focused by default after opening a post.

| Key | Action |
|---|---|
| `Tab` / `Shift+Tab` | Switch post body ↔ comments |
| `j` / `↓` | Scroll body down, or select next comment |
| `k` / `↑` | Scroll body up, or select previous comment |
| `PageDown` / `PageUp` | Page the focused pane |
| `Home` / `End` | Start/end of the focused pane; reaching the end loads more posts |
| `Enter` (comments) | Collapse/expand a comment, or load a “more replies” node |
| `c` (comments) | Collapse/expand the selected comment |
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
| `←` / `h` | Previous image |
| `→` / `l` | Next image |
| `Esc` | Close viewer (returns to feed or detail) |
| `o` | Open the current image externally |
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
| `Enter` | Open Change config / choose Theme |
| `Esc` | Close Settings |
| `j` / `k` in Change config | Choose a bundled config preset |
| `Enter` in Change config | Install the selected config (existing config is backed up) |
| `Esc` in Change config | Return to Settings |
| `j` / `k` in Theme | Preview the highlighted theme live |
| `Enter` in Theme | Apply and persist the previewed theme |
| `Esc` in Theme | Revert to the theme that was active when the list opened |

Changing the config replaces only the provider settings; favorites, recents,
theme, image and UI preferences are preserved. Provider changes take effect after
restarting ReddTTY. Theme changes apply immediately and are persisted.

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

| Key | Action |
|---|---|
| `Esc` or `?` | Close |
| Click outside | Close |

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

Mouse support supplements the keyboard; it never replaces it.
