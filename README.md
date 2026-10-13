# Optimarr

Optimarr scans your movie and series folders and tells you, for every file, how likely it is to
direct-play on your clients and how good the encode is. It links files to Sonarr and Radarr so
you can request a better release, reads Jellyfin's playback history to show what actually got
transcoded, and gives you a one-line ffmpeg command for files that a lossless remux would fix.

Optimarr 2 is a rewrite in Rust: a single small binary with the web UI built in, a SQLite
database, and live progress in the browser.

## Features

- **Two scores per file.** Compatibility (0–100: will it direct-play?) and quality (0–100: is the
  bitrate, codec, resolution and audio good enough?), each with a verdict and a list of issues.
- **Fixes.** Every issue says how to fix it; fixable files get a ready-to-run ffmpeg command.
- **Fast, incremental scans.** Unchanged files are skipped, folders are watched for new files, and
  progress is shown live.
- **Sonarr and Radarr.** Sync root folders as libraries, link files to series and movies, and
  request a replacement for a bad file. Import webhooks trigger a rescan right away.
- **Jellyfin.** Imports playback history (Playback Reporting plugin) and shows which files are
  transcoded, on which clients, and why.
- **Device profiles.** Describe your TVs, phones and browsers, and see per file and per device whether
  it direct-plays, gets remuxed or gets transcoded, and why.
- **Fix queue.** Run the suggested ffmpeg fix for one file or hundreds; the result replaces the
  original (kept as `.orig` by default) and Sonarr/Radarr are told to rescan. Off by default.
- **Upgrade finder.** Ask Sonarr/Radarr to look for a better release of low-quality files without
  deleting them, by hand or on a schedule.
- **Space savings.** See how much re-encoding to HEVC or AV1 would save, per file and in total.
- **Language checks.** Flag files missing your audio or subtitle languages, or with image-only
  subtitles that force a transcode.
- **Notifications.** Discord, ntfy or a webhook when new files arrive broken or poor, or when a fix
  finishes.
- **Agent API.** A small, stable JSON API for AI agents and scripts, see [docs/API.md](docs/API.md).
- **Upgrade from 1.x.** Settings and the analysis database of Optimarr 1.x are imported on first start.

## Install with Docker

```yaml
services:
  optimarr:
    image: ghcr.io/davidsteg/optimarr:latest
    container_name: optimarr
    ports:
      - "8080:8080"
    environment:
      - PUID=1000
      - PGID=1000
    volumes:
      - ./data:/app/data
      - ./logs:/app/logs
      - ./config:/app/config:ro
      - /path/to/media:/media:ro
    restart: unless-stopped
```

Open `http://<host>:8080`, go to **Libraries** and add `/media/movies`, `/media/tv` and so on, or
connect Sonarr and Radarr under **Settings** and press **Sync**.

Mount your media at the same path Sonarr, Radarr and Jellyfin use if you can. Otherwise add a path
mapping in each integration's settings.

### Upgrading from Optimarr 1.x

Keep the same `data` and `config` volumes. On first start Optimarr 2 imports
`config/appsettings.json` and `data/optimarr.db` into a new `data/optimarr2.db`; the old files are
left untouched, so you can go back to 1.x. The port inside the container is still 8080.

### Environment variables

| Variable | Default | Purpose |
| --- | --- | --- |
| `OPTIMARR_API_KEY` | generated | Fix the API key instead of generating one |
| `OPTIMARR_BIND` | `0.0.0.0:8080` | Listen address |
| `OPTIMARR_DATA_DIR` | `/app/data` | Database folder |
| `OPTIMARR_CONFIG_DIR` | `/app/config` | Where a 1.x `appsettings.json` is looked for |
| `OPTIMARR_LOG_DIR` | `/app/logs` | Daily log files (14 kept); empty disables |
| `OPTIMARR_LOG` | `info` | Log filter, e.g. `debug` or `info,optimarr=debug` |
| `OPTIMARR_CORS_ORIGINS` | none | Extra allowed origins, comma separated |
| `PUID` / `PGID` | `1000` | User and group the server runs as |

### Fixing files in place

The fix queue writes to your media folders, so mount them without `:ro` and turn the worker on under
**Settings → Fixes & upgrades**. The image includes ffmpeg.

## Releases

Every version is a GitHub Release with notes from [CHANGELOG.md](CHANGELOG.md) and an amd64
image: `ghcr.io/davidsteg/optimarr:<version>` and `:latest`.

To cut a release: bump the version in `VERSION`, `backend/Cargo.toml` and `frontend/package.json`,
add a `## [x.y.z]` section to `CHANGELOG.md`, merge to master, then tag it:

```bash
git tag v2.0.0 && git push origin v2.0.0
```

The release workflow checks the versions, runs the tests, pushes the image and publishes the release.

## Security

The API key is shown under **Settings → Security**. By default it is only required for requests
from outside your network (LAN, loopback and Tailscale addresses are trusted). If Optimarr sits
behind a reverse proxy, every request looks local, so set the mode to **Always**.

## Agent API

`/api/agent/…` gives agents a library overview, file search, a quality report per file (by id,
path or title), the most-transcoded files, per-device verdicts, space savings, upgrade candidates,
the fix queue and recent notifications, plus rescan, fix, upgrade and redownload actions. Every
report includes a plain-language summary. See [docs/API.md](docs/API.md).

## Development

You need Rust (stable), Node 22, and MediaInfo or ffprobe on the `PATH`.

```bash
cd frontend && npm ci && npm run build && cd ..
cargo run -- --data-dir ./data
```

For UI work, run the backend and `npm run dev` in `frontend/` (Vite proxies `/api` to
`http://127.0.0.1:8080`, or `OPTIMARR_URL`).

```bash
cargo test
cd frontend && npm run check
```

The backend lives in `backend/` (Axum, Tokio, sqlx/SQLite) and embeds `frontend/build` at compile
time. The UI is SvelteKit with the static adapter.

## License

MIT
