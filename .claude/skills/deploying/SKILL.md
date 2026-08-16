---
name: deploying
description: Use when deploying to the Hetzner server, editing deploy/ or .github/workflows/deploy.yml, touching nginx config, enabling drinking-game sounds on the server, or running the emergency manual server update.
---

# Deploying

**Server:** Hetzner cx23 (x86_64/amd64), Ubuntu — GitHub Actions runner must be `ubuntu-24.04` (not arm).

Deployment is automated via `.github/workflows/deploy.yml` — push to `master` builds on GitHub's x86_64 runner and deploys to the server. Manual command below is for emergency use only.

Deploy config is in `deploy/`:
- `portfolio.service` — systemd unit (runs as `portfolio` user, reads `.env`)
- `nginx.conf` — reverse proxy with rate limiting on `/api/auth/` (10 req/min, burst 5). **Not deployed by CI/CD** — must be manually copied to `/etc/nginx/sites-available/portfolio` and nginx reloaded. Use `127.0.0.1:3000` not `localhost:3000` (nginx resolves localhost to IPv6 `[::1]` but Axum only binds IPv4). Certbot manages SSL lines — always include them or HTTPS breaks. Also has two manual locations for the drinking game: `/drinks/room/*/sse` disables proxy buffering (SSE would never arrive otherwise), and `/drinks/login` has its own `zone=drinks_login` rate limit (30 req/min, burst 10) so a party's worth of guests behind one NAT IP registering at once don't hit raw 503s. All three `/drinks`-serving locations (the two above plus the catch-all `location /`) also set `proxy_set_header X-Forwarded-Proto $scheme;` — `request_origin()` (`drinkinggame/src/routes.rs`) reads it to build the absolute URL encoded into the room QR code; without it every scan would embed an `http://` link even though the site is HTTPS-only.

> **Always `diff -u` the live file against the repo file before copying, and never copy blind.**
>
> Measured 2026-08-16: the live config was still the 2026-07-28 file, missing
> everything added since — gzip, the `/static/` cache headers, SSL session cache,
> OCSP stapling, the `/drinks/room/*/sse` location, the `drinks_login` zone, and
> `X-Forwarded-Proto` on all three `/drinks` locations. So the drinking game has
> been running on nginx's default `proxy_buffering` in production, and every page
> has been served uncompressed.
>
> The same diff caught drift in the *other* direction, which is why the rule is
> "diff", not "copy": the live file carried `listen [::]:443 ssl http2;` and
> `listen [::]:80;` that `deploy/nginx.conf` did **not** have. `portfolio.dblo.net`
> has an AAAA record (`2a01:4f9:c013:e3c2::1`) alongside its A record and clients
> prefer IPv6, so copying the repo file as-is would have refused connections for
> most visitors — not degraded them, refused them. Those lines are in the repo
> file now. If a future diff shows the live config holding a directive the repo
> lacks, assume the repo is wrong until proven otherwise.
>
> Apply with a backup and an automatic restore, never a bare `cp`:
> `cp …/portfolio …/portfolio.bak-<date> && cp /tmp/new …/portfolio && { nginx -t && systemctl reload nginx; } || cp …/portfolio.bak-<date> …/portfolio`

Only `static/` (served from disk) and `.env` must be present alongside the binary — Askama templates are compiled in. `/opt/portfolio/src/` on the server is a stale old checkout unused by the deploy process.

On first deploy of the drinking game, add `DRINKS_DATABASE_URL=sqlite:///opt/portfolio/drinkinggame.db` to the server's `.env` — the relative-path fallback only works locally because `portfolio.service` sets `WorkingDirectory`.

The drinking game's fonts (woff2) are `include_bytes!`-compiled into the binary — nothing to copy to the server for those. Its sound effects are the opposite: no mp3s are committed to the repo (out of scope by design), so the game ships silent until mp3s are dropped in. To enable sound, create the directory named by `DRINKS_SOUNDS_DIR` (default `drinks-sounds`, relative to `portfolio.service`'s `WorkingDirectory`) on the server and drop in `drink.mp3`, `shot.mp3`, `card-draw.mp3`, `card-use.mp3`, `dice-roll.mp3`, `dice-give.mp3` — any other filename 404s. No restart needed; the route reads from disk per request.

Server update command:
```bash
cd /opt/portfolio/src && git pull && SQLX_OFFLINE=true cargo build --release && cp target/release/drawingportfolio /opt/portfolio/ && systemctl restart portfolio
```
