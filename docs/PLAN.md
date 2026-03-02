# PLAN.md — Architect Agent Deliverable
### Generated: 2026-03-01 | Architect Agent (A1–A6)

> **Trigger Phrase:** "I have read PROJECT_CONTEXT.md and MEMORY.md. I am designing solutions that adhere to the layered architecture, async/await, constructor DI, and no-Result-or-Wait constraints defined there."

---

## Summary of Tasks Designed

| ID | Priority | Title | Status |
|----|----------|-------|--------|
| A1 | 🔴 | Design `IServarrService` interface + base class | ✅ Complete |
| A2 | 🔴 | Design `ServarrController` split (3 controllers) | ✅ Complete |
| A3 | 🟠 | Design `VideoServarrMatcherService` decomposition | ✅ Complete |
| A4 | 🟠 | Design `PathNormalizer` static helper | ✅ Complete |
| A5 | 🟠 | Design authentication strategy (ADR) | ✅ Complete |
| A6 | 🟡 | Design memory-efficient caching strategy | ✅ Complete |

---

## A1 — `IServarrService` Interface + Base Class

### Finding

`SonarrService` (473 lines) and `RadarrService` (349 lines) share:
- **Identical fields:** `_configuration`, `_httpClient`, `_isEnabled`, `_isConnected`, `_apiKey`, `_baseUrl`, `_basicAuthUsername`, `_basicAuthPassword`
- **Identical methods:** `Initialize()`, `ReloadConfiguration()`, `EnsureApiKeyHeader()`, `CheckConnection()`, `GetVersion()`, `GetRootFolders()` — these differ only in the config key prefix (`"Sonarr"` vs `"Radarr"`)
- **Identical DTOs:** `SonarrRootFolder` and `RadarrRootFolder` are structurally identical (Id, Path, Accessible, FreeSpace)

### Decision

**Option A (chosen):** `IServarrService` interface + `ServarrServiceBase` abstract class. Both services inherit the base, implement the interface, and add their unique methods. Unify `SonarrRootFolder` / `RadarrRootFolder` into a single `ServarrRootFolder`.

**Option B (rejected):** Generic `IServarrService<TRootFolder>` — unnecessary complexity for two concrete types.

**Option C (rejected):** Interface only, no base class — leaves duplicated constructor logic in both services.

### Interface Design

**File:** `Services/IServarrService.cs`

```csharp
namespace Optimarr.Services
{
	public interface IServarrService
	{
		bool IsEnabled { get; }
		bool IsConnected { get; }
		string? GetVersion();
		Task<List<ServarrRootFolder>> GetRootFoldersAsync();
		void ReloadConfiguration();
	}
}
```

---

## RQ-A1 — Feature Request Architecture Mapping (`FR-20260302-01`)

### Problem statement

`FR-20260302-01` requests three coupled capabilities:
1. Detect compatibility quality regressions early and surface actionable alerts.
2. Make scoring playback-aware so ratings reflect real-world playback outcomes over time.
3. Run periodic refresh jobs automatically without manual operator actions.

Current gaps:
- No persisted regression baseline model or alert lifecycle.
- Rating pipeline is media-property based only; playback-history signal is not part of score output.
- No config-driven scheduler for periodic alert/score refresh loops.

### Approach options

#### Option A (selected): In-process pipeline with persisted alert state and hosted scheduler
- Add persistent regression artifacts in SQLite.
- Extend scoring service with optional playback-aware weighting.
- Use ASP.NET hosted background scheduler with config-driven cadence and overlap protection.
- Keep API/UI incremental and backward-compatible.

Pros:
- Aligns with current architecture (`Controller -> Service -> DbContext`).
- No new infrastructure dependency.
- Deterministic and testable with existing service patterns.

Cons:
- Requires careful job coordination and migration changes.
- Must guard against noisy alerts in early rollout.

#### Option B (rejected): External scheduler + out-of-process workers
Pros:
- Better operational isolation and horizontal scaling.
Cons:
- Adds deployment complexity and operational burden outside current project model.
- Over-scoped for next minor release window.

#### Option C (rejected): Playback-aware scoring only (no alert persistence)
Pros:
- Lowest implementation complexity.
Cons:
- Does not satisfy regression-detection ask; no lifecycle for triage/ack/resolution.

### Selected architecture and flow

1. **Baseline + regression detection service**
   - Introduce `QualityRegressionService` to compute deltas between latest score and baseline snapshot.
   - Emit/update alerts with dedupe key `(VideoAnalysisId, RuleId, BaselineVersion)`.
   - Alert states: `New`, `Acknowledged`, `Resolved`, `Reopened`.

2. **Playback-aware scoring extension**
   - Extend `MediaPropertyRatingService` output with:
     - `BaseScore` (existing property-based score)
     - `PlaybackAdjustment`
     - `EffectiveScore`
     - `ScoreExplanation[]`
   - Weight playback impact by confidence:
     - sparse history -> capped impact
     - sufficient history -> full configured impact range

3. **Scheduler orchestration**
   - Add `RegressionSchedulerService : BackgroundService`.
   - Jobs:
     - periodic regression scan
     - periodic playback-aware score refresh
   - Non-overlap execution guard per job key, cancellation support, jittered retry/backoff.

### API/data/model/config touchpoints

#### API touchpoints
- `GET /api/alerts` (filter by status/severity/date)
- `POST /api/alerts/{id}/acknowledge`
- `POST /api/alerts/{id}/resolve`
- `GET /api/scoring/playback/explain/{videoId}`
- `GET /api/scheduler/status`

#### Data/model touchpoints
- New model/entity: `QualityRegressionAlert`
- New model/entity: `QualityScoreSnapshot` (or equivalent persisted baseline)
- Optional additive fields on `VideoAnalysis`:
  - `EffectiveCompatibilityRating`
  - `PlaybackScoreAdjustment`
  - `LastPlaybackWeightedAt`
- EF touchpoints: `Data/AppDbContext.cs` + migration SQL under `Data/Migrations/`

#### Service touchpoints
- `Services/MediaPropertyRatingService.cs` (playback-aware composition)
- New `Services/QualityRegressionService.cs`
- New `Services/RegressionSchedulerService.cs`
- `Services/PlaybackSyncService.cs` integration hook to trigger targeted recalculation when new history arrives

#### Config touchpoints
- `appsettings.json` + `config/appsettings.json`:
  - `Alerts` (enable, regression threshold, dedupe window, severity bands)
  - `PlaybackScoring` (enable, min-history threshold, max adjustment cap, recency weighting)
  - `Scheduling` (enable, cron/interval, retry/backoff, max concurrent jobs)
- Documentation touchpoint: `config/README.md`

### Risks and mitigations

- **Risk: noisy/duplicate alerts**
  - Mitigation: deterministic dedupe key, cooldown window, state transition rules, QA boundary tests.
- **Risk: score volatility with sparse playback history**
  - Mitigation: confidence gating and adjustment cap; explicit fallback to base score.
- **Risk: scheduler overlap / runaway jobs**
  - Mitigation: per-job lock, cancellation token propagation, max runtime guard, structured job telemetry.
- **Risk: API contract drift**
  - Mitigation: additive endpoints/payload fields only; preserve existing score field semantics for backward compatibility.
- **Risk: migration complexity**
  - Mitigation: explicit forward-only SQL migration + startup migration verification in existing migration pipeline.

### Acceptance criteria (RQ-A1 architecture)

- [ ] Design specifies one selected approach with explicit rejected alternatives.
- [ ] API endpoints, model additions, service additions, and config keys are concrete and implementation-ready.
- [ ] Alert lifecycle and dedupe rules are unambiguous.
- [ ] Playback-aware scoring fallback behavior is defined for sparse/no history.
- [ ] Scheduler semantics (cadence, overlap, retry, cancellation, status visibility) are defined.
- [ ] Execution/testing handoff maps directly to file-level tasks.

### Implementation/testing handoff checklist

#### Execution (Agent 3)
- [ ] Implement alert entities + EF wiring + migration (`Models/*`, `Data/AppDbContext.cs`, `Data/Migrations/*`) — depends on this `RQ-A1` design.
- [ ] Implement `QualityRegressionService` and integrate with scoring pipeline (`Services/*`).
- [ ] Implement playback-aware scoring extension in `MediaPropertyRatingService` with config toggles.
- [ ] Implement `RegressionSchedulerService` and register in `Program.cs` with graceful cancellation.
- [ ] Implement alert/scoring/scheduler API endpoints in `Controllers/*` (additive, backward-compatible).
- [ ] Add config defaults/docs in `appsettings.json`, `config/appsettings.json`, `config/README.md`.

#### QA (Agent 5)
- [ ] Unit tests: regression detection threshold boundaries and dedupe/lifecycle transitions.
- [ ] Unit tests: playback-aware weighting for no/sparse/rich history cases.
- [ ] Integration tests: scheduler cadence, non-overlap, cancellation/shutdown behavior.
- [ ] API tests: alert list/filter/ack/resolve and playback explanation schema.

#### Security (Agent 8)
- [ ] Validate new alert/scheduler endpoints are covered by API key middleware policy.
- [ ] Validate playback-derived API fields/logs do not expose sensitive internals.
- [ ] Validate scheduler config inputs are bounded and safe against abuse/misconfiguration.

---

## Release Gate Plan (2026-03-01)

### Goal

Close remaining release blockers by defining implementation-ready gates for:
- **S6:** MediaInfo path validation against allowed library roots
- **S8:** production-safe error handling (no stack trace or internal message leakage)
- **Version alignment:** `VERSION`, `optimarr.csproj`, and Docker image metadata alignment with a single source of truth

### Gate RG-S6 — MediaInfo Path Whitelist Enforcement

#### Requirement decomposition
1. Any file path passed to MediaInfo must be canonicalized and validated before process invocation.
2. Allowed path classes:
   - Under active `LibraryPaths` roots (`LibraryPaths.IsActive == true`)
   - Under OS temp root for upload-analyze flow
3. Disallowed paths must be rejected before process start and logged server-side.
4. Validation must resist prefix tricks (`/media/tv2` vs `/media/tv`) and traversal attempts.

#### Technical approach (selected)
- Keep central validation in `VideoAnalyzerService.EnsurePathIsAllowed(...)` and always call it from the internal analyze pipeline before MediaInfo execution.
- Keep process invocation with `ProcessStartInfo.ArgumentList` (no raw argument string composition).
- Use canonical full paths and separator-boundary checks (`IsPathUnderRoot`) with case-insensitive comparison.
- **Fail-closed policy update:** if no active library roots are configured, only temp-root files are allowed; non-temp files are denied with `UnauthorizedAccessException`.
- Log blocked attempts with sanitized metadata (full input path is acceptable in server logs only).

#### Why this approach
- Reuses current architecture and minimizes regression risk.
- Preserves upload workflow via temp-root allowlist.
- Eliminates the current ambiguous no-root behavior that can bypass whitelist guarantees.

#### Affected files
- `Services/VideoAnalyzerService.cs`
- `Controllers/AnalysisController.cs` (only if explicit mapping of `UnauthorizedAccessException` to HTTP 403 is needed)
- `Optimarr.Tests/Services/VideoAnalyzerServiceTests.cs` (or equivalent new test file)

#### Acceptance criteria (RG-S6)
- [ ] Analyze request for file under active library root succeeds (assuming valid media file).
- [ ] Analyze request for temp-uploaded file succeeds.
- [ ] Analyze request for path outside active roots and outside temp fails before process start.
- [ ] With zero active roots configured: temp file is allowed, non-temp file is denied.
- [ ] No command-argument concatenation is used for MediaInfo invocation.
- [ ] Security regression tests cover traversal/prefix edge cases.

---

### Gate RG-S8 — Production-Safe Error Response Policy

#### Requirement decomposition
1. API responses must not expose stack traces, exception messages, file paths, SQL/provider internals, or secrets.
2. Controller-level catches must return sanitized payloads only.
3. Full diagnostic detail remains in structured server logs.
4. Error payload shape should be consistent for API consumers.

#### Technical approach (selected)
- Retain global exception middleware in `Program.cs` as the terminal safety net.
- Standardize controller catch responses to a shared payload contract:
  - `500`: `{ "error": "An internal error occurred. Check logs for details." }`
  - `400`/`401`/`403` keep explicit user-safe business messages
- Remove direct `ex.Message` returns from API responses, specifically:
  - `Controllers/PlaybackController.cs` (known open S8 paths)
  - `Services/ServarrSyncService.cs` result message propagation where serialized to clients
  - Any remaining controllers returning interpolated exception content
- Keep logging at `LogError(ex, ...)` or equivalent structured logger calls.

#### Why this approach
- Defense in depth: controller normalization + global exception fallback.
- Preserves observability while eliminating information disclosure risk.
- Minimal API contract change (same status codes, sanitized message bodies).

#### Affected files
- `Program.cs`
- `Controllers/PlaybackController.cs`
- `Services/ServarrSyncService.cs`
- Optional sweep: `Controllers/*.cs` for residual `ex.Message` payloads
- Tests: controller/API integration tests for 500 body sanitization

#### Acceptance criteria (RG-S8)
- [ ] No API endpoint returns raw `ex.Message` in response payloads.
- [ ] Unhandled exceptions always return sanitized JSON body from global middleware.
- [ ] Logs still include full exception + stack trace server-side.
- [ ] Existing client flows continue to receive expected status codes.
- [ ] Automated tests assert that 500 responses do not contain exception text.

---

### Gate RG-V1 — Version Alignment and Source-of-Truth Policy

#### Requirement decomposition
1. Resolve version mismatch between `VERSION`, `optimarr.csproj`, and Docker image label.
2. Define one authoritative source and propagation rules.
3. Ensure release process cannot publish mismatched versions.

#### Technical approach (selected)
- **Single source of truth:** `VERSION` file.
- Propagation policy:
  1. Update `VERSION` first for each release.
  2. Sync `optimarr.csproj` (`Version`, `AssemblyVersion`, `FileVersion`) to match.
  3. Sync Docker image metadata label `org.opencontainers.image.version`.
  4. Validate alignment in CI/release script (fail build on mismatch).
- Patch policy:
  - `Version` mirrors `VERSION` exactly (e.g., `1.2.1`).
  - `AssemblyVersion` and `FileVersion` follow `x.y.z.0` unless explicit assembly compatibility policy changes.

#### Why this approach
- Keeps runtime `/api/system/version`, package metadata, and container metadata coherent.
- Reduces manual release mistakes by making mismatch a gate failure.

#### Affected files
- `VERSION`
- `optimarr.csproj`
- `Dockerfile`
- Release automation script/workflow (if present) for mismatch guard
- `docs/CHANGELOG.md` (release note alignment)

#### Acceptance criteria (RG-V1)
- [ ] `VERSION` equals `optimarr.csproj` `Version`.
- [ ] `AssemblyVersion`/`FileVersion` aligned to release policy.
- [ ] Docker label version matches `VERSION`.
- [ ] Build/release check fails if any mismatch is detected.
- [ ] `docs/CHANGELOG.md` release entry reflects the same version.

---

### Cross-Gate Execution Sequence

1. **S8 first:** sanitize all leaking responses to reduce immediate disclosure risk.
2. **S6 second:** enforce fail-closed root policy while preserving temp upload flow.
3. **Version alignment third:** synchronize artifacts and add mismatch guard.
4. Run QA + Security gate validation before release tagging.

### Handoff Checklist

#### Execution Agent
- Implement RG-S8 sanitization sweep first, then RG-S6 fail-closed update, then RG-V1 version sync.
- Add/adjust tests for S6 path matrix and S8 response sanitization.
- Do not change endpoint routes or success payload contracts unless required for security correctness.

#### QA Agent
- Execute targeted matrix:
  - S6 allow/deny path cases (root, temp, outside root, no-root-config).
  - S8 negative-path tests proving no exception text leaks.
  - Version consistency checks across API/version endpoint, assembly metadata, and container label.
- Record evidence in `docs/reports/REPORT_QA_AGENT.md`.

#### Security Agent
- Re-verify S6 for traversal/prefix-bypass resistance.
- Re-scan all API error bodies for leaked internals.
- Confirm release gate closure state and update `docs/reports/REPORT_SECURITY_AGENT.md`.

### Risk Flags for Execution
- S6 fail-closed behavior can change dev environments with no configured library roots; keep temp allowlist explicit and test it.
- S8 sanitization can break UI flows that currently depend on detailed error text; preserve user-safe business errors where needed.
- Version policy adoption requires discipline in release workflow; add an automated mismatch check to avoid regressions.

**File:** `Services/ServarrRootFolder.cs` (unified DTO, replaces both `SonarrRootFolder` and `RadarrRootFolder`)

```csharp
namespace Optimarr.Services
{
	public class ServarrRootFolder
	{
		public int Id { get; set; }
		public string Path { get; set; } = string.Empty;
		public bool Accessible { get; set; }
		public long? FreeSpace { get; set; }
	}
}
```

**File:** `Services/ServarrServiceBase.cs`

```csharp
namespace Optimarr.Services
{
	public abstract class ServarrServiceBase : IServarrService
	{
		protected readonly IConfiguration _configuration;
		protected readonly HttpClient _httpClient;
		protected readonly ILogger _logger;
		protected bool _isEnabled;
		protected bool _isConnected;
		protected string? _apiKey;
		protected string? _baseUrl;
		protected string? _basicAuthUsername;
		protected string? _basicAuthPassword;

		// Subclasses declare "Sonarr" or "Radarr" — drives config key prefix
		protected abstract string ProviderName { get; }

		protected ServarrServiceBase(IConfiguration configuration, ILogger logger)
		{
			_configuration = configuration;
			_logger = logger;
			_httpClient = new HttpClient();
			_httpClient.Timeout = TimeSpan.FromMinutes(5);
			Initialize();
		}

		protected void Initialize()
		{
			_baseUrl = _configuration[$"Servarr:{ProviderName}:BaseUrl"];
			_apiKey = _configuration[$"Servarr:{ProviderName}:ApiKey"];
			_basicAuthUsername = _configuration[$"Servarr:{ProviderName}:BasicAuthUsername"];
			_basicAuthPassword = _configuration[$"Servarr:{ProviderName}:BasicAuthPassword"];
			_isEnabled = _configuration.GetValue<bool>($"Servarr:{ProviderName}:Enabled")
				&& !string.IsNullOrEmpty(_baseUrl)
				&& !string.IsNullOrEmpty(_apiKey);

			if (_isEnabled)
			{
				_httpClient.BaseAddress = new Uri(_baseUrl!);
				_httpClient.DefaultRequestHeaders.Clear();
				_httpClient.DefaultRequestHeaders.Add("X-Api-Key", _apiKey!);
				if (!string.IsNullOrEmpty(_basicAuthUsername) && !string.IsNullOrEmpty(_basicAuthPassword))
				{
					var authValue = Convert.ToBase64String(
						System.Text.Encoding.UTF8.GetBytes($"{_basicAuthUsername}:{_basicAuthPassword}"));
					_httpClient.DefaultRequestHeaders.Authorization =
						new System.Net.Http.Headers.AuthenticationHeaderValue("Basic", authValue);
				}
				_httpClient.DefaultRequestHeaders.Accept.Add(
					new System.Net.Http.Headers.MediaTypeWithQualityHeaderValue("application/json"));
				CheckConnection();
			}
			else
			{
				_isConnected = false;
				_httpClient.BaseAddress = null;
				_httpClient.DefaultRequestHeaders.Clear();
			}
		}

		public void ReloadConfiguration() => Initialize();

		public bool IsEnabled => _isEnabled;
		public bool IsConnected => _isConnected;

		protected void EnsureApiKeyHeader()
		{
			if (!string.IsNullOrEmpty(_apiKey) && !_httpClient.DefaultRequestHeaders.Contains("X-Api-Key"))
				_httpClient.DefaultRequestHeaders.Add("X-Api-Key", _apiKey);
		}

		private void CheckConnection()
		{
			try
			{
				EnsureApiKeyHeader();
				// NOTE: CheckConnection is called from Initialize() which runs synchronously at startup.
				// This is the one place .Result is intentionally kept — see risk flags.
				var response = _httpClient.GetAsync("/api/v3/system/status").GetAwaiter().GetResult();
				_isConnected = response.IsSuccessStatusCode;
				if (!_isConnected)
					_logger.LogWarning("{Provider} connection check failed: {StatusCode}", ProviderName, response.StatusCode);
			}
			catch (Exception ex)
			{
				_isConnected = false;
				_logger.LogWarning(ex, "{Provider} connection check failed", ProviderName);
			}
		}

		public string? GetVersion()
		{
			if (!_isEnabled) return null;
			try
			{
				EnsureApiKeyHeader();
				var response = _httpClient.GetAsync("/api/v3/system/status").GetAwaiter().GetResult();
				if (response.IsSuccessStatusCode)
				{
					var content = response.Content.ReadAsStringAsync().GetAwaiter().GetResult();
					var status = System.Text.Json.JsonSerializer.Deserialize<System.Text.Json.JsonElement>(content);
					return status.GetProperty("version").GetString();
				}
				if (response.StatusCode == System.Net.HttpStatusCode.Unauthorized)
				{
					_logger.LogWarning("Unauthorized access to {Provider} system status. Check API key.", ProviderName);
					_isConnected = false;
				}
			}
			catch (Exception ex)
			{
				_logger.LogError(ex, "Error getting {Provider} version", ProviderName);
				_isConnected = false;
			}
			return null;
		}

		public async Task<List<ServarrRootFolder>> GetRootFoldersAsync()
		{
			if (!_isEnabled || !_isConnected) return new List<ServarrRootFolder>();
			EnsureApiKeyHeader();
			try
			{
				var response = await _httpClient.GetAsync("/api/v3/rootFolder");
				if (response.IsSuccessStatusCode)
				{
					var content = await response.Content.ReadAsStringAsync();
					var rootFolders = System.Text.Json.JsonSerializer.Deserialize<List<ServarrRootFolder>>(
						content, new System.Text.Json.JsonSerializerOptions { PropertyNameCaseInsensitive = true });
					return rootFolders ?? new List<ServarrRootFolder>();
				}
			}
			catch (Exception ex)
			{
				_logger.LogError(ex, "Error getting {Provider} root folders", ProviderName);
			}
			return new List<ServarrRootFolder>();
		}
	}
}
```

**`SonarrService` after refactor:**

```csharp
public class SonarrService : ServarrServiceBase
{
    protected override string ProviderName => "Sonarr";

    public SonarrService(IConfiguration configuration, ILogger<SonarrService> logger)
        : base(configuration, logger) { }

    // Keep ONLY Sonarr-specific methods:
    // GetSeries(), GetEpisode(), GetEpisodesBySeries(), GetEpisodeFile(),
    // GetAllEpisodeFiles(), FindEpisodeByPath(), DeleteEpisodeFile(), TriggerEpisodeSearch()

    // DELETE: Initialize(), ReloadConfiguration(), EnsureApiKeyHeader(),
    //         CheckConnection(), GetVersion(), GetRootFolders(), NormalizePath()
    //         (NormalizePath → use PathNormalizer.Normalize after A4)
}
```

**`RadarrService` after refactor:**

```csharp
public class RadarrService : ServarrServiceBase
{
    protected override string ProviderName => "Radarr";

    public RadarrService(IConfiguration configuration, ILogger<RadarrService> logger)
        : base(configuration, logger) { }

    // Keep ONLY Radarr-specific methods:
    // GetMovie(), GetMovies(), FindMovieByPath(), DeleteMovieFile(), TriggerMovieSearch()

    // DELETE: same shared methods as above
}
```

### Migration Plan for `ServarrSyncService`

- Change `GetRootFolders()` call on both services to `GetRootFoldersAsync()` (method rename + await)
- Change `SonarrRootFolder` → `ServarrRootFolder`, `RadarrRootFolder` → `ServarrRootFolder`
- Keep concrete service injection (`SonarrService`, `RadarrService`) — `IServarrService` injection deferred to E10

### Risk Flags

1. **`GetVersion()` and `CheckConnection()` use `.GetAwaiter().GetResult()`** — these are called synchronously from `Initialize()` which runs in the constructor. This is acceptable in ASP.NET Core because it's called at startup (before request processing). E4 must NOT change `Initialize()` to async — it would break the DI constructor pattern. Document this exception in code comments.
2. **`ReloadConfiguration()` thread safety** — if called during an in-flight request, `_httpClient.BaseAddress` changes mid-use (see D4). The current design does not solve this. Flag to Architect for a follow-up `SemaphoreSlim` design.
3. **DTO unification** — deleting `SonarrRootFolder` and `RadarrRootFolder` is a breaking change to these classes' public API. Check all usages before deleting.

### Files Created/Modified

| Action | File |
|--------|------|
| CREATE | `Services/IServarrService.cs` |
| CREATE | `Services/ServarrServiceBase.cs` |
| CREATE | `Services/ServarrRootFolder.cs` |
| MODIFY | `Services/SonarrService.cs` (inherit base, delete shared methods, rename GetRootFolders→GetRootFoldersAsync, delete SonarrRootFolder class) |
| MODIFY | `Services/RadarrService.cs` (same, delete RadarrRootFolder class) |
| MODIFY | `Services/ServarrSyncService.cs` (update type refs, await GetRootFoldersAsync) |
| MODIFY | `Program.cs` (no change needed — DI registration stays as concrete types until E10) |

---

## A2 — `ServarrController` Split (3 Controllers)

### Finding

`ServarrController.cs` is 890 lines containing 17 action methods across three unrelated concerns. The controller also directly injects 8 dependencies, most of which are only used by 1–2 methods.

### Decision

Split into **3 controllers**, all under `[Route("api/servarr")]` to preserve the existing URL surface exactly. The `[controller]` token must NOT be used — use the literal route string.

**Option A (chosen):** Explicit `[Route("api/servarr")]` on all 3 controllers.
**Option B (rejected):** Sub-routes like `api/servarr-status` — breaks all existing frontend JS calls.

### Controller Responsibility Matrix

#### `ServarrStatusController` — `Controllers/ServarrStatusController.cs`

Route: `[Route("api/servarr")]`

| HTTP | Path | Action | Services Needed |
|------|------|--------|----------------|
| GET | `/api/servarr/status` | `GetStatus()` | SonarrService, RadarrService |
| POST | `/api/servarr/sonarr/test` | `TestSonarrConnection()` | None (creates own HttpClient) |
| POST | `/api/servarr/radarr/test` | `TestRadarrConnection()` | None (creates own HttpClient) |
| POST | `/api/servarr/sonarr/analyze-series/{id}` | `AnalyzeSonarrSeries()` | SonarrService |
| POST | `/api/servarr/radarr/analyze-movie/{id}` | `AnalyzeRadarrMovie()` | RadarrService |

DI: `SonarrService`, `RadarrService`, `ILogger<ServarrStatusController>`

#### `ServarrSettingsController` — `Controllers/ServarrSettingsController.cs`

Route: `[Route("api/servarr")]`

| HTTP | Path | Action | Services Needed |
|------|------|--------|----------------|
| GET | `/api/servarr/sonarr/settings` | `GetSonarrSettings()` | IConfiguration |
| POST | `/api/servarr/sonarr/settings` | `SaveSonarrSettings()` | SonarrService, IConfiguration, IWebHostEnvironment |
| GET | `/api/servarr/radarr/settings` | `GetRadarrSettings()` | IConfiguration |
| POST | `/api/servarr/radarr/settings` | `SaveRadarrSettings()` | RadarrService, IConfiguration, IWebHostEnvironment |
| GET | `/api/servarr/sonarr/path-mappings` | `GetSonarrPathMappings()` | IConfiguration |
| POST | `/api/servarr/sonarr/path-mappings` | `SaveSonarrPathMappings()` | IConfiguration, IWebHostEnvironment |
| GET | `/api/servarr/radarr/path-mappings` | `GetRadarrPathMappings()` | IConfiguration |
| POST | `/api/servarr/radarr/path-mappings` | `SaveRadarrPathMappings()` | IConfiguration, IWebHostEnvironment |

DI: `SonarrService`, `RadarrService`, `ILogger<ServarrSettingsController>`, `IConfiguration`, `IWebHostEnvironment`

#### `ServarrSyncController` — `Controllers/ServarrSyncController.cs`

Route: `[Route("api/servarr")]`

| HTTP | Path | Action | Services Needed |
|------|------|--------|----------------|
| POST | `/api/servarr/radarr/sync` | `SyncRadarr()` | ServarrSyncService |
| POST | `/api/servarr/sonarr/sync` | `SyncSonarr()` | ServarrSyncService |
| POST | `/api/servarr/sync-all` | `SyncAll()` | ServarrSyncService |
| POST | `/api/servarr/match-videos` | `MatchVideosWithServarr()` | SonarrService, RadarrService, VideoMatchingProgressService, IServiceProvider |
| GET | `/api/servarr/match-videos/progress/{id}` | `GetMatchProgress()` | VideoMatchingProgressService |
| GET | `/api/servarr/match-videos/active` | `GetActiveMatches()` | VideoMatchingProgressService |
| POST | `/api/servarr/match-videos/{pathId}` | `MatchVideosForLibraryPath()` | VideoServarrMatcherService |

DI: `SonarrService`, `RadarrService`, `ServarrSyncService`, `VideoServarrMatcherService`, `VideoMatchingProgressService`, `ILogger<ServarrSyncController>`, `IServiceProvider`

### DTO Migration

The DTOs currently defined inside `ServarrController.cs` must move to a shared location:

**File:** `Controllers/ServarrDtos.cs` (new file in Controllers folder)

Move these types:
- `TestConnectionRequest`
- `ServarrStatusResponse`
- `ServiceStatus`
- `ServarrSettingsRequest`
- `PathMappingRequest`

Keep namespace `Optimarr.Controllers`. This avoids polluting the Models layer with HTTP-contract types.

### Risk Flags

1. **Route collision risk** — Three controllers all using `[Route("api/servarr")]`. ASP.NET Core resolves this by action method HTTP verb and path template; no ambiguity exists since all 17 action paths are unique. Verify with a routing test.
2. **Original controller deletion** — Do NOT delete `ServarrController.cs` until all 3 replacement controllers compile and routing is verified.
3. **`IServiceProvider` injection** — `ServarrSyncController` injects `IServiceProvider` (used in `MatchVideosWithServarr` to create a scoped DI scope for background work). This violates the SERVICE_LOCATOR anti-pattern but is acceptable here because the intent is to create a fresh scope for a background `Task.Run`. Document the exception in a code comment.

### Files Created/Modified

| Action | File |
|--------|------|
| CREATE | `Controllers/ServarrStatusController.cs` |
| CREATE | `Controllers/ServarrSettingsController.cs` |
| CREATE | `Controllers/ServarrSyncController.cs` |
| CREATE | `Controllers/ServarrDtos.cs` |
| DELETE | `Controllers/ServarrController.cs` (after all 3 new controllers verified) |

---

## A3 — `VideoServarrMatcherService` Decomposition

### Finding

`VideoServarrMatcherService.cs` is 857 lines. The two public matching methods share ~80% logic:

| Logic Block | `MatchAllVideosAsync()` | `MatchVideosForLibraryPathAsync()` |
|-------------|------------------------|-----------------------------------|
| Preload Sonarr data | Lines 225–319 | Lines 577–656 |
| Preload Radarr data | Lines 321–367 | Lines 658–694 |
| Parallel batch loop | Lines 397–524 | Lines 705–800 |
| Progress tracking | Yes | No |

### Decision

**Option A (chosen):** Extract `ServarrDataCacheService` + private `MatchVideoBatchAsync()`.
**Option B (rejected):** Full service decomposition into 4 classes — over-engineered for current scale.

### New Class: `ServarrDataCacheService`

**File:** `Services/ServarrDataCacheService.cs`
**DI Lifetime:** Scoped (data is point-in-time; each background scope gets a fresh instance)

```csharp
namespace Optimarr.Services
{
	// Value object returned by BuildCacheAsync — immutable after construction
	public sealed class ServarrDataCache
	{
		public Dictionary<string, SonarrEpisode>? EpisodesByPath { get; init; }
		public Dictionary<string, SonarrEpisode>? EpisodesByTitle { get; init; }
		public Dictionary<int, SonarrSeries>? SeriesById { get; init; }
		public Dictionary<string, RadarrMovie>? MoviesByPath { get; init; }
		public Dictionary<string, RadarrMovie>? MoviesByTitle { get; init; }
	}

	public class ServarrDataCacheService
	{
		private readonly SonarrService _sonarrService;
		private readonly RadarrService _radarrService;
		private readonly ILogger<ServarrDataCacheService> _logger;

		public ServarrDataCacheService(
			SonarrService sonarrService,
			RadarrService radarrService,
			ILogger<ServarrDataCacheService> logger)
		{ ... }

		// Loads all Sonarr episodes + Radarr movies into lookup dictionaries.
		// Returns an immutable cache object. Logs warnings on partial failures
		// (e.g., Sonarr unavailable → EpisodesByPath = null, Radarr data still loaded).
		public async Task<ServarrDataCache> BuildCacheAsync() { ... }
	}
}
```

**`BuildCacheAsync()` responsibilities:**
1. Check `_sonarrService.IsEnabled && IsConnected` → if yes, load series + episode files in parallel, build `EpisodesByPath` and `EpisodesByTitle` dictionaries
2. Check `_radarrService.IsEnabled && IsConnected` → if yes, load movies, build `MoviesByPath` and `MoviesByTitle` dictionaries
3. Return a `ServarrDataCache` with any non-loaded fields set to `null`
4. Log progress at each stage at `Information` level

### Simplified `VideoServarrMatcherService`

Remove the embedded preload blocks. Add:

```csharp
private async Task<int> MatchVideoBatchAsync(
    IList<VideoAnalysis> batch,
    ServarrDataCache cache,
    SemaphoreSlim semaphore,
    CounterState counters,   // small record holding matchedCount, processedCount, errorCount
    int totalVideos,
    VideoMatchingProgressService? progressService,
    string? matchId)
{ ... }
```

`MatchAllVideosAsync()` becomes:
1. Count total videos (DB query)
2. Call `_cacheService.BuildCacheAsync()` — one call, shared result
3. Paginated batch loop → calls `MatchVideoBatchAsync()` per batch
4. Final `SaveChangesAsync()`

`MatchVideosForLibraryPathAsync()` becomes:
1. Find library path (DB query)
2. Count scoped videos (DB query)
3. Call `_cacheService.BuildCacheAsync()` — same cache service
4. Paginated batch loop (filtered by library path) → calls `MatchVideoBatchAsync()`
5. Final `SaveChangesAsync()`

### Data Flow Diagram

```
MatchAllVideosAsync()          MatchVideosForLibraryPathAsync()
         │                                    │
         ▼                                    ▼
ServarrDataCacheService.BuildCacheAsync()
         │
         ├── SonarrService.GetSeries()
         ├── SonarrService.GetAllEpisodeFiles()
         ├── SonarrService.GetEpisodesBySeries() [parallel per series]
         └── RadarrService.GetMovies()
         │
         ▼ ServarrDataCache (immutable)
         │
         ▼
MatchVideoBatchAsync(batch, cache, ...)
         │
         ├── MatchVideoWithServarrAsync(video, cache...)  [parallel per video]
         └── _dbContext.SaveChangesAsync() [per batch]
```

### Risk Flags

1. `ServarrDataCacheService` must be **Scoped** (not Singleton). Verify DI registration in `Program.cs`. A Singleton would serve stale data across all subsequent operations.
2. `MatchVideoWithServarrAsync()` signature changes — it currently takes 5 nullable dict params. After refactor, pass `ServarrDataCache cache` instead. Verify all callers.
3. `CounterState` for progress tracking — use a simple `record` or a small inner class with `volatile int` or `Interlocked` for thread-safe increment. Do not use bare `int` + `lock` on a boxed value.

### Files Created/Modified

| Action | File |
|--------|------|
| CREATE | `Services/ServarrDataCacheService.cs` |
| MODIFY | `Services/VideoServarrMatcherService.cs` (remove preload blocks, add `_cacheService` dep, extract `MatchVideoBatchAsync()`, simplify both public methods) |
| MODIFY | `Program.cs` (register `ServarrDataCacheService` as Scoped) |

---

## A4 — `PathNormalizer` Static Helper

### Finding

`NormalizePath()` is duplicated identically in 4 files:

```csharp
// Identical in all 4 files:
private string NormalizePath(string path)
{
    if (string.IsNullOrEmpty(path)) return string.Empty;
    return path.Replace('\\', '/').TrimEnd('/').ToLowerInvariant();
}
```

Files: `RadarrService.cs:319`, `SonarrService.cs:433`, `VideoServarrMatcherService.cs:810`, `ServarrSyncService.cs:393`

### Decision

Extract to `Helpers/PathNormalizer.cs` as a `static class`. Add URL-decoding to satisfy A4's requirement (handle URL-encoded paths).

**Option A (chosen):** Static class, `static string Normalize(string? path)` — no DI registration needed, callable anywhere.
**Option B (rejected):** Instance service — overkill for a pure, stateless string transformation.

### Canonical Implementation

**File:** `Helpers/PathNormalizer.cs`

```csharp
namespace Optimarr.Helpers
{
	/// <summary>
	/// Canonical path normalization for Servarr path comparison.
	/// Handles: Windows backslashes, trailing slashes, URL-encoded characters.
	/// </summary>
	public static class PathNormalizer
	{
		public static string Normalize(string? path)
		{
			if (string.IsNullOrEmpty(path)) return string.Empty;

			// Decode URL-encoded characters (e.g., %20 → space, %2F → /)
			// Uri.UnescapeDataString is safe for already-decoded strings (returns unchanged)
			var decoded = Uri.UnescapeDataString(path);

			// Normalize: forward slashes, strip trailing slash, lowercase
			return decoded.Replace('\\', '/').TrimEnd('/').ToLowerInvariant();
		}
	}
}
```

### Migration Plan

For each of the 4 files:
1. Add `using Optimarr.Helpers;` to the top
2. Replace all calls to `NormalizePath(x)` with `PathNormalizer.Normalize(x)`
3. Delete the `private string NormalizePath(string path)` method

`ServarrSyncService.MapPath()` also calls `NormalizePath()` internally — update those calls too.

**Edge case table for QA (Q2):**

| Input | Expected Output |
|-------|----------------|
| `null` | `""` |
| `""` | `""` |
| `"C:\Movies\Film.mkv"` | `"c:/movies/film.mkv"` |
| `"/media/movies/"` | `"/media/movies"` |
| `"/media//movies"` | `"/media//movies"` *(no double-slash normalization — not needed)* |
| `"/media/My%20Movies"` | `"/media/my movies"` |
| `"\\server\share\file.mkv"` | `"//server/share/file.mkv"` |

### Risk Flags

1. **URL-decoding behavior change** — `NormalizePath()` currently does NOT decode. Adding `Uri.UnescapeDataString` changes comparison behavior for paths with `%20` etc. This is intentionally additive and safe, but must be flagged in the PR and covered by Q2 tests.
2. **`NormalizeTitle()` in `VideoServarrMatcherService`** — this is a different operation (filename-without-extension extraction) and must NOT be moved to `PathNormalizer`. Leave it in the service.
3. **Do NOT create the `/Helpers` directory without creating the file** — check if directory exists with `ls Helpers/` first; create if absent.

### Files Created/Modified

| Action | File |
|--------|------|
| CREATE | `Helpers/PathNormalizer.cs` |
| MODIFY | `Services/RadarrService.cs` |
| MODIFY | `Services/SonarrService.cs` |
| MODIFY | `Services/VideoServarrMatcherService.cs` |
| MODIFY | `Services/ServarrSyncService.cs` |

---

## A5 — Authentication Strategy (ADR)

### Architectural Decision Record: Optimarr API Authentication

**Status:** Proposed — awaiting user approval before implementation (E10 / S7)

**Context:** Optimarr exposes a full REST API at `/api/*` with zero authentication. Any process or user on the network can read settings (including masked API keys), trigger syncs, and modify path mappings. The Security Agent flagged this as HIGH priority (S7).

**Constraints from PROJECT_CONTEXT.md:**
- No new NuGet packages without approval
- Must be operable without a separate identity provider
- Frontend is Vanilla JS — no npm, no frameworks

---

**Options Evaluated:**

| | X-Api-Key Header | HTTP Basic Auth | JWT Bearer |
|--|--|--|--|
| Stateless | ✅ | ✅ | ✅ |
| No login UI needed | ✅ | ✅ (browser popup) | ❌ |
| Frontend JS support | ✅ fetch() header | ⚠️ fetch() with credentials | ❌ needs token endpoint |
| Self-hosted familiarity | ✅ (same as Sonarr/Radarr/Prowlarr) | ✅ | ❌ |
| Implementation complexity | Low | Low | High |
| New NuGet packages | None | None | Required |

---

**Decision: X-Api-Key header via `X-Optimarr-Key`**

Rationale: The target user is a self-hoster who already manages Sonarr/Radarr API keys. This pattern is immediately familiar. It requires no login page, no session state, no new dependencies. The JS frontend adds one header to every fetch() call.

---

**Implementation Design:**

**1. Configuration**

Add to `appsettings.json` (and document in `config/README.md`):
```json
{
  "Auth": {
    "ApiKey": ""
  }
}
```

If `Auth:ApiKey` is empty string or absent → authentication is disabled (backward-compatible default). Existing deployments continue to work without configuration changes.

**2. Middleware**

**File:** `Middleware/ApiKeyMiddleware.cs`

```csharp
namespace Optimarr.Middleware
{
	public class ApiKeyMiddleware
	{
		private readonly RequestDelegate _next;
		private readonly IConfiguration _configuration;
		private const string ApiKeyHeaderName = "X-Optimarr-Key";
		private const string HealthCheckPath = "/api/system/health";

		public ApiKeyMiddleware(RequestDelegate next, IConfiguration configuration)
		{
			_next = next;
			_configuration = configuration;
		}

		public async Task InvokeAsync(HttpContext context)
		{
			var configuredKey = _configuration["Auth:ApiKey"];

			// Auth disabled if no key configured
			if (string.IsNullOrEmpty(configuredKey))
			{
				await _next(context);
				return;
			}

			// Health check is always public (required for Docker health checks)
			if (context.Request.Path.StartsWithSegments(HealthCheckPath))
			{
				await _next(context);
				return;
			}

			// Only protect /api/* routes
			if (!context.Request.Path.StartsWithSegments("/api"))
			{
				await _next(context);
				return;
			}

			if (!context.Request.Headers.TryGetValue(ApiKeyHeaderName, out var providedKey)
				|| !string.Equals(configuredKey, providedKey, StringComparison.Ordinal))
			{
				context.Response.StatusCode = StatusCodes.Status401Unauthorized;
				context.Response.ContentType = "application/json";
				await context.Response.WriteAsync("{\"error\":\"Unauthorized\"}");
				return;
			}

			await _next(context);
		}
	}
}
```

**3. Registration in `Program.cs`**

```csharp
// Add BEFORE app.UseRouting():
app.UseMiddleware<ApiKeyMiddleware>();
```

**4. Frontend changes**

All `fetch()` calls in `wwwroot/js/*.js` must add the header. Add a shared helper in a new `wwwroot/js/api.js` module (or update the existing API module):

```javascript
// Get configured API key from a meta tag or a dedicated /api/system/auth-config endpoint
// Recommended: expose Auth:ApiKey presence (not value) via GET /api/system/auth-required → { authRequired: true/false }
// Store key in sessionStorage (user enters it once on first load if required)
export function getAuthHeaders() {
    const key = sessionStorage.getItem('optimarr-api-key');
    return key ? { 'X-Optimarr-Key': key } : {};
}
```

**Note:** Frontend auth UX design is out of scope for this architectural task. A minimal implementation: if the first API call returns 401, prompt the user for the key and store in sessionStorage. This is a separate frontend task.

**5. Excluded from authentication:**
- `GET /api/system/health` — must remain public for Docker health checks
- `wwwroot/` static files — not under `/api/`, not protected
- `GET /api/system/auth-config` (new endpoint, optional) — returns `{ authRequired: bool }` so the frontend knows whether to prompt for a key

---

**Risk Flags:**

1. **Key in HTTP headers** — safe over HTTPS, problematic over plain HTTP. For self-hosted LAN use this is acceptable. Document in config/README.md that HTTPS is strongly recommended when Auth is enabled.
2. **CORS interaction** — if S3 is implemented (restricted CORS), the `X-Optimarr-Key` header must be listed in `Access-Control-Allow-Headers`. Add it to the CORS policy in `Program.cs`.
3. **No rate limiting** — brute-force of API key is theoretically possible. Acceptable risk for self-hosted LAN tool. Document this limitation.
4. **Frontend UX** — this plan covers only backend middleware. Frontend key entry/storage requires a separate design decision (sessionStorage vs. cookie vs. env-var-injected meta tag).

---

## A6 — Memory-Efficient Episode/Movie Caching Strategy

### Finding (from D3 investigation basis)

`MatchAllVideosAsync()` builds 5 in-memory dictionaries from ALL Sonarr episodes and Radarr movies before processing any videos. Estimated peak memory:

| Library Size | Episodes Dict (×2) | Series Dict | Movies Dict (×2) | API JSON | **Peak** |
|-------------|-------------------|-------------|-----------------|----------|---------|
| 10K episodes, 2K movies | ~25 MB | ~4 MB | ~8 MB | ~30 MB | **~67 MB** |
| 50K episodes, 10K movies | ~125 MB | ~20 MB | ~40 MB | ~150 MB | **~335 MB** |
| 100K episodes, 30K movies | ~250 MB | ~40 MB | ~120 MB | ~300 MB | **~710 MB** |

At 100K+ episodes the current approach risks OOM in a constrained Docker container (default 512MB).

### Options Evaluated

**Option 1: Stream-process per root folder (chunked loading)**
- Load episodes only for series in one root folder at a time
- Process those videos immediately, then discard the dict
- Memory: O(one-root-folder) instead of O(all-series)
- Pros: Dramatic memory reduction for large libraries
- Cons: Multiple API roundtrips; requires knowing which series belong to which root folder (needs `rootFolderId` on SonarrSeries — not currently present in the SonarrSeries DTO)
- Implementation complexity: High (requires DTO change + logic restructure)

**Option 2: Paginated API calls + rolling dictionary**
- Sonarr v3 does NOT support server-side pagination on `/episode` — must call per-series
- Radarr v3 `/api/v3/movie` returns all movies (no pagination)
- Can evict entries from dict after matching their videos — reduces peak, not initial load
- Complexity: High; partial benefit
- **Rejected** for current scope

**Option 3: EF Core index-based lookup (local DB cache)**
- Store episode+movie paths in a `ServarrEpisodeCache` table (new migration required)
- Match against local DB with indexed lookup instead of in-memory dict
- Memory: O(video-batch-size) — near-zero for the dictionary
- Speed: Depends on SQLite index performance; should be fast with proper index
- Pros: Most memory-efficient long-term; eliminates repeated API calls on incremental runs
- Cons: Requires a new migration, cache invalidation strategy, background refresh job
- **Deferred to v1.3** — significant scope increase

### Decision: No immediate structural change; establish guardrails

**For current v1.2.x:** The existing dict-based approach is adequate for the realistic user profile (≤20K episodes, ≤5K movies ≈ ~67–100 MB peak). The immediate actions are:
1. **E8 (already planned):** Add `.AsNoTracking()` to reduce EF Core change-tracking memory
2. **E9 (already planned):** Make concurrency values configurable — allow operators to tune down `SemaphoreSlim` values to reduce concurrent memory

**Scaling ceiling (document in config/README.md):**
- Libraries >50K episodes: recommend tuning `MatchingOptions:SeriesConcurrency` down to 5 to reduce peak memory
- Libraries >100K episodes: trigger Option 1 (chunked loading) — file an issue before implementation

**Option 1 pre-design (for v1.3, not implemented now):**

When implemented, the root-folder-chunked approach requires:
1. Add `RootFolderId` to `SonarrSeries` DTO — available from Sonarr API response
2. `ServarrDataCacheService.BuildCacheForRootFolderAsync(int rootFolderId)` — loads only episodes for series under that root folder
3. `MatchAllVideosAsync()` outer loop becomes: for each root folder → build cache → match videos in that folder → discard cache
4. A3 (decomposition) is a prerequisite — this is why A3 is designed now even though Option 1 is deferred

### Risk Flags

1. **The current approach works for the majority of self-hosted users.** Do not optimize prematurely. Memory pressure should be confirmed via D3 investigation before committing to Option 1.
2. **Option 3 (DB cache) is the most maintainable long-term** but requires careful cache invalidation design. Flag for v1.3 planning.
3. **A3 (decomposition into `ServarrDataCacheService`) is a prerequisite** for any future caching strategy change — ensure A3 is implemented before any v1.3 caching work begins.

### Files Created/Modified

| Action | File | Notes |
|--------|------|-------|
| MODIFY | `config/README.md` | Document scaling ceiling + `MatchingOptions` config keys |
| No structural code changes for A6 in current scope | | |

---

## Execution Order for Architect-Dependent Tasks

```
A4 → E7 (PathNormalizer implementation) → Q2 (tests)
A1 → E10 (interface implementation)
A2 → E5, E6 (duplicate extraction within new controller structure)
A3 → RF2, RF3 (refactor passes after decomposition)
A5 → S7 (security implementation of auth middleware)
A6 → (no immediate execution task — monitor via D3)
```

---

## Handoff Summary

**From Agent:** Architect Agent (Agent 2)
**To Agent:** Execution Agent (Agent 3)

**Tasks Completed:**
- A1: `IServarrService` interface + `ServarrServiceBase` abstract class + `ServarrRootFolder` unified DTO fully designed
- A2: Three-controller split with explicit routes, responsibility matrix, and DTO migration plan
- A3: `ServarrDataCacheService` extraction + `MatchVideoBatchAsync()` private method design with data flow diagram
- A4: `PathNormalizer.Normalize()` canonical implementation including URL-decoding, edge case table for QA
- A5: API key header auth ADR with middleware design, configuration, and frontend notes
- A6: Memory scaling analysis, current-state assessment, Option 1 pre-design for v1.3

**Artifacts Produced:**
- This file: `docs/PLAN.md`

**Open Issues (Execution Agent must handle):**
- A1/E10: `ReloadConfiguration()` thread safety (D4) is NOT solved by this design — flag as a separate follow-up
- A2/E5+E6: Duplicate settings-save logic is within `ServarrSettingsController` — the extraction (private `SaveServarrSettingsAsync()` + `SaveServarrPathMappingsAsync()`) should be done within that controller, not the original
- A5: Frontend key entry/storage UX is not designed — Execution Agent must decide minimal implementation (sessionStorage prompt) or escalate back to Architect

**Constraints to Note:**
- No `.Result` or `.Wait()` in new code (exception: `GetVersion()` and `CheckConnection()` in base class constructor chain — documented above)
- All new async methods must accept `CancellationToken` where operations are long-running (backlog — add in refactor pass)
- `ServarrDataCacheService` **must** be registered as **Scoped** in `Program.cs`
- All three new controllers use `[Route("api/servarr")]` explicitly — never `[Route("api/[controller]")]`
- Tabs for indentation, PascalCase methods/classes, `_camelCase` private fields
```
