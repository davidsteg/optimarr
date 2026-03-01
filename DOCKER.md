# Docker Deployment Guide

## Quick Start

### Build the Image

```bash
docker build -t optimarr .
```

### Run the Container

```bash
docker run -d \
  --name optimarr \
  -p 5000:8080 \
  -v $(pwd)/appsettings.json:/app/appsettings.json:ro \
  optimarr
```

### Using Docker Compose

```bash
docker-compose up -d
```

## Configuration

### Mount appsettings.json

The application reads configuration from `appsettings.json`. Mount it as a volume:

```bash
-v /path/to/appsettings.json:/app/appsettings.json:ro
```

### Environment Variables

You can override configuration using environment variables with double underscores:

```bash
-e Servarr__Sonarr__BaseUrl=http://sonarr:8989
-e Servarr__Sonarr__ApiKey=your-api-key
-e Servarr__Sonarr__Enabled=true
```

### Volume Mounts

#### For File Path Analysis

If you want to analyze files by path (not upload), mount your video directory:

```bash
-v /path/to/videos:/videos:ro
```

Then use paths like `/videos/movie.mp4` in the API.

#### For File Uploads

File uploads work without additional mounts - files are processed in memory or temporary storage.

## Networking

### Connect to Servarr Apps

If Sonarr/Radarr/Jellyfin are running in Docker:

1. Create a network:
   ```bash
   docker network create servarr-network
   ```

2. Run containers on the same network:
   ```bash
   docker run -d \
     --name optimarr \
     --network servarr-network \
     -p 5000:8080 \
     optimarr
   ```

3. Update `appsettings.json` to use container names:
   ```json
   {
     "Servarr": {
       "Sonarr": {
         "BaseUrl": "http://sonarr:8989"
       }
     }
   }
   ```

### External Servarr Apps

If Servarr apps are on the host or another machine:

- Use `host.docker.internal` (Docker Desktop) or host IP
- Example: `http://host.docker.internal:8989`

## Development

### Development Container (`docker-compose.dev.yml`)

```bash
docker compose -f docker-compose.dev.yml up --build
```

What this does:
- Builds from `Dockerfile.dev` (SDK image + `dotnet watch`)
- Runs Optimarr on internal port `8080` (mapped to host `5000`)
- Bind-mounts source code into `/app` for hot reload
- Persists runtime state with bind mounts:
  - `./config` -> `/app/config`
  - `./data` -> `/app/data`
  - `./logs` -> `/app/logs`
  - `./videos` -> `/videos` (read-only)

Configuration behavior:
- The app first looks for `/app/config/appsettings.json`
- If missing, it falls back to `/app/appsettings.json`
- For Docker-based development, keep `./config/appsettings.json` as your primary runtime config

Pointing to local Sonarr/Radarr:
- If Sonarr/Radarr run on your host machine, use `host.docker.internal` in `config/appsettings.json`:
  - `http://host.docker.internal:8989` (Sonarr)
  - `http://host.docker.internal:7878` (Radarr)
- If they run in Docker, connect services to the same network and use container names.

### View Logs

```bash
docker logs -f optimarr
```

### Stop and Remove

```bash
docker stop optimarr
docker rm optimarr
```

Or with compose:

```bash
docker-compose down
```

## Troubleshooting

### Health check failing

1. Inspect container health:
   ```bash
   docker ps
   docker inspect --format='{{json .State.Health}}' optimarr
   ```
2. Verify API endpoints manually:
   ```bash
   curl -i http://localhost:5000/api/system/health
   curl -i http://localhost:5000/api/system/ready
   ```
3. If `/ready` returns `503`, migrations are still running or failed; check logs for migration status details.

### How to view logs

Use one of these depending on your workflow:

```bash
# Single container
docker logs -f optimarr

# Compose
docker compose logs -f optimarr
```

Logs are also persisted to host `./logs` when that volume is mounted.

### Reset the database safely

Stop the app, remove the SQLite database file, then start again:

```bash
docker compose down
rm -f ./data/optimarr.db
docker compose up -d
```

On next startup, Optimarr recreates schema and reruns startup migrations.

### Update config without rebuilding

1. Edit `./config/appsettings.json` on the host.
2. Restart the container (no image rebuild needed):
   ```bash
   docker compose restart optimarr
   ```

Optimarr reads runtime config from `/app/config/appsettings.json` when mounted.

### Permission errors with mounted volumes

Symptoms include failures writing to `/app/data` or `/app/logs`.

Checks and fixes:

```bash
# Check host directory ownership/permissions
ls -la ./config ./data ./logs

# Linux example: grant current user ownership
sudo chown -R $(id -u):$(id -g) ./config ./data ./logs
```

For development, `docker-compose.dev.yml` runs as UID `1000`; ensure mounted paths are writable by that UID.

### MediaInfo not working

If MediaInfo-related analysis fails:

1. Check logs:
   ```bash
   docker logs optimarr
   ```
2. Verify package availability inside container:
   ```bash
   docker exec optimarr dpkg -l | grep mediainfo
   ```

### Port already in use

Map a different host port:

```bash
docker run -d -p 8080:8080 optimarr
```

## Production Deployment

### Multi-Architecture Build

Build for multiple architectures:

```bash
docker buildx create --use
docker buildx build --platform linux/amd64,linux/arm64 -t optimarr .
```

### Push to Registry

```bash
docker tag optimarr your-registry/optimarr:latest
docker push your-registry/optimarr:latest
```

### Health Check

The container health check is built in and migration-aware:
- During startup migrations, Docker health check allows the container to continue starting.
- After migration completes, health requires `GET /api/system/health` to return success.

You can still override it at runtime if needed:

```bash
docker run -d \
  --health-cmd="curl -f http://localhost:8080/api/system/health || exit 1" \
  --health-interval=30s \
  optimarr
```

## Example docker-compose.yml

See `docker-compose.yml` for a complete example with optional Sonarr/Radarr integration.

