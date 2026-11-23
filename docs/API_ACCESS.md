# Reddit API access: what ReddTTY does and does not do

Last reviewed: **2026-11-23**. Reddit's access rules change; re-check the official
pages before shipping or using live mode.

Official/primary sources:

- Reddit migration announcement (30 Sep 2026) — <https://www.reddit.com/r/redditdev/comments/1wubcvf/moving_data_api_apps_to_the_developer_platform/>
- Official app registration page — <https://developers.reddit.com/app-registration>
- Responsible Builder Policy — <https://support.reddithelp.com/hc/en-us/articles/42728983564564-Responsible-Builder-Policy>
- Reddit Data API Wiki — <https://support.reddithelp.com/hc/en-us/articles/16160319875092-Reddit-Data-API-Wiki>
- Reddit developer portal — <https://developers.reddit.com/>
- API reference — <https://www.reddit.com/dev/api/>
- OAuth documentation — <https://github.com/reddit-archive/reddit/wiki/OAuth2>

Secondary reporting used to corroborate the announcement dates:

- TechRepublic, “Reddit Is Killing RSS and Public API Access” (2 Oct 2026) —
  <https://www.techrepublic.com/article/news-reddit-rss-public-api-shutdown/>

### Source verification status

On **2026-11-23**:

- The Responsible Builder Policy and the official app-registration page were
  fetched successfully.
- The app-registration page explicitly states that removal begins **12 January
  2027** and remaining public API access ends in **March 2027**.
- The Reddit announcement post could not be fetched by the automated browser
  (403/empty responses). Its 31 October date and full timeline are corroborated
  by Reddit search snippets, the official registration page for the later dates,
  and the independent reporting above. Treat the October date as announced but
  re-check the Reddit post directly before relying on it.
- This document is not legal advice.

## Critical 2026–2027 transition

Reddit announced a staged sunset of the **public Data API** and migration toward
its hosted Developer Platform (Devvit):

| Date | Change |
|---|---|
| **31 Oct 2026** | New requests for public Data API access stopped being accepted. This has already passed. |
| 13 Nov 2026 | Reddit RSS support ended. RSS is not a fallback and must not be used. |
| 30 Nov 2026 | Registration closes for the migration bounty program. |
| **12 Jan 2027** | Reddit begins removing API access for apps/users that have not registered or engaged with the Developer Platform team. |
| 23 Feb 2027 | Further migration update/reminder. |
| **March 2027** | Remaining public API access ends for all remaining apps and users. |

The replacement path is Reddit's **Developer Platform (Devvit)**, where apps run
inside Reddit's clients. Devvit is not a drop-in replacement for a standalone
terminal application such as ReddTTY. A separate written/commercial agreement
would be the only possible official path for a standalone client after the
public API closes; none is claimed here.

### What this means for ReddTTY

- As of the review date, there is **no legitimate way to obtain new public Data
  API access**. Creating a new app now cannot restore public API access.
- The RedditProvider is therefore **not viable as a new long-term integration**.
  It can only be used with credentials from an already approved app that is still
  active, and only until Reddit's cutoff. Access after March 2027 is not
  guaranteed.
- Existing approved users should follow Reddit's registration/migration notices
  directly (registration by the dates above, and Devvit migration if their use
  case fits). ReddTTY cannot perform or bypass that process.
- DemoProvider remains the fully supported, offline way to use the application.
  It is clearly labelled `DEMO` and is the recommended default.

### Handling the access barrier without violating policy

Acceptable options:

1. **Use Demo Mode** for development, UI evaluation and offline reading of
   synthetic data.
2. If you already have an approved app and valid credentials, use them only
   until Reddit's announced sunset, monitor Reddit's migration notices and
   compliance requirements, and do not attempt to extend them beyond the
   approved scope.
3. Migrate the *use case* to the official Developer Platform if Reddit's hosted
   app model can express it. It cannot host a general-purpose desktop terminal
   client as-is, so this is a product decision, not a code patch.
4. Obtain a separate written agreement from Reddit if you qualify. This project
   has not done so and cannot imply that access will be granted.

Unacceptable options (not implemented, by design):

- scraping HTML/JSON endpoints or old Reddit;
- reusing browser cookies or session tokens;
- rotating accounts/client IDs to evade limits or bans;
- bypassing approval, rate limits or access controls;
- treating RSS as a replacement (it is being retired);
- pretending demo data is live content.

## Summary of the current access rules

- **OAuth is required.** Reddit states that clients must authenticate with a
  registered OAuth token. Traffic without OAuth/login credentials can be throttled
  or blocked; unauthenticated access is not a supported fallback.
- **Approval was required and the new-access window is closed.** Under the
  Responsible Builder Policy, Reddit required approval before Data API access.
  Reddit's 30 September 2026 migration announcement closed new public API
  requests on 31 October 2026; remaining public access is scheduled to end in
  March 2027. Creating an account or app now does not restore public API access.
- **Free approved access is rate limited**, currently documented as **100 queries
  per minute per OAuth client id**, enforced as an average over a rolling
  ten-minute window. Commercial or higher-volume use may require a separate
  agreement.
- **Respect response headers.** Reddit exposes `x-ratelimit-used`,
  `x-ratelimit-remaining` and `x-ratelimit-reset`, and may return `429` with
  `Retry-After`.
- **The OAuth provider does no scraping, credential theft or impersonation.** It
  does not use browser cookies, hidden endpoints, fake User-Agents or account
  impersonation. The optional HTML providers (below) are clearly separate and
  are not presented as the Data API.
- **A descriptive User-Agent is expected.** Set a unique value identifying the
  application and a contact. Reddit may block generic or misleading agents.

## HTML providers are not the Data API

ReddTTY also contains optional, read-only HTML providers:

- **OldRedditProvider** parses `old.reddit.com` listing, post and comment pages.
- **RedlibProvider** parses a **user-selected** Redlib instance. Redlib is an
  independent third-party front end, not an official Reddit service or API.

This is a different mechanism from OAuth. It is not a workaround for the API
shutdown, and it does not grant any right to access Reddit.

Rules enforced by the implementation:

- **No automatic live access.** Network requests are disabled unless
  `provider.html.allow_network = true` is explicitly set. Selecting
  `--provider old-reddit` or `--provider redlib` without that opt-in produces a
  visible error, not a silent demo fallback.
- **No credential impersonation.** The User-Agent identifies ReddTTY; there is
  no fake Firefox/mobile agent, no OAuth token imitation, no browser-cookie
  extraction, no proxy rotation and no attempt to defeat access controls.
- **Stop on gates.** HTTP 401/403, login walls, CAPTCHAs, explicit block pages,
  rate-limit pages and unexpected content types are surfaced as errors. They are
  never treated as empty feeds.
- **No Redlib instance rotation.** The instance must be configured by the user.
  ReddTTY never tries a list of random instances after a failure.
- **Third-party exposure.** A Redlib instance can see request paths, headers and
  your IP address. Its operator's terms and privacy policy apply.
- **No broad live success claim.** The parsers are verified against synthetic
  HTML fixtures and a local mock server. A manual live Old Reddit attempt on
  2026-11-23 hit Reddit's CAPTCHA/access page. One manual Redlib smoke test
  against working third-party instances loaded feeds, sorts, post details and
  nested comments, but that is not an endorsement of any instance, not a
  stability guarantee, and not a claim of supported live access. HTML layouts can change without notice.

Old Reddit is Reddit-owned, so automated access remains subject to Reddit's User
Agreement, robots policy and any technical restrictions. `allow_network = true`
is an acknowledgement that you have checked those rules for your use; it is not
legal permission. If Reddit or an instance operator blocks the request, the
correct behaviour is to stop and use demo mode or an approved source.

### Enabling an HTML provider

Only after confirming the source's terms and permissions:

```toml
[provider]
type = "old-reddit"          # or "redlib"

[provider.html]
allow_network = true
redlib_instance = "https://your-redlib-instance.example"  # redlib only
```

Or for one session:

```bash
reddtty --provider old-reddit
reddtty --provider redlib --redlib-instance https://your-redlib-instance.example
```

Redlib instance URLs are validated: HTTP(S), a host, no embedded credentials,
no query/fragment, and HTTPS for anything except loopback. A failed instance is
reported, not replaced.

## What the RedditProvider implements

Endpoints (all `GET`, all on `https://oauth.reddit.com` unless noted):

| Purpose | Endpoint |
|---|---|
| Home listings | `/best`, `/hot`, `/new`, `/top`, `/rising`, `/controversial` |
| Subreddit listings | `/r/{subreddit}/{sort}` |
| Popular | `/r/popular/{sort}` |
| Search all | `/search?q=…&type=link` |
| Search subreddit | `/r/{subreddit}/search?q=…&restrict_sr=1&type=link` |
| Post + comments | `/comments/{id}` |
| More comments | `/api/morechildren` |
| About community | `/r/{subreddit}/about` |
| Token | `https://www.reddit.com/api/v1/access_token` (POST, Basic auth) |

Every read request adds `raw_json=1` so links and bodies are not HTML-escaped.
Listing requests use Reddit's opaque `after` cursor and `limit`; ReddTTY never
assumes numeric pages.

### Authentication

Supported grants:

1. **`client_credentials`** (app-only, public read data), when a client id and
   secret are configured and the app is approved.
2. **`refresh_token`** (user-context read access), when a refresh token is
   supplied. The requested scope in ReddTTY is `read`.

Security properties:

- Client id/secret are sent only to Reddit's token endpoint using HTTP Basic auth.
- Access tokens are cached **in memory** only; they are never written to disk or
  logged.
- Refresh tokens/client secrets are read from environment variables (preferred) or
  the config file. Environment variables override config values.
- An access token is refreshed once after a `401`, then the request fails with an
  explicit authentication error. There is no infinite retry loop.
- ReddTTY does not implement the interactive authorization-code browser flow
  yet. Live user-context use requires supplying a refresh token obtained through
  Reddit's documented OAuth flow.

### Rate limiting

- Before a request, ReddTTY checks whether a previous `429` or exhausted
  window put it into a cooldown.
- `x-ratelimit-remaining == 0` sets a cooldown until `x-ratelimit-reset`.
- `429` uses `Retry-After` (or `x-ratelimit-reset`) and reports the wait to the
  UI.
- 5xx/network errors do not trigger automatic retry storms; the user can refresh.
  This is deliberate and conservative.
- There is no separate proactive token bucket in this version; the server headers
  are authoritative.

### Error mapping shown to the user

| Condition | User-facing category |
|---|---|
| 401 / missing or rejected token | Authentication required (includes approval hint) |
| 403 | Access denied / approval or permission problem |
| 404 | Content not found |
| 429 | Rate limited, with retry seconds when available |
| 5xx | Reddit server error |
| Timeout | Timed out |
| Network/connect | Network problem |
| Invalid JSON | Invalid response |
| Unsupported operation | Not supported |

The UI renders these as an error state (empty feed) or a toast while retaining
available data, and never claims a live connection that does not exist.

## Enabling live mode (only for already-approved existing access)

1. Review the Responsible Builder Policy, the migration announcement and the
   app-registration page above.
2. Confirm that you already have a valid, approved app and any required refresh
   token from before the 31 October 2026 cutoff. **Do not create a new app and
   assume it grants public Data API access.**
3. If you are an existing app owner, follow Reddit's direct migration notices;
   registration for the migration programme closes 30 November 2026, and access
   removal begins 12 January 2027.
4. Configure existing credentials:
   ```bash
   export REDDTTY_REDDIT_CLIENT_ID="existing-approved-client-id"
   export REDDTTY_REDDIT_CLIENT_SECRET="existing-approved-client-secret"
   export REDDTTY_REDDIT_REFRESH_TOKEN="optional-existing-refresh-token"
   export REDDTTY_REDDIT_USER_AGENT="reddtty/0.1.0 (terminal reader; contact: you@example.com)"
   ```
5. Run `reddtty --diagnostics` and confirm credentials are reported as
   configured (values are never printed), provider mode is live, and the image
   protocol is the expected one.

Legacy `THREADLINE_*` environment variable names are accepted only as a
fallback when the corresponding `REDDTTY_*` variable is absent. The new name
always wins. Legacy user-agent strings are normalized to the ReddTTY name.
6. Run `reddtty`.
7. If Reddit denies access or the endpoint has been retired, read the error.
   ReddTTY does not attempt to work around access controls or restore access
   through unofficial means.

You can also place credentials in the config file under `[reddit]`, but the file
is plain text. Environment variables are safer. The config file is written with
`0600` permissions on Unix when saved.

## Data handling

- Feed/post/comment data lives in memory and is replaced when refreshed. There is
  no persistent content cache.
- Encoded images live in a bounded in-memory LRU cache and are discarded on exit.
- No analytics or telemetry are sent. If you explicitly enable an HTML provider,
  requests go to Old Reddit or to your chosen Redlib instance; no other
  third-party service is contacted.
- Logs contain provider errors and image URLs but never tokens or secrets. URLs
  are sanitised before logging.
- Removing or deleted posts are rendered as placeholders; ReddTTY does not
  retain inaccessible Reddit content in a disk cache.

## Honest status

The live provider is implemented against the documented API, unit-tested for JSON
mapping, and integration-tested against a local mock OAuth/HTTP server (including
401/403/404/429/5xx, invalid JSON, token refresh, request timeout and OAuth form
fields). It has **not** been exercised against the real Reddit service in this
repository because no approved OAuth credentials were available. Do not treat the
existence of this provider as proof that Reddit will grant or retain access.

**Long-term verdict:** the RedditProvider is legacy/sunsetting, not a viable
future integration for a new standalone client. The public Data API's remaining
access is scheduled to end in March 2027, new access requests have closed, and
Devvit does not provide a drop-in standalone terminal API. The supported durable
mode is Demo Mode; any future live integration would require a written agreement
or an officially supported successor API, neither of which this project has.

The HTML providers do not change that verdict. They are opt-in parsers for pages
the user is already able to request, not an approved API. Manual third-party Redlib smoke tests worked on some instances, but these are
single observations, not a supported replacement for Reddit data access. Old Reddit HTML was blocked in the manual
attempt. The supported durable mode remains Demo Mode.
