# NexusPurge TODO

## P0 - 운영 정확성

### Backend

- [x] 삭제 시 CDN Purge
  - S3 오브젝트 삭제 후 해당 CDN URL도 Purge한다.
  - 다중 선택 삭제 시 삭제 성공한 key만 Purge 대상으로 보낸다.
  - CloudFront/Akamai 모두 동일한 인터페이스로 처리한다.

- [x] CDN Provider 연결 테스트
  - CloudFront: Distribution ID 유효성, 권한, 배포 도메인 조회 확인.
  - Akamai: EdgeGrid 인증, Fast Purge 권한, CDN Domain 필수값 확인.
  - S3 연결 테스트와 CDN 연결 테스트를 분리한다.

- [x] Purge 상태 조회
  - CloudFront Invalidation ID 상태를 `InProgress`에서 `Completed`까지 조회한다.
  - Akamai Fast Purge 요청 결과와 상태 확인 가능 여부를 어댑터에 반영한다.

- [x] 업로드 후 CDN 반영 검증
  - CDN URL에 `HEAD` 또는 작은 `GET` 요청을 보내 `200 OK`, `ETag`, `Last-Modified`, `Cache-Control`을 확인한다.
  - Purge 후 새 파일이 실제 CDN에서 응답되는지 검증한다.

### UI/UX

- [x] 삭제 확인 다이얼로그에 CDN Purge 안내 표시
  - CDN Provider가 설정된 경우 “삭제 후 CDN 캐시도 Purge됩니다”를 명시한다.
  - Purge 실패 시 S3 삭제 성공과 CDN Purge 실패를 분리해서 보여준다.

- [x] CDN 연결 테스트 버튼 추가
  - 프로파일 모달의 CDN 설정 영역에 `CDN 테스트` 버튼을 추가한다.
  - S3 연결 성공 여부와 별도로 CDN 인증/도메인 상태를 표시한다.

- [x] Purge 상태 배지
  - 전송 큐에 `Purge 요청됨`, `Purge 진행중`, `Purge 완료`, `Purge 실패` 상태를 표시한다.
  - 실패 시 원인 메시지를 툴팁 또는 상세 패널에서 볼 수 있게 한다.

---

## P1 - 배포 확인 흐름

### Backend

- [x] CDN URL 생성 유틸
  - `cdnDomain + remoteKey`를 일관되게 조합한다.
  - 프로토콜 중복, 앞뒤 공백, 중복 slash를 정규화한다.

- [x] Cache-Control 메타데이터 설정
  - 업로드 요청에 `Cache-Control`을 선택적으로 포함한다.
  - 파일 패턴별 기본값을 지원한다.
  - 예: `*.html -> no-cache`, hashed assets -> `max-age=31536000, immutable`.

- [x] Content-Type override
  - 자동 감지된 MIME type을 사용하되, 필요 시 사용자가 override할 수 있게 한다.

### UI/UX

- [x] CDN URL 복사/열기
  - Remote 파일 우클릭 메뉴에 `CDN URL 복사`, `CDN URL 열기`를 추가한다.
  - CDN Domain이 없는 경우 메뉴를 비활성화하고 이유를 표시한다.

- [x] 업로드 완료 후 CDN URL 목록 표시
  - Progress Dialog 완료 화면에 업로드된 파일의 CDN URL을 표시한다.
  - 여러 파일 업로드 시 URL 목록 복사 기능을 제공한다.

- [x] CDN 반영 확인 버튼
  - 파일 컨텍스트 메뉴에 `CDN 확인`을 추가한다.
  - `200 OK`, 응답 헤더, 캐시 상태를 간단히 표시한다.

- [x] Cache-Control 입력 UI
  - 프로파일 또는 업로드 옵션에 Cache-Control preset을 제공한다.
  - `기본`, `HTML 짧게`, `정적 에셋 길게`, `직접 입력` 정도로 시작한다.

---

## P2 - Smart Sync / Dry-run 개선

### Backend

- [x] Dry-run 결과에 Purge 대상 포함
  - `new`, `modified`, `deleted`, `unchanged` 외에 `purgeTargets`를 반환한다.
  - `purgeOnNewUpload` 설정을 반영한다.

- [x] multipart ETag 호환성 옵션
  - S3-compatible 서비스마다 ETag 정책이 다를 수 있으므로 fallback 비교 전략을 추가한다.
  - 크기/수정일/메타데이터 비교 fallback을 제공한다.

### UI/UX

- [x] Dry-run UI 버튼
  - Toolbar에 `미리보기` 버튼을 추가한다.
  - 업로드 전 신규/수정/스킵/삭제/파일 수와 크기를 보여준다.

- [x] Purge Preview 패널
  - 업로드 실행 전 Purge될 CDN URL 목록을 보여준다.
  - 신규 업로드 Purge 옵션이 켜져 있으면 신규 파일도 Purge 대상으로 표시한다.

- [x] Smart Sync 배지 개선
  - 현재 `신규`, `교체` 배지에 더해 `스킵 예정`, `Purge 예정` 상태를 분리한다.
  - 색상과 용어를 실제 동작에 맞게 정리한다.

---

## P3 - 사용성 / 안전장치

### Backend

- [x] 프로파일 검증 강화
  - CDN Provider별 필수 필드 누락을 저장 전에 검증한다.
  - S3 Custom Endpoint URL 형식을 검증한다.

- [x] 작업 취소
  - 진행 중인 업로드/다운로드 취소를 지원한다.
  - multipart 업로드 취소 시 `AbortMultipartUpload`를 호출한다.

- [x] 재시도 정책
  - S3 업로드, 다운로드, CDN Purge에 제한적 retry/backoff를 적용한다.

### UI/UX

- [x] 프로파일 모달 정보 구조 개선
  - S3 설정, CDN 설정, Purge 정책을 탭 또는 접이식 섹션으로 분리한다.
  - `신규 업로드도 Purge` 옵션에는 설명 문구를 붙인다.

- [x] 비용/무료 테스트 경고 표시
  - AWS/Akamai 실제 계정 프로파일에서는 테스트 전 확인 문구를 보여준다.
  - LocalStack 프로파일은 비용 없음으로 명시한다.

- [x] 빈 상태 안내 개선
  - Remote 빈 버킷, 연결 전, CDN 미설정 상태를 각각 다른 메시지로 안내한다.

- [x] 접근성/키보드 조작
  - 주요 버튼에 단축키와 포커스 스타일을 추가한다.
  - 파일 목록에서 키보드 선택/삭제/이름 변경을 지원한다.

---

## P4 - 테스트 / 문서

- [x] CDN Purge mock adapter 추가
  - 실제 CloudFront/Akamai 호출 없이 Purge 요청 생성 여부를 테스트한다.
  - CI 또는 로컬 무료 테스트에서 조건부 Purge를 검증한다.

- [x] LocalStack 기반 통합 테스트 스크립트
  - 버킷 생성, 업로드, Smart Sync, 다운로드, 삭제를 자동화한다.

- [x] TEST_GUIDE.md와 TODO.md 동기화
  - 기능이 구현될 때마다 테스트 절차를 업데이트한다.

- [x] 스크린샷 기반 UI 확인
  - 프로파일 모달, 전송 큐, Purge 이력, Dry-run UI를 캡처해 회귀 확인한다.
