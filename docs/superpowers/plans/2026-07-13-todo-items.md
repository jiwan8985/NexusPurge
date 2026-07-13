# TODO 항목 (프로필 잠금 · 폴더 DnD · 효성 DNS 진단) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** TODO 파일의 4개 항목 구현 — (1) 모든 저장 프로필 정보 열람 차단, (2) 패널 간 폴더 DnD 수정 + Windows 탐색기 드랍 업로드, (3·4) 효성 CDN URL DNS 미등록 진단·안내.

**Architecture:** 스펙 `docs/superpowers/specs/2026-07-13-todo-items-design.md` 참조. 패널 간 드래그를 HTML5 DnD에서 pointer 이벤트 기반으로 교체하고 `dragDropEnabled: true`로 전환해 탐색기 드랍(실제 경로)을 Tauri 네이티브 이벤트로 수신한다. 프로필은 저장 후 write-only(연결/테스트/내보내기/삭제만). `inspect_url`은 요청 전 DNS 사전 조회로 NXDOMAIN을 즉시 진단한다.

**Tech Stack:** React 18 + TypeScript + Zustand 4 + CSS Modules / Tauri 2 (Rust, tokio, reqwest) / vitest + @testing-library/react / cargo test

## Global Constraints

- 패키지 매니저는 **pnpm** (`pnpm test`, `pnpm typecheck`, `pnpm tauri dev`). 테스트는 `vitest` (jsdom), Rust는 `cargo test` (src-tauri에서 실행).
- `invoke()` 호출은 hooks에서만. 컴포넌트 직접 호출 금지 (기존 ProfileModal의 `runtime.invoke` 직접 호출은 기존 패턴 유지 범위 내에서만 허용).
- CSS Modules만 사용, 인라인 스타일 금지 (imperative하게 생성하는 드래그 고스트 DOM은 global.css 클래스 사용).
- TS 타입(`src/types/index.ts`)과 Rust serde 구조체 필드명 동기화 필수.
- Rust 커맨드 반환은 `Result<T, String>`, 에러 메시지는 한국어.
- 커밋 메시지 끝에 `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` 추가.

---

### Task 1: Rust — `inspect_url` DNS 사전 진단 + 오류 분류

**Files:**
- Modify: `src-tauri/src/commands/cdn.rs` (UrlInspection 구조체 ~line 630, `inspect_url` ~line 648, `mod tests` ~line 691)

**Interfaces:**
- Produces: `UrlInspection`에 `error_kind: Option<String>` 필드 추가 (serde rename `errorKind`, 값: `"dns" | "timeout" | "connect" | "tls" | "other"`). Task 2가 TS 타입으로 동기화.
- Produces: 순수 함수 `inspect_target(url: &str) -> Option<(String, u16)>`, `dns_failure_message(host: &str) -> String`, `classify_send_error(is_timeout: bool, is_connect: bool, detail: &str, url: &str) -> (&'static str, String)`.

- [ ] **Step 1: 실패하는 테스트 작성** — `cdn.rs`의 기존 `mod tests` 안에 추가:

```rust
    #[test]
    fn inspect_target_parses_host_and_port() {
        assert_eq!(
            inspect_target("http://cdn.example.com/a.txt"),
            Some(("cdn.example.com".to_string(), 80))
        );
        assert_eq!(
            inspect_target("https://cdn.example.com:8443/contents/a.txt"),
            Some(("cdn.example.com".to_string(), 8443))
        );
        // 스킴 없는 상대 경로는 URL이 아님
        assert_eq!(inspect_target("contents/a.txt"), None);
    }

    #[test]
    fn dns_failure_message_mentions_host_and_guidance() {
        let msg = dns_failure_message("sklb-test.dn.nexoncdn.co.kr");
        assert!(msg.contains("sklb-test.dn.nexoncdn.co.kr"));
        assert!(msg.contains("NXDOMAIN"));
        assert!(msg.contains("CNAME"));
    }

    #[test]
    fn classify_send_error_priority() {
        // timeout이 최우선
        let (kind, msg) = classify_send_error(true, false, "operation timed out", "http://a/b");
        assert_eq!(kind, "timeout");
        assert!(msg.contains("http://a/b"));

        // TLS 문구는 connect보다 우선 (TLS 핸드셰이크 실패도 is_connect=true로 옴)
        let (kind, _) = classify_send_error(false, true, "invalid peer certificate", "http://a/b");
        assert_eq!(kind, "tls");

        let (kind, msg) = classify_send_error(false, true, "connection refused", "http://a/b");
        assert_eq!(kind, "connect");
        assert!(msg.contains("connection refused"));

        let (kind, _) = classify_send_error(false, false, "unexpected", "http://a/b");
        assert_eq!(kind, "other");
    }
```

- [ ] **Step 2: 테스트가 실패하는지 확인**

Run: `cd src-tauri; cargo test inspect_target dns_failure classify_send`
Expected: 컴파일 오류 — `inspect_target` 등 함수 미정의.

- [ ] **Step 3: 구현** — `UrlInspection` 구조체에 필드 추가:

```rust
#[derive(Debug, Serialize)]
pub struct UrlInspection {
    pub url: String,
    #[serde(rename = "statusCode")]
    pub status_code: Option<u16>,
    /// 응답 헤더 원본 순서 그대로 (key, value) — DevTools Response Headers와 동일한 형태
    pub headers: Vec<(String, String)>,
    #[serde(rename = "durationMs")]
    pub duration_ms: u64,
    pub error: Option<String>,
    /// 오류 분류: "dns" | "timeout" | "connect" | "tls" | "other" (성공 시 None)
    #[serde(rename = "errorKind")]
    pub error_kind: Option<String>,
}
```

`inspect_url` 위에 헬퍼 3개 추가:

```rust
/// URL에서 DNS 사전 조회용 (호스트, 포트) 추출. 유효한 절대 URL이 아니면 None.
fn inspect_target(url: &str) -> Option<(String, u16)> {
    let parsed = reqwest::Url::parse(url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;
    Some((host, port))
}

fn dns_failure_message(host: &str) -> String {
    format!(
        "DNS 조회 실패: {} — 도메인이 DNS에 등록되어 있지 않습니다(NXDOMAIN). \
         CDN 서비스 도메인의 DNS/CNAME 등록 후 다시 시도하세요.",
        host
    )
}

/// reqwest 전송 오류를 (errorKind, 한국어 메시지)로 분류.
/// reqwest::Error는 테스트에서 직접 생성할 수 없어 판별 플래그 + 오류 원문으로 분리했다.
fn classify_send_error(
    is_timeout: bool,
    is_connect: bool,
    detail: &str,
    url: &str,
) -> (&'static str, String) {
    let lower = detail.to_lowercase();
    if is_timeout {
        (
            "timeout",
            format!(
                "응답 시간 초과(10초): {} — 서버가 응답하지 않습니다. 방화벽/사설망 여부를 확인하세요.",
                url
            ),
        )
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        ("tls", format!("TLS 인증서 오류: {} ({})", url, detail))
    } else if is_connect {
        (
            "connect",
            format!(
                "연결 실패: {} — 호스트에 연결할 수 없습니다(포트 차단 또는 서버 다운). ({})",
                url, detail
            ),
        )
    } else {
        ("other", format!("요청 실패: {} ({})", url, detail))
    }
}
```

`inspect_url` 본문 교체:

```rust
#[tauri::command]
pub async fn inspect_url(url: String) -> Result<UrlInspection, String> {
    let started = std::time::Instant::now();

    // DNS 사전 조회 — 미등록 도메인(NXDOMAIN)이면 10초 타임아웃을 기다리지 않고 즉시 원인 반환
    let Some((host, port)) = inspect_target(&url) else {
        return Ok(UrlInspection {
            url: url.clone(),
            status_code: None,
            headers: vec![],
            duration_ms: 0,
            error: Some(format!("URL 형식이 올바르지 않습니다: {}", url)),
            error_kind: Some("other".into()),
        });
    };
    if tokio::net::lookup_host((host.as_str(), port)).await.is_err() {
        return Ok(UrlInspection {
            url,
            status_code: None,
            headers: vec![],
            duration_ms: started.elapsed().as_millis() as u64,
            error: Some(dns_failure_message(&host)),
            error_kind: Some("dns".into()),
        });
    }

    let client = reqwest::Client::builder()
        .use_native_tls()
        .danger_accept_invalid_certs(true)
        // 접근 불가한 사설망 도메인일 경우 무한 대기하지 않고 명확한 오류로 빨리 실패시킴
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    let request_started = std::time::Instant::now();
    let response = match client.head(&url).send().await {
        Ok(resp) if resp.status().as_u16() != 405 => Ok(resp),
        _ => {
            client
                .get(&url)
                .header(reqwest::header::RANGE, "bytes=0-0")
                .send()
                .await
        }
    };
    let duration_ms = request_started.elapsed().as_millis() as u64;

    match response {
        Ok(resp) => {
            let status_code = Some(resp.status().as_u16());
            let headers = resp
                .headers()
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_str().unwrap_or("<binary>").to_string()))
                .collect();
            Ok(UrlInspection { url, status_code, headers, duration_ms, error: None, error_kind: None })
        }
        Err(e) => {
            // 오류 원인 체인 전체를 모아 TLS/연결 오류 문구를 판별
            let detail = std::iter::successors(
                Some(&e as &dyn std::error::Error),
                |err| err.source(),
            )
            .map(|err| err.to_string())
            .collect::<Vec<_>>()
            .join(": ");
            let (kind, message) = classify_send_error(e.is_timeout(), e.is_connect(), &detail, &url);
            Ok(UrlInspection {
                url,
                status_code: None,
                headers: vec![],
                duration_ms,
                error: Some(message),
                error_kind: Some(kind.into()),
            })
        }
    }
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `cd src-tauri; cargo test`
Expected: 신규 3개 테스트 포함 전체 PASS.

- [ ] **Step 5: 커밋**

```bash
git add src-tauri/src/commands/cdn.rs
git commit -m "feat: inspect_url에 DNS 사전 진단·오류 분류(errorKind) 추가"
```

---

### Task 2: FE — `UrlInspection.errorKind` 동기화 + PropertiesDialog DNS 안내

**Files:**
- Modify: `src/types/index.ts:295-301` (UrlInspection)
- Modify: `src/components/modals/PropertiesDialog.tsx` (실시간 확인 오류 표시부, ~line 300)
- Modify: `src/components/modals/PropertiesDialog.module.css` (`.hintBox` 추가)

**Interfaces:**
- Consumes: Task 1의 `errorKind` 직렬화 값.

- [ ] **Step 1: 타입 동기화** — `src/types/index.ts`의 `UrlInspection`:

```ts
export interface UrlInspection {
  url: string;
  statusCode?: number;
  headers: [string, string][];
  durationMs: number;
  error?: string;
  errorKind?: "dns" | "timeout" | "connect" | "tls" | "other";
}
```

- [ ] **Step 2: PropertiesDialog 안내 표시** — `inspectBox` 내부 오류 렌더링 부분을 다음으로 교체 (기존 `{inspErr && <div className={styles.errorBox}>{inspErr}</div>}`):

```tsx
{inspErr && (
  <>
    <div className={styles.errorBox}>{inspErr}</div>
    {inspection?.errorKind === "dns" && (
      <div className={styles.hintBox}>
        이 도메인은 브라우저에서도 접속할 수 없습니다(ERR_NAME_NOT_RESOLVED).
        앱 오류가 아니라 도메인이 아직 DNS에 등록되지 않은 상태입니다.
      </div>
    )}
  </>
)}
```

- [ ] **Step 3: CSS 추가** — `PropertiesDialog.module.css` 끝에 (`.errorBox`와 동일 톤의 정보성 박스):

```css
.hintBox {
  margin-top: 6px;
  padding: 8px 10px;
  border-radius: var(--radius-sm);
  background: rgba(59, 130, 246, 0.08);
  border: 1px solid rgba(59, 130, 246, 0.25);
  color: var(--color-text-secondary);
  font-size: 11px;
  line-height: 1.6;
}
```

- [ ] **Step 4: 타입체크**

Run: `pnpm typecheck`
Expected: 오류 없음.

- [ ] **Step 5: 커밋**

```bash
git add src/types/index.ts src/components/modals/PropertiesDialog.tsx src/components/modals/PropertiesDialog.module.css
git commit -m "feat: 실시간 확인 DNS 미등록 시 원인 안내 표시 (errorKind 동기화)"
```

---

### Task 3: FE — `startUpload(paths?: string[])` 시그니처 확장

**Files:**
- Modify: `src/hooks/useTransfer.ts:147-154`

**Interfaces:**
- Produces: `startUpload(paths?: string[]): Promise<void>` — 인자 없으면 기존처럼 로컬 패널 선택 항목 사용, 인자가 있으면 해당 절대경로(파일/폴더) 사용. Task 4·5가 소비.

- [ ] **Step 1: 구현** — `startUpload` 서두를 다음으로 교체:

```ts
  // paths 미지정 시 로컬 패널에서 선택된 항목을 업로드 (DnD/탐색기 드랍은 paths로 명시 전달)
  const startUpload = useCallback(async (paths?: string[]) => {
    const selectedPaths = paths && paths.length > 0 ? paths : Array.from(local.selectedPaths);
    if (!activeProfile || selectedPaths.length === 0) return;

    setTransferring(true);
    // M-8: dialog는 실제 전송 항목이 있을 때만 열기

    addLog("info", `업로드 시작: ${selectedPaths.length}개 파일 선택됨`, "transfer");
```

(기존 `const selectedPaths = Array.from(local.selectedPaths);` 줄과 기존 가드 `if (!activeProfile || local.selectedPaths.size === 0) return;`는 제거 — 이후 로직은 `selectedPaths`를 그대로 사용하므로 변경 없음)

- [ ] **Step 2: 타입체크 + 기존 테스트**

Run: `pnpm typecheck; pnpm test`
Expected: 모두 PASS (기존 호출부 `startUpload()`는 시그니처 호환).

- [ ] **Step 3: 커밋**

```bash
git add src/hooks/useTransfer.ts
git commit -m "refactor: startUpload가 명시적 경로 목록을 받을 수 있도록 확장"
```

---

### Task 4: FE — 패널 간 pointer 기반 드래그로 교체 (HTML5 DnD 제거)

**Files:**
- Create: `src/hooks/usePanelDrag.ts`
- Test: `src/hooks/usePanelDrag.test.ts`
- Modify: `src/store/appStore.ts` (panelDrag 상태), `src/styles/global.css` (고스트 스타일), `src/components/panels/LocalPanel.tsx`, `src/components/panels/RemotePanel.tsx`

**Interfaces:**
- Consumes: Task 3의 `startUpload(paths?)`, 기존 `startDownload(keys?)`.
- Produces:
  - appStore: `panelDrag: { source: "local" | "remote" | "os"; over: "local" | "remote" | null } | null`, `setPanelDrag(drag): void` (Task 5가 `source: "os"` 사용).
  - `usePanelDrag({ side, onDropToOpposite, ghostLabel }): { onRowPointerDown(e: React.PointerEvent, path: string): void; isDropTarget: boolean }`
  - export `resolvePanelAt(x: number, y: number): "local" | "remote" | null` (Task 5가 재사용).

**사전 확인 (스펙 요구):** 구현 전 `pnpm tauri dev`로 로컬 패널의 폴더 행을 S3 패널로 드래그해 현 증상(무반응)을 재현하고, 콘솔/로그에서 관찰된 원인을 커밋 메시지에 한 줄 기록한다. 재현이 안 되면 그대로 진행 (교체로 무의미해짐).

- [ ] **Step 1: 실패하는 테스트 작성** — `src/hooks/usePanelDrag.test.ts`:

```ts
import { describe, it, expect, vi, afterEach } from "vitest";
import { resolvePanelAt } from "./usePanelDrag";

describe("resolvePanelAt", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });

  it("data-panel 조상을 가진 요소 위면 해당 패널을 반환한다", () => {
    document.body.innerHTML = `<div data-panel="remote"><div id="row">file.txt</div></div>`;
    vi.spyOn(document, "elementFromPoint").mockReturnValue(
      document.getElementById("row")
    );
    expect(resolvePanelAt(10, 10)).toBe("remote");
  });

  it("패널 밖 요소면 null을 반환한다", () => {
    document.body.innerHTML = `<div id="outside">x</div>`;
    vi.spyOn(document, "elementFromPoint").mockReturnValue(
      document.getElementById("outside")
    );
    expect(resolvePanelAt(10, 10)).toBeNull();
  });

  it("요소가 없으면(창 밖) null을 반환한다", () => {
    vi.spyOn(document, "elementFromPoint").mockReturnValue(null);
    expect(resolvePanelAt(-1, -1)).toBeNull();
  });
});
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `pnpm test usePanelDrag`
Expected: FAIL — `usePanelDrag.ts` 모듈 없음.

- [ ] **Step 3: appStore에 panelDrag 상태 추가** — `AppState` 인터페이스에:

```ts
  // 패널 간 pointer 드래그 / OS 파일 드래그 상태 (over: 현재 커서가 올라간 패널)
  panelDrag: { source: "local" | "remote" | "os"; over: "local" | "remote" | null } | null;
  setPanelDrag: (drag: { source: "local" | "remote" | "os"; over: "local" | "remote" | null } | null) => void;
```

초기값·액션 구현 (스토어 본문의 기존 패턴대로):

```ts
  panelDrag: null,
  setPanelDrag: (drag) => set({ panelDrag: drag }),
```

- [ ] **Step 4: `usePanelDrag.ts` 구현**

```ts
import { useCallback, useEffect, useRef } from "react";
import { useAppStore } from "../store/appStore";

export type PanelSide = "local" | "remote";

const DRAG_THRESHOLD_PX = 5;

/** 화면 좌표의 요소에서 가장 가까운 data-panel 패널을 찾는다 */
export function resolvePanelAt(x: number, y: number): PanelSide | null {
  const el = document.elementFromPoint(x, y);
  const panel = el?.closest?.("[data-panel]");
  const side = panel?.getAttribute("data-panel");
  return side === "local" || side === "remote" ? side : null;
}

interface Options {
  side: PanelSide;
  /** 드래그 확정 시 미선택 행이면 단일 선택으로 교체 (스토어 직접 접근으로 stale closure 방지) */
  ensureSelected: (path: string) => void;
  /** 반대편 패널에 드랍 완료 시 호출 (업로드/다운로드) */
  onDropToOpposite: () => void | Promise<void>;
  /** 드래그 고스트에 표시할 텍스트 (예: "3개 항목") */
  ghostLabel: () => string;
}

/**
 * 패널 간 파일 이동용 pointer 이벤트 드래그.
 * HTML5 DnD는 dragDropEnabled: true(탐색기 드랍 수신에 필요)와 공존할 수 없어 직접 구현한다.
 */
export function usePanelDrag({ side, ensureSelected, onDropToOpposite, ghostLabel }: Options) {
  const panelDrag = useAppStore((s) => s.panelDrag);
  const setPanelDrag = useAppStore((s) => s.setPanelDrag);

  const dragRef = useRef<{
    startX: number;
    startY: number;
    path: string;
    started: boolean;
    ghost: HTMLDivElement | null;
  } | null>(null);

  const cleanup = useCallback(() => {
    dragRef.current?.ghost?.remove();
    dragRef.current = null;
    document.body.classList.remove("panel-dragging");
    if (useAppStore.getState().panelDrag?.source === side) setPanelDrag(null);
  }, [side, setPanelDrag]);

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      if (!d.started) {
        if (Math.hypot(e.clientX - d.startX, e.clientY - d.startY) < DRAG_THRESHOLD_PX) return;
        d.started = true;
        ensureSelected(d.path);
        document.body.classList.add("panel-dragging");
        const ghost = document.createElement("div");
        ghost.className = "panel-drag-ghost";
        ghost.textContent = ghostLabel();
        document.body.appendChild(ghost);
        d.ghost = ghost;
      }
      if (d.ghost) {
        d.ghost.style.left = `${e.clientX + 14}px`;
        d.ghost.style.top = `${e.clientY + 14}px`;
      }
      const over = resolvePanelAt(e.clientX, e.clientY);
      const cur = useAppStore.getState().panelDrag;
      if (!cur || cur.source !== side || cur.over !== over) {
        setPanelDrag({ source: side, over });
      }
    };

    const onUp = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const started = d.started;
      const target = started ? resolvePanelAt(e.clientX, e.clientY) : null;
      cleanup();
      if (started) {
        // 드래그 종료 직후 발생하는 click이 선택 상태를 덮어쓰지 않도록 1회 차단
        window.addEventListener(
          "click",
          (ce) => {
            ce.stopPropagation();
            ce.preventDefault();
          },
          { capture: true, once: true }
        );
        if (target && target !== side) void onDropToOpposite();
      }
    };

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") cleanup();
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("keydown", onKey);
    };
  }, [side, ensureSelected, onDropToOpposite, ghostLabel, setPanelDrag, cleanup]);

  /** 파일 행의 onPointerDown에 연결 */
  const onRowPointerDown = useCallback((e: React.PointerEvent, path: string) => {
    if (e.button !== 0) return; // 좌클릭만 드래그 시작
    dragRef.current = { startX: e.clientX, startY: e.clientY, path, started: false, ghost: null };
  }, []);

  const isDropTarget =
    panelDrag !== null && panelDrag.source !== side && panelDrag.over === side;

  return { onRowPointerDown, isDropTarget };
}
```

- [ ] **Step 5: 테스트 통과 확인**

Run: `pnpm test usePanelDrag`
Expected: PASS (3개).

- [ ] **Step 6: 고스트 스타일** — `src/styles/global.css` 끝에 추가:

```css
/* 패널 간 pointer 드래그 고스트 (usePanelDrag가 imperative하게 생성) */
.panel-drag-ghost {
  position: fixed;
  z-index: 9999;
  pointer-events: none;
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  background: var(--color-accent);
  color: #fff;
  font-size: 12px;
  font-weight: 600;
  opacity: 0.9;
  box-shadow: var(--shadow-md);
}

body.panel-dragging {
  user-select: none;
  cursor: grabbing;
}
```

(변수 `--radius-sm`, `--color-accent`, `--shadow-md`가 `variables.css`에 없으면 존재하는 유사 토큰으로 대체)

- [ ] **Step 7: LocalPanel 적용** — `LocalPanel.tsx`:

1. import 추가: `import { usePanelDrag } from "../../hooks/usePanelDrag";`, `import { useTransfer } ...`에서 `startUpload`도 구조분해 (`const { startDownload, startUpload } = useTransfer();`)
2. 훅 연결 (컴포넌트 본문, `useVirtualList` 호출 근처):

```ts
  const ensureLocalSelected = useCallback((path: string) => {
    const s = useAppStore.getState();
    if (!s.local.selectedPaths.has(path)) {
      s.clearLocalSelection();
      s.toggleLocalSelection(path);
    }
  }, []);

  const { onRowPointerDown, isDropTarget } = usePanelDrag({
    side: "local",
    ensureSelected: ensureLocalSelected,
    // 로컬 → S3: 드래그한 선택 항목(폴더 포함)을 업로드
    onDropToOpposite: () =>
      startUpload(Array.from(useAppStore.getState().local.selectedPaths)),
    ghostLabel: () => `${useAppStore.getState().local.selectedPaths.size}개 항목`,
  });
```

3. 루트 div: `data-panel="local"` 추가, `isDragOver` 상태와 `onDragOver`/`onDragLeave`/`onDrop` 핸들러 제거, className의 `isDragOver ? styles.dragOver : ""`를 `isDropTarget ? styles.dragOver : ""`로 교체.
4. 푸터: `{isDropTarget && <span className={styles.dropHint}>여기에 놓으면 다운로드됩니다.</span>}` 추가 (기존 RemotePanel 패턴과 동일 — LocalPanel엔 원래 없었으므로 신규).
5. 파일 행: `draggable`과 `onDragStart` 제거, `onPointerDown={(event) => onRowPointerDown(event, file.path)}` 추가.
6. 이제 사용하지 않는 `isDragOver` state 선언 제거.

단, **원격 → 로컬 다운로드는 RemotePanel 훅의 `onDropToOpposite`에서 실행**되므로 LocalPanel의 기존 `onDrop`(startDownload 호출) 로직은 삭제만 하면 된다. LocalPanel의 `startDownload` import가 다른 곳에서 안 쓰이면 제거.

- [ ] **Step 8: RemotePanel 적용** — `RemotePanel.tsx`: 동일 패턴.

```ts
  const ensureRemoteSelected = useCallback((path: string) => {
    const s = useAppStore.getState();
    if (!s.remote.selectedPaths.has(path)) {
      s.clearRemoteSelection();
      s.toggleRemoteSelection(path);
    }
  }, []);

  const { onRowPointerDown, isDropTarget } = usePanelDrag({
    side: "remote",
    ensureSelected: ensureRemoteSelected,
    // S3 → 로컬: 드래그한 선택 항목을 다운로드 (폴더 확장은 startDownload 내부 처리)
    onDropToOpposite: () =>
      startDownload(Array.from(useAppStore.getState().remote.selectedPaths)),
    ghostLabel: () => `${useAppStore.getState().remote.selectedPaths.size}개 항목`,
  });
```

1. 루트 div: `data-panel="remote"` 추가, HTML5 `onDragOver`/`onDragLeave`/`onDrop` 제거, `isDragOver` → `isDropTarget`로 교체 (state 제거).
2. 파일 행: `draggable`/`onDragStart` 제거, `onPointerDown={(event) => onRowPointerDown(event, file.path)}` 추가.
3. 푸터의 `{isDragOver && ...}` → `{isDropTarget && <span className={styles.dropHint}>여기에 놓으면 업로드됩니다.</span>}`.
4. `startUpload` import가 더 이상 안 쓰이면 구조분해에서 제거.

- [ ] **Step 9: 전체 확인**

Run: `pnpm typecheck; pnpm test`
Expected: 모두 PASS.

- [ ] **Step 10: 수동 검증** — `pnpm tauri dev`로 (모두 확인):
  - 로컬 파일 1개 드래그 → S3 패널: 업로드 실행
  - 로컬 **폴더** 드래그 → S3 패널: 하위 파일 전체 업로드 (TODO 항목 2 해소)
  - Ctrl+클릭 다중 선택 후 드래그: 선택 유지된 채 업로드, 드랍 후 선택 유지(클릭 차단 확인)
  - S3 파일/폴더 드래그 → 로컬 패널: 저장 폴더 선택 다이얼로그 후 다운로드
  - 드래그 중 대상 패널 하이라이트·고스트 표시, Esc로 취소
  - 행 클릭/더블클릭/우클릭 기존 동작 유지

- [ ] **Step 11: 커밋**

```bash
git add src/hooks/usePanelDrag.ts src/hooks/usePanelDrag.test.ts src/store/appStore.ts src/styles/global.css src/components/panels/LocalPanel.tsx src/components/panels/RemotePanel.tsx
git commit -m "feat: 패널 간 드래그를 pointer 이벤트 기반으로 교체 (폴더 DnD 업로드 수정)"
```

---

### Task 5: FE — Windows 탐색기 드랍 업로드 (Tauri 네이티브 drag-drop)

**Files:**
- Modify: `src-tauri/tauri.conf.json:25` (`dragDropEnabled`)
- Create: `src/hooks/useOsFileDrop.ts`
- Test: `src/hooks/useOsFileDrop.test.ts`
- Modify: `src/App.tsx` (훅 호출)

**Interfaces:**
- Consumes: Task 3 `startUpload(paths)`, Task 4 `resolvePanelAt`, appStore `panelDrag`/`setPanelDrag` (`source: "os"`).
- Produces: `useOsFileDrop(): void` (App에서 1회 호출), `physicalToLogical(pos: {x,y}, scale: number): {x,y}` (테스트용 export).

- [ ] **Step 1: 실패하는 테스트 작성** — `src/hooks/useOsFileDrop.test.ts`:

```ts
import { describe, it, expect } from "vitest";
import { physicalToLogical } from "./useOsFileDrop";

describe("physicalToLogical", () => {
  it("물리 좌표를 devicePixelRatio로 나눠 논리 좌표로 변환한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 1.5)).toEqual({ x: 200, y: 100 });
  });

  it("scale이 0 이하이면 1로 취급한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 0)).toEqual({ x: 300, y: 150 });
  });
});
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `pnpm test useOsFileDrop`
Expected: FAIL — 모듈 없음.

- [ ] **Step 3: `useOsFileDrop.ts` 구현**

```ts
import { useEffect, useRef } from "react";
import { runtime } from "../services/runtime";
import { useAppStore } from "../store/appStore";
import { useTransfer } from "./useTransfer";
import { resolvePanelAt } from "./usePanelDrag";

// Tauri 네이티브 drag-drop 이벤트 페이로드 (물리 픽셀 좌표)
interface OsDragPayload {
  position: { x: number; y: number };
}
interface OsDropPayload {
  paths: string[];
  position: { x: number; y: number };
}

/** Tauri PhysicalPosition → CSS 논리 좌표 (elementFromPoint용) */
export function physicalToLogical(
  pos: { x: number; y: number },
  scale: number
): { x: number; y: number } {
  const s = scale > 0 ? scale : 1;
  return { x: pos.x / s, y: pos.y / s };
}

/**
 * Windows 탐색기 등 OS에서 드래그한 파일/폴더를 S3 패널에 드랍하면 업로드.
 * tauri.conf.json의 dragDropEnabled: true 필요 (패널 간 드래그는 usePanelDrag가 담당).
 */
export function useOsFileDrop() {
  const { startUpload } = useTransfer();

  // 리스너는 1회만 등록하고, 렌더마다 바뀌는 startUpload는 ref로 참조
  const startUploadRef = useRef(startUpload);
  startUploadRef.current = startUpload;

  useEffect(() => {
    const unlisteners: (() => void)[] = [];
    let disposed = false;

    const overRemote = (pos: { x: number; y: number }) => {
      const { x, y } = physicalToLogical(pos, window.devicePixelRatio);
      return resolvePanelAt(x, y) === "remote";
    };

    (async () => {
      const unOver = await runtime.listen<OsDragPayload>("tauri://drag-over", (payload) => {
        const store = useAppStore.getState();
        const over = overRemote(payload.position);
        const cur = store.panelDrag;
        if (over && (cur?.source !== "os" || cur.over !== "remote")) {
          store.setPanelDrag({ source: "os", over: "remote" });
        } else if (!over && cur?.source === "os") {
          store.setPanelDrag(null);
        }
      });

      const unDrop = await runtime.listen<OsDropPayload>("tauri://drag-drop", (payload) => {
        const store = useAppStore.getState();
        store.setPanelDrag(null);
        if (!overRemote(payload.position) || payload.paths.length === 0) return;
        if (!store.isConnected) {
          store.addLog("warn", "S3에 연결된 상태에서만 파일을 드랍해 업로드할 수 있습니다.", "transfer");
          return;
        }
        void startUploadRef.current(payload.paths);
      });

      const unLeave = await runtime.listen<null>("tauri://drag-leave", () => {
        const store = useAppStore.getState();
        if (store.panelDrag?.source === "os") store.setPanelDrag(null);
      });

      if (disposed) {
        unOver();
        unDrop();
        unLeave();
        return;
      }
      unlisteners.push(unOver, unDrop, unLeave);
    })().catch((err) => console.error("[useOsFileDrop] 리스너 등록 실패:", err));

    return () => {
      disposed = true;
      unlisteners.forEach((fn) => fn());
    };
  }, []);
}
```

- [ ] **Step 4: 테스트 통과 확인**

Run: `pnpm test useOsFileDrop`
Expected: PASS (2개).

- [ ] **Step 5: tauri.conf.json 변경** — `"dragDropEnabled": false` → `"dragDropEnabled": true`.

- [ ] **Step 6: App.tsx에서 훅 호출** — import 추가 후 `App()` 본문 서두에:

```tsx
import { useOsFileDrop } from "./hooks/useOsFileDrop";
// ...
export default function App() {
  useOsFileDrop();
  // ...기존 코드
```

- [ ] **Step 7: 전체 확인**

Run: `pnpm typecheck; pnpm test`
Expected: 모두 PASS.

- [ ] **Step 8: 수동 검증** — `pnpm tauri dev`:
  - 탐색기에서 파일 1개 → S3 패널 드랍: 업로드 실행, 드래그 중 S3 패널 하이라이트
  - 탐색기에서 **폴더** → S3 패널 드랍: 하위 전체 업로드 (TODO 항목 2 탐색기 케이스)
  - 로컬 패널 위/패널 밖 드랍: 아무 동작 없음
  - 미연결 상태 S3 패널 드랍: 경고 로그만
  - **회귀**: Task 4의 패널 간 드래그가 dragDropEnabled: true에서도 정상 동작하는지 재확인 (pointer 이벤트는 OS DnD와 무관해야 함)

- [ ] **Step 9: 커밋**

```bash
git add src-tauri/tauri.conf.json src/hooks/useOsFileDrop.ts src/hooks/useOsFileDrop.test.ts src/App.tsx
git commit -m "feat: Windows 탐색기 파일/폴더 드랍 업로드 지원 (Tauri drag-drop 이벤트)"
```

---

### Task 6: FE — 프로필 전면 잠금 (write-only) + 목록 행 [테스트]

**Files:**
- Modify: `src/components/modals/ProfileModal.tsx`
- Modify: `src/components/modals/ProfileModal.module.css` (`.testResultLine` 추가)
- Test: `src/components/modals/ProfileModal.test.tsx`

**Interfaces:**
- Consumes: 기존 커맨드 `connect_s3`, `test_cdn_connection`; `utils/cdn`의 `availableCdns`, `cdnDistributionIdFor`, `CDN_LABELS`.
- Produces: 없음 (UI 정책 변경).

- [ ] **Step 1: 실패하는 테스트 작성** — `src/components/modals/ProfileModal.test.tsx`:

```tsx
import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ProfileModal from "./ProfileModal";
import { useAppStore } from "../../store/appStore";
import type { S3Profile } from "../../types";

vi.mock("../../services/runtime", () => ({
  runtime: {
    invoke: vi.fn().mockResolvedValue([]),
    listen: vi.fn().mockResolvedValue(() => undefined),
    openDirectory: vi.fn(),
  },
}));

const profile: S3Profile = {
  id: "p1",
  name: "고객사 프로필",
  region: "ap-northeast-2",
  bucket: "secret-bucket-name",
  accessKeyId: "AKIASECRETKEY",
  secretAccessKey: "",
  cdnDomain: "cdn.secret-domain.com",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("ProfileModal — 프로필 정보 잠금", () => {
  beforeEach(() => {
    useAppStore.setState({ profiles: [profile] });
  });

  it("목록에 프로필 이름만 보이고 버킷/키/도메인은 노출되지 않는다", () => {
    render(<ProfileModal />);
    expect(screen.getByText("고객사 프로필")).toBeInTheDocument();
    expect(screen.queryByText(/secret-bucket-name/)).not.toBeInTheDocument();
    expect(screen.queryByText(/AKIASECRETKEY/)).not.toBeInTheDocument();
    expect(screen.queryByText(/secret-domain/)).not.toBeInTheDocument();
  });

  it("프로필 이름을 클릭해도 편집 폼이 열리지 않는다 (폼은 새 프로필 전용, 빈 상태 유지)", () => {
    render(<ProfileModal />);
    fireEvent.click(screen.getByText("고객사 프로필"));
    // 폼 제목은 항상 "새 프로필"
    expect(screen.getByText("새 프로필")).toBeInTheDocument();
    expect(screen.queryByText("프로필 편집")).not.toBeInTheDocument();
    // 폼 입력에 기존 프로필 값이 채워지지 않음
    expect(screen.queryByDisplayValue("secret-bucket-name")).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("AKIASECRETKEY")).not.toBeInTheDocument();
  });

  it("행 액션은 연결/테스트/내보내기/삭제만 제공한다", () => {
    render(<ProfileModal />);
    expect(screen.getByRole("button", { name: "연결" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "테스트" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "내보내기" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "삭제" })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: 테스트 실패 확인**

Run: `pnpm test ProfileModal`
Expected: FAIL — 클릭 시 편집 폼이 열리고("프로필 편집" 표시), "테스트" 버튼 없음.

- [ ] **Step 3: ProfileModal 잠금 구현** — `ProfileModal.tsx` 변경 사항:

1. **삭제**: `editingId` state, `cdnDetailsRevealed` state, `cdnFieldsMasked`, `CdnDetailsMasked` 컴포넌트, `handleEdit` 함수, `handleTestCdnConnection`/`runCdnTest`/`isTestingCdn`/`cdnTestResult` (폼 내 CDN 연결 테스트 UI 포함).
2. **`handleNew` → `resetForm`으로 개명** (역할 명확화): `setForm(emptyForm()); setError(null); setTestResult(null);`
3. **`buildProfilePayload`**: `editingId`/기존 프로필 병합 제거 — 신규 전용:

```ts
  const buildProfilePayload = (): S3Profile => ({
    id: crypto.randomUUID(),
    name: form.name,
    // ...이하 기존 필드 매핑 동일 (cdnProviders: undefined 로 두고 기존 병합 로직 삭제)
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  });
```

4. **`handleSubmit`**: `await saveProfile(buildProfilePayload()); resetForm();`
5. **`handleTestConnection`/`runS3Test`**: `editingId` 분기 제거 — Secret Key 미입력 시 항상 `"연결 테스트를 위해 Secret Access Key를 입력하세요."` 오류. `runS3Test`는 직접 입력값 경로만 유지.
6. **`handleProviderChange`**: `editingId` 조회 제거 — `setForm((f) => ({ ...f, cdnProvider: provider }))`만 남김 (도메인/ID는 사용자가 새로 입력).
7. **placeholder**: `placeholder={editingId ? "변경하려면 입력" : ...}` 형태 전부 고정 문자열로 교체 (예: Secret Access Key는 `placeholder=""`, 각 CDN 비밀값은 원래의 신규 입력 안내 문구).
8. **목록 행**: `profileInfo` 버튼의 `onClick={() => handleEdit(p)}` 제거 → 클릭 동작 없는 `<div className={styles.profileInfo}>`로 변경. `editingId === p.id ? styles.active` 클래스 제거.
9. **행 [테스트] 버튼 + 결과 상태** 추가:

```ts
  // 목록 행 [테스트]: 저장된 자격증명으로 S3 + 구성된 CDN 연결을 검사 (설정값은 표시하지 않음)
  const [rowTests, setRowTests] = useState<
    Record<string, { testing: boolean; lines: { label: string; success: boolean; error?: string }[] }>
  >({});

  const handleRowTest = async (p: S3Profile) => {
    setRowTests((s) => ({ ...s, [p.id]: { testing: true, lines: [] } }));
    const lines: { label: string; success: boolean; error?: string }[] = [];
    try {
      await runtime.invoke("connect_s3", { profileId: p.id });
      lines.push({ label: "S3", success: true });
    } catch (err) {
      lines.push({ label: "S3", success: false, error: String(err) });
    }
    for (const provider of availableCdns(p)) {
      try {
        const result = await runtime.invoke<CdnConnectionTestResult>("test_cdn_connection", {
          profileId: p.id,
          provider,
          distributionId: cdnDistributionIdFor(p, provider) ?? "",
        });
        lines.push({ label: CDN_LABELS[provider], success: result.success, error: result.error });
      } catch (err) {
        lines.push({ label: CDN_LABELS[provider], success: false, error: String(err) });
      }
    }
    setRowTests((s) => ({ ...s, [p.id]: { testing: false, lines } }));
  };
```

import 추가: `import { availableCdns, cdnDistributionIdFor, CDN_LABELS } from "../../utils/cdn";`

행 렌더링 (기존 `profileActions` 안 [연결] 다음에 [테스트], 행 아래 결과):

```tsx
<button
  type="button"
  className={styles.testBtn}
  disabled={rowTests[p.id]?.testing}
  onClick={() => void handleRowTest(p)}
>
  {rowTests[p.id]?.testing ? "테스트 중" : "테스트"}
</button>
```

```tsx
{/* profileItem 내부, 액션 아래 */}
{rowTests[p.id]?.lines.length ? (
  <div className={styles.testResultLines}>
    {rowTests[p.id].lines.map((line) => (
      <span
        key={line.label}
        className={line.success ? styles.testOk : styles.testFail}
        title={line.error}
      >
        {line.success ? "✓" : "✗"} {line.label}
        {!line.success && line.error ? ` — ${line.error}` : ""}
      </span>
    ))}
  </div>
) : null}
```

(주: `profileItem`이 flex row라면 결과 줄이 들어갈 수 있도록 행을 `flex-direction: column` 래퍼로 감싸거나 `.testResultLines`를 `width: 100%; flex-basis: 100%;`로 처리 — 기존 CSS 구조에 맞춰 조정)

10. **폼 헤더**: `{editingId ? "프로필 편집" : "새 프로필"}` → `새 프로필` 고정. 취소 버튼은 `resetForm` 호출.
11. **deleteConfirm onConfirm**: `if (editingId === deleteConfirmId) handleNew();` 줄 제거.

- [ ] **Step 4: CSS 추가** — `ProfileModal.module.css` 끝에:

```css
.testResultLines {
  flex-basis: 100%;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding: 2px 0.6rem 6px;
  font-size: 11px;
  overflow-wrap: anywhere;
}
```

(`.testOk`/`.testFail` 클래스는 기존 폼 테스트 결과용으로 이미 존재 — 재사용. 없으면 성공=`var(--color-success)`, 실패=`var(--color-error)`로 추가)

- [ ] **Step 5: 테스트 통과 확인**

Run: `pnpm test ProfileModal; pnpm typecheck`
Expected: 신규 3개 테스트 PASS, 타입 오류 없음.

- [ ] **Step 6: 수동 검증** — `pnpm tauri dev`:
  - 프로필 목록에서 이름 클릭 → 아무 정보도 안 열림
  - [테스트] → S3 ✓/✗ + CDN별 ✓/✗ 만 표시 (도메인/ID 등 설정값 비노출)
  - [연결]/[내보내기]/[삭제] 정상 동작
  - 새 프로필 생성(입력 → S3 연결 테스트 → 저장) 후 목록에 나타나고 폼은 초기화
  - JSON 프로필 가져오기 → 목록에 이름만 표시, [테스트]로 검증 가능

- [ ] **Step 7: 커밋**

```bash
git add src/components/modals/ProfileModal.tsx src/components/modals/ProfileModal.module.css src/components/modals/ProfileModal.test.tsx
git commit -m "feat: 저장된 프로필 정보 열람 차단 (write-only) + 목록 행 연결 테스트"
```

---

### Task 7: 마무리 — TODO/CLAUDE.md 갱신 + 전체 검증

**Files:**
- Modify: `TODO`
- Modify: `CLAUDE.md` (dragDropEnabled 주의사항, 프로필 관련 설명)

- [ ] **Step 1: 전체 테스트/빌드 검증**

Run: `pnpm typecheck; pnpm test; cd src-tauri; cargo test`
Expected: 모두 PASS.

- [ ] **Step 2: TODO 파일 갱신** — 4개 항목을 완료 처리로 교체:

```
[완료] 1. 프로필 전면 잠금 — 저장/가져온 프로필은 이름만 표시, 연결/테스트/내보내기/삭제만 가능 (정보 열람 불가)
[완료] 2. 패널 간 폴더 드래그앤드랍 업로드 + Windows 탐색기 폴더/파일 드랍 업로드 지원
[완료] 3·4. 효성ITX CDN URL 접속 불가 원인 진단 — sklb-test.dn.nexoncdn.co.kr / *.gtmc.hscdn.net 모두 DNS 미등록(NXDOMAIN).
       도구가 실시간 확인 시 DNS 사전 조회로 원인을 즉시 안내하도록 개선. 실제 해소는 효성/고객사 DNS(CNAME) 등록 필요.
```

- [ ] **Step 3: CLAUDE.md 갱신** — "주의 사항"의 `dragDropEnabled: false` 항목을 다음으로 교체:

```markdown
- `tauri.conf.json`의 `dragDropEnabled: true`는 탐색기 파일/폴더 드랍 업로드(`useOsFileDrop.ts`, `tauri://drag-drop` 이벤트)에 필수. 이 설정에서는 WebView2가 HTML5 DnD를 가로채므로 패널 간 드래그는 HTML5 DnD가 아닌 pointer 이벤트 기반 커스텀 구현(`usePanelDrag.ts`)을 사용한다 — 패널 루트의 `data-panel="local|remote"` 속성이 드랍 대상 판정 기준.
```

멀티 CDN 프로필 섹션에 한 줄 추가:

```markdown
- 저장된 프로필은 write-only: 목록에서 정보 열람/편집 불가(연결·테스트·내보내기·삭제만). 수정은 삭제 후 재생성/재임포트.
```

- [ ] **Step 4: 커밋**

```bash
git add TODO CLAUDE.md
git commit -m "docs: TODO 항목 완료 처리 및 DnD/프로필 정책 문서 갱신"
```

---

## Self-Review 결과

- **스펙 커버리지**: 항목 1 → Task 6, 항목 2(패널 DnD) → Task 3·4, 항목 2(탐색기) → Task 5, 항목 3·4 → Task 1·2, 마무리(문서) → Task 7. 누락 없음.
- **타입 일관성**: `panelDrag.source`에 `"os"` 포함(Task 4 정의, Task 5 소비), `startUpload(paths?)` (Task 3 정의, Task 4·5 소비), `errorKind` serde/TS 동일 문자열. 확인됨.
- **주의**: Task 4 Step 7·8은 기존 파일의 큰 수정이므로 삭제 대상(HTML5 핸들러/state)을 남기지 말 것 — `isDragOver` 잔존 시 typecheck로 검출됨.
