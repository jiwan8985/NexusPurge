# Purge Hardening & Audit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 효성(Hyosung) CDN purge encoding bug, add S3 key character validation, extend folder-purge support to LG U+/KT, and surface existing start/end timestamps in the LogPanel UI.

**Architecture:** Backend (Rust/Tauri) changes are additive to existing adapter/command modules — no new crates, reuse the existing `percent-encoding` dependency and `anyhow`/`Result<T, String>` command conventions. Frontend (React/TypeScript) changes reuse existing store/hook patterns (`useAppStore`, `runtime.invoke`) with no new dependencies.

**Tech Stack:** Rust (Tauri 2, `percent-encoding`, `reqwest`), React 18 + TypeScript + Zustand, Vitest for frontend unit tests, `cargo test` for backend unit tests.

## Global Constraints

- Rust `#[tauri::command]` functions return `Result<T, String>` with `.map_err(|e| e.to_string())` — follow this in every new/modified command (per `CLAUDE.md`).
- No new external UI libraries; CSS Modules only, no inline styles beyond what existing files already do.
- Every new Rust module/function that can be tested without a live network call must have a `#[cfg(test)] mod tests` block (matches existing pattern in `hyosung.rs`/`mod.rs`); do not fabricate HTTP-mocking tests since no mocking crate is present in `Cargo.toml`.
- Korean-language error messages for anything user-facing (matches all existing error strings in this codebase).
- S3 key validation applies only to S3-side names; local filesystem naming keeps OS-native rules (out of scope).

---

### Task 1: Shared percent-encoding helper in `adapters/cdn/mod.rs`

**Files:**
- Modify: `src-tauri/src/adapters/cdn/mod.rs:195-225` (the `build_cdn_url`/`build_cdn_urls` functions and their test module)

**Interfaces:**
- Produces: `pub fn percent_encode_path_segments(raw: &str) -> String` — takes a path with no scheme/domain (e.g. `"assets/한글 파일.txt"` or `"/assets/한글 파일.txt"`), returns the same path with every `/`-separated segment percent-encoded except `- _ . ~`, slashes preserved. Used by Task 2.

- [ ] **Step 1: Write the failing test**

Add to the existing `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/adapters/cdn/mod.rs` (after the existing `build_cdn_url_normalizes_scheme_domain_and_path` test):

```rust
    #[test]
    fn percent_encode_path_segments_encodes_korean_and_space_but_keeps_slashes() {
        let encoded = percent_encode_path_segments("/contents/한글 파일.txt");
        assert_eq!(encoded, "/%ED%95%9C%EA%B8%80%20%ED%8C%8C%EC%9D%BC.txt");
        // leading slash preserved, no double-encoding of the dot
        assert!(encoded.starts_with('/'));
        assert!(!encoded.contains(".txt".to_string().as_str().replace('.', "%2E").as_str()));
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cdn-upload-tool percent_encode_path_segments --manifest-path src-tauri/Cargo.toml`
Expected: FAIL with `cannot find function 'percent_encode_path_segments' in this scope`

- [ ] **Step 3: Extract the encoding logic into the new function and use it from `build_cdn_url`**

Replace the current `build_cdn_url` function (lines 195-218) with:

```rust
/// 경로를 슬래시 단위로 분리하여 각 세그먼트만 percent-encode
/// (슬래시 자체는 그대로 유지, 한글/공백/특수문자만 인코딩)
pub fn percent_encode_path_segments(raw: &str) -> String {
    const PATH_SAFE: &percent_encoding::AsciiSet = &NON_ALPHANUMERIC
        .remove(b'-')
        .remove(b'_')
        .remove(b'.')
        .remove(b'~');

    let had_leading_slash = raw.starts_with('/');
    let trimmed = raw.trim_start_matches('/');
    let encoded = trimmed
        .split('/')
        .map(|seg| utf8_percent_encode(seg, PATH_SAFE).to_string())
        .collect::<Vec<_>>()
        .join("/");

    if had_leading_slash {
        format!("/{}", encoded)
    } else {
        encoded
    }
}

pub fn build_cdn_url(cdn_domain: &str, object_path: &str) -> String {
    let domain = cdn_domain
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/');

    let encoded = percent_encode_path_segments(object_path.trim_start_matches('/'));

    format!("https://{}/{}", domain, encoded)
}
```

Note: `object_path.trim_start_matches('/')` is passed in (no leading slash), so `percent_encode_path_segments` will not add one back — verify this matches the existing `build_cdn_url_normalizes_scheme_domain_and_path` test still passing in the next step.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml adapters::cdn::mod::tests`
Expected: PASS — both `percent_encode_path_segments_encodes_korean_and_space_but_keeps_slashes` and the pre-existing `build_cdn_url_normalizes_scheme_domain_and_path` / `hyosung_requires_service_id` / `hyosung_requires_cdn_domain` tests pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/adapters/cdn/mod.rs
git commit -m "refactor: extract shared percent-encoding helper for CDN purge URLs"
```

---

### Task 2: Fix 효성(Hyosung) CDN purge URL encoding

**Files:**
- Modify: `src-tauri/src/adapters/cdn/hyosung.rs:86-100` (`build_urls`) and its test module at the bottom of the file

**Interfaces:**
- Consumes: `crate::adapters::cdn::percent_encode_path_segments(raw: &str) -> String` from Task 1.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `src-tauri/src/adapters/cdn/hyosung.rs` (after `build_urls_without_scheme_defaults_to_http`):

```rust
    #[test]
    fn build_urls_percent_encodes_korean_and_space() {
        let adapter = HyosungCdnAdapter {
            client:     Client::new(),
            api_key:    "key".into(),
            api_secret: "secret".into(),
            endpoint:   "https://api.xtrmcdn.co.kr:28091".into(),
            service_id: "TID_18656".into(),
            cdn_domain: "https://cdn.example.com".into(),
        };

        let urls = adapter.build_urls(&["contents/한글 파일.txt".to_string()]);

        assert_eq!(
            urls[0],
            "https://cdn.example.com/contents/%ED%95%9C%EA%B8%80%20%ED%8C%8C%EC%9D%BC.txt"
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml build_urls_percent_encodes_korean_and_space`
Expected: FAIL — assertion fails because the current output is the raw un-encoded `"https://cdn.example.com/contents/한글 파일.txt"`.

- [ ] **Step 3: Use the shared encoder in `build_urls`**

Replace the body of `build_urls` (currently lines 86-100) with:

```rust
    fn build_urls(&self, paths: &[String]) -> Vec<String> {
        let raw = self.cdn_domain.trim().trim_end_matches('/');
        let (scheme, domain): (&str, &str) = if let Some(rest) = raw.strip_prefix("https://") {
            ("https", rest)
        } else if let Some(rest) = raw.strip_prefix("http://") {
            ("http", rest)
        } else {
            ("http", raw)
        };

        paths
            .iter()
            .map(|p| {
                let encoded = crate::adapters::cdn::percent_encode_path_segments(
                    p.trim_start_matches('/'),
                );
                format!("{}://{}/{}", scheme, domain, encoded)
            })
            .collect()
    }
```

- [ ] **Step 4: Run all hyosung tests to verify everything passes**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml adapters::cdn::hyosung::tests`
Expected: PASS — `build_urls_normalizes_domain_and_path`, `build_urls_without_scheme_defaults_to_http`, and the new `build_urls_percent_encodes_korean_and_space` all pass.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/adapters/cdn/hyosung.rs
git commit -m "fix: percent-encode Korean/special-character paths in Hyosung CDN purge (fixes purge failures)"
```

---

### Task 3: S3 key character-whitelist validation utility

**Files:**
- Create: `src-tauri/src/utils/validate.rs`
- Modify: `src-tauri/src/utils/mod.rs` (add `pub mod validate;` — check this file exists first; if `src-tauri/src/utils/` has no `mod.rs` and modules are declared in `lib.rs`, add it there instead — see Step 0)

**Interfaces:**
- Produces: `pub fn validate_s3_key_segment(name: &str) -> Result<(), String>` — validates one path segment (no `/`). `pub fn validate_s3_key(key: &str) -> Result<(), String>` — splits on `/` and validates every non-empty segment, used by Task 4.

- [ ] **Step 0: Confirm how `utils` submodules are declared**

Run: `grep -n "mod " src-tauri/src/utils.rs src-tauri/src/lib.rs 2>/dev/null | head -20`

If a `src-tauri/src/utils.rs` (or `utils/mod.rs`) file exists with `pub mod hash;` / `pub mod config;` style declarations, add `pub mod validate;` there. If instead `lib.rs` declares `mod utils;` and `utils` is a folder without its own `mod.rs`/`utils.rs` (Rust 2018+ style, each file in `utils/` auto-discovered via a single `utils.rs`), match whatever pattern `hash.rs` uses today.

- [ ] **Step 1: Write the failing test**

Create `src-tauri/src/utils/validate.rs` with just the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_ascii_letters_digits_and_s3_safe_symbols() {
        assert!(validate_s3_key_segment("my-file_v1.2(final)!'*.txt").is_ok());
    }

    #[test]
    fn rejects_korean_characters() {
        assert!(validate_s3_key_segment("한글파일.txt").is_err());
    }

    #[test]
    fn rejects_space() {
        assert!(validate_s3_key_segment("my file.txt").is_err());
    }

    #[test]
    fn rejects_windows_reserved_characters() {
        for ch in ['\\', ':', '*', '?', '"', '<', '>', '|'] {
            let name = format!("bad{}name", ch);
            assert!(validate_s3_key_segment(&name).is_err(), "expected rejection for {:?}", ch);
        }
    }

    #[test]
    fn validate_s3_key_checks_every_segment() {
        assert!(validate_s3_key("folder/sub-folder/file-1.txt").is_ok());
        assert!(validate_s3_key("폴더/file.txt").is_err());
        assert!(validate_s3_key("folder/파일.txt").is_err());
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml utils::validate::tests`
Expected: FAIL with `cannot find function 'validate_s3_key_segment' in this scope` (compile error).

- [ ] **Step 3: Implement the validation functions**

Add above the test module in `src-tauri/src/utils/validate.rs`:

```rust
/// S3 안전 문자 화이트리스트: 영문 대소문자, 숫자, `! - _ . * ' ( )`
/// (경로 구분자 `/`는 validate_s3_key에서 별도 처리)
fn is_s3_safe_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '!' | '-' | '_' | '.' | '*' | '\'' | '(' | ')')
}

/// 슬래시를 포함하지 않는 단일 경로 세그먼트(폴더명 또는 파일명)를 검증한다.
pub fn validate_s3_key_segment(name: &str) -> Result<(), String> {
    if name.trim().is_empty() {
        return Err("이름은 비워둘 수 없습니다".to_string());
    }
    if let Some(bad_char) = name.chars().find(|c| !is_s3_safe_char(*c)) {
        return Err(format!(
            "'{}'에 허용되지 않는 문자 '{}'가 포함되어 있습니다. \
             영문, 숫자, ! - _ . * ' ( ) 문자만 사용할 수 있습니다.",
            name, bad_char
        ));
    }
    Ok(())
}

/// `/`로 구분된 전체 S3 키의 각 세그먼트를 검증한다. 빈 세그먼트(연속 슬래시, 폴더 표시용
/// trailing slash)는 건너뛴다.
pub fn validate_s3_key(key: &str) -> Result<(), String> {
    for segment in key.split('/') {
        if segment.is_empty() {
            continue;
        }
        validate_s3_key_segment(segment)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml utils::validate::tests`
Expected: PASS — all 5 tests green.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/utils/validate.rs src-tauri/src/utils.rs
git commit -m "feat: add S3 key character-whitelist validation utility"
```

(If Step 0 found a different module-declaration file, `git add` that file instead of `src-tauri/src/utils.rs`.)

---

### Task 4: Wire S3 key validation into commands

**Files:**
- Modify: `src-tauri/src/commands/s3.rs:279-303` (`put_s3_object`)
- Modify: `src-tauri/src/commands/s3.rs:356-380` (`rename_s3_object`)
- Modify: `src-tauri/src/commands/sync.rs:493-575` (`start_uploads`, inside the `for item in items` loop, before spawning the upload task)

**Interfaces:**
- Consumes: `crate::utils::validate::validate_s3_key(key: &str) -> Result<(), String>` from Task 3.

- [ ] **Step 1: Write the failing test (integration-style, via existing test patterns)**

There are no existing `#[tokio::test]` command-level tests for `put_s3_object`/`rename_s3_object` in this codebase (they require a live/mocked S3 adapter), so this task is verified manually per Step 4 below rather than with a new automated test — consistent with the rest of `commands/s3.rs` having no unit tests of its own. Skip to Step 2.

- [ ] **Step 2: Add validation to `put_s3_object`**

In `src-tauri/src/commands/s3.rs`, the function currently reads (lines 279-303):

```rust
#[tauri::command]
pub async fn put_s3_object(
    profile_id:   String,
    key:          String,
    content:      Vec<u8>,
    content_type: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .put_object(&key, content, &content_type)
        .await
        .map_err(|e| e.to_string())
}
```

Add a validation call as the first line of the function body:

```rust
#[tauri::command]
pub async fn put_s3_object(
    profile_id:   String,
    key:          String,
    content:      Vec<u8>,
    content_type: String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    crate::utils::validate::validate_s3_key(&key)?;

    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .put_object(&key, content, &content_type)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 3: Add validation to `rename_s3_object`**

The function currently reads (lines 356-380):

```rust
#[tauri::command]
pub async fn rename_s3_object(
    profile_id: String,
    old_key:    String,
    new_key:    String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .rename_object(&old_key, &new_key)
        .await
        .map_err(|e| e.to_string())
}
```

Add validation of `new_key` (the destination name is what the user is choosing; `old_key` already exists in S3 and does not need re-validation) as the first line:

```rust
#[tauri::command]
pub async fn rename_s3_object(
    profile_id: String,
    old_key:    String,
    new_key:    String,
    store: State<'_, ProfileStore>,
    cache: State<'_, AdapterCache>,
) -> Result<(), String> {
    crate::utils::validate::validate_s3_key(&new_key)?;

    let (creds, region, bucket, endpoint) = store
        .get_connection_info(&profile_id)
        .await
        .map_err(|e| e.to_string())?;

    cache
        .get_or_create(&profile_id, || async {
            S3Adapter::new(&region, &bucket, &creds, endpoint.as_deref())
                .await
        })
        .await
        .map_err(|e| e.to_string())?
        .rename_object(&old_key, &new_key)
        .await
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 4: Add per-file validation to `start_uploads` (reject the individual file, not the whole batch)**

In `src-tauri/src/commands/sync.rs`, inside `start_uploads`, the `for item in items` loop (starting around line 493) currently begins:

```rust
    for item in items {
        let adapter  = adapter.clone();
        let app      = app.clone();
        let successful_purge_targets = successful_purge_targets.clone();
        let permit   = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
```

Add a validation check before spawning, emitting the same `transfer:complete` error event the upload-failure path already uses so the UI treats it identically to any other per-file error:

```rust
    for item in items {
        if let Err(msg) = crate::utils::validate::validate_s3_key(&item.remote_path) {
            let _ = app.emit(
                "transfer:complete",
                TransferCompletePayload {
                    id: item.id.clone(),
                    status: "error".to_string(),
                    cdn_purged: false,
                    cdn_purge_error: None,
                    cdn_invalidation_id: None,
                    error: Some(msg),
                },
            );
            continue;
        }

        let adapter  = adapter.clone();
        let app      = app.clone();
        let successful_purge_targets = successful_purge_targets.clone();
        let permit   = semaphore.clone().acquire_owned().await.expect("Semaphore 오류");

        tasks.spawn(async move {
```

- [ ] **Step 5: Run the full backend test suite to confirm no regressions**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS (all existing + new tests green; `put_s3_object`/`rename_s3_object`/`start_uploads` have no prior unit tests so none should newly fail).

- [ ] **Step 6: Manual verification**

Run: `npm run tauri dev`, connect a profile, try "새 폴더" with a name containing a space or Korean character from the remote panel, and confirm the app shows the Korean rejection message from `validate_s3_key_segment` instead of silently creating the folder.

- [ ] **Step 7: Commit**

```bash
git add src-tauri/src/commands/s3.rs src-tauri/src/commands/sync.rs
git commit -m "feat: enforce S3 key character whitelist on folder create, rename, and upload"
```

---

### Task 5: Frontend client-side S3 key validation

**Files:**
- Create: `src/utils/s3-key.ts`
- Create: `src/utils/s3-key.test.ts`
- Modify: `src/components/layout/Toolbar.tsx:165-184` (`handleNewFolder`) and `:233-270` (`handleRename`)
- Modify: `src/components/panels/RemotePanel.tsx:106-112` (`doRenameRemoteFile`)

**Interfaces:**
- Produces: `export function validateS3KeySegment(name: string): string | null` — returns `null` if valid, or a Korean error message string if invalid. Mirrors the Rust whitelist from Task 3 exactly (kept in sync manually — both are small enough that a shared codegen step is not worth the complexity, per YAGNI).

- [ ] **Step 1: Write the failing test**

Create `src/utils/s3-key.test.ts`:

```typescript
import { describe, expect, it } from "vitest";
import { validateS3KeySegment } from "./s3-key";

describe("validateS3KeySegment", () => {
  it("allows S3-safe characters", () => {
    expect(validateS3KeySegment("my-file_v1.2(final)!'*.txt")).toBeNull();
  });

  it("rejects Korean characters", () => {
    expect(validateS3KeySegment("한글파일.txt")).not.toBeNull();
  });

  it("rejects spaces", () => {
    expect(validateS3KeySegment("my file.txt")).not.toBeNull();
  });

  it("rejects Windows-reserved characters", () => {
    for (const ch of ["\\", ":", "*".repeat(0) + "?", '"', "<", ">", "|"]) {
      expect(validateS3KeySegment(`bad${ch}name`)).not.toBeNull();
    }
  });

  it("rejects empty names", () => {
    expect(validateS3KeySegment("")).not.toBeNull();
    expect(validateS3KeySegment("   ")).not.toBeNull();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npx vitest run src/utils/s3-key.test.ts`
Expected: FAIL — `Failed to resolve import "./s3-key"` (module does not exist yet).

- [ ] **Step 3: Implement `validateS3KeySegment`**

Create `src/utils/s3-key.ts`:

```typescript
// S3 안전 문자 화이트리스트 — src-tauri/src/utils/validate.rs의 규칙과 동일하게 유지할 것
const SAFE_CHAR_RE = /^[A-Za-z0-9!\-_.*'()]+$/;

/** 유효하면 null, 아니면 사용자에게 보여줄 한글 오류 메시지를 반환한다. */
export function validateS3KeySegment(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) {
    return "이름은 비워둘 수 없습니다";
  }
  if (!SAFE_CHAR_RE.test(trimmed)) {
    const badChar = [...trimmed].find((c) => !SAFE_CHAR_RE.test(c));
    return `'${trimmed}'에 허용되지 않는 문자 '${badChar}'가 포함되어 있습니다. 영문, 숫자, ! - _ . * ' ( ) 문자만 사용할 수 있습니다.`;
  }
  return null;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npx vitest run src/utils/s3-key.test.ts`
Expected: PASS — all 5 cases green.

- [ ] **Step 5: Wire the check into "새 폴더" (Toolbar.tsx)**

In `src/components/layout/Toolbar.tsx`, add the import near the other utility imports at the top:

```typescript
import { validateS3KeySegment } from "../../utils/s3-key";
```

The current `handleNewFolder` (lines 165-184) is:

```typescript
  const handleNewFolder = () => {
    setInputDialog({
      title: "새 폴더",
      label: focusedSide === "remote" ? `S3 경로 "${remote.path}" 아래에 새 폴더를 만듭니다.` : "로컬에 새 폴더를 만듭니다.",
      placeholder: "폴더 이름",
      confirmLabel: "만들기",
      onConfirm: async (name) => {
        if (focusedSide === "remote" && isConnected) {
          const prefix = remote.path.endsWith("/") ? remote.path : remote.path + "/";
          await createDirectory(prefix + name + "/");
          triggerRemoteRefresh();
        } else {
          const sep = local.path.includes("\\") ? "\\" : "/";
          const base = local.path.replace(/[/\\]+$/, "");
          await createDir(base + sep + name);
          triggerLocalRefresh();
        }
      },
    });
  };
```

Replace the `onConfirm` body's remote branch to validate first, only for the S3 side (local keeps OS-native rules):

```typescript
  const handleNewFolder = () => {
    setInputDialog({
      title: "새 폴더",
      label: focusedSide === "remote" ? `S3 경로 "${remote.path}" 아래에 새 폴더를 만듭니다.` : "로컬에 새 폴더를 만듭니다.",
      placeholder: "폴더 이름",
      confirmLabel: "만들기",
      onConfirm: async (name) => {
        if (focusedSide === "remote" && isConnected) {
          const invalid = validateS3KeySegment(name);
          if (invalid) {
            addLog("error", invalid, "system");
            return;
          }
          const prefix = remote.path.endsWith("/") ? remote.path : remote.path + "/";
          await createDirectory(prefix + name + "/");
          triggerRemoteRefresh();
        } else {
          const sep = local.path.includes("\\") ? "\\" : "/";
          const base = local.path.replace(/[/\\]+$/, "");
          await createDir(base + sep + name);
          triggerLocalRefresh();
        }
      },
    });
  };
```

This requires `addLog` to be in scope — check the destructured `useAppStore` selector near the top of `Toolbar.tsx` (around line 103-133) already includes other store fields; add `addLog: s.addLog,` to both the destructuring pattern and the selector object if not already present (`LogPanel.tsx` imports it the same way from `useAppStore`).

- [ ] **Step 6: Wire the check into "이름 변경" (Toolbar.tsx)**

The current `handleRename` remote branch (inside lines 233-270) is:

```typescript
  const handleRename = () => {
    if (focusedSide === "remote" && isConnected) {
      const keys = Array.from(remote.selectedPaths);
      if (keys.length !== 1) return;
      const oldKey  = keys[0];
      const oldName = oldKey.replace(/\/$/, "").split("/").pop() ?? oldKey;
      setInputDialog({
        title: "이름 변경",
        label: `"${oldName}"의 새 이름을 입력하세요.`,
        initialValue: oldName,
        placeholder: "새 이름",
        confirmLabel: "변경",
        onConfirm: async (newName) => {
          if (newName === oldName) return;
          const newKey = oldKey.replace(/[^/]*\/?$/, newName + (oldKey.endsWith("/") ? "/" : ""));
          await renameObject(oldKey, newKey);
          triggerRemoteRefresh();
        },
      });
    } else {
```

Add the same validation at the top of `onConfirm`:

```typescript
        onConfirm: async (newName) => {
          if (newName === oldName) return;
          const invalid = validateS3KeySegment(newName);
          if (invalid) {
            addLog("error", invalid, "system");
            return;
          }
          const newKey = oldKey.replace(/[^/]*\/?$/, newName + (oldKey.endsWith("/") ? "/" : ""));
          await renameObject(oldKey, newKey);
          triggerRemoteRefresh();
        },
```

- [ ] **Step 7: Wire the check into `RemotePanel.tsx`'s rename dialog**

Add the same import to `src/components/panels/RemotePanel.tsx`:

```typescript
import { validateS3KeySegment } from "../../utils/s3-key";
```

The current `doRenameRemoteFile` (lines 106-112) is:

```typescript
  const doRenameRemoteFile = async (file: FileItem, newName: string) => {
    const oldName = file.path.replace(/\/$/, "").split("/").pop() ?? file.name;
    if (newName === oldName) return;
    const newKey = file.path.replace(/[^/]*\/?$/, newName + (file.path.endsWith("/") ? "/" : ""));
    await renameObject(file.path, newKey);
    await loadPrefix(remote.path);
  };
```

`RemotePanel.tsx` already destructures `addLog`? Check — it does not currently; add `addLog: s.addLog,` to the `useAppStore` selector at the top of the component (alongside `remote`, `isConnected`, etc., lines 37-57), then update the function:

```typescript
  const doRenameRemoteFile = async (file: FileItem, newName: string) => {
    const oldName = file.path.replace(/\/$/, "").split("/").pop() ?? file.name;
    if (newName === oldName) return;
    const invalid = validateS3KeySegment(newName);
    if (invalid) {
      addLog("error", invalid, "system");
      return;
    }
    const newKey = file.path.replace(/[^/]*\/?$/, newName + (file.path.endsWith("/") ? "/" : ""));
    await renameObject(file.path, newKey);
    await loadPrefix(remote.path);
  };
```

- [ ] **Step 8: Run the full frontend test suite and typecheck**

Run: `npm run test && npm run typecheck`
Expected: PASS — all vitest suites green, no TypeScript errors.

- [ ] **Step 9: Manual verification**

Run: `npm run tauri dev`. In the S3 panel, try creating a folder named `테스트 폴더` — expect an immediate error in the log panel and no round-trip to the backend. Try `test-folder_1` — expect success.

- [ ] **Step 10: Commit**

```bash
git add src/utils/s3-key.ts src/utils/s3-key.test.ts src/components/layout/Toolbar.tsx src/components/panels/RemotePanel.tsx
git commit -m "feat: add client-side S3 key validation for immediate feedback"
```

---

### Task 6: LG U+/KT service type profile field (backend)

**Files:**
- Modify: `src-tauri/src/utils/config.rs:73-96` (`ProfileConfig` struct, LG U+/KT field blocks)
- Modify: `src-tauri/src/utils/config.rs:296-313` (`CdnCredentials::Lguplus`/`Kt` variants)
- Modify: `src-tauri/src/utils/config.rs:561-680` (`get_cdn_credentials` match arms for `"lguplus"`/`"kt"`)

**Interfaces:**
- Produces: `ProfileConfig.lguplus_service_type: Option<String>` and `ProfileConfig.kt_service_type: Option<String>` (serialized as `lguplusServiceType`/`ktServiceType`, values `"cloudcdn"` or `"volume"`, treated as `"volume"` when absent/empty). `CdnCredentials::Lguplus { .. }` and `::Kt { .. }` gain a `service_type: String` field, always `"cloudcdn"` or `"volume"`. Consumed by Task 8/9.

- [ ] **Step 1: Add the profile fields**

In `src-tauri/src/utils/config.rs`, the LG U+ block (lines ~73-84) currently is:

```rust
    // LG U+ CDN — username/password 기반 JWT 인증
    #[serde(rename = "lguplusUsername", skip_serializing_if = "Option::is_none")]
    pub lguplus_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "lguplusPassword", skip_serializing_if = "Option::is_none")]
    pub lguplus_password: Option<String>,
    #[serde(rename = "lguplusServiceName", skip_serializing_if = "Option::is_none")]
    pub lguplus_service_name: Option<String>,
    #[serde(rename = "lguplusVolumeName", skip_serializing_if = "Option::is_none")]
    pub lguplus_volume_name: Option<String>,
    #[serde(rename = "lguplusEndpoint", skip_serializing_if = "Option::is_none")]
    pub lguplus_endpoint: Option<String>,
```

Add a `service_type` field right after `lguplus_endpoint`:

```rust
    // LG U+ CDN — username/password 기반 JWT 인증
    #[serde(rename = "lguplusUsername", skip_serializing_if = "Option::is_none")]
    pub lguplus_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "lguplusPassword", skip_serializing_if = "Option::is_none")]
    pub lguplus_password: Option<String>,
    #[serde(rename = "lguplusServiceName", skip_serializing_if = "Option::is_none")]
    pub lguplus_service_name: Option<String>,
    #[serde(rename = "lguplusVolumeName", skip_serializing_if = "Option::is_none")]
    pub lguplus_volume_name: Option<String>,
    #[serde(rename = "lguplusEndpoint", skip_serializing_if = "Option::is_none")]
    pub lguplus_endpoint: Option<String>,
    /// "cloudcdn" | "volume" (기본 "volume") — cloudcdn이면 전체 Purge 시 Purge by Service 사용 가능
    #[serde(rename = "lguplusServiceType", skip_serializing_if = "Option::is_none")]
    pub lguplus_service_type: Option<String>,
```

Similarly, the KT block (lines ~86-96) currently is:

```rust
    // KT CDN — username/password 기반 JWT 인증
    #[serde(rename = "ktUsername", skip_serializing_if = "Option::is_none")]
    pub kt_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "ktPassword", skip_serializing_if = "Option::is_none")]
    pub kt_password: Option<String>,
    #[serde(rename = "ktServiceName", skip_serializing_if = "Option::is_none")]
    pub kt_service_name: Option<String>,
    #[serde(rename = "ktVolumeName", skip_serializing_if = "Option::is_none")]
    pub kt_volume_name: Option<String>,
    #[serde(rename = "ktEndpoint", skip_serializing_if = "Option::is_none")]
    pub kt_endpoint: Option<String>,
```

Add the matching field:

```rust
    // KT CDN — username/password 기반 JWT 인증
    #[serde(rename = "ktUsername", skip_serializing_if = "Option::is_none")]
    pub kt_username: Option<String>,
    /// keyring에 저장 (JSON 직렬화 제외)
    #[serde(rename = "ktPassword", skip_serializing_if = "Option::is_none")]
    pub kt_password: Option<String>,
    #[serde(rename = "ktServiceName", skip_serializing_if = "Option::is_none")]
    pub kt_service_name: Option<String>,
    #[serde(rename = "ktVolumeName", skip_serializing_if = "Option::is_none")]
    pub kt_volume_name: Option<String>,
    #[serde(rename = "ktEndpoint", skip_serializing_if = "Option::is_none")]
    pub kt_endpoint: Option<String>,
    /// "cloudcdn" | "volume" (기본 "volume") — cloudcdn이면 전체 Purge 시 Purge by Service 사용 가능
    #[serde(rename = "ktServiceType", skip_serializing_if = "Option::is_none")]
    pub kt_service_type: Option<String>,
```

- [ ] **Step 2: Update `CdnCredentials::Lguplus`/`Kt` to carry the service type**

The enum variants (currently, lines ~296-313) are:

```rust
    /// LG U+ CDN (Solbox CDN v3) — JWT 인증
    Lguplus {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    },
    /// KT CDN (Solbox CDN v3) — JWT 인증
    Kt {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    },
```

Add `service_type: String` to both:

```rust
    /// LG U+ CDN (Solbox CDN v3) — JWT 인증
    Lguplus {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
        /// "cloudcdn" | "volume" — 전체 Purge 시 Purge by Service 사용 가능 여부
        service_type: String,
    },
    /// KT CDN (Solbox CDN v3) — JWT 인증
    Kt {
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
        /// "cloudcdn" | "volume" — 전체 Purge 시 Purge by Service 사용 가능 여부
        service_type: String,
    },
```

- [ ] **Step 3: Populate `service_type` in `get_cdn_credentials`**

In the `"lguplus"` match arm (around line 561), the current code is:

```rust
            "lguplus" => {
                let (username, service_name, volume_name, endpoint, cdn_domain) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.lguplus_username.clone().unwrap_or_default(),
                        profile.lguplus_service_name.clone().unwrap_or_default(),
                        profile.lguplus_volume_name.clone().unwrap_or_default(),
                        profile.lguplus_endpoint.clone()
                            .unwrap_or_else(|| "https://api.lgucdn.com".to_owned()),
                        provider_domain(profile, "lguplus").unwrap_or_default(),
                    )
                };
```

Change the tuple to also capture `service_type`:

```rust
            "lguplus" => {
                let (username, service_name, volume_name, endpoint, cdn_domain, service_type) = {
                    let locked = self.profiles.read().await;
                    let profile = locked
                        .iter()
                        .find(|p| p.id == profile_id)
                        .context("Profile not found")?;
                    (
                        profile.lguplus_username.clone().unwrap_or_default(),
                        profile.lguplus_service_name.clone().unwrap_or_default(),
                        profile.lguplus_volume_name.clone().unwrap_or_default(),
                        profile.lguplus_endpoint.clone()
                            .unwrap_or_else(|| "https://api.lgucdn.com".to_owned()),
                        provider_domain(profile, "lguplus").unwrap_or_default(),
                        profile.lguplus_service_type.clone()
                            .filter(|v| !v.trim().is_empty())
                            .unwrap_or_else(|| "volume".to_owned()),
                    )
                };
```

Then update the `Ok(CdnCredentials::Lguplus { .. })` construction later in the same arm to add `service_type,`:

```rust
                Ok(CdnCredentials::Lguplus {
                    username,
                    password,
                    service_name,
                    volume_name,
                    endpoint,
                    cdn_domain,
                    service_type,
                })
```

Apply the identical pattern to the `"kt"` match arm (around line 641): capture `kt_service_type` into the tuple as `service_type`, default `"volume"`, and add `service_type,` to the final `Ok(CdnCredentials::Kt { .. })`.

- [ ] **Step 4: Fix the compile errors this introduces in `adapters/cdn/mod.rs`**

`mod.rs`'s `purge_with_credentials` destructures `CdnCredentials::Lguplus { .. }` / `::Kt { .. }` with named fields (around lines 108-131 and 168-191) and passes them to `LguplusCdnAdapter::new(...)`/`KtCdnAdapter::new(...)`. Since Task 8 will change those adapter constructors' signatures too, leave the destructuring pattern using `..` for now (Rust allows partial destructuring), i.e. confirm the existing:

```rust
        CdnCredentials::Lguplus {
            username,
            password,
            service_name,
            volume_name,
            endpoint,
            cdn_domain,
        } => {
```

still compiles by leaving `service_type` unbound — Rust will error "missing field service_type in pattern" for a non-`..` match, so add `service_type,` (bind it, even if unused in this task) to both the `Lguplus` and `Kt` match patterns in `mod.rs`, and prefix with `service_type: _service_type,` is unnecessary since Task 9 will use it — just bind it as `service_type` and accept an "unused variable" warning for now (it becomes used in Task 9).

- [ ] **Step 5: Run backend build to confirm no compile errors**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds successfully (warnings about unused `service_type` in `mod.rs` are acceptable at this stage — Task 9 uses it).

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/utils/config.rs src-tauri/src/adapters/cdn/mod.rs
git commit -m "feat: add LG U+/KT service type field for full-service purge support"
```

---

### Task 7: LG U+/KT service type dropdown (frontend)

**Files:**
- Modify: `src/types/index.ts:38-45` (`S3Profile` LG U+/KT fields)
- Modify: `src/components/modals/ProfileModal.tsx:69-84` (`FormState`), `:110-119` (`emptyForm`), `:191-203` (`handleEdit`), `:313-325` (`buildProfilePayload`), and the LG U+/KT `<details>` sections (`:932-1042`)

**Interfaces:**
- Produces: `S3Profile.lguplusServiceType?: "cloudcdn" | "volume"`, `S3Profile.ktServiceType?: "cloudcdn" | "volume"` — consumed by the backend fields added in Task 6 (same JSON shape, `camelCase` names already match serde `rename`).

- [ ] **Step 1: Add the type fields**

In `src/types/index.ts`, the LG U+ block (lines 34-39) is:

```typescript
  // LG U+ CDN (Solbox CDN v2)
  lguplusUsername?: string;
  lguplusPassword?: string;    // keyring에 저장, 로드 시 빈 값
  lguplusServiceName?: string;
  lguplusVolumeName?: string;
  lguplusEndpoint?: string;
```

Add `lguplusServiceType`:

```typescript
  // LG U+ CDN (Solbox CDN v2)
  lguplusUsername?: string;
  lguplusPassword?: string;    // keyring에 저장, 로드 시 빈 값
  lguplusServiceName?: string;
  lguplusVolumeName?: string;
  lguplusEndpoint?: string;
  lguplusServiceType?: "cloudcdn" | "volume"; // cloudcdn이면 전체 Purge 시 서비스 전체 즉시 플러시 사용
```

The KT block (lines 40-45) is:

```typescript
  // KT CDN (Solbox CDN v3)
  ktUsername?: string;
  ktPassword?: string;         // keyring에 저장, 로드 시 빈 값
  ktServiceName?: string;
  ktVolumeName?: string;
  ktEndpoint?: string;
```

Add `ktServiceType`:

```typescript
  // KT CDN (Solbox CDN v3)
  ktUsername?: string;
  ktPassword?: string;         // keyring에 저장, 로드 시 빈 값
  ktServiceName?: string;
  ktVolumeName?: string;
  ktEndpoint?: string;
  ktServiceType?: "cloudcdn" | "volume"; // cloudcdn이면 전체 Purge 시 서비스 전체 즉시 플러시 사용
```

- [ ] **Step 2: Add form fields in `ProfileModal.tsx`**

In the `FormState` interface (lines 69-84), after `lguplusEndpoint: string;` add `lguplusServiceType: "cloudcdn" | "volume";`, and after `ktEndpoint: string;` add `ktServiceType: "cloudcdn" | "volume";`:

```typescript
  // LG U+ CDN
  lguplusUsername: string;
  lguplusPassword: string;
  lguplusServiceName: string;
  lguplusVolumeName: string;
  lguplusEndpoint: string;
  lguplusServiceType: "cloudcdn" | "volume";
  // KT CDN
  ktUsername: string;
  ktPassword: string;
  ktServiceName: string;
  ktVolumeName: string;
  ktEndpoint: string;
  ktServiceType: "cloudcdn" | "volume";
```

In `emptyForm()` (lines 87-120), add defaults after the corresponding `lguplusEndpoint: ""`/`ktEndpoint: ""` lines:

```typescript
  lguplusEndpoint: "",
  lguplusServiceType: "volume",
  ...
  ktEndpoint: "",
  ktServiceType: "volume",
```

- [ ] **Step 3: Load the field in `handleEdit`**

In `handleEdit` (lines 191-203), after `lguplusEndpoint: profile.lguplusEndpoint ?? "",` add:

```typescript
      lguplusEndpoint: profile.lguplusEndpoint ?? "",
      lguplusServiceType: profile.lguplusServiceType ?? "volume",
```

and after `ktEndpoint: profile.ktEndpoint ?? "",` add:

```typescript
      ktEndpoint: profile.ktEndpoint ?? "",
      ktServiceType: profile.ktServiceType ?? "volume",
```

- [ ] **Step 4: Include the field when saving in `buildProfilePayload`**

In `buildProfilePayload` (lines 313-325), after `lguplusEndpoint: form.lguplusEndpoint || undefined,` add:

```typescript
    lguplusEndpoint: form.lguplusEndpoint || undefined,
    lguplusServiceType: form.lguplusServiceType,
```

and after `ktEndpoint: form.ktEndpoint || undefined,` add:

```typescript
    ktEndpoint: form.ktEndpoint || undefined,
    ktServiceType: form.ktServiceType,
```

- [ ] **Step 5: Add the dropdown to the LG U+ section**

In the `isLguplus` block (around lines 932-986), the last field before the closing `</>` is the "API 엔드포인트" input:

```typescript
                  <label className={styles.field}>
                    <span>API 엔드포인트 (FQDN)</span>
                    <input
                      value={form.lguplusEndpoint}
                      onChange={setField("lguplusEndpoint")}
                      placeholder="https://api.lgucdn.com (기본값)"
                    />
                  </label>
                </>
              ))}
```

Add a dropdown right after it:

```typescript
                  <label className={styles.field}>
                    <span>API 엔드포인트 (FQDN)</span>
                    <input
                      value={form.lguplusEndpoint}
                      onChange={setField("lguplusEndpoint")}
                      placeholder="https://api.lgucdn.com (기본값)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>서비스 타입</span>
                    <select value={form.lguplusServiceType} onChange={setField("lguplusServiceType")}>
                      <option value="volume">Volume (일반) — 폴더 Purge는 개별 파일로 처리</option>
                      <option value="cloudcdn">Delivery-cloudcdn — 전체 Purge 시 서비스 전체 즉시 플러시 사용</option>
                    </select>
                  </label>
                </>
              ))}
```

- [ ] **Step 6: Add the dropdown to the KT section**

In the `isKt` block (around lines 988-1042), apply the identical pattern after the "API 엔드포인트" input:

```typescript
                  <label className={styles.field}>
                    <span>API 엔드포인트 (FQDN)</span>
                    <input
                      value={form.ktEndpoint}
                      onChange={setField("ktEndpoint")}
                      placeholder="https://api.ktcdn.co.kr (기본값, 계약에 따라 https://api.ktcdn.com)"
                    />
                  </label>
                  <label className={styles.field}>
                    <span>서비스 타입</span>
                    <select value={form.ktServiceType} onChange={setField("ktServiceType")}>
                      <option value="volume">Volume (일반) — 폴더 Purge는 개별 파일로 처리</option>
                      <option value="cloudcdn">Delivery-cloudcdn — 전체 Purge 시 서비스 전체 즉시 플러시 사용</option>
                    </select>
                  </label>
                </>
              ))}
```

- [ ] **Step 7: Run typecheck**

Run: `npm run typecheck`
Expected: PASS — no TypeScript errors. (`setField` is typed to accept `keyof FormState` and works for `<select>` since its signature already accepts `HTMLInputElement | HTMLSelectElement`.)

- [ ] **Step 8: Manual verification**

Run: `npm run tauri dev`, open 프로필 관리, select LG U+ or KT as the CDN provider, confirm the "서비스 타입" dropdown appears and persists after saving and re-opening the profile.

- [ ] **Step 9: Commit**

```bash
git add src/types/index.ts src/components/modals/ProfileModal.tsx
git commit -m "feat: add LG U+/KT service type dropdown to profile form"
```

---

### Task 8: `purge_service()` adapter methods for LG U+/KT

**Files:**
- Modify: `src-tauri/src/adapters/cdn/lguplus.rs:28-43` (constructor) and add a new method after `purge_paths` (currently ending around line 186)
- Modify: `src-tauri/src/adapters/cdn/kt.rs:28-43` (constructor) and add a new method after `purge_paths` (currently ending around line 186)

**Interfaces:**
- Consumes: `service_type: String` field added to the adapter structs in this task (constructor parameter, stored on the struct).
- Produces: `pub async fn purge_service(&self) -> Result<Option<String>>` on both `LguplusCdnAdapter` and `KtCdnAdapter` — returns `Err` immediately if `service_type != "cloudcdn"`, otherwise `POST {endpoint}/v3/management/service/{serviceName}/purge` with Bearer auth and no body, returning any `transactionId` found in the response the same way `purge_paths` already does. Consumed by Task 9.
- Produces: `pub(crate) fn service_purge_url(endpoint: &str, service_name: &str) -> String` (private URL builder, unit-tested) on each adapter file.

- [ ] **Step 1: Write the failing test for the URL builder (LG U+)**

Add to the `#[cfg(test)] mod tests` block at the bottom of `src-tauri/src/adapters/cdn/lguplus.rs` (create the block if it doesn't exist yet — `lguplus.rs` currently has no tests):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_purge_url_has_no_trailing_slash_and_no_body_params() {
        let url = service_purge_url("https://api.lgucdn.com", "my-service");
        assert_eq!(url, "https://api.lgucdn.com/v3/management/service/my-service/purge");
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml adapters::cdn::lguplus::tests`
Expected: FAIL with `cannot find function 'service_purge_url' in this scope`.

- [ ] **Step 3: Add `service_type` to the struct/constructor and implement `service_purge_url` + `purge_service` (LG U+)**

The current struct and constructor (lines 18-43) are:

```rust
pub struct LguplusCdnAdapter {
    client:       Client,
    username:     String,
    password:     String,
    service_name: String,
    volume_name:  String,
    endpoint:       String,
    cdn_domain:     String, // FQDN — volume_name 미지정 시 domain 기반 purge에 사용
}

impl LguplusCdnAdapter {
    pub fn new(
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, username, password, service_name, volume_name, endpoint, cdn_domain })
    }
```

Replace with:

```rust
pub struct LguplusCdnAdapter {
    client:       Client,
    username:     String,
    password:     String,
    service_name: String,
    volume_name:  String,
    endpoint:       String,
    cdn_domain:     String, // FQDN — volume_name 미지정 시 domain 기반 purge에 사용
    service_type:   String, // "cloudcdn" | "volume" — cloudcdn만 Purge by Service 가능
}

/// `/v3/management/service/{serviceName}/purge` URL 구성 (전체 서비스 즉시 플러시, body 없음)
pub(crate) fn service_purge_url(endpoint: &str, service_name: &str) -> String {
    format!("{}/v3/management/service/{}/purge", endpoint.trim_end_matches('/'), service_name)
}

impl LguplusCdnAdapter {
    pub fn new(
        username:     String,
        password:     String,
        service_name: String,
        volume_name:  String,
        endpoint:     String,
        cdn_domain:   String,
        service_type: String,
    ) -> Result<Self> {
        let client = Client::builder()
            .use_native_tls()
            .build()
            .context("HTTP 클라이언트 생성 실패")?;
        let endpoint = endpoint.trim().trim_end_matches('/').to_owned();
        Ok(Self { client, username, password, service_name, volume_name, endpoint, cdn_domain, service_type })
    }
```

Then add `purge_service` right after the existing `purge_paths` method (before `get_transaction_status`):

```rust
    /// 서비스 전체 즉시 플러시 — Delivery-cloudcdn 타입 서비스에서만 지원됨
    /// (LG U+ CDN 3.0 OpenAPI v3 문서, "Purge by Service": "그 외 서비스는
    /// Purge by Volume 기능을 사용합니다.") body 없음 — 응답 스키마가 문서에
    /// 없어 filelist 기반 응답과 동일한 형태(transactionId 포함)로 가정한다.
    pub async fn purge_service(&self) -> Result<Option<String>> {
        if self.service_type != "cloudcdn" {
            return Err(anyhow::anyhow!(
                "LG U+ CDN 서비스 전체 Purge는 서비스 타입이 cloudcdn인 경우에만 지원됩니다 \
                 (현재: {}). 프로필에서 서비스 타입을 확인하세요.",
                self.service_type
            ));
        }
        if self.service_name.trim().is_empty() {
            return Err(anyhow::anyhow!("LG U+ CDN Purge에는 Service Name이 필요합니다"));
        }

        let token = self.acquire_token().await?;
        let url = service_purge_url(&self.endpoint, &self.service_name);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .context("LG U+ CDN 서비스 전체 Purge 요청 실패")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(anyhow::anyhow!(
                "LG U+ CDN 서비스 전체 Purge 실패 (HTTP {}): {}",
                status,
                text
            ));
        }

        let text = resp.text().await.unwrap_or_default();
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            let tid = json["transactionId"]
                .as_str()
                .or_else(|| json["transid"].as_str())
                .map(ToOwned::to_owned)
                .or_else(|| json["transid"].as_u64().map(|v| v.to_string()));
            if let Some(tid) = tid {
                tracing::info!("LG U+ CDN 서비스 전체 Purge 요청 수락: transactionId={}", tid);
                return Ok(Some(tid));
            }
        }

        tracing::info!("LG U+ CDN 서비스 전체 Purge 완료 (서비스: {})", self.service_name);
        Ok(None)
    }
```

- [ ] **Step 4: Repeat Steps 1-3 for KT (`kt.rs`)**

Apply the identical changes to `src-tauri/src/adapters/cdn/kt.rs`: add `service_type: String` to `KtCdnAdapter`, add the `service_type` parameter to `KtCdnAdapter::new`, add the same `service_purge_url` free function (it is identical logic — do not import from `lguplus.rs`, duplicate it in `kt.rs` since the two adapter modules are intentionally independent per the existing codebase pattern of no cross-adapter imports), and add `purge_service` with the KT-specific error message ("KT CDN 서비스 전체 Purge는..."). Add the matching test:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_purge_url_has_no_trailing_slash_and_no_body_params() {
        let url = service_purge_url("https://api.ktcdn.co.kr", "my-service");
        assert_eq!(url, "https://api.ktcdn.co.kr/v3/management/service/my-service/purge");
    }
}
```

- [ ] **Step 5: Run tests to verify both pass**

Run: `cargo test -p cdn-upload-tool --manifest-path src-tauri/Cargo.toml adapters::cdn::lguplus::tests adapters::cdn::kt::tests`
Expected: PASS.

- [ ] **Step 6: Fix call sites broken by the new constructor parameter**

Run: `cargo build --manifest-path src-tauri/Cargo.toml 2>&1 | grep -A3 "this function takes"`
Expected: compile errors at every `LguplusCdnAdapter::new(...)`/`KtCdnAdapter::new(...)` call site (in `commands/cdn.rs::test_cdn_connection` and `adapters/cdn/mod.rs::purge_with_credentials`). Update each call to pass `service_type` — for `mod.rs`, use the `service_type` field already destructured from `CdnCredentials::Lguplus`/`::Kt` in Task 6 Step 4; for `commands/cdn.rs::test_cdn_connection`, the `CdnCredentials::Lguplus { username, password, service_name, volume_name, endpoint, cdn_domain }` destructuring pattern (around line 159-173) needs `service_type` added to the pattern and passed through to `LguplusCdnAdapter::new(...)` (repeat for `Kt` around line 191-205).

- [ ] **Step 7: Run full backend build and test suite**

Run: `cargo build --manifest-path src-tauri/Cargo.toml && cargo test --manifest-path src-tauri/Cargo.toml`
Expected: builds and all tests pass.

- [ ] **Step 8: Commit**

```bash
git add src-tauri/src/adapters/cdn/lguplus.rs src-tauri/src/adapters/cdn/kt.rs src-tauri/src/adapters/cdn/mod.rs src-tauri/src/commands/cdn.rs
git commit -m "feat: add full-service purge (Purge by Service) for LG U+/KT cloudcdn services"
```

---

### Task 9: Generalize folder-purge wildcard handling in `commands/cdn.rs::purge_cdn`

**Files:**
- Modify: `src-tauri/src/commands/cdn.rs:332-424` (`purge_cdn`, the wildcard-expansion block currently gated on `provider == "hyosung"`)

**Interfaces:**
- Consumes: `HyosungCdnAdapter`/`LguplusCdnAdapter`/`KtCdnAdapter` methods and the `S3Adapter::list_keys_recursive` already used in this function; `CdnCredentials::Lguplus.service_type`/`CdnCredentials::Kt.service_type` from Task 6.

- [ ] **Step 1: Read the current wildcard-handling block**

The current code (lines 352-423) is:

```rust
    // 효성은 와일드카드 미지원(노드 purge 데몬이 "*" URL에 502 반환)
    // → 폴더/전체 Purge("prefix/*")를 S3 목록 조회로 개별 파일 경로로 확장
    let effective_paths: Vec<String> = if provider == "hyosung"
        && paths.iter().any(|p| p.ends_with('*'))
    {
        let (creds, region, bucket, endpoint) = store
            .get_connection_info(&profile_id)
            .await
            .map_err(|e| e.to_string())?;
        let adapter = cache
            .get_or_create(&profile_id, || async {
                crate::adapters::storage::s3::S3Adapter::new(
                    &region, &bucket, &creds, endpoint.as_deref(),
                )
                .await
            })
            .await
            .map_err(|e| e.to_string())?;

        let mut expanded = Vec::new();
        for p in &paths {
            if let Some(prefix) = p.strip_suffix('*') {
                let prefix = prefix.trim_start_matches('/');
                match adapter.list_keys_recursive(prefix).await {
                    Ok(keys) => {
                        expanded.extend(keys.into_iter().filter(|k| !k.ends_with('/')));
                    }
                    Err(e) => {
                        return Ok(CdnPurgeResult {
                            success: false,
                            provider,
                            invalidation_id: None,
                            paths,
                            purged_at: None,
                            error: Some(format!(
                                "효성 폴더 Purge 확장 실패 (S3 목록 조회 오류): {}",
                                e
                            )),
                            request_endpoint: None,
                            duration_ms: None,
                        });
                    }
                }
            } else {
                expanded.push(p.clone());
            }
        }
        expanded.sort();
        expanded.dedup();

        if expanded.is_empty() {
            // 빈 폴더 — 무효화할 파일 없음, 성공 처리
            return Ok(CdnPurgeResult {
                success: true,
                provider,
                invalidation_id: None,
                paths,
                purged_at: Some(chrono::Utc::now().to_rfc3339()),
                error: None,
                request_endpoint: None,
                duration_ms: None,
            });
        }
        tracing::info!(
            "효성 폴더 Purge 확장: {}개 와일드카드 → {}개 파일 경로",
            paths.len(),
            expanded.len()
        );
        expanded
    } else {
        paths.clone()
    };
```

- [ ] **Step 2: Add a "full bucket root wildcard" detector and a LG U+/KT full-service-purge branch before the existing expansion logic**

Replace the whole block above with:

```rust
    // 효성은 와일드카드 미지원(노드 purge 데몬이 "*" URL에 502 반환)
    // LG U+/KT는 filelist에 와일드카드/prefix를 지원하지 않음 (공식 API 문서 확인)
    // → 폴더/전체 Purge("prefix/*")는 개별 파일로 확장하되, "전체 Purge"(버킷 루트 전체,
    // "/*" 단일 항목)이고 서비스 타입이 cloudcdn이면 LG U+/KT는 Purge by Service(전체 즉시
    // 플러시)를 사용해 대량 S3 목록 조회를 피한다.
    let is_full_root_wildcard = paths.len() == 1 && paths[0].trim_start_matches('/') == "*";

    if (provider == "lguplus" || provider == "kt") && is_full_root_wildcard {
        let cloudcdn = matches!(
            &cdn_creds,
            crate::utils::config::CdnCredentials::Lguplus { service_type, .. } if service_type == "cloudcdn"
        ) || matches!(
            &cdn_creds,
            crate::utils::config::CdnCredentials::Kt { service_type, .. } if service_type == "cloudcdn"
        );

        if cloudcdn {
            let started = std::time::Instant::now();
            let result = match &cdn_creds {
                crate::utils::config::CdnCredentials::Lguplus {
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
                } => {
                    crate::adapters::cdn::lguplus::LguplusCdnAdapter::new(
                        username.clone(), password.clone(), service_name.clone(),
                        volume_name.clone(), endpoint.clone(), cdn_domain.clone(), service_type.clone(),
                    )
                    .map_err(|e| e.to_string())?
                    .purge_service()
                    .await
                }
                crate::utils::config::CdnCredentials::Kt {
                    username, password, service_name, volume_name, endpoint, cdn_domain, service_type,
                } => {
                    crate::adapters::cdn::kt::KtCdnAdapter::new(
                        username.clone(), password.clone(), service_name.clone(),
                        volume_name.clone(), endpoint.clone(), cdn_domain.clone(), service_type.clone(),
                    )
                    .map_err(|e| e.to_string())?
                    .purge_service()
                    .await
                }
                _ => unreachable!(),
            };
            let duration_ms = Some(started.elapsed().as_millis() as u64);
            return Ok(match result {
                Ok(id) => CdnPurgeResult {
                    success: true,
                    provider,
                    invalidation_id: id,
                    paths,
                    purged_at: Some(chrono::Utc::now().to_rfc3339()),
                    error: None,
                    request_endpoint: None,
                    duration_ms,
                },
                Err(e) => CdnPurgeResult {
                    success: false,
                    provider,
                    invalidation_id: None,
                    paths,
                    purged_at: None,
                    error: Some(e.to_string()),
                    request_endpoint: None,
                    duration_ms,
                },
            });
        }
        // service_type이 volume이면 아래 일반 확장 로직으로 계속 진행
    }

    let effective_paths: Vec<String> = if (provider == "hyosung" || provider == "lguplus" || provider == "kt")
        && paths.iter().any(|p| p.ends_with('*'))
    {
        let (creds, region, bucket, endpoint) = store
            .get_connection_info(&profile_id)
            .await
            .map_err(|e| e.to_string())?;
        let adapter = cache
            .get_or_create(&profile_id, || async {
                crate::adapters::storage::s3::S3Adapter::new(
                    &region, &bucket, &creds, endpoint.as_deref(),
                )
                .await
            })
            .await
            .map_err(|e| e.to_string())?;

        let mut expanded = Vec::new();
        for p in &paths {
            if let Some(prefix) = p.strip_suffix('*') {
                let prefix = prefix.trim_start_matches('/');
                match adapter.list_keys_recursive(prefix).await {
                    Ok(keys) => {
                        expanded.extend(keys.into_iter().filter(|k| !k.ends_with('/')));
                    }
                    Err(e) => {
                        return Ok(CdnPurgeResult {
                            success: false,
                            provider,
                            invalidation_id: None,
                            paths,
                            purged_at: None,
                            error: Some(format!(
                                "폴더 Purge 확장 실패 (S3 목록 조회 오류): {}",
                                e
                            )),
                            request_endpoint: None,
                            duration_ms: None,
                        });
                    }
                }
            } else {
                expanded.push(p.clone());
            }
        }
        expanded.sort();
        expanded.dedup();

        if expanded.is_empty() {
            // 빈 폴더 — 무효화할 파일 없음, 성공 처리
            return Ok(CdnPurgeResult {
                success: true,
                provider,
                invalidation_id: None,
                paths,
                purged_at: Some(chrono::Utc::now().to_rfc3339()),
                error: None,
                request_endpoint: None,
                duration_ms: None,
            });
        }
        tracing::info!(
            "[{}] 폴더 Purge 확장: {}개 와일드카드 → {}개 파일 경로",
            provider,
            paths.len(),
            expanded.len()
        );
        expanded
    } else {
        paths.clone()
    };
```

Note this reuses `provider`, `paths`, `store`, `cache`, `profile_id`, `cdn_creds` already bound earlier in `purge_cdn` (see the full existing function body for their definitions) — no new parameters needed.

- [ ] **Step 2: Verify the code compiles**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: builds successfully. Fix any borrow-checker complaints about `cdn_creds` being moved vs. referenced — `cdn_creds` is used both in the new `matches!`/branch (by reference) and later in the existing `cdn::purge_with_credentials(&distribution_id, &normalized_paths, cdn_creds).await` call (by value) further down in the function; since the new code returns early inside its `if cloudcdn` branch, the later move of `cdn_creds` only happens on the non-early-return path, which is fine, but double check the `match &cdn_creds { ... }` borrows are dropped before that later move (they are, since they're scoped inside the `if provider == ... { ... }` block that returns).

- [ ] **Step 3: Run the full backend test suite**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS.

- [ ] **Step 4: Manual verification**

Run: `npm run tauri dev` with an LG U+ or KT test profile (if credentials available) set to service type `volume`; select a sub-folder and Purge — confirm the log panel shows "폴더 Purge 확장" with an accurate count instead of an error. If a `cloudcdn`-type test profile is available, run "전체 Purge" at the bucket root and confirm a single fast request completes rather than a full S3 listing.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/cdn.rs
git commit -m "feat: generalize folder-purge expansion to LG U+/KT, add cloudcdn full-service purge path"
```

---

### Task 10: LogPanel — show transfer start/end time

**Files:**
- Modify: `src/components/log/LogPanel.tsx:48-76` (`TransferRow`)
- Modify: `src/components/log/LogPanel.module.css` (add a small time-range style)

**Interfaces:**
- Consumes: `TransferItem.startedAt?: string`, `TransferItem.completedAt?: string` (already populated by `useTransfer.ts`, no data-layer change needed).

- [ ] **Step 1: Add a time-formatting helper and render it in `TransferRow`**

The current `TransferRow` (lines 48-76) is:

```typescript
function TransferRow({ item, onRetry }: { item: TransferItem; onRetry?: (item: TransferItem) => void }) {
  const statusLabel: Record<TransferItem["status"], string> = {
    pending:     "대기",
    uploading:   "업로드",
    downloading: "다운로드",
    hashing:     "검증",
    skipped:     "건너뜀",
    overwriting: "교체",
    complete:    "완료",
    canceled:    "취소",
    error:       "오류",
  };

  return (
    <div className={`${styles.transferRow} ${item.status === "error" ? styles.transferError : ""}`}>
      <span className={styles.tFileName} title={item.localPath}>{item.fileName}</span>
      <span className={`${styles.tStatus} ${styles[`ts_${item.status}`]}`}>
        {statusLabel[item.status]}
        {item.cdnPurged && " + CDN"}
      </span>
      <span className={styles.tSize}>{item.transferredBytes > 0 ? fmtSize(item.transferredBytes) : "-"}</span>
      {item.status === "error" && onRetry && (
        <button className={styles.retryBtn} onClick={() => onRetry(item)} title={item.error ?? "재시도"}>
          재시도
        </button>
      )}
    </div>
  );
}
```

Replace with a version that adds a formatted time-range column between the status and size:

```typescript
function fmtTime(iso?: string) {
  if (!iso) return "-";
  return new Date(iso).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}

function TransferRow({ item, onRetry }: { item: TransferItem; onRetry?: (item: TransferItem) => void }) {
  const statusLabel: Record<TransferItem["status"], string> = {
    pending:     "대기",
    uploading:   "업로드",
    downloading: "다운로드",
    hashing:     "검증",
    skipped:     "건너뜀",
    overwriting: "교체",
    complete:    "완료",
    canceled:    "취소",
    error:       "오류",
  };

  return (
    <div className={`${styles.transferRow} ${item.status === "error" ? styles.transferError : ""}`}>
      <span className={styles.tFileName} title={item.localPath}>{item.fileName}</span>
      <span className={`${styles.tStatus} ${styles[`ts_${item.status}`]}`}>
        {statusLabel[item.status]}
        {item.cdnPurged && " + CDN"}
      </span>
      <span className={styles.tTimeRange} title={`시작: ${fmtTime(item.startedAt)} / 종료: ${fmtTime(item.completedAt)}`}>
        {fmtTime(item.startedAt)} → {fmtTime(item.completedAt)}
      </span>
      <span className={styles.tSize}>{item.transferredBytes > 0 ? fmtSize(item.transferredBytes) : "-"}</span>
      {item.status === "error" && onRetry && (
        <button className={styles.retryBtn} onClick={() => onRetry(item)} title={item.error ?? "재시도"}>
          재시도
        </button>
      )}
    </div>
  );
}
```

- [ ] **Step 2: Add the CSS class**

Append to `src/components/log/LogPanel.module.css` (after the existing `.tSize` rule, around line 250):

```css
.tTimeRange {
  flex-shrink: 0;
  color: var(--color-text-tertiary);
  font-family: var(--font-family-mono);
  font-variant-numeric: tabular-nums;
  font-size: 9px;
  white-space: nowrap;
}
```

- [ ] **Step 3: Run typecheck**

Run: `npm run typecheck`
Expected: PASS.

- [ ] **Step 4: Manual verification**

Run: `npm run tauri dev`, trigger an upload that fails (e.g. disconnect network mid-upload, or use an invalid remote path), open the "실패 항목" tab, confirm each row shows "시작 HH:MM:SS → 종료 HH:MM:SS".

- [ ] **Step 5: Commit**

```bash
git add src/components/log/LogPanel.tsx src/components/log/LogPanel.module.css
git commit -m "feat: show start/end time in LogPanel failed-transfer rows"
```

---

### Task 11: CDN purge log messages — show start/end time inline

**Files:**
- Modify: `src/hooks/usePurge.ts:74-79` (the per-batch `addLog` success/failure lines)
- Modify: `src/hooks/useTransfer.ts:338-343` (the per-batch `addLog` success line inside `startUpload`'s `purgeOneProvider`)
- Modify: `src/hooks/useS3.ts:90-96` (the delete-purge `addLog` success line)

**Interfaces:**
- Consumes: existing local variables already in scope at each call site (`batchStartedAt`/`finishedAt` in `usePurge.ts`; a new local timestamp captured immediately before/after each `purge_cdn` call in `useTransfer.ts`/`useS3.ts`, matching the pattern already used in `usePurge.ts`).

- [ ] **Step 1: Add a shared time formatter**

Create `src/utils/format-time.ts`:

```typescript
/** 로그 문자열에 쓰는 HH:MM:SS 포맷터 — LogPanel과 동일한 표시 형식 유지 */
export function fmtClockTime(iso: string): string {
  return new Date(iso).toLocaleTimeString("ko-KR", { hour: "2-digit", minute: "2-digit", second: "2-digit" });
}
```

- [ ] **Step 2: Use it in `usePurge.ts`**

The current code (lines 61-79) is:

```typescript
      for (let i = 0; i < batchArrays.length; i++) {
        const batch = batchArrays[i];
        const batchLabel = totalBatches > 1 ? ` (배치 ${i + 1}/${totalBatches})` : "";
        const batchStartedAt = new Date().toISOString();

        try {
          const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
            profileId: profile.id,
            provider,
            distributionId,
            paths: batch,
          });

          batchResults.push({
            paths: batch,
            success: result.success,
            invalidationId: result.invalidationId ?? undefined,
            error: result.error ?? undefined,
            startedAt: batchStartedAt,
            finishedAt: new Date().toISOString(),
            requestEndpoint: result.requestEndpoint,
            durationMs: result.durationMs,
          });

          if (result.success) {
            const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
            const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
            addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}`, "cdn");
          } else {
            failedCount += batch.length;
            addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${result.error}`, "cdn");
          }
        } catch (err) {
```

Add the import at the top of `src/hooks/usePurge.ts`:

```typescript
import { fmtClockTime } from "../utils/format-time";
```

Change the success log line to include the time range, using the `batchStartedAt`/`finishedAt` values already computed just above it:

```typescript
          const finishedAtIso = new Date().toISOString();
          batchResults.push({
            paths: batch,
            success: result.success,
            invalidationId: result.invalidationId ?? undefined,
            error: result.error ?? undefined,
            startedAt: batchStartedAt,
            finishedAt: finishedAtIso,
            requestEndpoint: result.requestEndpoint,
            durationMs: result.durationMs,
          });

          if (result.success) {
            const inv = result.invalidationId ? ` (${result.invalidationId})` : "";
            const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
            const timeRange = ` (시작 ${fmtClockTime(batchStartedAt)} · 종료 ${fmtClockTime(finishedAtIso)})`;
            addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}${timeRange}`, "cdn");
          } else {
            failedCount += batch.length;
            addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${result.error}`, "cdn");
          }
```

- [ ] **Step 3: Use it in `useTransfer.ts`**

The current code (lines 336-341, inside `purgeOneProvider`) is:

```typescript
              if (success) {
                const inv = invalidationId ? ` (${invalidationId})` : "";
                const dur = durationMs !== undefined ? ` [${durationMs}ms]` : "";
                addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}`, "cdn");
              } else {
                addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${error}`, "cdn");
              }
```

Look a few lines above this block (around line 313) for the `for` loop over batches — add a `batchStartedAt` capture right before the `try` block that calls `purge_cdn` (mirroring `usePurge.ts`), and a `finishedAtIso` right after. The loop currently is:

```typescript
            for (let i = 0; i < purgePaths.length; i += purgeBatchSize) {
              const batch = purgePaths.slice(i, i + purgeBatchSize);
              const batchLabel = totalBatches > 1 ? ` (배치 ${Math.floor(i / purgeBatchSize) + 1}/${totalBatches})` : "";
              let success = false;
              let invalidationId: string | undefined;
              let error: string | undefined;
              let requestEndpoint: string | undefined;
              let durationMs: number | undefined;
              try {
```

Change to:

```typescript
            for (let i = 0; i < purgePaths.length; i += purgeBatchSize) {
              const batch = purgePaths.slice(i, i + purgeBatchSize);
              const batchLabel = totalBatches > 1 ? ` (배치 ${Math.floor(i / purgeBatchSize) + 1}/${totalBatches})` : "";
              const batchStartedAt = new Date().toISOString();
              let success = false;
              let invalidationId: string | undefined;
              let error: string | undefined;
              let requestEndpoint: string | undefined;
              let durationMs: number | undefined;
              try {
```

Then update the log line to:

```typescript
              const finishedAtIso = new Date().toISOString();
              if (success) {
                const inv = invalidationId ? ` (${invalidationId})` : "";
                const dur = durationMs !== undefined ? ` [${durationMs}ms]` : "";
                const timeRange = ` (시작 ${fmtClockTime(batchStartedAt)} · 종료 ${fmtClockTime(finishedAtIso)})`;
                addLog("success", `[${label}] CDN Purge 완료${batchLabel}: ${batch.length}개${inv}${dur}${timeRange}`, "cdn");
              } else {
                addLog("error", `[${label}] CDN Purge 실패${batchLabel}: ${error}`, "cdn");
              }
```

Add the import at the top of `src/hooks/useTransfer.ts`:

```typescript
import { fmtClockTime } from "../utils/format-time";
```

- [ ] **Step 4: Use it in `useS3.ts`**

The current code (lines 76-100, inside `deleteObjects`) is:

```typescript
          await Promise.all(providers.map(async (provider) => {
            const label = CDN_LABELS[provider];
            try {
              const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
                profileId: activeProfile.id,
                provider,
                distributionId: cdnDistributionIdFor(activeProfile, provider) ?? "",
                paths: purgePaths,
              });
              purgeEntries.push({
                provider, paths: purgePaths,
                success: result.success, invalidationId: result.invalidationId ?? undefined, error: result.error ?? undefined,
                requestEndpoint: result.requestEndpoint, durationMs: result.durationMs,
              });
              if (result.success) {
                const id = result.invalidationId ? ` (${result.invalidationId})` : "";
                const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
                addLog("success", `[${label}] Delete CDN purge completed: ${purgePaths.length}${id}${dur}`, "cdn");
              } else {
                addLog("error", `[${label}] Delete CDN purge failed: ${result.error}`, "cdn");
              }
            } catch (err) {
```

Change to capture a start time before the call and include it in the success message:

```typescript
          await Promise.all(providers.map(async (provider) => {
            const label = CDN_LABELS[provider];
            const batchStartedAt = new Date().toISOString();
            try {
              const result = await runtime.invoke<CdnPurgeResult>("purge_cdn", {
                profileId: activeProfile.id,
                provider,
                distributionId: cdnDistributionIdFor(activeProfile, provider) ?? "",
                paths: purgePaths,
              });
              purgeEntries.push({
                provider, paths: purgePaths,
                success: result.success, invalidationId: result.invalidationId ?? undefined, error: result.error ?? undefined,
                requestEndpoint: result.requestEndpoint, durationMs: result.durationMs,
              });
              const finishedAtIso = new Date().toISOString();
              if (result.success) {
                const id = result.invalidationId ? ` (${result.invalidationId})` : "";
                const dur = result.durationMs !== undefined ? ` [${result.durationMs}ms]` : "";
                const timeRange = ` (시작 ${fmtClockTime(batchStartedAt)} · 종료 ${fmtClockTime(finishedAtIso)})`;
                addLog("success", `[${label}] Delete CDN purge completed: ${purgePaths.length}${id}${dur}${timeRange}`, "cdn");
              } else {
                addLog("error", `[${label}] Delete CDN purge failed: ${result.error}`, "cdn");
              }
            } catch (err) {
```

Add the import at the top of `src/hooks/useS3.ts`:

```typescript
import { fmtClockTime } from "../utils/format-time";
```

- [ ] **Step 5: Run typecheck and existing test suite**

Run: `npm run typecheck && npm run test`
Expected: PASS.

- [ ] **Step 6: Manual verification**

Run: `npm run tauri dev`, upload an overwriting file to trigger a purge, and confirm the "작업 로그" tab's CDN Purge success line now reads e.g. `[CloudFront] CDN Purge 완료: 3개 (I2J3K4) [420ms] (시작 14:02:11 · 종료 14:02:11)`.

- [ ] **Step 7: Commit**

```bash
git add src/utils/format-time.ts src/hooks/usePurge.ts src/hooks/useTransfer.ts src/hooks/useS3.ts
git commit -m "feat: show purge start/end time inline in CDN log messages"
```

---

## Self-Review

**Spec coverage:**
- Work item 1 (S3 key validation) → Tasks 3, 4, 5. ✓
- Work item 2 (Hyosung fix) → Tasks 1, 2. ✓
- Work item 3 (LG U+/KT folder purge) → Tasks 6, 7, 8, 9. ✓
- Work item 4 (LogPanel timestamps) → Tasks 10, 11. ✓
- "Already implemented" items (audit log, multi-thread upload, properties dialog, per-type log files) → no tasks needed, per spec. ✓

**Placeholder scan:** no TBD/TODO markers; every step has literal code or an exact command.

**Type consistency:** `validate_s3_key`/`validate_s3_key_segment` names match between Task 3 (definition) and Task 4 (usage). `percent_encode_path_segments` matches between Task 1 (definition) and Task 2 (usage). `service_type` field name and `CdnCredentials::Lguplus`/`::Kt` shape match across Tasks 6, 8, 9. `fmtClockTime` matches between Task 11's Step 1 (definition) and Steps 2-4 (usage). `lguplusServiceType`/`ktServiceType` match between Task 6 (Rust serde rename) and Task 7 (TypeScript field).

**Task ordering:** Task 6 must run before Task 8 (adapter constructors need `service_type` param) and Task 9 (uses `CdnCredentials` field + adapter method). Task 8 must run before Task 9 (uses `purge_service()`). Task 1 must run before Task 2 (uses the shared encoder). Tasks 3 before 4 before 5 (validation utility → backend wiring → frontend mirror). Task 10 and 11 are independent of every other task and each other.
