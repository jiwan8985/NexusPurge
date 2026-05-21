# NexusPurge — 구현 현황 & 요구사항 확인

> 미팅 목적: 구현된 기능을 공유하고, 요구사항 대비 방향성이 맞는지 확인

---

## 프로젝트 한 줄 요약

**FTP 스타일 듀얼 패널 S3 업로드 도구**
- 로컬 ↔ S3를 나란히 보며 파일 전송
- 파일 덮어쓰기 감지 시 CDN(CloudFront / Akamai) 자동 Purge

---

## 기술 스택

| 영역 | 사용 기술 |
|------|-----------|
| UI | React 18 + TypeScript + Vite |
| 상태 관리 | Zustand |
| 데스크톱 런타임 | Tauri 2 (Rust) |
| 스토리지 | AWS S3 (aws-sdk-s3) |
| CDN | CloudFront, Akamai Fast Purge |
| 보안 | OS Keyring (macOS Keychain / Windows Credential Manager) |

---

## 구현된 기능 목록

### 1. 듀얼 패널 파일 탐색

- **로컬 패널**: OS 파일시스템 탐색 (폴더 이동, 파일 목록)
- **S3 패널**: 버킷/prefix 탐색, 페이지네이션 지원
- 양쪽 패널에서 파일 선택 후 중앙 버튼으로 전송
- 대용량 파일 목록 (1,000개+) → 가상 스크롤로 렌더링

### 2. Smart Sync 업로드

- 로컬 파일의 **MD5 해시** 계산 → S3 **ETag 비교** (병렬)
- 결과를 3가지로 분류:
  - `toUpload` — 신규 파일
  - `toSkip` — 동일한 파일 (MD5 일치)
  - `toOverwrite` — 변경된 파일 (덮어쓰기 필요)
- 10MB 이상 → **Multipart Upload** 자동 전환
- 동시 전송 최대 **4개** 제한 (Semaphore)

### 3. Dry-run (미리보기)

- 업로드 전 `SyncPreviewDialog`에서 변경 항목 확인
- 신규 / 수정 / 삭제 항목을 목록으로 표시
- 실제 전송 없이 확인만 가능

### 4. CDN 자동 Purge

| CDN | 방식 | 상태 |
|-----|------|------|
| **CloudFront** | InvalidationBatch API + SigV4 서명 | ✅ 구현 완료 |
| **Akamai** | Fast Purge CCU v3 + EdgeGrid 인증 | ✅ 구현 완료 |

- 덮어쓰기 항목만 Purge (기본값)
- `purgeOnNewUpload = true` 설정 시 신규 업로드도 Purge
- S3 삭제 시에도 자동으로 CDN Purge
- **Purge 후 HTTP HEAD 요청**으로 반영 여부 검증
- CloudFront는 Invalidation 상태까지 조회

### 5. 프로파일 관리

- 여러 S3 버킷을 **프로파일**로 저장
- 저장 정보:
  - S3: 리전, 버킷명, AccessKeyId, Endpoint (선택)
  - CDN: Provider, Distribution ID, 도메인
  - Akamai: Client Token, Access Token, API Host
- **민감 정보 (SecretKey)** → OS Keyring에만 저장, JSON 파일에 미포함
- 앱 재시작 시 **마지막 사용 프로파일 자동 복원**
- 저장 전 **연결 테스트** 버튼 (S3 / CloudFront / Akamai 각각)

### 6. 파일 조작

| 기능 | 로컬 | S3 원격 |
|------|------|---------|
| 새 폴더 생성 | ✅ | ✅ |
| 파일/폴더 삭제 | ✅ | ✅ |
| 이름 변경 | ✅ | ✅ (CopyObject + DeleteObject) |
| Presigned URL 생성 | — | ✅ (1시간 만료) |

### 7. 다운로드

- S3 패널에서 파일 선택 → 로컬 폴더 지정 → 다운로드
- 동시 다운로드 최대 4개 제한

### 8. 실시간 피드백

- 파일별 **진행률 바** + 전송 속도 표시
- 하단 **로그 패널**: info / warn / error / success / debug 구분
- 전송 취소 기능

---

## 아키텍처 결정 사항 (확인 필요)

아래 항목들은 이미 구현된 방향이지만, 요구사항과 맞는지 확인이 필요합니다.

### Q1. CDN Provider 범위

현재 지원: **CloudFront**, **Akamai**
- 추가로 필요한 CDN이 있나요? (Cloudflare, Fastly 등)
- Akamai는 CCU v3 기준으로 구현됨 — 다른 버전 필요한가요?

### Q2. 동시 전송 수 (Semaphore 4)

현재 동시 전송 상한이 **4개**로 고정되어 있습니다.
- 사용자가 조절 가능해야 하나요? (설정 UI 필요 여부)

### Q3. Multipart 기준 (10MB)

현재 10MB 이상 파일은 자동으로 멀티파트 업로드합니다.
- 기준 변경이나 사용자 설정이 필요한가요?

### Q4. Purge 트리거 기준

현재: 덮어쓰기 시 자동 Purge, 신규 업로드는 선택 (`purgeOnNewUpload` 플래그)
- 신규 업로드도 항상 Purge해야 하나요?
- 삭제 시 Purge는 항상 실행 — 이것도 선택사항이어야 하나요?

### Q5. 마지막 프로파일 복원

앱 재시작 시 마지막으로 사용한 프로파일이 **선택만** 되고 **자동 연결은 안 됩니다**.
- 자동 연결(자격증명 자동 로드)도 필요한가요?

### Q6. Cache-Control 기본값

파일 확장자별로 기본 Cache-Control 값을 프로파일에 설정할 수 있습니다.
- 어떤 확장자 / 어떤 값들이 기본으로 필요한가요?

### Q7. 지원 OS

현재 빌드 타겟: **Windows**, **macOS**, **Linux**
- 우선순위 OS가 있나요? (Keyring은 OS별로 지원 여부 상이)

---

## 미구현 / 논의 필요 항목

| 항목 | 상태 | 비고 |
|------|------|------|
| CloudFlare / 기타 CDN 어댑터 | 미구현 | 아키텍처는 확장 가능하게 설계됨 |
| 설정 모달 상세 기능 | 부분 구현 | 어떤 설정이 필요한지 확인 필요 |
| 폴더 단위 동기화 (Sync) | Dry-run까지 구현, 실행 미연결 | 전체 디렉터리 sync 필요 여부 |
| 전송 이력 / 로그 저장 | 미구현 | 세션 내 로그만 표시 |

---

## 빌드 & 실행

```bash
# 개발 서버
npm run tauri dev

# 릴리즈 빌드
npm run tauri build
```

결과물:
- Windows: `src-tauri/target/release/bundle/msi/*.msi`
- macOS: `src-tauri/target/release/bundle/dmg/*.dmg`
- Linux: `src-tauri/target/release/bundle/appimage/*.AppImage`
