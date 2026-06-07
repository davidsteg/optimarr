# Requests Tracker

Single file for both bug reports and feature requests.
Append new rows; do not create one file per item.

## Bug Reports
| ID | Story ID | Linked RQ Task IDs | Priority | Title | Status | Evidence |
|---|---|---|---|---|---|---|
| - | - | - | - | No open bugs | - | Closed bugs archived below |

## Archived Closed Bugs
| ID | Story ID | Linked RQ Task IDs | Priority | Title | Status | Evidence |
|---|---|---|---|---|---|---|
| BUG-20260301-01 | STORY-REQ-BUG-001 | RQ-BA1, RQ-E1, RQ-R1, RQ-Q1, RQ-DOC1 | P2 | Database migration banner repeats on refresh | Closed (implemented + QA revalidated) | `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `wwwroot/js/migration.js` |
| BUG-20260301-02 | STORY-REQ-BUG-002 | RQ-BA1, RQ-E2, RQ-R1, RQ-Q1, RQ-DOC1 | P2 | Broken-only filter still shows non-broken media | Closed (regression fixed, pending QA recheck) | `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `Controllers/LibraryController.cs`, `wwwroot/js/browse.js` |
| BUG-20260301-03 | STORY-REQ-BUG-003 | RQ-BA1, RQ-E3, RQ-R1, RQ-Q1, RQ-DOC1 | P1 | Permission-handling failure on restricted files | Closed (implemented + QA/security revalidated) | `docs/TASKS.md`, `docs/STORY_TRACKING.md`, `Services/LibraryScannerService.cs`, `Controllers/LibraryController.cs` |

### Bug Details (for the selected ID)
- Related Story ID:
- Related RQ Task IDs:
- Version:
- Environment (OS + Docker/local):
- Owner:
- Repro:
  1. ...
  2. ...
  3. ...
- Expected:
- Actual:
- Evidence (logs/screenshot/error):

## Feature Requests
| ID | Story ID | Linked RQ Task IDs | Priority | Title | Status | Evidence |
|---|---|---|---|---|---|---|
| FR-20260302-01 | STORY-REQ-FEAT-001 | RQ-BA2, RQ-A1, RQ-DOC2, FP-BA1, FP-BA2, FP-A1, FP-A2, FP-A3, FP-A4 | P1 | Quality Regression Alerts with playback-aware scoring and scheduled refresh jobs | Open (planned) | `docs/PLAN.md` (`RQ-A1`), `docs/TASKS.md` (RQ + FP rows), `docs/STORY_TRACKING.md` (Active Stories) |

### Feature Details (for the selected ID)
- Related Story ID:
- Related RQ Task IDs:
- Requester: Product owner
- Target version/milestone: Next minor release planning window
- Problem to solve: Regressions in media compatibility quality are hard to detect early, and static scores do not reflect real playback behavior over time.
- Proposed solution: Add quality regression alerts, playback-aware score weighting from playback history, and scheduled background refresh jobs.
- Acceptance criteria:
  1. ...
  2. ...
  3. ...
- Impacted areas (API/UI/services/docs): API, UI dashboard/settings, scoring services, scheduler services, docs
- Dependencies/risks: Requires architecture and execution sequencing from `docs/TASKS.md` (`RQ-A1`, feature-pack `FP-*` items); risk of noisy alerts if thresholds are not tuned.
