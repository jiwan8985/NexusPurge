# NexusPurge 부문장 보고용 PPT 원고

> 기준: 현재 구현된 기능만 포함. 미구현 확장 계획은 제외.  
> 구성: 슬라이드별 핵심 메시지, 주요 내용, 캡처 가이드.

---

## Slide 1. 표지

**제목**  
NexusPurge - S3 파일 배포 및 CDN Purge 데스크톱 도구

**핵심 메시지**  
FTP 스타일 듀얼 패널 UI로 로컬 파일과 S3 버킷을 비교·전송하고, CDN Purge와 작업 로그를 함께 관리하는 데스크톱 앱입니다.

**캡처 가이드**  
앱 전체 메인 화면

---

## Slide 2. Executive Summary

**핵심 메시지**  
NexusPurge는 S3 파일 배포와 CDN Purge를 하나의 데스크톱 UI에서 처리합니다.

**주요 내용**

- 로컬 파일 시스템과 S3 버킷을 듀얼 패널로 탐색합니다.
- S3 프로필을 저장하고 연결할 수 있습니다.
- 파일 업로드/다운로드, 삭제, 폴더 생성 등 S3 작업을 수행합니다.
- 동기화 미리보기와 전송 진행률 UI가 구현되어 있습니다.
- 작업 로그, 전송 큐, Purge 이력 패널이 있습니다.
- CloudFront와 CDN adapter 구조가 Rust 백엔드에 포함되어 있습니다.
- Secret Access Key는 OS Keyring 기반 저장 구조를 사용합니다.
- Presigned URL 생성 기능이 구현되어 있습니다.

**캡처 가이드**  
듀얼 패널 + 로그 패널

---

## Slide 3. 현재 해결하는 업무 문제

**핵심 메시지**  
S3 배포와 CDN Purge를 GUI에서 연결해 콘솔/CLI 중심 작업 부담을 줄입니다.

**기존 문제**

- S3 콘솔 또는 CLI로 파일을 업로드해야 합니다.
- CDN Purge를 별도 화면/API에서 수행해야 합니다.
- 여러 버킷/리전의 자격증명 전환이 번거롭습니다.
- 전송 전 변경 파일을 확인하기 어렵습니다.
- 배포 진행률과 로그를 한 화면에서 보기 어렵습니다.
- 임시 공유 URL 생성이 별도 작업입니다.

**현재 구현된 해결 방식**

- 로컬/S3 듀얼 패널 탐색
- 프로필 기반 S3 연결
- S3 파일 작업
- 동기화 미리보기
- 전송 진행률
- CDN Purge command/adapter
- Presigned URL
- 작업 로그/전송 큐/Purge 이력

**캡처 가이드**  
LocalPanel, RemotePanel, ProfileModal

---

## Slide 4. 핵심 기능 구성

**핵심 메시지**  
데스크톱 파일 배포에 필요한 탐색, 전송, 동기화, CDN, 로그, 프로필 기능이 구현되어 있습니다.

| 영역 | 현재 구현 기능 |
| --- | --- |
| 앱 UI | Tauri 창, TitleBar, Toolbar, StatusBar, ErrorBoundary |
| 패널 | LocalPanel, RemotePanel, 파일 선택/정렬/경로 이동 |
| 전송 | TransferButtons, ProgressDialog, transfer queue |
| 동기화 | SyncPreviewDialog, SyncPlan, SyncPreviewResult |
| 로그 | LogPanel, 작업 로그, 전송 큐, Purge 이력 |
| 프로필 | ProfileModal, 프로필 CRUD, 마지막 프로필 복원 |
| S3 | list/get/put/delete/head, presigned URL |
| CDN | purge command, CloudFront adapter, CDN adapter trait |
| 보안 | keyring crate 기반 Secret 저장 구조 |

**캡처 가이드**  
전체 UI, Sync Preview, LogPanel

---

## Slide 5. 듀얼 패널과 프로필

**핵심 메시지**  
로컬 파일과 S3 객체를 나란히 표시하고, 프로필로 버킷/리전을 전환합니다.

**현재 기능**

- LocalPanel과 RemotePanel
- 파일 선택과 정렬
- 로컬/원격 경로 이동
- S3 프로필 목록 로드
- 활성 프로필 설정
- 연결 상태 관리
- 마지막 프로필 복원
- ProfileModal 제공

**프로필 주요 정보**

- 프로필 이름
- 버킷 이름
- 리전
- Access Key ID
- Secret Access Key
- 커스텀 엔드포인트
- CDN 관련 설정

**캡처 가이드**  
메인 패널, ProfileModal

---

## Slide 6. S3 파일 작업과 동기화 미리보기

**핵심 메시지**  
S3 객체 작업과 전송 전 미리보기로 배포 대상을 확인할 수 있습니다.

**현재 구현 영역**

- `commands/s3.rs`
- `adapters/storage/s3.rs`
- `utils/sigv4.rs`
- `utils/hash.rs`
- `utils/retry.rs`
- `SyncPreviewDialog`

**기능**

- S3 객체 목록 조회
- 객체 업로드/다운로드
- 객체 삭제
- metadata/head 조회
- Presigned URL 생성
- 전송 전 변경 내역 미리보기

**캡처 가이드**  
RemotePanel, Sync Preview Dialog

---

## Slide 7. 전송 진행률과 작업 로그

**핵심 메시지**  
전송 queue, 진행률 dialog, 작업 로그로 배포 상태를 확인합니다.

**현재 기능**

- 전송 queue 상태
- ProgressDialog
- 완료/실패/취소/스킵 상태 관리
- LogPanel
- 작업 로그
- 전송 큐 탭
- Purge 이력 탭
- 로그 최대 1000개 유지

**운영 의미**

- 전송 중 상태 확인
- 실패 원인 추적
- Purge 실행 여부 확인
- 작업 이력 확인

**캡처 가이드**  
ProgressDialog, LogPanel 3개 탭

---

## Slide 8. CDN Purge와 Presigned URL

**핵심 메시지**  
CDN Purge와 임시 URL 생성 기능을 S3 배포 도구 안에서 제공합니다.

**CDN 관련 구현**

- `commands/cdn.rs`
- `adapters/cdn/base.rs`
- `adapters/cdn/cloudfront.rs`
- CDN provider별 adapter 파일
- Mock adapter
- Purge 이력 UI

**Presigned URL**

- S3 객체 임시 접근 URL 생성
- QA 검수 링크
- 임시 파일 공유
- 콘솔 접속 없이 앱에서 URL 생성

**캡처 가이드**  
Purge 이력, Presigned URL 액션

---

## Slide 9. 시스템 아키텍처와 기술 스택

**핵심 메시지**  
React UI와 Rust/Tauri 백엔드가 IPC로 연결되어 S3/CDN 작업을 수행합니다.

```text
React Frontend
→ runtime.invoke
→ Tauri command
→ Rust async backend
→ S3 Adapter / CDN Adapter
→ AWS S3 / CDN API
→ OS Keyring / Local config
```

| 레이어 | 현재 기술 |
| --- | --- |
| Desktop | Tauri 2 |
| Frontend | React 18, TypeScript, Vite |
| State | Zustand |
| Backend | Rust, Tokio |
| HTTP | reqwest native-tls |
| 인증 | SigV4 자체 구현 |
| Secret | keyring crate |
| Test | Vitest, cargo test |

**캡처 가이드**  
PPT에서 구성도 작성

---

## Slide 10. 보안과 운영 효과

**핵심 메시지**  
프로필 단위 권한과 OS Keyring 기반 Secret 저장 구조로 배포 작업을 관리합니다.

**보안 요소**

- Secret Access Key를 keyring crate로 저장하는 구조
- 프로필 기반 계정/버킷/리전 분리
- SigV4 서명 구현
- OS native TLS 사용

**운영 효과**

- CLI/콘솔 중심 배포 작업 감소
- 로컬 파일과 S3 객체 비교 편의성 향상
- 전송 전 미리보기로 실수 감소
- 전송 진행률과 로그 기반 결과 확인
- CDN Purge 작업 UI 통합
- Presigned URL 생성으로 임시 공유 간소화

**캡처 가이드**  
ProfileModal, Sync Preview, LogPanel

---

## Slide 11. 데모 시나리오

**핵심 메시지**  
현재 구현 기능만으로 “프로필 연결 → 파일 탐색 → 미리보기 → 전송 → 로그 확인 → URL 생성” 흐름을 시연할 수 있습니다.

**데모 순서**

1. 앱 실행
2. 프로필 선택 또는 생성
3. S3 연결
4. 로컬 디렉토리와 S3 prefix 탐색
5. 파일 선택
6. 동기화 미리보기 확인
7. 업로드 또는 다운로드 실행
8. ProgressDialog 확인
9. LogPanel에서 작업 결과 확인
10. Presigned URL 생성

**캡처 가이드**  
순서대로 6~8개 화면 캡처

---

## Slide 12. 현재 구현 범위와 의사결정 사항

**현재 구현 범위**

- Tauri 데스크톱 앱
- 로컬/S3 듀얼 패널
- 프로필 관리
- S3 연결과 파일 작업
- 동기화 미리보기
- 전송 queue/progress UI
- CDN purge command/adapter 구조
- 작업 로그/전송 큐/Purge 이력
- Presigned URL
- OS Keyring 기반 Secret 저장 구조

**제외 범위**

- 중앙 서버 기반 배포 이력 관리
- 조직 승인 workflow
- 팀 프로필 배포 시스템
- 자동 CI/CD 연동

**의사결정 필요 사항**

- 우선 적용 대상 S3 버킷/서비스
- CloudFront 또는 기타 CDN 적용 범위
- 삭제/덮어쓰기 작업 승인 필요 여부
- Presigned URL 사용 정책
- 배포 로그 보관 기준

**캡처 체크리스트**

- 메인, ProfileModal, Sync Preview, ProgressDialog, LogPanel, Purge 이력, Presigned URL
