# NexusPurge 개발 현황 및 프로젝트 분석

## 1. 프로젝트 개요

NexusPurge는 로컬 파일과 S3 버킷을 양방향으로 탐색하고 전송하는 Tauri 기반 데스크톱 애플리케이션이다. 핵심 목적은 FTP 스타일의 파일 운영 경험을 제공하면서, S3 업로드 후 변경 파일에 대해 CDN Purge까지 이어지는 배포 운영 흐름을 단순화하는 것이다.

현재 구조는 React + TypeScript 프론트엔드와 Rust + Tauri 2 백엔드로 분리되어 있다. 프론트엔드는 패널형 파일 탐색 UI, 프로필 관리, 전송 상태, 로그 패널을 담당하고, 백엔드는 로컬 파일 접근, S3 API 호출, 동기화 계획 수립, CDN 무효화 요청을 담당한다.

## 2. 현재까지 구현된 주요 기능

### React 프론트엔드

- 로컬 파일 패널과 S3 원격 패널을 좌우로 배치한 듀얼 패널 UI
- 프로필 선택 및 연결 상태 표시
- 로컬/S3 경로 입력, 상위 경로 이동, 새로고침
- 파일 선택, 다중 선택, 드래그 기반 업로드 트리거
- 업로드/다운로드 버튼 및 전송 진행 다이얼로그
- 작업 로그, 전송 큐, CDN Purge 이력을 확인하는 하단 로그 패널
- Zustand 기반 전역 상태 관리
- 대량 파일 목록 렌더링을 위한 가상 리스트 훅 적용

### Rust/Tauri 백엔드

- Tauri command 기반 IPC 구조
- 로컬 디렉터리 조회
- S3 객체 목록 조회, 삭제, Presigned URL 생성
- 업로드/다운로드 실행 흐름
- MD5/ETag 비교 기반 Smart Sync 계획 수립
- CloudFront Purge 어댑터 기본 구현
- 프로필 메타데이터와 OS keyring 기반 비밀키 저장 구조

## 3. 디렉터리 구조 분석

```text
src/
  components/        React UI 컴포넌트
  hooks/             Tauri invoke, S3, 전송, 프로필 로직
  store/             Zustand 전역 상태
  styles/            디자인 토큰 및 전역 스타일
  types/             프론트엔드와 백엔드 간 공유 모델

src-tauri/src/
  commands/          Tauri IPC 엔드포인트
  adapters/          S3, CDN 공급자 추상화 및 구현
  utils/             해시, 설정, SigV4 등 공통 유틸
```

프론트엔드와 백엔드의 역할 분리는 비교적 명확하다. 특히 `hooks/`가 UI 컴포넌트와 Tauri command 사이의 중간 계층 역할을 맡고 있어, 컴포넌트가 직접 IPC 세부사항을 알지 않아도 되는 구조다.

## 4. UI/UX 개선 내용

이번 개선에서는 기존의 투박한 도구형 화면을 상용 운영 콘솔에 가까운 형태로 정리했다.

- 상단 타이틀바에 브랜드 마크와 제품 설명을 추가
- 프로필 드롭다운을 워크스페이스 선택 UI처럼 재구성
- 툴바 버튼을 정돈된 액션 그룹으로 재배치
- 로컬/S3 패널을 카드형 운영 패널로 개선
- 경로 입력창, 컬럼 헤더, 선택 행, 상태 배지의 시각 계층 강화
- 업로드/다운로드 버튼을 중앙 전송 컨트롤로 명확하게 강조
- 상태바와 로그 영역 문구를 정상 한글로 정리
- 깨져 보이던 한글 UI 문자열을 주요 화면 기준으로 교체

## 5. 기술적 강점

- Tauri를 사용해 네이티브 파일 접근과 웹 UI 개발 생산성을 동시에 확보했다.
- Rust 백엔드가 파일 처리, 해시 계산, 네트워크 요청을 담당해 장기적으로 안정성과 성능 면에서 유리하다.
- S3와 CDN을 어댑터 구조로 분리해 CloudFront 외 공급자를 추가하기 쉽다.
- Smart Sync 구조가 이미 존재하므로 단순 업로드 도구보다 운영 자동화 가치가 높다.
- 가상 리스트를 적용해 파일 수가 많아져도 UI 성능 저하를 줄일 수 있다.

## 6. 개선이 필요한 부분

- CDN 공급자는 CloudFront 중심이며 Akamai, LG U+, Hyosung 등은 확장 지점만 준비된 상태다.
- 에러 메시지와 사용자 안내 문구를 전역적으로 정리할 필요가 있다.
- 대용량 파일(≥10MB) 다운로드가 단순 GET으로 처리되어 멀티파트 다운로드 미지원이다.
- 단위 테스트 커버리지를 늘릴 여지가 있다 (현재 store, retry 모듈 중심).

## 7. 권장 다음 작업

1. Akamai / LG U+ / 효성 ITX CDN 어댑터 실제 구현
2. 대용량 파일 멀티파트 다운로드 지원 추가
3. 단위 테스트 커버리지 확대 (useTransfer, useS3, 해시 계산 로직 등)
4. LocalStack 기반 통합 테스트 시나리오 자동화
5. 파일 충돌, 대용량 업로드, 네트워크 중단 시 복구 플로우 강화

## 8. 실행 및 검증 명령

```bash
pnpm install
pnpm run dev
pnpm run build
pnpm test
pnpm tauri dev
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
```

프론트엔드 빌드는 `pnpm build`로, 단위 테스트는 `pnpm test` 및 `cargo test`로 검증한다. 전체 데스크톱 동작은 `pnpm tauri dev`로 확인하고, S3 연동은 `TEST_GUIDE.md`의 LocalStack 또는 AWS 테스트 환경 기준으로 검증하는 것이 적절하다.
