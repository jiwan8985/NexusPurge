# CDN Purge 연동 개발 보고서: KT, LG U+, & 효성 ITX

본 보고서는 [kt_cdn.md](file:///D:/Projects/NexusPurge/docs/kt_cdn.md), [lg_cdn.md](file:///D:/Projects/NexusPurge/docs/lg_cdn.md), 및 [효성 CDN_PURGE_API_GUIDE_ver.2026.pdf](file:///D:/Projects/NexusPurge/docs/효성%20CDN_PURGE_API_GUIDE_ver.2026.pdf) 명세에 맞추어 진행된 CDN 추가 개발 및 개선 사항을 기록합니다.

---

## 1. 변경 및 개선 사항 개요

KT, LG U+, 효성 ITX 등 3대 신규 CDN 제공자를 통합하고, 기존 연동 구조를 보완하여 트랜잭션 추적 및 프론트엔드 설정 기능을 완비하였습니다.

### 핵심 개발 사항
1. **KT 및 LG U+ CDN (Solbox CDN v3 API)**:
   - [kt.rs](file:///D:/Projects/NexusPurge/src-tauri/src/adapters/cdn/kt.rs) 및 [lguplus.rs](file:///D:/Projects/NexusPurge/src-tauri/src/adapters/cdn/lguplus.rs) 어댑터에서 Purge 요청 시 비동기 처리용 `transactionId`를 반환하도록 수정하였습니다.
   - 트랜잭션 상태를 비동기로 조회할 수 있도록 `GET /v3/management/transaction/{transactionId}` API 연동을 추가하고 `get_transaction_status` 인터페이스를 구현하였습니다.
2. **효성 ITX CDN**:
   - [hyosung.rs](file:///D:/Projects/NexusPurge/src-tauri/src/adapters/cdn/hyosung.rs) 어댑터에서 응답 봉투(Envelope) 분석을 통해 `meta.transactionId`를 추출하고 반환하도록 연동하였습니다.
   - 자격증명 유효성을 확인하는 연결 테스트(`test_connection()`)를 개발하여 `GET /api/v1/purge/{serviceId}/` 검증 API와 통합하였습니다.
3. **러스트 백엔드 커맨드 레이어**:
   - [cdn.rs](file:///D:/Projects/NexusPurge/src-tauri/src/commands/cdn.rs)에서 효성 ITX CDN 연결 테스트를 수행하고, KT/LG U+의 실시간 트랜잭션 조회를 `get_purge_status` 핸들러에 통합 연동하였습니다.
   - 파일 스마트 동기화 및 업로드 완료 후 자동 캐시 무효화 시 트랜잭션 ID를 [sync.rs](file:///D:/Projects/NexusPurge/src-tauri/src/commands/sync.rs) 및 [mod.rs](file:///D:/Projects/NexusPurge/src-tauri/src/adapters/cdn/mod.rs)로 전달하도록 연동을 강화하였습니다.
4. **S3 경로 보정 및 라우팅 정규화 (Base Path 스트립)**:
   - S3의 특정 폴더(예: `contents/`)가 CDN 도메인의 루트(`/`)로 직접 매핑되는 라우팅 기조를 지원하기 위해, 수동 및 자동 Purge 요청 전 S3 파일 키에서 `cdn_base_path`를 백엔드 레이어에서 자동으로 제거(Strip)한 뒤 API를 호출하도록 전격 수정하였습니다.
5. **리액트 프론트엔드 UI**:
   - [ProfileModal.tsx](file:///D:/Projects/NexusPurge/src/components/modals/ProfileModal.tsx)에 효성 ITX CDN의 설정 입력 폼(API Key, API Secret, Service ID, Domain, API Endpoint)을 탑재하고 예외 검증 및 저장/연결 테스트 플로우를 추가하였습니다.
   - [PurgeResultDialog.tsx](file:///D:/Projects/NexusPurge/src/components/modals/PurgeResultDialog.tsx)에서 무효화 결과 헤더에 "요청 ID" 대신 솔박스 및 효성 표준 명칭인 **"Transaction ID (트랜잭션 ID)"**를 노출하도록 수정하고 관련 안내를 번역 보강하였습니다.
6. **CDN 캐시 직접 입력 Purge (Custom Purge)**:
   - 원격 패널에 표시되지 않거나 개별 삭제된 와일드카드 경로(예: `/assets/css/*`, `/index.html`)를 사용자가 직접 줄바꿈이나 쉼표로 다중 입력해 Purge할 수 있도록 프론트엔드 툴바([Toolbar.tsx](file:///D:/Projects/NexusPurge/src/components/layout/Toolbar.tsx))와 [InputDialog.tsx](file:///D:/Projects/NexusPurge/src/components/common/InputDialog.tsx)를 확장 개편했습니다.

---

## 2. 제공자별 상세 연동 및 API 정보

### KT & LG U+ CDN (Solbox OpenAPI v3 규격)
- **인증 토큰 발급**: `POST /v3/auth/tokens`
- **캐시 무효화 (Purge)**: `POST /v3/management/service/{serviceName}/volume/{volumeName}/purge`
- **트랜잭션 추적**: `GET /v3/management/transaction/{transactionId}`
- **연동 상세**:
  - 사용자 자격증명으로 JWT Bearer 토큰을 실시간 발급하여 캐시합니다.
  - Purge 완료 시 `202 Accepted` 응답과 함께 전달되는 `transactionId`를 추적 값으로 삼아 UI에 갱신하고, 이후 백엔드 조회 API를 통해 상태가 `completed`로 완료되는 시점까지 추적합니다.

### 효성 ITX CDN (X-ITX API 규격)
- **인증 헤더**:
  - `X-ITX-Security-Principal`: 사용자 식별자 (API Key)
  - `X-ITX-Security-Secret`: 비밀 액세스 키 (API Secret)
- **캐시 무효화 (Purge)**: `POST /api/v1/purge/{serviceId}` (끝에 슬래시 없음)
- **인증 테스트**: `GET /api/v1/purge/{serviceId}/?target={cdn_url}` (끝에 슬래시 필수)
- **연동 상세**:
  - JSON 응답 봉투 내부의 `data` 필드가 이스케이프된 문자열 형태로 직렬화되어 오므로, 2차 역직렬화를 통해 `successCount` 및 `failedCount`를 파싱하여 부분 실패 처리를 철저하게 감지합니다.
  - API 엔드포인트를 입력하지 않고 빈칸으로 두더라도 효성 ITX 표준 포트 주소인 `https://api.xtrmcdn.co.kr:28091`로 자동 폴백(Fallback) 처리하여 편의성을 높였습니다.

---

## 3. 검증 결과

- **프론트엔드 컴파일**: `npm run typecheck` 실행 시 경고 및 타입 오류 없이 정상 통과하였습니다.
- **테스트 케이스**: 프론트엔드 Vitest 유닛 테스트 결과 전원 합격(Passed)을 기록했습니다.
- **러스트 백엔드 검증**: `cargo test` 실행 결과, CDN 주소 정규화 빌더 및 효성 ITX 예외 차단 등을 포함한 10개의 테스트가 **전체 성공(ok)** 하였습니다.
