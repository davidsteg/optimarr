# Changelog

All notable changes to Optimarr are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and versions follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html). Each `## [x.y.z]` section becomes the
notes of the GitHub Release for tag `vx.y.z`.

## [2.0.1] - 2026-10-17

### Fixed

- The dashboard and every page that loads it failed with "mismatched types; Rust type `f64` … is
  not compatible with SQL type `INTEGER`" while no media had been scanned yet, for example right
  after upgrading from 1.x or on a fresh install.

## [2.0.0] - 2026-10-13

Optimarr 2 is a complete rewrite in Rust with a new web UI. It is a single binary with the UI
built in, keeps its settings in the database, and imports your 1.x settings and analysis on first
start.

### Upgrading from 1.x

1. Switch the image to `davidsteg/optimarr:2.0.0` and keep your `data` and `config`
   volumes. The port inside the container is still 8080.
2. On first start, `config/appsettings.json` and `data/optimarr.db` are imported into a new
   `data/optimarr2.db`. The old files are opened read-only and left in place, so you can roll back.
3. Mount your media at the same paths as before so imported results still match.
4. Find the API key under **Settings → Security** (or set `OPTIMARR_API_KEY`). Agents and scripts
   now send it as `X-Api-Key`.

### Breaking changes

- The REST API is new. Scripts using 1.x endpoints (`/api/library/*`, `/api/analysis/*`,
  `/api/servarr/*`) must move to the new endpoints or to the stable agent API in `docs/API.md`.
- Settings are no longer read from `appsettings.json` after the first import; edit them in the UI.
- The compatibility score is now 0–100 per file with configurable weights, and there is a separate
  0–100 quality score, so 1.x ratings and thresholds do not carry over one to one.
- The Docker image moved from `bridgemill/optimarr` to `davidsteg/optimarr` on Docker Hub, also
  published to GHCR as `ghcr.io/davidsteg/optimarr`. `PUID` and `PGID` set the user the server runs as.

### Added

- **Two scores per file.** Compatibility (will it direct-play?) and quality (bitrate per pixel,
  resolution, codec, HDR, audio), each with issues, a fix for every issue, and one ffmpeg command
  that resolves all fixable compatibility problems.
- **Per-device playback profiles.** Describe each client (templates for web browsers, Apple TV,
  Android TV / Shield and smart TVs). Every file shows direct play, remux or transcode per device,
  and the Improve page totals it per device with the most common reasons.
- **Fix queue.** Queue the suggested ffmpeg fix for one or many files. A worker runs them one at a
  time, swaps the result in (keeping the original as `.orig` by default), re-analyzes it and asks
  Sonarr/Radarr to rescan. Off by default; needs writable media folders.
- **Upgrade finder.** Lists low-quality files linked to Sonarr or Radarr and asks them to search for
  a better release without deleting the current file, by hand or automatically on a schedule.
- **Disk savings estimate.** How much space re-encoding to HEVC or AV1 would save, per file and in
  total, with the biggest wins first.
- **Language checks.** Preferred audio and subtitle languages are checked, and subtitles that only
  exist as images (PGS, VobSub) are flagged because they force a transcode.
- **Notifications.** Discord, ntfy or any webhook, for new broken or poor files, finished or failed
  fixes, and automatic upgrade searches.
- **Agent API.** A small, stable JSON API under `/api/agent/` for AI agents and MCP servers: library
  overview, file search, per-file reports (by id, path or title), most-transcoded files, device
  summaries, savings, upgrade candidates, the fix queue, notifications, and rescan, upgrade, fix and
  redownload actions. Documented in `docs/API.md`.
- Incremental scans that skip unchanged files, a file watcher for new and changed files, periodic
  rescans for network mounts, and live progress in the browser.
- Jellyfin playback history (Playback Reporting plugin) with transcode reasons per client and user.
- Sonarr and Radarr: root folder sync, file matching, redownload, and import webhooks.
- GitHub Releases with an amd64 Docker image on Docker Hub and GHCR for every version tag.

### Changed

- Rewritten from ASP.NET to Rust (Axum, Tokio, SQLite) with a SvelteKit UI. The Docker image is a
  slim Debian image with MediaInfo and ffmpeg.
- API key auth is required for requests from outside your network by default (configurable).

### Removed

- The ASP.NET application, its jQuery UI, `appsettings.json` configuration and the 1.x docs.

### Security

- The 1.x repository contained `config/appsettings.json` with real Sonarr and Jellyfin API keys. The
  file is removed, but it remains in git history: regenerate those keys.

## [1.2.2] - 2026-03-02

### Added
- Added `docs/REQUESTS_TRACKER.md` as a single shared file for both bug reports and feature requests.

### Changed
- Cleaned up solved bug/request/task views for lower-noise active tracking.
  - `docs/REQUESTS_TRACKER.md` now shows only open bugs in the active section and moves solved bugs to `Archived Closed Bugs`.
  - `docs/TASKS.md` request-sync section now archives solved `RQ-*` details and keeps only active request/story direction.
  - Added `docs/reports/REPORT_ACTIVE_STATUS.md` as a concise active-items view while preserving full historical report files.
- Reconciled request-cycle wording across tracker/task docs after `RQ-Q1` completion.
  - Updated `docs/REQUESTS_TRACKER.md` bug rows to reflect QA-complete state (`RQ-Q1 ✅`) and removed stale "QA follow-up pending" language.
- Started Feature Pack sequence in task planning order.
  - Set kickoff state in `docs/TASKS.md`: `FP-BA1/FP-BA2` in progress, `FP-A1..FP-A4` queued next, and `FP-E*` queued after architecture.
  - Replaced stale RQ-focused execution-order block with Feature Pack phase flow (`FP-BA*` → `FP-A*` → `FP-E*` → review/test/stabilize/finalize).
- QA revalidated `RQ-E3` security behavior and request regression scope (`RQ-Q1`) on 2026-03-02.
  - Added regression assertion for permission-denied scan payload sanitization in `GetScan` failed-file response mapping.
  - Confirmed failed-file tracking remains intact in scan-status payloads (`FailedFiles` and `FailedFilesList` continuity).
  - Reconfirmed `RQ-E1` migration banner contract and `RQ-E2` broken-only filter strictness remain fixed.
  - Updated `docs/TASKS.md` to mark `RQ-Q1` final status as complete for targeted regression scope.
- Reconciled documentation wording after security reconciliation to remove status conflicts.
  - Standardized final request-cycle state across tracker docs:
    - implementation complete (`RQ-E1/E2/E3`),
    - security review complete (`RQ-R1`),
    - QA follow-up still partial (`RQ-Q1`).
  - Updated `docs/TASKS.md` release-gate block to explicitly distinguish:
    - release gate posture (`GO` with owner-accepted `S1` exception),
    - request-cycle security gate (`PASS`),
    - request-cycle QA gate (`PARTIAL`).
  - Updated `docs/REQUESTS_TRACKER.md` and `docs/STORY_TRACKING.md` wording to match the same final RQ outcome language and avoid completed-vs-not-approved ambiguity.
- Code Review Agent completed focused RQ-E3 security re-review (2026-03-02).
  - Confirmed failed-file/status API payloads are sanitized and do not expose raw exception details.
  - Confirmed server-side logging still retains diagnostic exception detail for investigation.
  - Confirmed permission-edge behavior is unchanged: permission-related analysis failures become failed scan items and do not mark media as broken by default.
  - Recommendation: `RQ-R1` set to `✅` (GO for QA on request-cycle scope).
- Closed RQ-E3 payload disclosure remediation path (2026-03-02).
  - `LibraryScannerService` now stores client-safe failed-file messages instead of raw `ex.Message` for scan failure records.
  - `LibraryController` scan-status response now maps failed-file messages to sanitized client-safe text, preventing exposure from legacy/raw records.
  - Permission-handling behavior remains unchanged: restricted files are treated as failed scan items and are not classified as broken media by default.
- Security Agent reconciled RQ-E3 signoff conflict in `docs/TASKS.md` (2026-03-02).
  - Confirmed live code still leaks raw exception text through failed-file API surfaces in `RQ-E3` path (`LibraryScannerService` persistence + `LibraryController` response mapping).
  - Reopened request-cycle task states for consistency:
    - `RQ-E3` -> `⚠️` (partial, remediation required),
    - `RQ-R1` -> `⚠️` (partial, re-review required),
    - `RQ-Q1` -> `⚠️` (partial, add/re-run sanitization regression test).
  - Added one unambiguous gate line in tasks: `RQ SECURITY GATE DECISION: FAIL`.
  - Existing owner-accepted `S1` exception remains unchanged.
- Security Agent final signoff review for request bug-fix cycle (`RQ-E1/E2/E3`) completed (2026-03-02).
  - `RQ-E1` (`wwwroot/js/migration.js`) and `RQ-E2` (`Controllers/LibraryController.cs`) show no new security regressions in reviewed paths.
  - New regression identified in `RQ-E3` path:
    - `Services/LibraryScannerService.cs` persists raw exception text (`ex.Message`) into failed-file records.
    - `Controllers/LibraryController.cs` returns `FailedFilesList.ErrorMessage` to clients in scan status responses.
  - Result: RQ-cycle security signoff is blocked until failed-file error payloads are sanitized (server logs can keep detailed exception text).
  - Existing owner-accepted `S1` exception posture remains unchanged and was not reopened.
- Performed an additional documentation consistency pass to enforce Request↔Story↔RQ linkage conventions.
  - Updated `docs/STORY_TRACKING.md` story template to require `Request Link` and `Related Task IDs` fields for new stories.
  - Updated `docs/REQUESTS_TRACKER.md` details templates to include `Related Story ID` and `Related RQ Task IDs`.
  - Updated `docs/TASKS.md` request/story sync block with a canonical linkage map table (`Request ID` -> `Story ID` -> `RQ Task IDs`) and corrected stale source-sync wording.
- Closed documentation sync tasks `RQ-DOC1` and `RQ-DOC2` and reconciled request/story/task traceability.
  - Updated `docs/REQUESTS_TRACKER.md` to include explicit `Story ID`, `Linked RQ Task IDs`, and `Evidence` columns for bug and feature request rows.
  - Updated request statuses/evidence for completed request-driven bug work (`BUG-20260301-01..03`) and open planned feature request (`FR-20260302-01`).
  - Updated `docs/STORY_TRACKING.md` so `Active Stories` contains only open work (`STORY-REQ-FEAT-001` for `FR-20260302-01`).
  - Moved request-driven bug stories into `Recent Stories` as completed entries (`STORY-REQ-BUG-001..003`) with explicit Request ID and RQ task links.
  - Updated `docs/TASKS.md` to mark `RQ-DOC1` and `RQ-DOC2` complete with evidence-backed wording.
- QA completed targeted `RQ-Q1` regression coverage for `RQ-E1/E2/E3` (2026-03-01).
  - Added `Optimarr.Tests/Regression/RequestRegressionTests.cs` with regression checks for:
    - migration-banner state contract (`RQ-E1`),
    - strict broken-only filtering in both paged video and ID-only paths (`RQ-E2`),
    - permission-edge behavior ensuring permission failures are rethrown and do not create broken-media records (`RQ-E3`).
  - Environment validation commands were attempted and blocked by missing SDK:
    - `dotnet restore`
    - `dotnet build -c Release`
    - `dotnet test`
  - Pending runtime validation command on SDK-enabled runner:
    - `dotnet test Optimarr.Tests/Optimarr.Tests.csproj --filter "RequestRegressionTests"`
- Code Review Agent completed `RQ-R1` review pass for `RQ-E1/E2/E3` (2026-03-01).
  - Result: **QA GO** for RQ scope (no blocking regressions found).
  - Confirmed `RQ-E1` migration banner behavior now suppresses completed/idle re-show on refresh while preserving active/error display.
  - Confirmed `RQ-E2` broken-only filtering remains strict after in-memory transform/recalculation stages in both paged video and ID-list flows.
  - Confirmed `RQ-E3` permission-related analysis failures are classified as failed scan items rather than forcing broken-media records.
- Completed request-driven bugfix tasks `RQ-E1`, `RQ-E2`, and `RQ-E3`.
  - `RQ-E1` (`BUG-20260301-01`): migration banner behavior now displays only during active migration states (or error) and no longer re-shows completed state on refresh.
  - `RQ-E2` (`BUG-20260301-02`): broken-only filter correctness hardened by re-applying `isBroken` constraints after in-memory filtering/recalculation stages.
  - `RQ-E3` (`BUG-20260301-03`): permission-restricted file analysis paths are treated as failed scan items rather than defaulting media records to broken.
- Completed Architect task `RQ-A1` for first concrete feature-request design mapping.
  - Added dedicated `RQ-A1` section to `docs/PLAN.md` for `FR-20260302-01` (quality regression alerts + playback-aware scoring + scheduled refresh jobs).
  - Documented problem statement, approach options with selected architecture, API/data/model/config touchpoints, risks/mitigations, acceptance criteria, and implementation/testing handoff checklist.
  - Updated `docs/TASKS.md` to mark `RQ-A1` complete and added explicit dependency notes for `FP-E*` execution tasks and `FP-Q*` testing scope.
- Completed BA intake alignment tasks `RQ-BA1` and `RQ-BA2` to synchronize request and story tracking artifacts.
  - Replaced placeholder feature request row in `docs/REQUESTS_TRACKER.md` with concrete intake `FR-20260302-01` and owner-ready status wording.
  - Populated `docs/STORY_TRACKING.md` Active Stories with open bug entries `BUG-20260301-01`, `BUG-20260301-02`, and `BUG-20260301-03`.
  - Added direct task traceability links from active bug stories to `docs/TASKS.md` request tasks (`RQ-E1`, `RQ-E2`, `RQ-E3`, `RQ-BA1`).
  - Marked `RQ-BA1` and `RQ-BA2` complete in `docs/TASKS.md` with evidence wording.
- Consolidated reporting policy to one tracker file (append rows) instead of one file per bug/feature request.
- Migrated existing bug examples into `docs/REQUESTS_TRACKER.md` with concrete IDs and open statuses.
- Updated `docs/TASKS.md` with a request/story-driven backlog sync section.
  - Added `RQ-*` tasks mapped from `docs/REQUESTS_TRACKER.md` open bugs (`BUG-20260301-01..03`).
  - Added traceability/documentation tasks to align request rows, story entries, and implementation follow-through.
- Expanded request/story tasks to cover feature-request intake and active-story synchronization.
  - Added `RQ-BA2` for replacing placeholder FR row with concrete feature request metadata.
  - Added `RQ-A1` for architecture/design mapping of first concrete feature request.
  - Added `RQ-DOC2` to keep `Active Stories` aligned with open request IDs.
- Trimmed `docs/TASKS.md` to active backlog view by removing completed task rows.
  - Kept only open/partial items in main agent sections.
  - Rebased snapshot and summary dashboards to reflect active (non-completed) tasks.
  - Simplified execution order flow to active request/security work.

### Removed
- Removed `docs/BUG_REPORT_TEMPLATE.md` (replaced by `docs/REQUESTS_TRACKER.md`).
- Removed `docs/FEATURE_REQUEST_TEMPLATE.md` (replaced by `docs/REQUESTS_TRACKER.md`).

## [1.2.1] - 2026-03-01

### Changed
- Prepared release version `1.2.1` across runtime/build metadata.
  - Updated `VERSION` to `1.2.1`.
  - Updated `optimarr.csproj` Version/AssemblyVersion/FileVersion to `1.2.1`.
  - Updated Docker image OCI label `org.opencontainers.image.version` to `1.2.1`.
- Applied release-policy exception alignment for Security `S1` (2026-03-01).
  - Documented owner decision that Sonarr key rotation is deferred ("the key will stay"), replacing prior hard NO-GO wording with explicit accepted-risk framing.
  - Updated `docs/TASKS.md` release-gate note to `GO with explicit owner-accepted security exception`.
  - Updated `S1` task state from `❌` to `⚠️` with residual-risk wording (policy exception, not technical closure).
  - Synced Progress Snapshot counts to reflect Security `8 done / 1 partial / 0 not started` and global totals `69 done / 1 partial / 0 not started`.
- QA targeted regression confirmation completed for already-closed items (`S6`, `S8`) on 2026-03-01.
  - `S6` scenario outcomes remain consistent in current code:
    - file under active root => allowed flow,
    - file outside active root => blocked (`UnauthorizedAccessException`),
    - no active roots configured => blocked (`UnauthorizedAccessException`).
  - `S8` remains closed on regression check:
    - updated controller/global `500` payloads are sanitized,
    - no response-body construction using `ex.Message` or stack trace text found in controllers.
  - No S8 regression evidence detected in this pass.
- Security Agent S1 closure-only revalidation pass completed (2026-03-01) with **NO-GO** retained.
  - Required ordered probes run:
    - old exposed key (`**33**`) -> `200`
    - rotated/new key probe -> `NO_NEW_KEY_EVIDENCE` (no concrete new key available for `200` validation)
  - Decision rule not satisfied (`new key` != `200` and old key != `401`), so S1 remains open.
- Security Agent S1 hard-evidence revalidation rerun completed (2026-03-01) with **NO-GO** retained.
  - Live Sonarr status probes this pass:
    - no key -> `401`
    - old exposed key (`**33**`) -> `200`
  - New-key proof remains unavailable in this pass (`NO_NEW_KEY_EVIDENCE`), so S1 closure criteria are not met.
  - Tracked runtime config remains secret-safe (placeholder-only Sonarr values in `config/appsettings.json`).
- Security Agent S1 closure revalidation rerun completed (2026-03-01) with **NO-GO** retained.
  - Live Sonarr probes: no-key request to `/api/v3/system/status` returned `401` (expected), but old exposed key still returned `200`.
  - Required closure evidence for S1 remains missing: no validated replacement/new-key `200` probe was available in this pass.
  - Tracked config hygiene remains compliant: no real API key values detected in tracked JSON config files; `config/appsettings.json` remains placeholder-only.
  - S8 remains PASS on revalidation: no `ex.Message`/stack-trace response payload leakage patterns found in controller response construction.
- QA Agent release-gate validation executed for `S6`/`S8` scope (2026-03-01).
  - Executed required commands: `dotnet restore`, `dotnet build -c Release`, `dotnet test`.
  - All three commands failed in this environment with `No .NET SDKs were found`, so runtime endpoint sign-off remains pending SDK-enabled execution.
  - Code-level scenario checks for `S6` pass: under-active-root path flow is allowed; outside-root and no-active-roots paths are explicitly blocked in `VideoAnalyzerService.EnsurePathIsAllowed`.
  - Code-level payload checks for `S8` pass: current controller/global error responses are sanitized and no `ex.Message`/stack-trace response leakage patterns are present.
- Code Review Agent re-reviewed only `S8/S6` gate scope (2026-03-01).
  - Confirmed no internal exception details are returned in controller API response payloads.
  - Confirmed `S6` remains unchanged in intent: library-root path allowlist + `ArgumentList`-based MediaInfo execution.
  - Recommended task state: set `S8` to `✅` (complete) while `S1` remains the independent release blocker.
- Security release-gate revalidation completed by Security Agent with **NO-GO** retained (2026-03-01).
  - S1 still fails closure criteria: old exposed Sonarr key (`**`) still returns `200` on `/api/v3/system/status`.
  - Required S1 evidence remains incomplete because replacement-key success proof was not provided in this gate pass.
  - Runtime/repo secret hygiene remains compliant (`config/appsettings.json` Sonarr values are placeholders only).
  - S8 revalidation passed: no `ex.Message`/stack-trace content detected in controller API payload construction paths.
- Security release-gate follow-up completed by Security Agent with **NO-GO** decision (2026-03-01).
  - Out-of-band Sonarr validation shows the previously exposed API key is still active (`GET /api/v3/system/status` returns `200` when called with old key), so `S1` remains blocking until key rotation + revocation are confirmed.
  - Repository/runtime config hygiene is now correct for tracked values: `config/appsettings.json` uses placeholder Sonarr credentials (empty `BaseUrl`/`ApiKey`, `Enabled: false`).
  - `S6` remains approved: MediaInfo call path enforces library-root whitelist and uses safe `ArgumentList` process invocation.
  - `S8` is now closed: production API error payloads have been normalized to sanitized client-safe messages while preserving detailed exception context in logs.
- Code Review release-gate pass completed for Security tasks `S6`/`S8` and config secret hygiene.
  - Confirmed `S6` hardening is effective: MediaInfo execution path now validates file paths against configured library roots before process start and uses `ProcessStartInfo.ArgumentList` for argument safety.
  - Confirmed tracked runtime config secret hygiene: `config/appsettings.json` uses placeholder Sonarr credentials (empty `BaseUrl`/`ApiKey`, `Enabled: false`).
  - Confirmed `S8` closure: no remaining `ex.Message`/`Exception.ToString()` content is returned in production API error payloads.
- Implemented security hardening fixes for S6 and S8.
  - Enforced strict library-root whitelist validation in `VideoAnalyzerService` before MediaInfo invocation (analysis is blocked when path cannot be validated or no active roots are configured).
  - Switched MediaInfo process argument handling to `ProcessStartInfo.ArgumentList` to avoid unsafe argument-string composition.
  - Added global exception handling middleware in `Program.cs` that logs server-side and returns sanitized JSON errors (`{ "error": "..." }`) for unhandled API exceptions.
  - Normalized `AnalysisController`, `LibraryController`, and `PlaybackController` 500 responses to avoid returning raw exception messages.
  - Completed follow-up normalization for remaining client-facing error paths in controller/service-originated payloads (`ServarrController` progress failure path, `ServarrSyncService` result messages, and migration error payload fields in `DatabaseMigrationService`).
- Removed real Sonarr credential values from tracked runtime config.
  - Reset `config/appsettings.json` Sonarr `BaseUrl` and `ApiKey` to placeholders and set `Enabled` to `false`.
  - Operational key rotation is still required out-of-band (tracked as `S1`) due to prior exposure.
- Fixed Docker publish/build picking up test sources from `Optimarr.Tests` when building `optimarr.csproj`.
  - Updated `optimarr.csproj` to exclude `Optimarr.Tests/**` from `Compile`/`Content`/`None`/`EmbeddedResource` item globs.
  - Added defensive cleanup in `Dockerfile` build stage (`rm -rf ./Optimarr.Tests`) before `dotnet publish` to prevent nested test sources from being compiled in container builds.
  - Prevents CS0246/CS0234 failures in container builds caused by test-only dependencies (`xUnit`, `Moq`, `Microsoft.AspNetCore.TestHost`) not referenced by the app project.
- Implemented Servarr service abstraction from architect plan A1.
  - Added `IServarrService`, `ISonarrService`, `IRadarrService`, shared `ServarrRootFolder`, and `ServarrServiceBase`.
  - Refactored `SonarrService` and `RadarrService` to inherit the base class while keeping provider-specific APIs.
  - Updated DI registrations in `Program.cs` to expose concrete services and interface mappings.
  - Updated `ServarrSyncService` and `VideoServarrMatcherService` to consume interface-based dependencies.
- Completed remaining Execution backlog status updates in `docs/TASKS.md`.
  - Marked E5/E6/E8/E9/E10 as done and synced Execution/TOTAL snapshot counters.
- Refactored rating and Servarr matching internals for maintainability without changing API contracts.
  - Decomposed `MediaPropertyRatingService.CalculateRating()` into focused penalty helpers.
  - Split `VideoServarrMatcherService` preload and batch logic into `PreloadSonarrDataAsync`, `PreloadRadarrDataAsync`, `MatchVideoBatchAsync`, and shared `MatchVideosCoreAsync`.
  - Consolidated full-library and library-path matching paths onto the same shared matching core.
- Simplified Servarr controller configuration handling.
  - Added `ConfigKeys` constants in `ServarrController` to remove repeated string literals.
  - Replaced duplicated Sonarr/Radarr save implementations with shared provider-based helpers using guard clauses.
- Added cancellation support and typed options wiring for service initialization.
  - Added optional `CancellationToken` parameters to `VideoAnalyzerService` public analyze methods and threaded cancellation into media process wait via `WaitForExitAsync`.
  - Added typed `ServarrOptions` binding and moved `SonarrService` / `RadarrService` initialization from inline `IConfiguration` keys to `IOptionsMonitor<ServarrOptions>`.
- Expanded runtime configuration documentation in `config/README.md`.
  - Added explicit coverage for `MatchingOptions` throughput keys (`SeriesConcurrency`, `MovieConcurrency`, `VideoBatchSize`, `SaveBatchSize`, `MaxMatchConcurrency`).
  - Documented `Cors:AllowedOrigins` behavior and safe defaults.
  - Documented `Auth:ApiKey` behavior and `X-Optimarr-Key` requirement when auth is enabled.
- Added a concise API reference table to `README.md` for the most-used endpoints.
  - Included analyze, scan, Servarr status, playback history, and system health examples.
- Expanded Docker troubleshooting guidance in `DOCKER.md`.
  - Added steps for failing health checks, log collection, database reset, live config updates, and mounted-volume permission issues.
- Added Swagger XML summaries across controller action methods.
  - Added one-line `/// <summary>` comments to all HTTP actions in `AnalysisController`, `LibraryController`, `PlaybackController`, `ServarrController`, and `SystemController`.
- Updated `docs/AGENTS_GUIDE.md` for consistency after moving agent artifacts into `docs/`.
  - Normalized artifact references to `docs/PROJECT_CONTEXT.md`, `docs/MEMORY.md`, and `docs/PLAN.md` across trigger phrases, inputs, outputs, and checklists.
  - Updated Security Agent assumptions to reflect current behavior: optional API-key-protected API and configured CORS allowlist logic.
  - Kept the global rule that all file-changing tasks must include a `docs/CHANGELOG.md` update.
- Updated `docs/TASKS.md` to reflect current workspace state and completed items.
  - Synced task references to `docs/...` artifact paths after documentation file moves.
  - Refreshed current branch-state snapshot with active modified/untracked files.
  - Marked already-implemented tasks as done/partial (PathNormalizer helper, URL/path-mapping validation, CORS hardening, API key masking, middleware auth).
- Reconciled the `Progress Snapshot` totals in `docs/TASKS.md` so Done/Partial/Not Started counts now exactly match the per-task statuses.
- Audited `docs/` for merge candidates and consistency.
  - No file merges were required; documents are distinct by role (contracts, plans, reviews, tracking).
  - Fixed remaining internal path references to canonical `docs/...` locations in `docs/REVIEW_R8.md`, `docs/REVIEW_R9.md`, `docs/PLAN.md`, and `docs/MEMORY.md`.
- Updated container startup/health behavior and development Docker workflow.
  - Added `GET /api/system/ready` that returns `503` until database migration completes.
  - Exposed `/api/system/migration` and `/api/system/ready` as API-key middleware public paths for container probing.
  - Made `Dockerfile` health check migration-aware and explicitly copied `VERSION` into runtime image for version endpoint consistency.
  - Reworked `Dockerfile.dev` + `docker-compose.dev.yml` to use internal port `8080`, non-root `optimarr` user (UID 1000), bind-mounted `config/data/logs`, and `dotnet watch` hot reload.
  - Updated `DOCKER.md` and `config/README.md` with current development-container usage and environment variables.
- Updated `docs/AGENTS_GUIDE.md` to require agent reports for easier review.
  - Added a global non-negotiable rule: every agent invocation must write a report `.md` file in `docs/reports/`.
  - Standardized report naming as `docs/reports/REPORT_<AGENT>_<YYYY-MM-DD>.md`.
  - Extended the handoff template with a required `Report File` field.
- Added `docs/reports/REPORT_TEMPLATE.md` as a shared, minimal report format for all agents.
  - Includes scope, actions, findings (blocking/warning/approved), artifacts, open issues, and verification sections.
- Added a new feature-pack backlog block to `docs/TASKS.md` for:
  - Quality Regression Alerts
  - Playback-Aware Scoring
  - Cron Jobs / Scheduler
  - Includes agent-specific task breakdowns (BA through Docs) and dependencies.
- Added a short feature-pack “Suggested Execution Order” to `docs/TASKS.md` with phased rollout steps (Define → Architect → Implement → API/Config/UI → Review/Test → Stabilize/Operate → Polish/Docs).
- Created `docs/reports/` as the dedicated folder for all agent report markdown files.
- Moved and standardized report-like markdown files into `docs/reports/`.
  - Renamed review/notes/report files to `REPORT_*` format for consistency.
  - Updated report path references in `docs/AGENTS_GUIDE.md`, `docs/TASKS.md`, and moved report files.
- Enforced dedicated per-agent report files in `docs/reports/`.
  - Updated `docs/AGENTS_GUIDE.md` to require each agent to append reports to its own dedicated `.md` file.
  - Updated `docs/reports/REPORT_TEMPLATE.md` to use dated sections inside dedicated files.
  - Added dedicated files: `REPORT_BA_AGENT.md`, `REPORT_ARCHITECT_AGENT.md`, `REPORT_EXECUTION_AGENT.md`, `REPORT_CODE_REVIEW_AGENT.md`, `REPORT_QA_AGENT.md`, `REPORT_DEBUGGER_AGENT.md`, `REPORT_DEVOPS_AGENT.md`, `REPORT_SECURITY_AGENT.md`, `REPORT_REFACTOR_AGENT.md`, `REPORT_DOCUMENTATION_AGENT.md`.
- Cleaned up and merged report files by agent in `docs/reports/`.
  - Merged dated report content into dedicated files for BA, Code Review, DevOps, and Refactor agents.
  - Removed redundant dated report files after consolidation.
  - Updated `docs/TASKS.md` branch-state note to reflect consolidated dedicated report files.
- Updated `docs/TASKS.md` tracker wording for current state accuracy.
  - Marked the historical `.gitignore` typo item as resolved context instead of active bug.
  - Synced current-branch notes to include completed report-governance scaffolding in `docs/reports/`.
- Recounted task progress in `docs/TASKS.md` from completed agent items and updated status tables.
  - Updated `Progress Snapshot` totals to: Done `47`, Partial `2`, Not Started `21` (total `70`).
  - Added explicit status column entries for Agent 4 review tasks (`R1-R8`) and marked them complete based on completed review artifacts.
- Added a BA follow-up validation entry to `docs/reports/REPORT_BA_AGENT.md`.
  - Captures latest `B1-B4` status re-check findings against `docs/PROJECT_CONTEXT.md` and `docs/MEMORY.md`.
  - Notes that `IServarrService` now exists in code and `B2` is no longer code-blocked (pending doc alignment).
- Completed BA contract alignment updates in `docs/PROJECT_CONTEXT.md` and tracker promotion in `docs/TASKS.md`.
  - Added explicit forbidden-commit guidance for real credentials in `config/appsettings.json` with production env/runtime secret injection policy.
  - Added an architecture rule to use `IServarrService` abstraction for shared Servarr behavior.
  - Added a Definition of Done check to avoid unextracted magic numbers in new code.
  - Updated BA task statuses `B1`, `B2`, and `B4` to complete in `docs/TASKS.md`.
- Appended a BA completion-pass entry to `docs/reports/REPORT_BA_AGENT.md` documenting closure of `B1/B2/B4`.
- Reconciled `docs/TASKS.md` Progress Snapshot counters after BA status promotions.
  - Updated BA row from `1/1/2` to `4/0/0` (Done/Partial/Not Started).
  - Updated global totals from `52/2/16` to `55/1/14` (total tasks unchanged at `70`).
- Synced `docs/TASKS.md` narrative text with dedicated agent report progress.
  - Updated the report-governance branch-state note to reflect which agent report files currently have active dated sections.
  - Corrected Debugger task `D1` wording to reflect completed DevOps verification (`V1`) instead of marking it pending.
- Closed remaining Debugger backlog tasks (`D2`–`D5`) with targeted root-cause fixes and telemetry.
  - Removed the Sonarr `FindEpisodeByPath()` N+1 fan-out by bulk-loading episode files once and resolving by dictionary lookup.
  - Corrected Sonarr preload episode-file mapping key usage in matcher cache building (`EpisodeFileId` now maps to episode file `Id`).
  - Mitigated `ReloadConfiguration()` race risk by using copy-on-swap `HttpClient` reinitialization in `ServarrServiceBase` instead of mutating a shared live client.
  - Added preload memory delta logging in `VideoServarrMatcherService` and made final `SaveChangesAsync()` conditional on pending tracked changes.
- Reconciled docs with current security reality after verification against runtime config/code.
  - `docs/TASKS.md` now marks `E2` and `R2` as partial (secret still present in `config/appsettings.json`).
  - `docs/TASKS.md` now marks `S8` as partial due to remaining controller-level `ex.Message` responses.
  - Added explicit release gate note: release remains blocked by open security findings (see `docs/reports/REPORT_SECURITY_AGENT.md`).

---

## [1.2.0] - 2026-01-11

### Changed - Major Architecture Update
- **Rating System Overhaul**: Completely redesigned compatibility rating system from client-based (0-11 scale) to property-based (0-100 scale)
  - Videos are now rated based on their media properties (codecs, containers, bit depth, HDR, etc.) rather than Jellyfin client compatibility
  - Rating starts at 100 and deducts points for unsupported properties based on configurable weights
  - Users can configure which media properties are "supported" or "unsupported" in settings
  - Rating impact weights are configurable for each property type (video codec, audio codec, container, etc.)
  - Overall score thresholds (Optimal, Good, Poor) are now configurable in settings (defaults: Optimal ≥80, Good ≥60, Poor <60)

### Removed
- **Jellyfin Client Compatibility System**: Removed all client-based compatibility features
  - Deleted `JellyfinCompatibilityData.cs` service containing hardcoded client compatibility matrix
  - Removed client compatibility modal and UI components
  - Removed "Jellyfin Clients" section from settings
  - Removed "Client Compatibility Overrides" section from settings
  - Removed "Understanding Compatibility Ratings" information banner from dashboard
  - Removed "Client Compatibility Overview" from dashboard
  - Removed `GetClientCompatibilityStats()` API endpoint
  - Removed client-related statistics from dashboard (avgDirectPlayClients, videosWithTranscoding)
  - Removed old client-based fallback code from `VideoAnalyzerService`
  - Removed unused helper methods: `GetEnabledClients()`, `LoadCodecThresholds()`, `GetCompatibilityOverrides()`, etc.

### Added
- **Media Property Settings**: New settings section for configuring supported media properties
  - Configure supported/unsupported status for video codecs, audio codecs, containers, subtitle formats, and bit depths
  - Default settings are provided for common media properties
  - Settings can be initialized with defaults if empty

- **Rating Impact Weights**: Configurable weights for how different properties affect the rating
  - Stereo Sound impact weight (penalizes when all audio tracks are stereo, ≤2 channels)
  - SDR Content impact weight (penalizes when video is SDR, no HDR)
  - High Bitrate impact weight
  - Incorrect Codec Tag impact weight
  - Unsupported Video Codec impact weight
  - Unsupported Audio Codec impact weight
  - Unsupported Container impact weight
  - Unsupported Subtitle Format impact weight
  - Unsupported Bit Depth impact weight
  - High Bitrate Threshold (Mbps)

- **Configurable Rating Thresholds**: Users can configure Optimal/Good/Poor thresholds
  - Optimal Threshold (default: 80) - minimum rating for "Optimal" classification
  - Good Threshold (default: 60) - minimum rating for "Good" classification
  - Ratings below Good threshold are classified as "Poor"

- **Rating Details Modal**: Click on rating in media info to see detailed breakdown
  - Shows starting rating (100), deductions, and final rating
  - Displays all issues and recommendations
  - Explains how the rating system works

### Changed
- **Database Migration**: Automatic migration of old 0-11 ratings to new 0-100 scale
  - Old ratings are detected and recalculated using the new property-based system
  - Old client-based fields (DirectPlayClients, RemuxClients, TranscodeClients, ClientResults) are cleared during migration
  - Migration runs automatically on application startup

- **API Changes**:
  - `GetMediaPropertySettings()` now returns configurable thresholds (Optimal, Good)
  - `SaveMediaPropertySettings()` now accepts thresholds for saving
  - Removed `GetClientCompatibilityStats()` endpoint
  - Dashboard endpoints no longer return client-related statistics
  - Dashboard now uses configurable thresholds for compatibility distribution

- **UI Changes**:
  - Rating display changed from "X/11" to "X/100" throughout the application
  - Dashboard issues now show compatibility rating instead of transcode client count
  - Removed client-related summary items from dashboard

### Technical Details
- **New Service**: `MediaPropertyRatingService` handles all rating calculations
- **Models**: `MediaPropertySettings`, `RatingWeights`, `RatingThresholds` (configurable)
- **Migration**: `DatabaseMigrationService` includes automatic rating migration logic
- **Backward Compatibility**: Old ratings are automatically migrated to new system
- **Rating Logic**: Stereo Sound and SDR Content now penalize (inverted from previous logic)

### Database Impact
- **Migration Required**: Yes - Ratings are automatically migrated from 0-11 to 0-100 scale
- **Backward Compatible**: Yes - Old data is preserved and migrated automatically
- **Deprecated Fields**: DirectPlayClients, RemuxClients, TranscodeClients, ClientResults (kept for backward compatibility but cleared)

---

## [1.1.4] - 2025-01-09

### Fixed
- **ProcessingStatus Migration**: Fixed database error "no such column: v.ProcessingStartedAt" by adding automatic migration detection and execution for ProcessingStatus fields
  - Added `CheckIfProcessingStatusMigrationNeededAsync()` method to detect missing `ProcessingStatus` and `ProcessingStartedAt` columns
  - Added `ApplyProcessingStatusMigrationAsync()` method to execute `AddProcessingStatusFields.sql` migration
  - Migration service now checks for ProcessingStatus migration after Servarr migration
  - Refactored common SQL execution logic into `ExecuteSqlMigrationAsync()` for code reuse
  - Added embedded SQL fallback for ProcessingStatus migration
  - Migration executes automatically on startup when columns are missing

#### Technical Details
- Updated `DatabaseMigrationService` to support multiple sequential migrations
- ProcessingStatus migration is detected and applied automatically if columns are missing
- Migration verification ensures columns are created successfully
- Both Servarr and ProcessingStatus migrations can run in sequence if needed

#### Database Impact
- **Migration Required**: Yes - ProcessingStatus migration will run automatically on first startup after update if columns are missing
- **Backward Compatible**: Yes - Existing databases will be migrated automatically
- **Migration Script**: `Data/Migrations/AddProcessingStatusFields.sql`

---

## [1.1.3] - 2025-01-09

### Fixed
- **Database Migration System**: Fixed database migration not working correctly after updates
  - Added automatic SQL migration detection and execution for Servarr fields
  - Migration service now checks if `ServarrType` column exists before applying migration
  - Improved path resolution for migration SQL file (searches multiple locations)
  - Added proper transaction handling with rollback on errors
  - Migration now executes automatically on startup when needed

- **Migration Warning Popup**: Fixed migration status banner display issues
  - Banner no longer shows for "unknown" status (when migration service hasn't started)
  - Error states now persist and don't auto-hide (users can see error messages)
  - Error details are now displayed in banner details section
  - Migration names are shown as comma-separated list instead of just counts
  - Improved error handling and status reporting

#### Technical Details
- Updated `DatabaseMigrationService` to detect and execute SQL migrations
- Added `CheckIfSqlMigrationNeededAsync()` method to check for missing columns
- Added `ApplySqlMigrationAsync()` method to execute SQL migration scripts
- Enhanced `migration.js` to handle all migration states correctly
- Error property now properly set in `MigrationProgress` class

#### Database Impact
- **Migration Required**: Yes - SQL migration will run automatically on first startup after update
- **Backward Compatible**: Yes - Existing databases will be migrated automatically

---

## [1.1.2] - 2025-01-XX

### Added
- **Basic Authentication Support for Sonarr/Radarr**: Added optional Basic Auth support to eliminate authentication warnings in Sonarr/Radarr logs
  - Optional `BasicAuthUsername` and `BasicAuthPassword` configuration fields
  - Automatically sends Basic Auth header when credentials are provided
  - Backward compatible - works with API key only if Basic Auth not configured
  - Fixes "Basic was not authenticated" log messages from Sonarr/Radarr

- **Progress Bar for Video Matching**: Added real-time progress tracking for rematching videos with Sonarr/Radarr
  - New `VideoMatchingProgressService` for tracking matching progress
  - Background task execution for video matching operations
  - Real-time progress updates showing processed/total count and percentage
  - Current file being processed displayed in tooltip
  - Progress polling every 500ms with status updates
  - Works for both Sonarr and Radarr matching operations

#### Technical Details
- New service: `VideoMatchingProgressService` (singleton) for in-memory progress tracking
- Updated `VideoServarrMatcherService` to support progress reporting
- New API endpoint: `GET /api/servarr/match-videos/progress/{matchId}` for polling progress
- Modified `POST /api/servarr/match-videos` to return `matchId` and run matching in background
- Frontend polling mechanism similar to library scan progress tracking
- Progress updates every 10 videos processed

#### UI Changes
- Status span next to "Match Videos" button shows real-time progress
- Progress format: "🔄 X/Y (Z%)" with current file in tooltip
- Button state management during matching operation
- Completion/error messages with detailed statistics

---
## [1.1.1] - 2025-01-XX

### Fixed
- **UI Bug Fix:** Fixed checkbox overlapping tags in media card header
  - Reduced checkbox z-index to prevent overlap with rating badges and tags
  - Added left padding to media-card-header to accommodate checkbox
  - Tags (SDR, Surround, 1080p, etc.) now display properly without being covered

---

## [1.1.0] - 2025-01-XX

### Added - Story: Servarr Video Matching Integration
**Story ID:** STORY-001  
**Developer:** AI Assistant  
**Date:** 2025-01-XX

#### Features
- **Servarr Video Matching Service**: Created `VideoServarrMatcherService` to automatically match video files with Sonarr series and Radarr movies based on file paths
- **Database Schema Enhancement**: Added Servarr integration fields to `VideoAnalysis` model:
  - `ServarrType` (Sonarr/Radarr indicator)
  - Sonarr fields: `SonarrSeriesId`, `SonarrSeriesTitle`, `SonarrEpisodeId`, `SonarrEpisodeNumber`, `SonarrSeasonNumber`
  - Radarr fields: `RadarrMovieId`, `RadarrMovieTitle`, `RadarrYear`
  - `ServarrMatchedAt` timestamp
- **UI Enhancements**:
  - Added Servarr icons (📺 Sonarr, 🎬 Radarr) in browse grid view media cards
  - Added Servarr information section in media info modal showing series/movie details
  - Added "Match Videos" buttons in Settings for manual matching
  - Automatic video matching after library sync operations
- **API Endpoints**:
  - `POST /api/servarr/match-videos` - Match all videos with Servarr data
  - `POST /api/servarr/match-videos/{libraryPathId}` - Match videos for specific library path
- **Database Migration**: Created SQL migration script `Data/Migrations/AddServarrFields.sql` for existing databases

#### Technical Details
- Service registered in `Program.cs` as scoped service
- Path normalization and matching logic implemented
- Year extraction from file paths for Radarr movies
- Automatic matching triggered after successful Sonarr/Radarr library syncs

#### Database Impact
- **Breaking Change**: Yes - New fields added to `VideoAnalyses` table
- **Migration Required**: Yes - See `Data/Migrations/AddServarrFields.sql`
- **Backward Compatible**: Yes - New fields are nullable, existing data unaffected

#### Documentation Updates
- Updated README.md with migration instructions
- B-MAD documentation structure established (PRD, ARCHITECTURE, DEVELOPER_GUIDE)

---

## [1.0.0] - 2025-01-XX

### Initial Release
**Story ID:** STORY-000  
**Developer:** Initial Development  
**Date:** 2025-01-XX

#### Features
- Video file analysis with MediaInfo CLI
- Jellyfin compatibility scoring (0-11 rating scale)
- Library scanning and management
- Sonarr/Radarr integration for library path syncing
- Jellyfin playback history synchronization
- Web-based UI with Servarr-style dark theme
- RESTful API
- Docker support

#### Technical Stack
- ASP.NET Core 8.0
- SQLite database
- Entity Framework Core
- Vanilla JavaScript frontend

