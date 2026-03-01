# Configuration Directory

This directory contains the runtime `appsettings.json` mounted into the container at `/app/config/appsettings.json`.

Optimarr loads configuration from:

1. `/app/config/appsettings.json` (preferred runtime source)
2. `/app/appsettings.json` (fallback when config mount is missing)

Use this file for deployment-specific values and keep secrets out of source control.

## Setup

1. Create `config/appsettings.json` from the root template:
   ```bash
   cp appsettings.json config/appsettings.json
   ```
2. Update values for your environment (URLs, API keys, allowed origins, matching settings).
3. Restart the container after changes.

## Required Security Notes

- Do not commit real API keys or internal service URLs.
- Keep `Auth:ApiKey` empty to disable API-key protection, or set a strong value to require `X-Optimarr-Key` for `/api/*` routes.
- Prefer Docker environment variables for secret injection in production.

## Key Configuration Reference

### Authentication

- `Auth:ApiKey`
  - Empty: API auth disabled (backward-compatible mode)
  - Set: API auth enabled; clients must send `X-Optimarr-Key`

### CORS

- `Cors:AllowedOrigins` (array)
  - Example:
    ```json
    "Cors": {
      "AllowedOrigins": [
        "http://localhost:5000",
        "https://optimarr.example.com"
      ]
    }
    ```
  - If empty, Optimarr falls back to localhost-only origins.

### Matching Options

These keys control matching throughput in `VideoServarrMatcherService`:

- `MatchingOptions:SeriesConcurrency` - max concurrent Sonarr matching operations
- `MatchingOptions:MovieConcurrency` - max concurrent Radarr matching operations
- `MatchingOptions:VideoBatchSize` - number of videos loaded/processed per batch
- `MatchingOptions:SaveBatchSize` - number of updates before persisting with `SaveChangesAsync`
- `MatchingOptions:MaxMatchConcurrency` - upper bound for match workers in shared paths

Default values:

```json
"MatchingOptions": {
  "SeriesConcurrency": 20,
  "MovieConcurrency": 50,
  "VideoBatchSize": 2000,
  "SaveBatchSize": 200,
  "MaxMatchConcurrency": 50
}
```

### Servarr and Jellyfin

- `Servarr:Sonarr:*` and `Servarr:Radarr:*` contain URL/API key/enabled toggles.
- `Jellyfin:*` contains URL/API key and optional credentials for playback sync.

## Docker Volume Mounts

Standard mounts:

- `./config` -> `/app/config` (runtime configuration)
- `./data` -> `/app/data` (SQLite database persistence)
- `./logs` -> `/app/logs` (application logs)
- `./videos` -> `/videos` (media library, read-only recommended)

## Development Container Defaults

When running `docker-compose.dev.yml`, these defaults are set:

- `ASPNETCORE_ENVIRONMENT=Development`
- `ASPNETCORE_URLS=http://+:8080`
- `DOTNET_USE_POLLING_FILE_WATCHER=1`

