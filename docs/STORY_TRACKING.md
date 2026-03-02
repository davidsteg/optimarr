# Story Tracking
## Optimarr - Development Stories

This document tracks all development stories, features, and changes made to the Optimarr project.

---

## Story Template

When creating a new story, use this template:

```markdown
**STORY-XXX: Story Title**
- **Status:** 🟡 In Progress / ✅ Completed / 🔴 Blocked / ⏸️ On Hold
- **Priority:** High / Medium / Low
- **Started:** YYYY-MM-DD
- **Completed:** YYYY-MM-DD
- **Developer:** Developer Name
- **Version:** X.Y.Z
- **Description:** Brief description of the story
- **Request Link:** `docs/REQUESTS_TRACKER.md` (`BUG-*` or `FR-*`)
- **Related Task IDs:** `RQ-*` (and `FP-*` where applicable)
- **Database Changes:** Yes/No - Details
- **Migration Script:** Path to migration script (if applicable)
- **UI Changes:** List of UI changes
- **API Changes:** List of API endpoint changes
- **Documentation Updates:** List of documentation files updated
- **Testing:** Testing notes
- **Notes:** Additional notes or blockers
```

---

## Active Stories

### STORY-REQ-FEAT-001: Quality Regression Alerts + Playback-Aware Scoring + Scheduled Jobs
- **Status:** 🟡 In Progress
- **Priority:** High
- **Started:** 2026-03-02
- **Developer:** Unassigned (planning)
- **Version:** Unreleased
- **Description:** Planned feature set for quality regression alerts, playback-aware scoring, and scheduled refresh execution.
- **Request Link:** `docs/REQUESTS_TRACKER.md` (`FR-20260302-01`)
- **Related Task IDs:** `RQ-BA2`, `RQ-A1`, `RQ-DOC2`, `FP-BA1`, `FP-BA2`, `FP-A1`, `FP-A2`, `FP-A3`, `FP-A4`
- **Documentation Updates:** `docs/PLAN.md`, `docs/TASKS.md`, `docs/STORY_TRACKING.md`

---

## Recent Stories

### STORY-REQ-BUG-001: Migration banner repeats on refresh
- **Status:** ✅ Completed
- **Priority:** Medium
- **Started:** 2026-03-01
- **Completed:** 2026-03-01
- **Developer:** AI Assistant (implementation + review + QA)
- **Version:** Unreleased
- **Description:** Migration-status banner now shows only for active migration/error states and no longer reappears after completed refresh.
- **Request Link:** `docs/REQUESTS_TRACKER.md` (`BUG-20260301-01`)
- **Related Task IDs:** `RQ-BA1`, `RQ-E1`, `RQ-R1`, `RQ-Q1`, `RQ-DOC1`
- **Verification Status:** Implemented and security-reviewed; QA follow-up remains partial at `RQ-Q1`.
- **Documentation Updates:** `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `docs/CHANGELOG.md`

### STORY-REQ-BUG-002: Broken-only filter includes non-broken media
- **Status:** ✅ Completed
- **Priority:** Medium
- **Started:** 2026-03-01
- **Completed:** 2026-03-01
- **Developer:** AI Assistant (implementation + review + QA)
- **Version:** Unreleased
- **Description:** Broken-only filter logic was tightened so non-broken media is excluded consistently after in-memory processing.
- **Request Link:** `docs/REQUESTS_TRACKER.md` (`BUG-20260301-02`)
- **Related Task IDs:** `RQ-BA1`, `RQ-E2`, `RQ-R1`, `RQ-Q1`, `RQ-DOC1`
- **Verification Status:** Implemented and security-reviewed; QA follow-up remains partial at `RQ-Q1`.
- **Documentation Updates:** `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `docs/CHANGELOG.md`

### STORY-REQ-BUG-003: Permission-handling failure on restricted files
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2026-03-01
- **Completed:** 2026-03-01
- **Developer:** AI Assistant (implementation + review + QA)
- **Version:** Unreleased
- **Description:** Permission-restricted analysis paths now route to failed-scan handling and avoid forced broken-media classification.
- **Request Link:** `docs/REQUESTS_TRACKER.md` (`BUG-20260301-03`)
- **Related Task IDs:** `RQ-BA1`, `RQ-E3`, `RQ-R1`, `RQ-Q1`, `RQ-DOC1`
- **Verification Status:** Implemented and security-reviewed; QA follow-up remains partial at `RQ-Q1`.
- **Documentation Updates:** `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `docs/CHANGELOG.md`

### STORY-006: Rating System Overhaul - Property-Based Rating (v1.2.0)
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2026-01-11
- **Completed:** 2026-01-11
- **Developer:** AI Assistant
- **Version:** 1.2.0
- **Description:** 
  Completely redesigned the compatibility rating system from client-based (0-11 scale) to property-based (0-100 scale). Videos are now rated based on their media properties (codecs, containers, bit depth, HDR, etc.) rather than Jellyfin client compatibility. The system is fully configurable, allowing users to define which properties are supported/unsupported and adjust impact weights and rating thresholds.

- **Database Changes:** 
  - Automatic migration of old 0-11 ratings to new 0-100 scale
  - Old client-based fields (DirectPlayClients, RemuxClients, TranscodeClients, ClientResults) are cleared during migration
  - Migration runs automatically on application startup

- **Migration Script:** 
  Automatic migration logic in `DatabaseMigrationService` - no SQL script required

- **UI Changes:**
  - Rating display changed from "X/11" to "X/100" throughout the application
  - New "Media Property Settings" section in Settings
  - Rating Details Modal showing detailed breakdown
  - Dashboard issues now show compatibility rating instead of transcode client count
  - Removed client-related summary items from dashboard
  - Removed "Jellyfin Clients" and "Client Compatibility Overrides" sections from settings
  - Removed "Understanding Compatibility Ratings" information banner from dashboard
  - Removed "Client Compatibility Overview" from dashboard

- **API Changes:**
  - `GetMediaPropertySettings()` now returns configurable thresholds (Optimal, Good)
  - `SaveMediaPropertySettings()` now accepts thresholds for saving
  - Removed `GetClientCompatibilityStats()` endpoint
  - Dashboard endpoints no longer return client-related statistics
  - Dashboard now uses configurable thresholds for compatibility distribution
  - New endpoint: `POST /api/library/recalculate-compatibility` - Recalculate all video ratings when settings change

- **Service Updates:**
  - **New Service:** `MediaPropertyRatingService` - Handles all rating calculations
    - `CalculateRating()` - Calculate 0-100 rating based on media properties
    - `LoadMediaPropertySettings()` - Load supported/unsupported property settings
    - `LoadRatingWeights()` - Load configurable impact weights
    - `LoadRatingThresholds()` - Load configurable rating thresholds
  - **Removed Service:** `JellyfinCompatibilityData` - Deleted hardcoded client compatibility matrix
  - **Updated Service:** `VideoAnalyzerService` - Now delegates rating calculation to MediaPropertyRatingService
  - **Updated Service:** `DatabaseMigrationService` - Added automatic rating migration logic

- **New Models:**
  - `MediaPropertySettings` - Configurable supported/unsupported properties
  - `RatingWeights` - Configurable impact weights for different property types
  - `RatingThresholds` - Configurable thresholds (Optimal, Good)

- **Documentation Updates:**
  - Updated CHANGELOG.md with comprehensive v1.2.0 release notes
  - Updated ARCHITECTURE.md with MediaPropertyRatingService documentation
  - Updated PRD.md version and date
  - Updated DEVELOPER_GUIDE.md version and date
  - Updated STORY_TRACKING.md with this story

- **Testing:**
  - Verified rating migration from 0-11 to 0-100 scale
  - Verified configurable settings work correctly
  - Verified rating calculations match expected behavior
  - Verified UI updates display correctly
  - Verified backward compatibility with old ratings

- **Notes:**
  This is a major architecture change that fundamentally alters how compatibility is calculated. The new system is more flexible and user-configurable, allowing users to define their own compatibility criteria rather than relying on hardcoded client compatibility data. Old ratings are automatically migrated to the new system on startup.

---

### STORY-005: ProcessingStatus Migration Auto-Detection (v1.1.4)
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2025-01-09
- **Completed:** 2025-01-09
- **Developer:** AI Assistant
- **Version:** 1.1.4
- **Description:** 
  Fixed database error "no such column: v.ProcessingStartedAt" that occurred when the ProcessingStatus migration had not been applied. Enhanced the DatabaseMigrationService to automatically detect and apply the ProcessingStatus migration on startup, similar to how Servarr migrations are handled.

- **Database Changes:** 
  No new schema changes - fixes migration execution for existing ProcessingStatus schema changes

- **Migration Script:** 
  Uses existing `Data/Migrations/AddProcessingStatusFields.sql` - now executed automatically when columns are missing

- **UI Changes:**
  None - backend fix only

- **API Changes:**
  None - internal service improvements only

- **Service Updates:**
  - `DatabaseMigrationService` - Enhanced with ProcessingStatus migration detection and execution
    - Added `CheckIfProcessingStatusMigrationNeededAsync()` to detect missing ProcessingStatus and ProcessingStartedAt columns
    - Added `ApplyProcessingStatusMigrationAsync()` to execute ProcessingStatus migration script
    - Refactored common SQL execution into `ExecuteSqlMigrationAsync()` for code reuse
    - Added `GetEmbeddedProcessingStatusMigrationSql()` for fallback SQL
    - Migration service now checks for both Servarr and ProcessingStatus migrations sequentially
    - Each migration is verified after application

- **Documentation Updates:**
  - Updated CHANGELOG.md with ProcessingStatus migration fix
  - Updated version to 1.1.4
  - Updated STORY_TRACKING.md with this story

- **Testing:**
  - Verified ProcessingStatus migration detection works correctly
  - Verified SQL migration executes successfully when columns are missing
  - Verified migration verification confirms columns are created
  - Verified sequential migrations (Servarr then ProcessingStatus) work correctly

- **Notes:**
  This fix resolves the "SQLite Error 1: 'no such column: v.ProcessingStartedAt'" error that occurred when the database schema was missing the ProcessingStatus fields. The migration now runs automatically on startup, ensuring the database schema matches the model.

---

### STORY-004: Database Migration System Fixes (v1.1.3)
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2025-01-09
- **Completed:** 2025-01-09
- **Developer:** AI Assistant
- **Version:** 1.1.3
- **Description:** 
  Fixed database migration system that was not working correctly after updates. The system now automatically detects when SQL migrations are needed (specifically for Servarr fields) and executes them. Also fixed migration warning popup to properly display status, errors, and not show for unknown states.

- **Database Changes:** 
  No new schema changes - fixes migration execution for existing schema changes

- **Migration Script:** 
  Uses existing `Data/Migrations/AddServarrFields.sql` - now executed automatically

- **UI Changes:**
  - Fixed migration status banner to handle all states correctly
  - Banner no longer shows for "unknown" status
  - Error states persist (don't auto-hide)
  - Error details displayed in banner
  - Migration names shown as comma-separated list

- **API Changes:**
  None - internal service improvements only

- **Service Updates:**
  - `DatabaseMigrationService` - Enhanced with SQL migration detection and execution
    - Added `CheckIfSqlMigrationNeededAsync()` to detect missing columns
    - Added `ApplySqlMigrationAsync()` to execute SQL migration scripts
    - Improved path resolution for migration files
    - Better error handling with full error details in progress object
  - `migration.js` - Fixed frontend migration status handling
    - Proper handling of "unknown" status
    - Error state persistence
    - Better error details display

- **Documentation Updates:**
  - Updated CHANGELOG.md with migration fixes
  - Updated version to 1.1.3

- **Testing:**
  - Verified migration detection works correctly
  - Verified SQL migration executes successfully
  - Verified banner displays correctly for all states
  - Verified error states persist

- **Notes:**
  This fix resolves the "Error loading dashboard: 500 Internal Server Error" issue that occurred when database schema was out of sync with the model after updates.

---

### STORY-003: Basic Authentication Support for Sonarr/Radarr (v1.1.2)
- **Status:** ✅ Completed
- **Priority:** Low
- **Started:** 2025-01-08
- **Completed:** 2025-01-08
- **Developer:** AI Assistant
- **Version:** 1.1.2
- **Description:** 
  Added optional Basic Authentication support to SonarrService and RadarrService to eliminate "Basic was not authenticated" log messages from Sonarr/Radarr instances that require Basic Auth. This is a backward-compatible enhancement that allows users to optionally configure Basic Auth credentials alongside API keys.

- **Database Changes:** 
  No database changes required

- **Migration Script:** 
  None required

- **UI Changes:**
  None - Configuration-only feature

- **API Changes:**
  None - Internal service enhancement only

- **Service Updates:**
  - `SonarrService` - Added optional Basic Auth support
    - New configuration fields: `BasicAuthUsername`, `BasicAuthPassword`
    - Automatically sets Authorization header when credentials provided
    - Backward compatible with existing API key-only configuration
  - `RadarrService` - Added optional Basic Auth support
    - Same implementation as SonarrService
    - Consistent configuration pattern across both services

- **Configuration Changes:**
  - New optional fields in `appsettings.json`:
    - `Servarr:Sonarr:BasicAuthUsername` (optional)
    - `Servarr:Sonarr:BasicAuthPassword` (optional)
    - `Servarr:Radarr:BasicAuthUsername` (optional)
    - `Servarr:Radarr:BasicAuthPassword` (optional)

- **Documentation Updates:**
  - Updated `.cursor/rules/integrations.mdc` with Basic Auth documentation
  - Updated `docs/CHANGELOG.md` with new feature entry

- **Testing:**
  - Verified backward compatibility (works without Basic Auth credentials)
  - Verified Basic Auth header is set when credentials provided
  - Confirmed no breaking changes to existing functionality

- **Notes:** 
  This enhancement addresses user-reported log noise from Sonarr instances that check for Basic Auth even when API key authentication is used. The feature is optional and does not affect existing installations.

---

### STORY-002: Progress Bar for Video Matching (v1.1.2)
- **Status:** ✅ Completed
- **Priority:** Medium
- **Started:** 2025-01-XX
- **Completed:** 2025-01-XX
- **Developer:** AI Assistant
- **Version:** 1.1.2
- **Description:** 
  Added real-time progress tracking for video matching operations with Sonarr/Radarr. Users can now see progress updates showing how many videos have been processed, matched count, and the current file being processed. This improves user experience for large video libraries where matching can take significant time.

- **Database Changes:** 
  No database changes required

- **Migration Script:** 
  None required

- **UI Changes:**
  - Progress display in status span next to "Match Videos" button
  - Real-time updates showing "🔄 X/Y (Z%)" format
  - Current file name displayed in tooltip
  - Button state management (disabled during matching)
  - Completion/error messages with detailed statistics

- **API Changes:**
  - Modified `POST /api/servarr/match-videos` - Now returns `matchId` and runs matching in background
  - New `GET /api/servarr/match-videos/progress/{matchId}` - Poll for matching progress

- **New Services:**
  - `VideoMatchingProgressService` - Singleton service for in-memory progress tracking
    - Tracks status (running, completed, error)
    - Stores processed/total counts, matched count, errors
    - Provides current item being processed
    - Auto-cleanup of old progress records

- **Service Updates:**
  - `VideoServarrMatcherService` - Added progress reporting support
    - New overload: `MatchAllVideosAsync(progressService, matchId)`
    - Reports progress every 10 videos
    - Updates current file being processed

- **Frontend Changes:**
  - `servarr.js` - Enhanced `matchVideosWithServarr()` function
    - Polls for progress every 500ms
    - Updates status span with progress information
    - Handles completion and error states
    - Restores button state after completion

- **Documentation Updates:**
  - Updated `docs/CHANGELOG.md` - Added v1.1.2 entry
  - Updated `docs/STORY_TRACKING.md` - Added STORY-002
  - Updated `docs/ARCHITECTURE.md` - Documented new service and endpoints
  - Updated `VERSION` file to 1.1.2

- **Testing:**
  - Progress tracking tested with various video counts
  - Polling mechanism verified for responsiveness
  - Error handling tested for failed matching operations
  - Button state management verified
  - Progress cleanup tested for old records

- **Notes:**
  - Progress tracking uses in-memory storage (ConcurrentDictionary)
  - Progress records auto-cleanup after 1 hour
  - Follows same pattern as library scan progress tracking
  - No breaking changes - backward compatible

---

## Bug Fixes

### UI Fix: Checkbox Overlap with Tags (v1.1.1)
- **Status:** ✅ Completed
- **Priority:** Medium
- **Date:** 2025-01-XX
- **Developer:** AI Assistant
- **Version:** 1.1.1
- **Description:** 
  Fixed checkbox in media cards overlapping with rating badges and tags (SDR, Surround, 1080p, etc.) in the header section.

- **Changes:**
  - Reduced `.media-card-checkbox` z-index from 10 to 1
  - Added `padding-left: 2.5rem` to `.media-card-header` to accommodate checkbox
  - Set `.media-card-header` z-index to 2 to ensure tags appear above checkbox

- **Files Modified:**
  - `wwwroot/styles.css` - Updated media card checkbox and header styles

- **Testing:**
  - Verified tags are no longer covered by checkbox
  - Confirmed checkbox remains clickable and functional
  - Checked responsive behavior on different screen sizes

---

## Completed Stories

### STORY-001: Servarr Video Matching Integration
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2025-01-XX
- **Completed:** 2025-01-XX
- **Developer:** AI Assistant
- **Version:** 1.1.0
- **Description:** 
  Integrate Sonarr/Radarr metadata with video analysis results. Display icons in browse grid view to indicate if videos are found in Sonarr/Radarr. Show combined information (MediaInfo, Sonarr/Radarr, Jellyfin) in media info modal. Add automatic and manual video matching functionality.

- **Database Changes:** 
  Yes - Added 10 new nullable fields to `VideoAnalyses` table:
  - `ServarrType` TEXT
  - `SonarrSeriesId` INTEGER
  - `SonarrSeriesTitle` TEXT
  - `SonarrEpisodeId` INTEGER
  - `SonarrEpisodeNumber` INTEGER
  - `SonarrSeasonNumber` INTEGER
  - `RadarrMovieId` INTEGER
  - `RadarrMovieTitle` TEXT
  - `RadarrYear` INTEGER
  - `ServarrMatchedAt` TEXT

- **Migration Script:** 
  `Data/Migrations/AddServarrFields.sql`
  - Creates new table with Servarr fields
  - Copies existing data
  - Drops old table and renames new one
  - Recreates indexes

- **UI Changes:**
  - Added Servarr icons (📺 Sonarr, 🎬 Radarr) in browse grid media cards
  - Added Servarr information section in media info modal
  - Added "Match Videos" buttons in Settings (Sonarr/Radarr sections)
  - Added version display in sidebar (v1.1.0)
  - Automatic matching triggered after library sync

- **API Changes:**
  - `POST /api/servarr/match-videos` - Match all videos
  - `POST /api/servarr/match-videos/{libraryPathId}` - Match videos for library path
  - `GET /api/system/version` - Updated to read from VERSION file

- **New Services:**
  - `VideoServarrMatcherService` - Handles video matching logic

- **Documentation Updates:**
  - Created `docs/CHANGELOG.md` - Version history and changelog
  - Created `docs/STORY_TRACKING.md` - This file
  - Updated `docs/PRD.md` - Added version history section
  - Updated `docs/ARCHITECTURE.md` - Added VideoServarrMatcherService, updated VideoAnalysis schema
  - Updated `docs/DEVELOPER_GUIDE.md` - Added version management and story tracking section
  - Updated `README.md` - Added database migration instructions

- **Testing:**
  - Migration script tested for SQLite compatibility
  - Path matching logic tested with normalized paths
  - UI icons display correctly for matched/unmatched videos
  - Automatic matching works after library sync

- **Notes:**
  - All new fields are nullable for backward compatibility
  - Migration is safe for existing databases
  - Version tracking system implemented

---

### STORY-000: Initial Release
- **Status:** ✅ Completed
- **Priority:** High
- **Started:** 2025-01-XX
- **Completed:** 2025-01-XX
- **Developer:** Initial Development
- **Version:** 1.0.0
- **Description:** 
  Core application functionality including video analysis, library management, Sonarr/Radarr library path syncing, and Jellyfin playback history integration.

- **Database Changes:** 
  Initial database schema creation

- **UI Changes:**
  - Complete web UI with Servarr-style dark theme
  - Dashboard, Browse, Library, Playback, Settings tabs

- **API Changes:**
  - Full REST API for all operations

- **Documentation Updates:**
  - Initial B-MAD documentation (PRD, ARCHITECTURE, DEVELOPER_GUIDE)

---

## Story Statistics

| Status | Count |
|--------|-------|
| ✅ Completed | 7 |
| 🟡 In Progress | 1 |
| 🔴 Blocked | 0 |
| ⏸️ On Hold | 0 |

**Total Stories:** 8

---

## Version History

| Version | Release Date | Stories | Description |
|---------|--------------|---------|-------------|
| 1.2.0 | 2026-01-11 | STORY-006 | Rating System Overhaul - Property-Based Rating (0-100 scale) |
| 1.1.4 | 2025-01-09 | STORY-005 | ProcessingStatus Migration Auto-Detection |
| 1.1.3 | 2025-01-09 | STORY-004 | Database Migration System Fixes |
| 1.1.2 | 2025-01-XX | STORY-002, STORY-003 | Progress bar for video matching, Basic Auth support |
| 1.1.1 | 2025-01-XX | - | UI Bug Fix: Checkbox overlap with tags |
| 1.1.0 | 2025-01-XX | STORY-001 | Servarr Video Matching Integration |
| 1.0.0 | 2025-01-XX | STORY-000 | Initial Release |

---

## Notes

- Stories are tracked with unique IDs (STORY-XXX)
- Each story includes complete change documentation
- Database changes always include migration scripts
- Version numbers follow Semantic Versioning
- See [CHANGELOG.md](CHANGELOG.md) for detailed version history
