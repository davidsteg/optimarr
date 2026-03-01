# Optimarr - Media Optimization

A web-based application for analyzing video files to determine their compatibility based on media properties. Built in the Servarr family style with integration support for Sonarr, Radarr, and Jellyfin.

## ⚠️ Disclaimer

**This software is in early development stage. Use it at your own risk. The developers are not responsible for any data loss, system damage, or other issues that may arise from using this software. Always backup your data before installation and use.**

## Features

- **Video Analysis**: Analyze video files based on media properties (codecs, containers, bit depth, HDR, etc.)
- **Compatibility Rating**: 0-100 rating scale with configurable thresholds and impact weights
- **Library Management**: Scan and manage multiple video libraries with background processing
- **Servarr Integration**: Sonarr, Radarr, and Jellyfin integration for automated analysis and redownloads
- **Tdarr Webhook**: Automatic rescan after transcoding
- **Modern Web UI**: Servarr-style dark theme interface

## Installation

**Docker Hub Image:** `bridgemill/optimarr:latest`

### Quick Start (Docker Compose)

1. Clone this repository
2. Edit `docker-compose.yml` to configure volumes
3. Start the container:
   ```bash
   docker-compose up -d
   ```
4. Open `http://localhost:5000`

### Docker Run

```bash
docker pull bridgemill/optimarr:latest

docker run -d \
  --name optimarr \
  -p 5000:8080 \
  -v $(pwd)/config:/app/config \
  -v $(pwd)/data:/app/data \
  -v $(pwd)/logs:/app/logs \
  -v $(pwd)/videos:/videos:ro \
  --restart unless-stopped \
  bridgemill/optimarr:latest
```

## Documentation

- **[Product Requirement Document](docs/PRD.md)** - Product specifications and requirements
- **[System Architecture](docs/ARCHITECTURE.md)** - Technical architecture and design
- **[Developer Guide](docs/DEVELOPER_GUIDE.md)** - Development workflows and patterns
- **[Changelog](docs/CHANGELOG.md)** - Version history

## API Reference (Most Used Endpoints)

Base API path: `/api`

| Method | Path | Description | Example |
|---|---|---|---|
| `POST` | `/api/analysis/analyze` | Analyze a single video file path and return compatibility report data. | Request body: `{ "videoPath": "/videos/Movie.mkv", "subtitlePath": null }` |
| `POST` | `/api/library/scan` | Start a background scan for a library path. | Request body: `{ "path": "/videos/movies", "name": "Movies", "category": "Movies" }` |
| `GET` | `/api/servarr/status` | Return Sonarr/Radarr enabled/connected/version status used by the settings UI. | Response includes `sonarr` and `radarr` status objects |
| `GET` | `/api/playback/history` | Return paged playback history with filters (device, user, play method, library path). | Example: `/api/playback/history?page=1&pageSize=50&isTranscode=true` |
| `GET` | `/api/system/health` | Liveness endpoint used by Docker/container health checks. | Response: `{ "status": "healthy", "version": "x.x.x", "timestamp": "..." }` |

For the full endpoint set and schemas, run the app in development and open Swagger UI at `/swagger`.

## Screenshots

![Dashboard View](screenshot/dashboard-view.png "Dashboard View")

![Library Management](screenshot/library-management.png "Library Management")

![Media Analysis](screenshot/media-analysis.png "Media Analysis")

## Support

For issues and questions, please open an issue on the project repository.

---

**License:** MIT License
