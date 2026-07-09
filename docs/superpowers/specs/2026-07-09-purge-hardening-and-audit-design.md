# Purge Hardening & Audit — Design Spec

Date: 2026-07-09
Status: Approved for planning

## Background

The user requested 8 items covering audit logging, S3 key validation, log timestamps,
multi-threaded uploads, a properties dialog, vendor-specific folder purge logic, log
file separation, and a Hyosung (효성) ITX CDN purge bug fix. A codebase review found
that 5 of the 8 items are already implemented; this spec covers only the remaining
gaps, confirmed through brainstorming with the user.

## Already implemented (no code change, verification only)

- **Audit log for failures/delays**: `services/operation_log.rs` records `startedAt`/
  `finishedAt`, per-file/per-purge `error`, and `durationMs` for every operation, split
  into JSON (`operation_logs.json`, full history) and per-type text logs.
- **Multi-threaded upload**: `sync.rs`/`s3.rs` use `tokio::sync::Semaphore` +
  `JoinSet` for parallel transfers; concurrency is user-configurable in Settings
  (`maxConcurrentTransfers`, default 8, `batch-settings.ts`).
- **Right-click properties dialog**: `PropertiesDialog.tsx` is wired to
  `RemotePanel.tsx`'s context menu ("속성") and shows S3 key, bucket/region, and
  per-CDN edge domain / CDN URL / purge request endpoint.
- **Per-type log files**: `OperationLogService::append_typed_log` already splits
  output into `system-*.log`, `transfer-*.log`, `cdn-*.log`.

One small gap found during brainstorming: the LogPanel UI itself did not surface
start/end timestamps (they existed only in the on-disk log files). This spec adds
that as work item 4 below.

## Work item 1 — S3 key special-character validation

**Rule** (user-selected): S3-safe whitelist only. Allowed characters in any S3 key
segment (folder or file name, not counting the `/` separator): ASCII letters, digits,
and `! - _ . * ' ( )`. Everything else — including Korean characters, spaces, and
symbols like `\ : ? " < > | { } ^ % \`` — is rejected.

**Where enforced**:
- `src-tauri/src/utils/validate.rs` (new file): `validate_s3_key_segment(name: &str) -> Result<()>`,
  checks each `/`-separated segment against the whitelist regex/char-set and returns a
  Korean-language error naming the offending character.
- `commands/s3.rs::put_s3_object` — covers folder creation (`createDirectory` in
  `useS3.ts` calls this with a trailing-slash key) and any direct object writes.
- `commands/s3.rs::rename_s3_object` — validate `new_key`'s final segment.
- `commands/sync.rs::start_uploads` / `commands/s3.rs::upload_files` — validate each
  item's `remote_path` before upload (reject the individual file with a clear error;
  do not abort the whole batch).
- Frontend: `InputDialog` (used for "새 폴더" and "이름 변경") gets a client-side
  pre-check using the same character set, so the user gets immediate inline feedback
  instead of waiting for the Rust round-trip.

**Out of scope**: local filesystem file/folder creation keeps OS-native rules only
(no new restriction) — this item is S3-side only, per the user's request.

## Work item 2 — Hyosung (효성) CDN purge fix

**Root cause**: `adapters/cdn/hyosung.rs::build_urls()` concatenates the CDN domain
and S3 key into a purge URL with no percent-encoding. The vendor's own API guide
(`docs/효성 CDN_PURGE_API_GUIDE_ver.2026.pdf`, §8 Operational Notes) states:
"한글/특수문자 파일명은 URL 인코딩 형태 전달을 권장합니다" (Korean/special-character
filenames should be URL-encoded). Any purge path containing Korean text, spaces, or
symbols currently reaches the 효성 API un-encoded and is rejected or mis-parsed.

**Fix**: `adapters/cdn/mod.rs` already implements correct segment-wise percent-encoding
in `build_cdn_url()` (splits on `/`, encodes each segment with `NON_ALPHANUMERIC` minus
`- _ . ~`, leaves `/` untouched). Expose this encoding step as a reusable `pub` helper
and call it from `hyosung.rs::build_urls()` instead of the current raw
`format!("{}://{}/{}", scheme, domain, path)`. No behavior change for already-ASCII-safe
paths; Korean/special-character paths become correctly encoded.

**Testing**: extend the existing `hyosung.rs` unit tests
(`build_urls_normalizes_domain_and_path`, `build_urls_without_scheme_defaults_to_http`)
with a case containing Korean characters and a space, asserting the output URL is
percent-encoded and matches the shared `build_cdn_url` behavior.

## Work item 3 — CDN vendor-specific folder purge logic

Trigger (per user decision): whenever the purge target is a **folder** (a wildcard
path ending in `*`, produced by selecting a folder or "전체 Purge"), always use that
vendor's broadest natively-supported purge unit — not a raw file-count threshold.

| Vendor | Folder purge behavior | Status |
|---|---|---|
| CloudFront | Wildcard path passed directly to `CreateInvalidation` (native support) | Already correct, no change |
| Akamai | Wildcard routed to CP Code invalidation (`purge_cp_codes`) in `mod.rs` | Already correct, no change |
| 효성 (Hyosung) | Wildcard expanded to individual file paths via S3 listing (`commands/cdn.rs::purge_cdn`) — vendor API has no wildcard support | Already correct, no change |
| LG U+ / KT (Solbox v3) | **New work** — see below | |

**LG U+/KT findings** (from official PDF API docs, `docs/LG U+ CDN 3.0 OpenAPI v3.pdf`
and `docs/KT CDN3.0 OpenAPI v3.pdf`): the `filelist` body field on both
`Purge by Volume` and `Purge by Domain` only accepts literal file paths — no
wildcard/prefix syntax is documented. Both APIs additionally expose a
`POST /v3/management/service/{serviceName}/purge` ("Purge by Service") endpoint that
flushes the entire service immediately, but "Service에 대한 Purge는 Delivery-cloudcdn
타입 서비스에서만 사용 가능합니다" (only usable for `Delivery-cloudcdn`-type services;
other types must use Purge by Volume).

**Design**:
1. Add `lguplusServiceType` / `ktServiceType` fields to `ProfileConfig`
   (`utils/config.rs`), values `"cloudcdn"` or `"volume"` (default `"volume"`), plus a
   dropdown in `ProfileModal.tsx`.
2. Add `purge_service()` to `lguplus.rs`/`kt.rs`: `POST {endpoint}/v3/management/service/{serviceName}/purge`
   with the same Bearer-token auth, no request body (assumption — the vendor docs list
   the endpoint but do not show its request schema; this is the standard shape for a
   full-flush action and is easy to adjust if the vendor returns a schema error).
3. In `commands/cdn.rs::purge_cdn`, generalize the existing 효성-only wildcard-handling
   branch: for `lguplus`/`kt`, if the wildcard covers the entire bucket root (i.e. the
   caller's `allPrefix` value, `"/*"` with no sub-path) **and** the profile's service
   type is `cloudcdn`, call `purge_service()`. For any other wildcard (a specific
   sub-folder, or service type `volume`), expand to individual file paths via S3
   listing — the same safe pattern already used for 효성.

## Work item 4 — LogPanel start/end time visibility

Timestamps already exist in app state (`TransferItem.startedAt`/`completedAt`) and in
purge batch results, but the LogPanel UI didn't render them.

- `LogPanel.tsx`'s `TransferRow` (used in the "실패 항목" tab): add a
  "시작 HH:MM:SS → 종료 HH:MM:SS" line using the existing `item.startedAt`/`completedAt`.
- CDN purge log lines (`usePurge.ts`, `useTransfer.ts`, `useS3.ts` — everywhere
  `addLog(... , "cdn")` is called after a purge batch) already have `batchStartedAt`/
  `finishedAt` in scope; extend the log message text to include
  "시작 HH:MM:SS · 종료 HH:MM:SS" alongside the existing duration-ms suffix, so delays
  are visible directly in the "작업 로그" tab without opening the log folder.

## Testing / validation approach

- Rust unit tests: extend `hyosung.rs` tests for encoding; add tests for
  `validate_s3_key_segment` (allowed/rejected character cases) and for the new
  `lguplus`/`kt` `purge_service` path selection logic in `commands/cdn.rs`.
- Manual verification (`/verify`-style): run `npm run tauri dev`, create an S3 folder
  with a disallowed character and confirm rejection with a clear message; purge a
  Korean-named object with a 효성 profile (if test credentials available) or inspect
  the outgoing URL via logs; select a folder and confirm the LogPanel now shows
  start/end times.

## Out of scope

- No changes to CloudFront, Akamai, or 효성 purge behavior (already correct).
- No changes to local filesystem naming rules.
- No new UI settings beyond the LG U+/KT service-type dropdown.
