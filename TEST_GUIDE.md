# NexusPurge 테스트 환경 구성 및 테스트 가이드

## 자동화 테스트 (로컬 실행)

### 프론트엔드 단위 테스트 (Vitest)

```bash
# 1회 실행
pnpm test

# 감시 모드 (파일 변경 시 자동 재실행)
pnpm test:watch
```

현재 커버리지:
- `src/test/appStore.test.ts` — Zustand store의 `addLog` (필드 검증, 1000개 상한), `addTransfer`/`updateTransfer` 동작 검증

### Rust 단위 테스트

```bash
cargo test --manifest-path src-tauri/Cargo.toml
```

현재 커버리지:
- `utils/retry.rs` — `retryable_codes_are_recognised` (429/500/502/503/504), `non_retryable_codes_are_rejected` (200/400/401/403/404 등)

### CI (GitHub Actions)

`main` 브랜치 push 및 PR 시 `.github/workflows/ci.yml`이 자동 실행됩니다:
- **frontend job**: `pnpm typecheck` → `pnpm test` → `pnpm build`
- **backend job**: `cargo check --release` → `cargo test`

---

## 통합 테스트 환경 선택

| 환경 | 비용 | S3 업로드 | CDN Purge | 멀티파트 | 권장 용도 |
|------|------|-----------|-----------|----------|-----------|
| **A. LocalStack** | 완전 무료 | ✅ | ❌ (Pro만) | ✅ | 로컬 개발 / CI |
| **B. AWS Free Tier** | 12개월 무료 | ✅ | ✅ CloudFront | ✅ | 전체 기능 통합 테스트 |
| **C. Cloudflare R2** | 영구 무료 | ✅ | ❌ (다른 API) | ✅ | S3 호환성 테스트 |

---

## 환경 A — LocalStack (로컬, 무료)

> S3 업로드·다운로드·삭제·Smart Sync 기능 테스트. CDN Purge 제외.

### 1. 사전 요구사항

```bash
# Docker Desktop 설치 확인
docker --version

# Python pip (LocalStack CLI용)
pip install localstack awscli-local
```

### 2. LocalStack 실행

```bash
# Docker로 LocalStack 커뮤니티 버전 실행
docker run --rm -d \
  -p 4566:4566 \
  -e SERVICES=s3 \
  -e DEFAULT_REGION=us-east-1 \
  --name localstack \
  localstack/localstack:latest

# 실행 확인
curl http://localhost:4566/_localstack/health
# {"services":{"s3":"running",...}}
```

### 3. 테스트용 S3 버킷 생성

```bash
# awslocal (LocalStack 전용 CLI)
awslocal s3api create-bucket \
  --bucket nexuspurge-test \
  --region us-east-1

# 버킷 확인
awslocal s3 ls
# 2024-01-01 00:00:00 nexuspurge-test
```

### 4. 앱에서 프로파일 설정

앱 실행 후 **프로파일 추가** 버튼 클릭:

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `LocalStack 테스트` |
| Region | `us-east-1` |
| Bucket | `nexuspurge-test` |
| Access Key ID | `test` (임의 값, LocalStack은 검증 안 함) |
| Secret Access Key | `test` |
| **Custom Endpoint** | `http://localhost:4566` ← 필수 |
| CDN Provider | (비워둠) |

### 5. 연결 테스트

1. 프로파일 선택 → **Connect** 클릭
2. 상태바에 `연결 성공: nexuspurge-test (us-east-1)` 확인

---

## 환경 B — AWS Free Tier (권장: 전체 기능 테스트)

> S3 + CloudFront 전체 워크플로우 테스트. CDN Purge까지 포함.

### 무료 한도 (신규 계정 기준)

- **S3**: 5 GB 저장, GET 20,000건, PUT 2,000건 / 월 (12개월)
- **CloudFront**: 데이터 전송 1 TB, 요청 10M건 / 월 (12개월)
- **데이터 전송**: S3 → CloudFront 무료 (같은 리전)

### 1. S3 버킷 생성

AWS Console → S3 → 버킷 만들기:

```
버킷 이름:    nexuspurge-test-[yourname]   (전역 고유)
리전:         ap-northeast-2 (서울)
퍼블릭 액세스: 모든 퍼블릭 액세스 차단 (기본값 유지)
버전 관리:    비활성화
```

### 2. CloudFront 배포 생성

AWS Console → CloudFront → 배포 생성:

```
원본 도메인:           [버킷명].s3.ap-northeast-2.amazonaws.com
원본 액세스:           Origin access control (OAC) 설정 권장
뷰어 프로토콜 정책:    Redirect HTTP to HTTPS
캐시 정책:             CachingOptimized
가격 분류:             Use only North America and Europe (비용 절감)
```

> 배포 완료까지 약 5~15분 소요. Distribution ID(E로 시작)를 메모해 둘 것.

### 3. IAM 사용자 + 정책 생성

AWS Console → IAM → 사용자 → 사용자 생성:

```
사용자 이름: nexuspurge-tester
액세스 유형: 프로그래밍 방식 액세스
```

**인라인 정책 (JSON)**:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "S3BucketAccess",
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:ListBucket",
        "s3:HeadObject"
      ],
      "Resource": [
        "arn:aws:s3:::nexuspurge-test-*",
        "arn:aws:s3:::nexuspurge-test-*/*"
      ]
    },
    {
      "Sid": "CloudFrontInvalidation",
      "Effect": "Allow",
      "Action": [
        "cloudfront:CreateInvalidation",
        "cloudfront:GetDistribution"
      ],
      "Resource": "*"
    }
  ]
}
```

사용자 생성 후 **Access Key ID**와 **Secret Access Key** 저장 (이 화면에서만 표시).

### 4. 앱에서 프로파일 설정

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `AWS 테스트` |
| Region | `ap-northeast-2` |
| Bucket | `nexuspurge-test-[yourname]` |
| Access Key ID | 위에서 발급한 키 |
| Secret Access Key | 위에서 발급한 시크릿 |
| Custom Endpoint | (비워둠) |
| CDN Provider | `cloudfront` |
| Distribution ID | CloudFront의 Distribution ID (E로 시작) |
| CDN Domain | `[xxxx].cloudfront.net` |

---

## 환경 C — Cloudflare R2 (영구 무료, S3 호환)

> S3 업로드/다운로드/삭제/멀티파트 테스트. CDN Purge 제외.

### 무료 한도

- 저장: 10 GB / 월
- Class A 오퍼레이션 (쓰기): 1,000,000건 / 월
- Class B 오퍼레이션 (읽기): 10,000,000건 / 월

### 1. R2 버킷 생성

Cloudflare Dashboard → R2 → 버킷 만들기:

```
버킷 이름: nexuspurge-test
위치:      자동
```

### 2. API 토큰 발급

Cloudflare Dashboard → R2 → API Tokens → 토큰 생성:

```
권한:       Object Read & Write (특정 버킷 지정 가능)
```

발급 후 **Access Key ID**와 **Secret Access Key** 저장.

### 3. R2 엔드포인트 확인

```
https://[ACCOUNT_ID].r2.cloudflarestorage.com
```

Account ID는 Cloudflare Dashboard 우측 하단에서 확인.

### 4. 앱에서 프로파일 설정

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `R2 테스트` |
| Region | `auto` |
| Bucket | `nexuspurge-test` |
| Access Key ID | R2 토큰의 Access Key ID |
| Secret Access Key | R2 토큰의 Secret |
| **Custom Endpoint** | `https://[ACCOUNT_ID].r2.cloudflarestorage.com` |
| CDN Provider | (비워둠) |

---

## 테스트 시나리오

### 시나리오 1: 기본 연결 및 디렉터리 탐색

```
목표: S3 연결 및 파일 목록 표시
환경: A / B / C 모두
```

1. 앱 실행 → 프로파일 선택 → **Connect**
2. Remote 패널에 버킷 내용 표시 확인
3. 빈 버킷이면 "(비어있음)" 또는 폴더 없음 표시 확인

**기대 결과**: Remote 패널 정상 로드, 에러 없음

---

### 시나리오 2: 단일 파일 업로드

```
목표: 소형 파일(< 10 MB) 업로드 및 Smart Sync 동작 확인
환경: A / B / C 모두
```

**테스트 파일 준비**:

```bash
# 테스트 파일 생성 (Windows PowerShell)
echo "Hello NexusPurge" > C:\test-files\hello.txt
echo "Test image placeholder" > C:\test-files\image.png
```

**단계**:

1. Local 패널 → `C:\test-files` 이동
2. `hello.txt` 선택
3. Remote 패널 → 업로드 대상 경로 이동 (예: 루트 `/`)
4. **Upload →** 버튼 클릭
5. Progress Bar 및 진행률 확인

**기대 결과**:
- 업로드 완료 100%
- Remote 패널 새로고침 시 `hello.txt` 목록에 표시
- 로그 패널에 `complete` 이벤트 기록

---

### 시나리오 3: Smart Sync (ETag 비교)

```
목표: ETag 비교로 변경 파일만 업로드, 미변경 파일 스킵 확인
환경: A / B / C 모두
```

**단계**:

1. `hello.txt` 다시 선택 후 업로드 → **같은 파일**이므로 `toSkip` 분류 확인
2. `hello.txt` 내용 변경:
   ```bash
   echo "Modified content" > C:\test-files\hello.txt
   ```
3. 변경된 `hello.txt` 업로드 → `toOverwrite` 분류 확인

**앱의 Sync Preview 확인 방법** (UI 사용):

1. Local 패널 경로를 `C:\test-files`로 설정
2. **⚖ 미리보기** 버튼 클릭
3. SyncPreviewDialog에서 탭별 파일 목록 확인:
   - **새 파일** 탭: 신규 업로드 예정 파일
   - **수정됨** 탭: ETag 불일치로 덮어쓰기 예정 파일
   - **삭제 예정** 탭: 리모트에만 존재하는 파일
   - **변경 없음** 탭: ETag 일치로 스킵될 파일

**기대 결과**:
- 동일 파일: `toSkip` → 업로드 생략
- 변경 파일: `toOverwrite` → 업로드 실행
- 로그에 각 파일의 판단 근거 표시

---

### 시나리오 4: 멀티파트 업로드 (10 MB 이상)

```
목표: 대용량 파일 자동 멀티파트 분할 업로드 확인
환경: A / B / C 모두
```

**테스트 파일 생성**:

```powershell
# 15 MB 더미 파일 생성 (PowerShell)
$bytes = New-Object byte[] (15 * 1024 * 1024)
[System.Random]::new().NextBytes($bytes)
[System.IO.File]::WriteAllBytes("C:\test-files\large-file.bin", $bytes)
```

**단계**:

1. `large-file.bin` (15 MB) 선택 → 업로드
2. Progress Bar가 파트별로 증가하는지 확인 (4개 병렬 파트)
3. 완료 후 Remote 패널에서 파일 크기 확인

**기대 결과**:
- 자동 멀티파트 업로드 실행 (Rust 로그에 `is_multipart: true`)
- S3 ETag 형식: `"xxxx-2"` (멀티파트 ETag, 파트 수 suffix)
- 업로드 완료 시 원본 파일과 크기 일치

---

### 시나리오 5: CDN Purge 자동 실행 (AWS Free Tier 전용)

```
목표: 파일 덮어쓰기 시 CloudFront Invalidation 자동 생성 확인
환경: B (AWS Free Tier)만 해당
```

**사전 준비**:
- CloudFront 배포가 S3 버킷을 원본으로 사용 중
- 프로파일에 CDN Provider = `cloudfront`, Distribution ID 설정 완료

**단계**:

1. `hello.txt` 최초 업로드 → CloudFront URL로 접근 확인:
   ```
   https://[xxx].cloudfront.net/hello.txt
   ```
2. `hello.txt` 내용 변경 후 재업로드 (덮어쓰기)
3. 앱 로그 패널에서 CDN Purge 결과 확인:
   ```
   [success] CloudFront Invalidation 생성: I1ABCDEF... (dist=E1XXXXX)
   ```
4. AWS Console → CloudFront → 무효화 탭에서 Invalidation ID 확인

**기대 결과**:
- `cdnPurged: true` 이벤트 수신
- CloudFront 무효화 상태: `InProgress` → `Completed` (약 1~5분)
- 변경된 파일이 CDN 캐시에서 제거되어 새 버전 서빙

---

### 시나리오 6: 파일 삭제

```
목표: S3 오브젝트 삭제 확인
환경: A / B / C 모두
```

1. Remote 패널에서 `hello.txt` 선택
2. **Delete** 버튼 클릭 → 확인 다이얼로그
3. 삭제 후 목록에서 사라지는 것 확인

```bash
# LocalStack에서 검증
awslocal s3 ls s3://nexuspurge-test/
# hello.txt 없음 확인
```

---

### 시나리오 7: 다운로드

```
목표: S3 파일 스트리밍 다운로드 확인
환경: A / B / C 모두
```

1. Remote 패널에서 `large-file.bin` 선택
2. Local 패널 목적 경로 확인 (예: `C:\Downloads`)
3. **← Download** 버튼 클릭
4. 진행률 및 완료 확인

**검증**:
```powershell
# 원본과 다운로드 파일 MD5 비교
Get-FileHash "C:\test-files\large-file.bin" -Algorithm MD5
Get-FileHash "C:\Downloads\large-file.bin" -Algorithm MD5
# 두 해시값 동일해야 함
```

---

## 로그 확인 방법

앱 하단 **Log Panel** 에서 각 동작의 상세 로그 확인:

| 로그 레벨 | 의미 |
|-----------|------|
| `[info]` | 일반 동작 정보 |
| `[success]` | 업로드/다운로드/Purge 완료 |
| `[warn]` | 스킵된 파일 (ETag 일치) |
| `[error]` | 업로드 실패, 인증 오류 |

**Rust 백엔드 로그 활성화** (개발 모드):

```bash
# 개발 서버 실행 시 상세 로그 출력
RUST_LOG=cdn_upload_tool_lib=debug npm run tauri dev
```

---

## 트러블슈팅

### 연결 실패: "버킷 접근 실패: HTTP 403"

| 원인 | 해결 |
|------|------|
| IAM 권한 부족 | `s3:ListBucket` 정책 추가 확인 |
| 버킷 이름 오타 | 프로파일의 Bucket 필드 재확인 |
| Region 불일치 | 버킷 생성 리전과 프로파일 Region 일치 확인 |
| LocalStack 미실행 | `docker ps` 로 컨테이너 상태 확인 |

### LocalStack 연결 실패: "connection refused"

```bash
# 컨테이너 로그 확인
docker logs localstack

# 포트 바인딩 확인
docker port localstack
# 4566/tcp -> 0.0.0.0:4566

# 상태 재확인
curl http://localhost:4566/_localstack/health
```

### R2 연결 실패: "InvalidAccessKeyId"

- Endpoint에 Account ID 포함 확인: `https://[ACCOUNT_ID].r2.cloudflarestorage.com`
- Region 필드를 정확히 `auto` 로 입력

### CloudFront Purge 실패: "AccessDenied"

```json
// IAM 정책에 아래 권한 추가 필요
{
  "Action": [
    "cloudfront:CreateInvalidation",
    "cloudfront:GetDistribution"
  ],
  "Resource": "*"
}
```

### 멀티파트 업로드 중단: "AbortMultipartUpload"

S3 콘솔 → 버킷 → **불완전한 멀티파트 업로드** 탭에서 잔여 파트 정리:

```bash
# AWS CLI로 정리
aws s3api list-multipart-uploads --bucket [버킷명]
aws s3api abort-multipart-upload \
  --bucket [버킷명] \
  --key [key] \
  --upload-id [upload-id]
```

---

## 빠른 연기 테스트 체크리스트

```
[ ] 자동화 테스트: pnpm test (Vitest 통과)
[ ] 자동화 테스트: cargo test (Rust 단위 테스트 통과)
[ ] 앱 빌드 성공: pnpm tauri dev
[ ] 환경 A (LocalStack) 또는 환경 B (AWS) 선택 및 구성
[ ] 프로파일 생성 및 연결 성공
[ ] 소형 파일 업로드 확인
[ ] Smart Sync: 동일 파일 스킵 확인
[ ] Smart Sync: 변경 파일 덮어쓰기 확인
[ ] 미리보기 버튼으로 SyncPreviewDialog 확인 (4탭 동작)
[ ] 대용량 파일(>10MB) 멀티파트 업로드 확인
[ ] 파일 다운로드 (폴더 선택 다이얼로그 포함) 및 MD5 검증
[ ] Presigned URL 3가지 만료 옵션 (15분·1시간·24시간) 확인
[ ] (환경 B만) CDN Purge 자동 실행 확인
[ ] 파일 삭제 확인
[ ] 에러 케이스: 잘못된 자격증명으로 연결 시도 → 에러 로그 확인
```
