# osu! Map Manager Worker

Cloudflare Worker backend for repair downloads. The desktop app calls this Worker endpoint:

```text
https://YOUR-WORKER.workers.dev/beatmapsets/{beatmapset_id}/download
```

## Setup

Deploy the Worker:

```powershell
cd backend/cloudflare-worker
npm install
npx wrangler deploy
```

Put the deployed Worker URL into the desktop app's `Backend URL` field, for example:

```text
https://osu-map-manager.YOUR_SUBDOMAIN.workers.dev
```

## OAuth credentials

Create an OAuth application at <https://osu.ppy.sh/home/account/edit#oauth> with
redirect URI `http://127.0.0.1:3000/callback`, then store its credentials on the
Worker (never in the app):

```powershell
npx wrangler secret put OSU_CLIENT_ID
npx wrangler secret put OSU_CLIENT_SECRET
npx wrangler deploy
```

`GET /oauth/check` verifies the stored credentials live against osu!; the
desktop app calls it before opening the browser so a bad secret fails fast.

## Troubleshooting

`invalid_client` / `Client authentication failed` means osu! rejected the
request. Where it happens matters:

- **In the browser on `/oauth/authorize` (HTTP 401):** osu! does not accept the
  `client_id` + `redirect_uri` combination. Either the OAuth application was
  deleted, or — more commonly — its registered callback URL does not exactly
  match what the app sends. Open the app at
  <https://osu.ppy.sh/home/account/edit#oauth> and set its Application Callback
  URL to exactly:
  ```text
  http://127.0.0.1:3000/callback
  ```
  No trailing slash, no extra path, `http` (not `https`), port `3000`.
- **At the token exchange (`/oauth/token`):** the stored
  `OSU_CLIENT_ID`/`OSU_CLIENT_SECRET` pair is wrong. Common causes:

- The values were swapped, truncated, or pasted with extra characters.
- The secret was regenerated on osu! after being stored on the Worker.
- The secrets were set on a different Worker/environment than the deployed one.

Fix: re-enter both secrets with the commands above (paste the numeric client ID
and the exact client secret, no quotes), redeploy, then sign in again.

## Security model

Do not put private credentials into the desktop app. Any value embedded in a
distributed EXE can be extracted.

The desktop app signs the user in with osu! OAuth through this Worker
(`GET /oauth/authorize`, `POST /oauth/token`, `POST /oauth/refresh`); the
Worker holds `OSU_CLIENT_ID`/`OSU_CLIENT_SECRET` and the browser-facing app
only ever sees short-lived user tokens.

Sign-in uses PKCE (RFC 7636): the app sends `code_challenge`/`code_challenge_method=S256`
to `GET /oauth/authorize` and `code_verifier` to `POST /oauth/token`. Requests
without them are rejected, so an intercepted authorization code is useless on
its own.

Metadata routes (`GET /beatmapsets/:id`, `GET /beatmaps/:id`) prefer the
caller's own osu! token when an `Authorization: Bearer` header is present and
fall back to the Worker's app token otherwise. The Worker sends no CORS
headers on purpose: the only client is the desktop app, so browsers are denied
by default.

The download route prefers the official osu! API when the app sends the
user's token:

```text
GET /beatmapsets/{beatmapset_id}/download
Authorization: Bearer <osu! user token>
```

Without a token (or when the official download fails), the Worker falls back
to the configured mirror base URL:

```text
BEATMAP_MIRROR_DOWNLOAD_BASE_URL = "https://catboy.best/d"
```

For public distribution, add Cloudflare rate limiting or WAF rules on:

```text
/beatmapsets/*/download
```

The Worker also caches the osu! access token in memory until close to expiry to avoid
requesting a new token per download.
