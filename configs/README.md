# Ready-to-use configs

## `demo.toml` — offline

No network and no Reddit account. Safe first-run default when you want to try
the interface immediately.

## `redlib-artemislena.toml` — recommended on 2026-11-24

Uses the third-party Redlib instance `https://red.artemislena.eu`.

A full manual TUI smoke test passed on 2026-11-24:

- Home feed loaded (25 posts, more available)
- New and Rising returned listings
- Top · All time returned posts
- `r/burdurland` opened
- Post detail and comments loaded
- Pagination and post links worked

## `redlib-utilibre.toml` — recommended on 2026-11-24

Uses the third-party Redlib instance `https://redlib.utilibre.org`.

A full manual TUI smoke test passed on 2026-11-24:

- Home feed, New, Rising and Top · All time loaded
- `r/burdurland` opened
- Post detail and comments loaded
- Post links and sort routes worked

The instance is protected by Anubis, so availability can vary.

## `redlib-zanity.toml` — recommended on 2026-11-23

Uses the third-party Redlib instance `https://redlib.zanity.net`.

Manual smoke test on this date passed:

- Home feed loaded (25 posts)
- New and Rising returned different listings
- Top · All time returned posts
- `r/burdurland` opened
- Post detail loaded
- Comments and nested replies loaded
- Post links pointed back to the same Redlib instance

This is still a third-party instance and can change or block requests at any
time. No instance is endorsed or guaranteed.

## `redlib-hbubli.toml` — recommended on 2026-11-23

Uses the third-party Redlib instance `https://redlib.hbubli.cc`.

A full manual TUI smoke test passed:

- Home feed loaded (25 posts)
- New and Rising returned different listings
- Top · All time returned posts
- `r/burdurland` opened
- Post detail loaded
- Comments and nested replies loaded
- Post links pointed back to the same Redlib instance

This is still a third-party instance and can change or block requests at any
time. No instance is endorsed or guaranteed.

## `safereddit.toml` — alternative

Uses the third-party Redlib instance `https://safereddit.com`.

During the same check it loaded the home feed and some comment pages, but New,
Rising and some comment pages returned Reddit access/rate-limit errors. Keep it
only as an alternative.

## Install with make

List and choose interactively:

```bash
make configs
```

Install a specific config without prompting:

```bash
make configs CONFIG=redlib-zanity.toml
make configs CONFIG=2
```

Only list the choices:

```bash
make configs-list
```

The target creates the destination directory, backs up an existing
`config.toml` as `config.toml.bckp_<unixtimestamp>`, and installs with mode
`0600`.

## Try a config without installing

From the repository:

```bash
cargo run --release -- --config configs/redlib-zanity.toml
```

Or with an already-built binary:

```bash
./target/release/reddtty --config configs/redlib-zanity.toml
```

## Install a config as your normal config

The default config path depends on the platform:

| Platform | Path |
|---|---|
| macOS | `${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml` |
| Linux | `${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml` |
| Windows | `%APPDATA%\reddtty\config.toml` |

macOS and Linux (replace the source file if you prefer `safereddit.toml`):

```bash
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/reddtty"
cp configs/redlib-zanity.toml "${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml"
chmod 600 "${XDG_CONFIG_HOME:-$HOME/.config}/reddtty/config.toml"
```

On macOS this is normally `~/.config/reddtty/config.toml`; it deliberately
does **not** use `~/Library/Application Support` anymore. Older `threadline`
configs (including the old macOS Application Support location) are migrated
automatically on first run if no `reddtty` config exists.

Windows PowerShell:

```powershell
New-Item -ItemType Directory -Force "$env:APPDATA\reddtty"
Copy-Item configs\redlib-zanity.toml "$env:APPDATA\reddtty\config.toml"
```

Then run:

```bash
reddtty
```

## Instances checked

These were researched from the public Redlib instance list and were not added
because they did not pass the full feed/sort/comment test:

| Instance | Result |
|---|---|
| `redlib.catsarch.com` | 429 Too Many Requests |
| `redlib-1.privadency.com` | Bot/CAPTCHA page |
| `redlib.nadeko.net` | 418 Bot check |
| `redlib.privadency.com` | Bot/CAPTCHA page |
| `redlib.reallyaweso.me` | Redirects to `redlib.blueberry.reallyaweso.me`; Hot feed is partial, New/Rising return access-denied |
| `reddit.nerdvpn.de` | HTTP/2 protocol error / empty reply |
| `redlib.tiekoetter.com` | Home loaded, but New/Rising/Top returned empty responses |

Additional candidates from the current public list were checked on 2026-11-24.
Most returned DNS/connectivity failures, Cloudflare/Anubis challenges, HTTP 5xx,
or a placeholder page. `red.artemislena.eu` passed the full test and was
added as `redlib-artemislena.toml`; `redlib.utilibre.org` also passed and was
added as `redlib-utilibre.toml`.

## Important limitations

- These configs send requests to third-party Redlib instances. The instance can
  see your request paths, headers and IP address.
- Redlib is not Reddit's official Data API, and these configs do not grant any
  Reddit access rights. Check the instance's terms and Reddit's access rules.
- An instance may rate-limit or block a listing. ReddTTY shows the error and
  does not switch instances, scrape around the block, or fall back to demo data
  silently.
- `default_sort = "hot"` is intentional: Redlib HTML has no "best" listing.
- Reddit OAuth is a separate provider and requires pre-approved credentials.
