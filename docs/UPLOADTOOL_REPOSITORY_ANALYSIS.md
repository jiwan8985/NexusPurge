# UploadTool Repository Analysis

Updated: 2026-05-22

## Current Baseline

This repository is a Tauri 2 desktop application with a Vite React frontend and a Rust backend. The current implementation already covers the core UploadTool replacement direction:

- AWS S3 profile connection, browsing, upload, download, delete, prefix folder creation, rename, and presigned URL creation.
- MD5/ETag based smart sync and dry-run preview.
- Existing CloudFront invalidation support.
- Existing Akamai URL purge support.
- CDN URL mapping helper for URL-purge providers.
- External auth adapter contracts without local account/password implementation.
- Operation log/result model scaffolding.

## UploadTool Phase Status

| Phase | Status | Repository State |
| --- | --- | --- |
| Phase 1 - replacement scope cleanup | In progress | README/TODO/docs now clarify replacement scope and exclusions. |
| Phase 2 - S3 feature parity | Mostly implemented | S3 browse/upload/download/delete/mkdir/rename/presigned URL and sync preview exist. Base Prefix was added to profiles. |
| Phase 3 - CDN provider expansion | Implemented structurally | `cloudfront`, `akamai`, `lguplus`, `hyosung`, and `kt` are present in frontend/backend provider models. |
| Phase 4 - CDN adapter and dispatch | Implemented structurally | CloudFront/Akamai remain functional. LG U+/Hyosung/KT are explicit NotImplemented stub adapters. |
| Phase 5 - external auth structure | Implemented structurally | Auth interfaces and external auth stub exist. No local login/account DB was added. |
| Phase 6 - operation log/result structure | Implemented structurally | Frontend/Rust operation log models, local JSON persistence, and Tauri save/list/detail/clear commands are wired. |
| Phase 7 - customer-confirmed API work | Waiting | LG U+/Hyosung/KT real API calls, Akamai CP Code mode, query-string/wildcard/polling policy require customer confirmation. |
| Phase 8 - advanced operations | Waiting | CSV export, audit logs, reporting, admin dashboard, and detailed role permissions remain later work. |

## Recent Applied Changes

- Added KT CDN to `CdnProvider`.
- Added KT API Key/API Secret/Endpoint profile fields.
- Added KT credential storage and keyring-backed secret handling.
- Added `KtCdnAdapter` stub returning: `KT CDN purge API is not implemented yet. API specification is required.`
- Added KT dispatch branches in CDN purge and status/test command flow.
- Added `basePrefix` to profile models and Profile UI.
- On profile connect, the remote panel path now initializes to the profile Base Prefix when present.
- Wired operation log save/list/detail/clear through Tauri commands with local JSON persistence fallback in the frontend store.
- Moved app-level Tauri IPC, event listening, window controls, and desktop directory selection behind the runtime bridge so desktop/web delivery can share call sites.
- Added operation log persistence calls for upload, download, S3 delete, mkdir, and rename workflows.
- Added focused Rust tests for CDN URL mapping, LG U+/Hyosung/KT NotImplemented behavior, and operation log persistence.

## Existing Behavior Preserved

- CloudFront invalidation still uses Distribution ID and object paths.
- Akamai still uses URL purge with EdgeGrid credentials.
- S3 upload/download/list/delete/rename/presigned URL behavior was not replaced.
- LG U+/Hyosung real API calls remain intentionally unimplemented.

## Next Recommended Steps

1. Add focused tests for provider-specific credential validation, especially KT/LG U+/Hyosung missing secret cases.
2. Define the real web backend API contract before enabling hosted web mode.
3. Add large-list and large-transfer profiling targets for 10k/50k/100k object scenarios.
4. Wait for customer API documentation before implementing LG U+, Hyosung, or KT real purge calls.
