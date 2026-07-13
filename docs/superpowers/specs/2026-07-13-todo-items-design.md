# TODO 항목 구현 설계 (2026-07-13)

TODO 파일의 4개 항목에 대한 설계. 항목 3·4는 동일 원인(효성 도메인 DNS 미등록)이므로 하나로 묶는다.

## 배경

- **항목 1**: 프로필 파일(profile-multi.json / profile-single.json)을 고객사에 전달하면 고객사는 가져오기 → 연결만 하면 된다. 현재는 목록에서 프로필을 클릭하면 편집 폼에 버킷/리전/AccessKey/CDN 설정이 노출된다. **모든 프로필**(직접 생성 포함)에 대해 저장 후 정보 열람을 차단한다 (사용자 결정).
- **항목 2**: 로컬 패널에서 폴더 행을 S3 패널로 드래그해도 업로드되지 않는다. 또한 Windows 탐색기에서 폴더를 S3 패널로 드랍하는 기능도 필요하다 (사용자 결정: 둘 다).
- **항목 3·4**: 효성 ITX 프로필에서 속성 다이얼로그의 CDN URL을 크롬에서 열면 `ERR_NAME_NOT_RESOLVED`, 앱 내 "실시간 확인"은 `error sending request for url (...)`. 원인 확인 결과 `sklb-test.dn.nexoncdn.co.kr`, `sklb-test.dn.nexoncdn.co.kr.gtmc.hscdn.net`, `gtmc.hscdn.net` 모두 **NXDOMAIN** — DNS 자체가 등록되지 않아 어떤 URL 조합으로도 접속이 불가능하다. 도구에서는 이를 명확히 진단·안내한다 (사용자 결정).

## 항목 1 — 프로필 전면 잠금 (write-only)

UI 레벨 변경만으로 구현한다. 시크릿은 이미 keyring에만 있고 폼에 재노출되지 않으므로, 백엔드 변경은 없다.

`src/components/modals/ProfileModal.tsx`:

- `handleEdit` 및 `editingId` 기반 편집 흐름 제거. 목록의 프로필은 클릭해도 아무 정보도 열리지 않는다.
- 목록 행: 프로필 이름 + 액션 버튼 **[연결] [테스트] [내보내기] [삭제]**.
  - **[테스트]** (신규): 저장된 자격증명으로 `connect_s3`(S3 연결) + 구성된 각 CDN provider에 `test_cdn_connection`을 실행하고 행 아래에 ✓/✗ 결과만 표시한다. 설정값(도메인/ID 등)은 표시하지 않는다. 편집 폼이 사라지면서 기존 "CDN 연결 테스트"(저장된 프로필 필요)를 대체한다.
- 오른쪽 폼은 "새 프로필" 생성 전용:
  - 저장 성공 시 폼 초기화. 수정은 삭제 후 재생성/재임포트로만 가능.
  - 입력값 기반 "S3 연결 테스트"(`testConnection`)는 저장 전 검증용으로 유지.
  - 폼 내 기존 "CDN 연결 테스트" 버튼은 저장된 프로필(`editingId`)이 전제라 제거하고, 목록 행 [테스트]로 대체.
  - `editingId` 분기 제거에 따라 `CdnDetailsMasked`, `cdnDetailsRevealed`, "변경하려면 입력" placeholder, `buildProfilePayload`의 기존 `cdnProviders` 병합 로직 등 편집 전용 코드를 삭제한다.
- `useProfile` 훅의 `saveProfile`은 신규 생성 전용으로만 호출된다 (시그니처 변경 없음).

**비범위**: 프런트 상태(`profiles`)에는 목록 렌더링·내보내기·테스트를 위해 프로필 데이터가 여전히 로드된다. 요구사항은 "화면에서 단순하게, 정보 비노출"이며 devtools 수준의 은닉은 목표가 아니다.

## 항목 2 — 패널 간 폴더 DnD 수정 + 탐색기 드랍 지원

### 제약

`tauri.conf.json`의 `dragDropEnabled: false`는 패널 간 HTML5 DnD의 전제 조건이다(true면 WebView2가 드랍을 가로챔 — CLAUDE.md). 반대로 탐색기 드랍에서 **실제 파일 경로**를 받으려면 Tauri 네이티브 drag-drop 이벤트(`dragDropEnabled: true`)가 필요하다. 둘은 공존할 수 없으므로 패널 간 드래그를 HTML5에서 분리한다.

### 설계

1. **패널 간 드래그를 pointer 이벤트 기반으로 재구현** (`LocalPanel.tsx`, `RemotePanel.tsx` + 공용 훅 `useDragBetweenPanels` 신설):
   - 행 `pointerdown` → 이동 임계값(수 px) 초과 시 드래그 시작, `setPointerCapture`로 추적.
   - 드래그 중 고스트 요소(선택 항목 수 표시)를 커서에 붙여 렌더링(`createPortal`).
   - `pointerup` 시 `document.elementFromPoint`로 반대편 패널 위인지 히트테스트 → 로컬→원격이면 업로드, 원격→로컬이면 다운로드.
   - 드래그 시작 시 미선택 행이면 해당 행만 선택으로 교체(기존 동작 유지). 파일/폴더 구분 없이 동일 — 폴더 DnD 버그도 함께 해소.
   - 기존 행의 `draggable`/`onDragStart`, 패널의 `onDragOver`/`onDrop`(HTML5) 제거. 드래그오버 하이라이트(`isDragOver`)는 새 훅이 제공.
2. **탐색기 드랍**: `dragDropEnabled: true`로 변경 후 `getCurrentWebview().onDragDropEvent()` 구독 (App 레벨 1곳).
   - `over` 이벤트의 물리 좌표를 S3 패널 DOM 영역과 비교해 드랍 하이라이트 표시.
   - `drop` 이벤트: S3 패널 위이고 연결 상태이면 `startUpload(event.payload.paths)` 호출. 경로가 폴더면 기존 `build_sync_plan`의 `expand_paths_to_files`가 재귀 확장하므로 백엔드 변경 없음.
   - 로컬 패널 위 드랍은 무시(로컬→로컬 복사는 비범위).
3. **`useTransfer.startUpload(paths?: string[])`**: 인자 미지정 시 기존처럼 `local.selectedPaths` 사용, 지정 시 해당 경로 사용 (기존 `startDownload(keys?)`와 동일 패턴).

### 구현 순서 주의

구현 시작 시 먼저 `npm run tauri dev`로 현 폴더 DnD 실패를 재현해 실제 원인을 확인·기록한 뒤 교체 작업을 진행한다 (교체 후 동일 원인이 새 구현에 남지 않도록).

## 항목 3·4 — 효성 CDN URL: DNS 진단 + 명확한 안내

`src-tauri/src/commands/cdn.rs::inspect_url`:

- 요청 전 host를 URL에서 추출해 `tokio::net::lookup_host((host, port))`로 사전 조회.
  - 실패 시 즉시 반환(10초 타임아웃 대기 없음): `errorKind: "dns"`, 메시지 `"DNS 조회 실패: {host} — 도메인이 DNS에 등록되어 있지 않습니다(NXDOMAIN). CDN 서비스 도메인의 DNS/CNAME 등록 후 다시 시도하세요."`
- 요청 실패 시 reqwest 오류 분류: `is_timeout` → `"timeout"`, `is_connect` → `"connect"`, TLS 관련 → `"tls"`, 그 외 `"other"`. 각각 시도한 URL을 포함한 한국어 메시지로 변환.
- `UrlInspection`에 `errorKind: Option<String>` 추가 (serde rename `errorKind`), `src/types/index.ts`의 `UrlInspection`에 `errorKind?: "dns" | "timeout" | "connect" | "tls" | "other"` 동기화.

`src/components/modals/PropertiesDialog.tsx`:

- 실시간 확인 결과 `errorKind === "dns"`이면 오류 메시지 아래 부가 안내 표시: *"이 도메인은 브라우저에서도 접속할 수 없습니다(ERR_NAME_NOT_RESOLVED). 앱 오류가 아니라 도메인이 아직 DNS에 등록되지 않은 상태입니다."* — 항목 3의 크롬 증상을 함께 설명.

## 테스트

- Rust: `inspect_url` 오류 분류 로직을 순수 함수로 분리해 단위 테스트 (DNS 실패 메시지, reqwest 오류 분류).
- 프런트(vitest): `startUpload(paths)` 인자 분기, DnD 훅의 히트테스트/선택 교체 로직 등 순수 로직 위주.
- 수동 검증: `npm run tauri dev`로 (1) 프로필 잠금 UI, (2) 패널 간 파일/폴더 드래그, (3) 탐색기 파일/폴더 드랍, (4) 효성 프로필 실시간 확인 오류 문구.

## 마무리

- 구현 완료 항목은 TODO 파일에서 제거(또는 완료 표시)한다.
- CLAUDE.md의 `dragDropEnabled: false` 관련 주의사항을 새 구조(true + 커스텀 포인터 DnD)에 맞게 갱신한다.
