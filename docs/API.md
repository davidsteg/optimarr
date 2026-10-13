# Optimarr agent API

A small, stable JSON API under `/api/agent/` meant for AI agents, MCP servers and scripts.
Every endpoint returns compact JSON; reports carry a plain-language `summary` that an agent
can relay to a user as-is.

The rest of `/api/` serves the web UI and may change between releases. The `/api/agent/`
endpoints keep their shape within a major version: fields may be added, never renamed or removed.

## Authentication

Send the API key (Settings > General) in one of:

- `X-Api-Key: <key>` (preferred)
- `Authorization: Bearer <key>`

The key can also be fixed with the `OPTIMARR_API_KEY` environment variable.

Whether a key is required is set by the auth mode in Settings > General:

| Mode | Behaviour |
| --- | --- |
| `external` (default) | Key required, except for requests from loopback, private LAN, link-local and Tailscale (100.64.0.0/10) addresses |
| `always` | Key required for every request |
| `never` | No key required |

If Optimarr sits behind a reverse proxy on the same network, every request looks local. Use
`always` in that case, and always send the key from agents.

Unauthorized requests get `401` with `{"error": "Unauthorized: ..."}`.

## Errors

Errors use the HTTP status and a JSON body: `{"error": "message"}`.

| Status | Meaning |
| --- | --- |
| 400 | Bad or missing parameter |
| 401 | Missing or wrong API key |
| 404 | No such file |
| 500 | Internal error (message included) |

## Concepts

Every analyzed file gets two scores from 0 to 100, each with a verdict:

- **Compatibility**: how likely the file is to direct-play on your clients without transcoding.
  Verdicts: `optimal`, `good`, `poor`.
- **Quality**: how good the encode is for its resolution (bitrate, codec, HDR, audio).
  Verdicts: `excellent`, `good`, `fair`, `poor`.

A file's `status` is one of `ok`, `broken` (could not be read), `processing` (a replacement or
re-encode is in progress) or `missing` (no longer on disk).

An **issue** is one finding:

```json
{
  "code": "audio_codec",
  "area": "compatibility",
  "severity": "critical",
  "message": "DTS audio is not supported and will be transcoded",
  "penalty": 30,
  "fix": "Convert the audio to AAC, or add an AAC track"
}
```

`area` is `compatibility` or `quality`; `severity` is `info`, `warning` or `critical`; `penalty`
is the number of points deducted; `fix` is present when there is a concrete remedy.

## Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/api/agent/overview` | Library-wide breakdown, top issues, worst files |
| GET | `/api/agent/files` | Search and filter files |
| GET | `/api/agent/files/{id}` | Full quality report for one file |
| GET | `/api/agent/lookup` | Report by exact path or by title |
| GET | `/api/agent/transcodes` | Files Jellyfin transcoded most |
| POST | `/api/agent/files/{id}/rescan` | Re-analyze a file now |
| POST | `/api/agent/files/{id}/redownload` | Delete the file in Sonarr/Radarr and search for a replacement |
| GET | `/api/agent/devices` | Direct play / remux / transcode counts per device profile |
| GET | `/api/agent/devices/{id}/transcodes` | Files a device would transcode, with reasons |
| GET | `/api/agent/savings` | Space saved by re-encoding to HEVC or AV1 |
| GET | `/api/agent/upgrades` | Low-quality files Sonarr/Radarr could upgrade |
| POST | `/api/agent/files/{id}/upgrade` | Search for a better release (keeps the file) |
| POST | `/api/agent/files/{id}/fix` | Queue the suggested ffmpeg fix |
| GET | `/api/agent/fixes` | The fix queue |
| GET | `/api/agent/notifications` | Recent notifications |

### GET /api/agent/overview

```bash
curl -H "X-Api-Key: $KEY" http://optimarr:8080/api/agent/overview
```

```json
{
  "summary": "1250 files (14.2 TB). Compatibility: 61.4% optimal, 27.0% good, 11.6% poor. Quality: 22.0% excellent, 48.3% good, 21.1% fair, 8.6% poor. 3 broken.",
  "files": 1250,
  "sizeBytes": 14200000000000,
  "compatibility": { "optimal": 768, "good": 337, "poor": 145 },
  "quality": { "excellent": 275, "good": 604, "fair": 264, "poor": 107 },
  "broken": 3,
  "topIssues": [
    { "code": "audio_codec", "count": 212, "example": "DTS audio is not supported and will be transcoded" }
  ],
  "worstFiles": [ /* up to 5 file briefs, see below */ ]
}
```

Files with status `missing` are excluded. `topIssues` lists up to 8 warning or critical issue
codes by how many files have them.

### GET /api/agent/files

Query parameters (all optional):

| Name | Description |
| --- | --- |
| `q` | Text search on title, file name and path |
| `compatibility` | Verdict filter: `optimal`, `good`, `poor` |
| `quality` | Verdict filter: `excellent`, `good`, `fair`, `poor` |
| `issue` | Issue code, e.g. `audio_codec`, `container`, `subtitle_format`, `low_bitrate` |
| `status` | `ok`, `broken`, `processing`, `missing` |
| `libraryId` | Only files in this library |
| `sort` | `worst` (default), `best`, `largest`, `newest`, `name` |
| `limit` | 1 to 100, default 20 |

```bash
curl -H "X-Api-Key: $KEY" "http://optimarr:8080/api/agent/files?compatibility=poor&sort=largest&limit=10"
```

```json
{
  "total": 145,
  "returned": 10,
  "files": [
    {
      "id": 812,
      "title": "The Expanse S02E05",
      "path": "/tv/The Expanse/Season 02/The Expanse - S02E05.mkv",
      "compatibility": { "score": 42, "verdict": "poor" },
      "quality": { "score": 78, "verdict": "good" },
      "status": "ok",
      "mainIssues": ["DTS audio is not supported and will be transcoded"]
    }
  ]
}
```

A file brief lists at most 3 `mainIssues` (warning or critical messages).

### GET /api/agent/files/{id}

Returns a full report.

```json
{
  "id": 812,
  "title": "The Expanse S02E05",
  "path": "/tv/The Expanse/Season 02/The Expanse - S02E05.mkv",
  "status": "ok",
  "summary": "Plays on most clients but DTS audio will be transcoded; video quality is good for 1080p.",
  "compatibility": { "score": 42, "verdict": "poor" },
  "quality": { "score": 78, "verdict": "good" },
  "technical": {
    "container": "matroska",
    "video": "h264 8-bit 1080p (1920x1080), 8.2 Mbps",
    "audio": ["dts 5.1 [eng]"],
    "subtitles": ["PGS [eng]", "SRT [eng] (external)"],
    "durationMinutes": 47,
    "sizeGB": 2.91,
    "overallBitrate": "9.8 Mbps"
  },
  "issues": [ /* issues, see Concepts */ ],
  "ffmpegCommand": "ffmpeg -i \"in.mkv\" -map 0 -c copy -c:a:0 aac -b:a:0 384k \"out.mkv\"",
  "managedBy": { "service": "Sonarr", "seriesId": 12, "episodeId": 3456 },
  "playback": { "plays": 4, "transcodes": 3, "lastPlayed": "2026-10-01T19:22:10Z" }
}
```

- `technical.video` is `null` for audio-only or unreadable files; `overallBitrate` may be `null`.
- `ffmpegCommand` is one command that fixes every fixable compatibility issue, or `null`.
- `managedBy` is `null` when the file is not linked to Sonarr or Radarr; for Radarr it is
  `{"service": "Radarr", "movieId": 77}`.
- `playback` counts Jellyfin plays recorded by Optimarr.

### GET /api/agent/lookup

Find files without knowing the id. Pass exactly one of:

| Name | Description |
| --- | --- |
| `path` | Exact path as Optimarr sees it; returns one match or 404 |
| `title` | Movie or series title (substring match); returns up to 5 matches, exact title first, then by season and episode |

```bash
curl -G -H "X-Api-Key: $KEY" --data-urlencode "title=The Expanse" http://optimarr:8080/api/agent/lookup
```

```json
{ "matches": [ /* full reports */ ] }
```

Neither parameter gives `400`. A title with no matches gives `{"matches": []}`.

### GET /api/agent/transcodes

Files that Jellyfin transcoded during playback, most transcodes first (up to 20). Requires the
Jellyfin integration.

| Name | Description |
| --- | --- |
| `days` | Look-back window, 1 to 3650, default 30 |

```json
{
  "days": 30,
  "files": [
    {
      "file": { /* file brief */ },
      "plays": 5,
      "transcodes": 5,
      "jellyfinReasons": "AudioCodecNotSupported"
    }
  ]
}
```

`jellyfinReasons` is a comma-separated list of Jellyfin's transcode reasons, or `null`.

### POST /api/agent/files/{id}/rescan

Probes the file again, re-scores it and returns the fresh report (same shape as
`GET /api/agent/files/{id}`). Takes a few seconds for large files.

### POST /api/agent/files/{id}/redownload

**Destructive.** Deletes the file through Sonarr or Radarr, then starts a search for a replacement.
The file's status becomes `processing` until the new file is imported. Agents should confirm with
the user before calling this.

```json
{ "id": 812, "success": true, "service": "Sonarr", "message": "Search started in Sonarr" }
```

On failure `success` is `false` and `message` says why, for example
`"Not managed by Sonarr or Radarr (run a match first)"` or `"Sonarr is not configured"`.

### GET /api/agent/devices

What each enabled device profile (Settings → Devices) would do with the library.

```json
{
  "summary": "Living room TV: 1102 direct play, 98 remux, 50 transcode; Browser: 610 direct play, 420 remux, 220 transcode.",
  "devices": [
    {
      "id": 1,
      "name": "Living room TV",
      "directPlay": 1102,
      "directStream": 98,
      "transcode": 50,
      "transcodeSizeBytes": 310000000000,
      "topReasons": [{ "reason": "DTS audio", "count": 31 }, { "reason": "HDR10 needs tone mapping", "count": 12 }]
    }
  ]
}
```

`directStream` means the container is remuxed on the fly (cheap); `transcode` means video or audio
is re-encoded. Files with status `ok` are counted.

### GET /api/agent/devices/{id}/transcodes

Files that would be transcoded on one device, most reasons first. `limit` 1 to 200, default 20.

```json
{ "total": 50, "files": [{ "file": { /* file brief */ }, "reasons": ["H.265 10-bit video", "DTS audio"] }] }
```

### GET /api/agent/savings

Estimated space saved by re-encoding the video to `target=hevc` (default) or `target=av1`. Files
that are already efficient or would save under 15% are left out.

```json
{
  "target": "hevc",
  "summary": "Re-encoding 412 files to H.265 would save about 3.1 TB (58% of 5.4 TB).",
  "files": 412,
  "currentBytes": 5400000000000,
  "savingsBytes": 3100000000000,
  "byCodec": [{ "codec": "H.264", "files": 380, "savingsBytes": 2900000000000 }],
  "top": [
    {
      "file": { /* file brief */ },
      "estimate": { "target": "hevc", "currentSize": 30000000000, "estimatedSize": 9000000000, "savings": 21000000000, "targetVideoBitrate": 3480000 }
    }
  ]
}
```

The estimate assumes visually transparent settings for the file's resolution and frame rate and
keeps audio as is. It is a guide, not a promise.

### GET /api/agent/upgrades

Files linked to Sonarr or Radarr whose quality score is at or below `maxScore` (default from
Settings → Fixes & upgrades), worst first, skipping files searched within the cooldown.
`limit` 1 to 100, default 20.

```json
{ "summary": "37 files managed by Sonarr/Radarr have a quality score of 60 or lower and were not searched in the last 14 days.", "total": 37, "files": [ /* file briefs */ ] }
```

### POST /api/agent/files/{id}/upgrade

Asks Sonarr or Radarr to search for a better release. **The current file is kept**; they only grab
a release their quality profile ranks higher.

```json
{ "id": 812, "title": "The Expanse S02E05", "success": true, "message": "Search started in Sonarr" }
```

### POST /api/agent/files/{id}/fix

Queues the suggested ffmpeg fix. The fix worker (Settings → Fixes & upgrades, off by default)
replaces the file, keeping the original as `.orig` unless configured otherwise, then re-analyzes it.
Returns `400` when there is nothing ffmpeg can fix or a fix is already queued.

```json
{
  "fixId": 14,
  "status": "queued",
  "command": "ffmpeg -hide_banner -i '/tv/…/S02E05.mkv' -map 0:v:0 -map 0:a? -c:v copy -tag:v hvc1 -c:a:0 aac … '/tv/…/S02E05.optimized.mp4'",
  "workerEnabled": true,
  "summary": "Queued; the fix worker will run it next."
}
```

### GET /api/agent/fixes

The fix queue, running and queued first. Optional `status` (`queued`, `running`, `done`, `failed`,
`cancelled`) and `limit` (default 20).

```json
{ "fixes": [{ "id": 14, "fileId": 812, "path": "/tv/…/S02E05.mkv", "status": "done", "progress": 1.0, "error": null, "sizeBefore": 2910000000, "sizeAfter": 2840000000, "finishedAt": "2026-10-13T21:40:02Z" }] }
```

### GET /api/agent/notifications

Recent notifications Optimarr sent (new problem files, finished fixes, upgrade searches), newest
first. Useful for "what changed since I last looked". `limit` default 20.

```json
{ "notifications": [{ "event": "new_problems", "title": "2 new file(s) with problems", "message": "• Movie (2024): compatibility poor, quality good\n• …", "sentAt": "2026-10-13T21:30:00Z" }] }
```

`event` is `new_problems`, `fix_done`, `fix_failed` or `upgrades`. Only notifications sent while at
least one target is configured are recorded.

### Reports include

Each full report (`/api/agent/files/{id}`, `/api/agent/lookup`, rescan) also carries:

- `devices`: per device `{ "id", "name", "outcome": "directPlay" | "directStream" | "transcode", "reasons", "subtitleNote" }`.
- `savings`: the HEVC estimate for this file, or `null`.
- `fix`: the latest fix-queue entry `{ "id", "status", "progress", "error", "finishedAt" }`, or `null`.
- `upgradeRequestedAt`: when an upgrade search was last requested, or `null`.

Language issues (`missing_audio_language`, `missing_subtitle_language`, `image_subtitles`) appear in
`issues` when preferred languages are set under Settings → Quality.

## Wrapping it in an MCP server

Each endpoint maps onto one tool. A suggested set:

| Tool | Endpoint |
| --- | --- |
| `library_overview` | `GET /api/agent/overview` |
| `search_files` | `GET /api/agent/files` |
| `file_report` | `GET /api/agent/files/{id}` |
| `find_file` | `GET /api/agent/lookup` |
| `most_transcoded` | `GET /api/agent/transcodes` |
| `rescan_file` | `POST /api/agent/files/{id}/rescan` |
| `redownload_file` | `POST /api/agent/files/{id}/redownload` (mark as destructive) |
| `device_overview` | `GET /api/agent/devices` |
| `device_transcodes` | `GET /api/agent/devices/{id}/transcodes` |
| `space_savings` | `GET /api/agent/savings` |
| `upgrade_candidates` | `GET /api/agent/upgrades` |
| `search_upgrade` | `POST /api/agent/files/{id}/upgrade` |
| `queue_fix` | `POST /api/agent/files/{id}/fix` (changes files on disk when the worker is on) |
| `fix_queue` | `GET /api/agent/fixes` |
| `recent_notifications` | `GET /api/agent/notifications` |

Return the `summary` field (or the briefs' titles and verdicts) as the tool's text, and the full
JSON as structured content.
