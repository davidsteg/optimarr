# Optimarr - Agent Memory

## Identity
- Name: Optimarr
- Purpose: media compatibility analysis + Servarr/Jellyfin integration
- Stack: ASP.NET Core 8, EF Core SQLite, Serilog, vanilla JS modules
- Runtime config roots: `appsettings.json`, `config/appsettings.json`

## Current Structure Snapshot
- Controllers: 5 (`Controllers/*.cs`)
- Services: 13 (`Services/*.cs`)
- Models: 9 (`Models/*.cs`)
- Frontend modules: 13 (`wwwroot/js/*.js`)
- SQL migration scripts: 2 (`Data/Migrations/*.sql`)

## Core Flow
- Layered flow: Controller -> Service -> DbContext.
- Video analysis path: `VideoAnalyzerService` -> `MediaInfo` CLI -> `MediaPropertyRatingService`.
- Servarr path: `SonarrService`/`RadarrService` -> `ServarrSyncService` + `VideoServarrMatcherService`.
- Hosted services: `DatabaseMigrationService`, `PlaybackSyncService`, `ProcessingRescanService`.

## Security/Runtime Notes
- Optional API key auth exists (`Middleware/ApiKeyMiddleware.cs`), header: `X-Optimarr-Key`.
- Health and auth-config endpoints are public bypass paths.
- CORS uses configured allowlist with localhost fallback when empty.

## Coding Conventions (observed)
- C#: PascalCase APIs, `_camelCase` fields, structured logging with `ILogger`.
- JS: ES modules + `fetch`; many handlers intentionally exported to `window.*`.
- Formatting is mixed spaces/tabs; do not do global formatting churn.

## High-Risk Areas
- Runtime DB/content paths: `data/`, `logs/`, `videos/`, `config/` (do not treat runtime files as source).
- Secrets in config keys (`Auth`, `Servarr`, `Jellyfin`) must never be committed.
- Deployment files (`Dockerfile`, `docker-entrypoint.sh`, compose) require explicit task intent.

## Known Contradictions To Resolve In Future Work
- Version mismatch: `VERSION` (1.2.0) vs `optimarr.csproj` (1.1.3).
- Documentation still contains pre-auth assumptions in places.
- Prior rules about "no window globals" conflict with current frontend pattern.

## Contract Link
- Full execution contract and Definition of Done: `docs/PROJECT_CONTEXT.md`.
