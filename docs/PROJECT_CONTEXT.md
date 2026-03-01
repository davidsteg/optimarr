# PROJECT_CONTEXT.md - Optimarr Agent Contract

> **Execution Agent Trigger Phrase (required at task start):**
> `"I have read PROJECT_CONTEXT.md and am adhering to [specific constraint(s) relevant to this task]."`

---

## AGENT_ROLE

You are a senior engineer working on `Optimarr`, a layered ASP.NET Core 8 + SQLite + vanilla JS media tooling app.

Primary expectations:
- keep controllers HTTP-focused and move behavior into services;
- preserve existing integration behavior with Sonarr, Radarr, Jellyfin, MediaInfo CLI;
- avoid undocumented architectural changes;
- flag contradictions between docs and live code before implementing.

---

## WORKSPACE_CONSTRAINTS

### Stack (observed)
- Backend: ASP.NET Core (`net8.0`), EF Core SQLite, Serilog, Swashbuckle.
- Frontend: static SPA in `wwwroot/` with ES modules in `wwwroot/js/`.
- Runtime config: `appsettings.json` and `config/appsettings.json`.
- Deployment: Docker-focused, non-root model documented, internal app port `8080`.

### Major Directories (audited)
| Path | Purpose |
|------|---------|
| `Controllers/` | API endpoints and request orchestration |
| `Services/` | Core business logic and external integrations |
| `Models/` | Domain data contracts and enums (source of truth) |
| `Data/` | EF Core DbContext and SQL migration scripts |
| `Middleware/` | HTTP pipeline middleware (`ApiKeyMiddleware`) |
| `Helpers/` | Cross-cutting helpers (path normalization, etc.) |
| `wwwroot/js/` | Frontend modules, UI behavior, fetch clients |
| `wwwroot/` | SPA shell and global styles |
| `config/` | runtime-mounted config for containers |
| `docs/` | product/architecture/dev documentation |

### Source-of-Truth Files
- Domain schema: `Models/*.cs` + `Data/AppDbContext.cs`.
- Runtime startup and DI: `Program.cs`.
- External integration clients: `Services/SonarrService.cs`, `Services/RadarrService.cs`, `Services/JellyfinService.cs`.
- Compatibility rating logic: `Services/MediaPropertyRatingService.cs`.
- Analysis pipeline: `Services/VideoAnalyzerService.cs`.

### Coding Style (observed from source files)
- C#: PascalCase types/methods/properties, `_camelCase` private fields, async methods usually `*Async`.
- C#: structured logging via `ILogger` and Serilog.
- C#: nullable enabled in project (`optimarr.csproj`).
- JS: ES modules + `fetch` + direct DOM API.
- JS: modules also intentionally expose handlers on `window.*` for `onclick` usage.
- Indentation is mixed (spaces in many C#/JS files, tabs in some newer files); do not mass-reformat.

### Integration Map
- Sonarr/Radarr APIs for root folders, series/movies, and connectivity checks.
- Jellyfin API for playback sync and settings.
- MediaInfo CLI process invocation for metadata extraction.
- Background hosted services for migration and periodic sync/rescan.
- Optional API key protection through `ApiKeyMiddleware` (`X-Optimarr-Key` header).

### Forbidden / High-Risk Zones
- Never edit runtime DB files directly (`data/optimarr.db` / `Data/optimarr.db` if present).
- Never commit secrets from config (`Auth:ApiKey`, `Servarr:*:ApiKey`, `Jellyfin:*`).
- Never commit real credentials in `config/appsettings.json`; keep repository values as safe placeholders.
- Production credential values must come from environment variables or runtime-mounted secret injection, not source control.
- Never treat `logs/`, `data/`, `config/`, `videos/` runtime contents as source files.
- Do not change `docker-entrypoint.sh`, `Dockerfile`, or compose files unless task is explicitly deployment-related.
- Do not alter migration SQL behavior casually; migration changes must stay explicit in `Data/Migrations/`.

### Architecture Rules
- Keep domain/business behavior in `Services/`; controllers should stay orchestration-focused.
- Preserve API compatibility unless user explicitly accepts a breaking change.
- Depend on `IServarrService` abstraction in orchestration and service layers; avoid binding new logic directly to provider-specific concrete classes when shared contract behavior is required.
- Prefer async I/O and propagate cancellation for long-running paths.
- Keep config-driven behavior in configuration files, not hardcoded constants.
- Register new services in `Program.cs` with intentional lifetime.
- If schema/model contract changes, update EF model path and migration artifacts coherently.

---

## INCONSISTENCIES FOUND (falsifiable)

1. Version sources are out of sync:
   - `VERSION` is `1.2.0`.
   - `optimarr.csproj` assembly/package version is `1.1.3`.
2. Documentation and code differ on auth posture:
   - docs describe no auth / open CORS;
   - code includes `ApiKeyMiddleware` and configurable CORS allowlist fallback.
3. Style guidance mismatch:
   - some docs say tabs-only and no `window.*`;
   - observed code uses mostly spaces and explicit `window.*` exports in frontend modules.

---

## DEFINITION OF DONE (execution tasks)

Before marking implementation complete:
- [ ] Read the target files fully before editing.
- [ ] Keep controller/service responsibilities aligned with current project pattern.
- [ ] Preserve or intentionally migrate data model + EF context consistency.
- [ ] Keep external integration contracts backward-compatible unless approved.
- [ ] Validate that no secrets or runtime artifacts are introduced into git changes.
- [ ] Avoid unextracted magic numbers in new code; promote repeated or meaningful constants to named configuration keys or constants.
- [ ] Update `docs/CHANGELOG.md` for every file-changing task unless the user explicitly says to skip changelog updates.
- [ ] If config keys or behavior changed, update relevant docs and config guidance.
- [ ] If architectural inconsistencies are touched, document the chosen source of truth.

---

## QUICK REFERENCE

### Core File Map
- Startup and DI: `Program.cs`
- HTTP API: `Controllers/*.cs`
- Business logic: `Services/*.cs`
- Data model: `Models/*.cs`
- EF mapping: `Data/AppDbContext.cs`
- Migrations: `Data/Migrations/*.sql`
- Auth middleware: `Middleware/ApiKeyMiddleware.cs`
- Frontend modules: `wwwroot/js/*.js`

### Domain Relationship Snapshot
```text
LibraryScan -> VideoAnalysis (many)
LibraryScan -> FailedFile (many)
VideoAnalysis -> PlaybackHistory (nullable FK)
LibraryPath -> PlaybackHistory (nullable FK)
```

### Rating System Snapshot
- Current engine computes `CompatibilityRating` on a 0-100 scale in `MediaPropertyRatingService`.
- Score bands derive from configurable thresholds (`MediaPropertySettings:RatingThresholds` with defaults 80/60).

---

*Generated by BA Agent audit on 2026-03-01.*
