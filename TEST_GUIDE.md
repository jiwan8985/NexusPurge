# NexusPurge 구현 검증 및 무료 테스트 가이드

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

이 시스템에서 Tencent는 사용하지 않는다.

업로드는 항상 AWS S3 또는 S3-compatible 객체 스토리지로만 간다. CloudFront와 Akamai는 업로드 대상이 아니라, 업로드 후 캐시 URL을 Purge하는 CDN Provider다.

무료 실제 연동 테스트는 다음 3가지를 사용한다.

| 대상 | 용도 | 이 문서의 테스트 방식 |
|------|------|----------------------|
| AWS S3 무료 범위 | 실제 S3 업로드/다운로드/Smart Sync | AWS S3 테스트 프로파일 |
| CloudFront Free 플랜 | 실제 CloudFront Invalidation | AWS S3 origin + CloudFront CDN |
| Akamai Free Trial/무료 테스트 권한 | Akamai Object Storage 또는 Akamai Fast Purge | S3-compatible 업로드 또는 Akamai CDN Purge |

공식 조건은 변동될 수 있으므로 콘솔에서 무료 플랜, 크레딧, 사용량을 확인한 뒤 진행한다.

---

## 1. 요구사항 기준 구현 상태

### Must-Have

| 요구사항 | 현재 상태 | 확인 내용 |
|----------|-----------|-----------|
| 멀티 스토리지 업로드 | 충족 | AWS S3와 S3-compatible endpoint 지원. 업로드는 CDN이 아니라 S3 계열로만 수행 |
| Smart Sync | 충족 | 로컬 MD5 또는 multipart ETag와 원격 ETag 비교 후 신규/스킵/덮어쓰기 분류 |
| 조건부 자동 Purge | 충족 | 기본은 덮어쓰기만 Purge. 프로파일 옵션으로 신규 업로드도 Purge 가능 |
| 멀티 CDN 어댑터 | 충족 | CloudFront, Akamai 지원. Tencent 제외 |
| GUI | 충족 | 프로파일, 로컬/원격 패널, 드래그 앤 드롭, 클릭 기반 전송 |
| 삭제 시 Purge | 충족 | S3 삭제 성공 후 CloudFront/Akamai 동일 Purge 명령으로 캐시 무효화 |
| CDN 연결 테스트 | 충족 | S3 연결 테스트와 별도로 CloudFront/Akamai 인증 및 권한 확인 |
| Purge 상태 조회 | 충족 | CloudFront Invalidation 상태 조회 지원. Akamai는 Fast Purge 요청 성공 상태를 반환 |
| CDN 반영 검증 | 충족 | 업로드 완료 후 CDN URL에 `HEAD`, 필요 시 작은 `GET`으로 응답 헤더 확인 |
| Cache-Control 설정 | 충족 | 프로파일의 Cache-Control 값을 업로드 메타데이터로 전달 |
| Content-Type override | 충족 | 비어 있으면 자동 감지, 입력하면 지정 MIME type으로 업로드 |
| Dry-run Purge Preview | 충족 | `new`, `modified`, `deleted`, `unchanged`, `purgeTargets`를 UI에서 확인 |
| multipart ETag fallback | 충족 | S3-compatible multipart ETag 차이가 있을 때 크기 기반 fallback 비교 옵션 제공 |
| 프로파일 검증 강화 | 충족 | CDN Provider별 필수값과 S3 Custom Endpoint URL 형식을 저장 전에 검증 |
| 작업 취소 | 충족 | 전송 큐에서 업로드/다운로드 취소 가능. multipart 업로드는 취소 시 AbortMultipartUpload 호출 |
| 재시도 정책 | 충족 | S3 요청과 CDN Purge에 제한적 retry/backoff 적용 |
| 프로파일 모달 구조 | 충족 | S3 설정, CDN 설정, Purge 정책을 접이식 섹션으로 분리 |
| 비용/무료 테스트 경고 | 충족 | 실제 계정 테스트 전 확인, LocalStack은 비용 없음 표시 |
| 접근성/키보드 조작 | 충족 | 주요 단축키와 파일 목록 키보드 선택/삭제/이름 변경 지원 |
| CDN Purge mock adapter | 충족 | 실제 CDN 호출 없이 purge URL 기록을 unit test로 검증 |
| LocalStack 통합 스크립트 | 충족 | `pnpm run test:localstack`로 기본 S3 흐름 자동 확인 |

### Nice-to-Have

| 요구사항 | 현재 상태 |
|----------|-----------|
| 실시간 진행률 표시 | 구현됨. 업로드/다운로드 Progress Dialog와 전송 이벤트 |
| 작업 이력 로깅 | 구현됨. 작업 로그, 전송 큐, Purge 이력, 로그 저장 |
| Dry-run 모드 | 부분 구현. `sync_preview` 커맨드와 업로드 직전 Smart Sync 계획 표시 |
| 병렬 처리 | 구현됨. 파일 단위 4개, multipart part 단위 4개 |

---

## 2. 무료 테스트 경로

권장 순서:

```text
1. LocalStack으로 기본 기능 검증
2. AWS S3 무료 범위로 실제 S3 연동 검증
3. CloudFront Free 플랜으로 조건부 Purge 검증
4. Akamai Free Trial/무료 권한으로 Akamai Object Storage 또는 Akamai Fast Purge 검증
```

비용 방지 체크:

```text
[ ] AWS Billing에서 Free/credit 상태 확인
[ ] CloudFront 배포 생성 시 Free 플랜 선택 가능 여부 확인
[ ] Akamai Trial credit 및 Object Storage 활성화 조건 확인
[ ] 테스트 파일은 15 MB 이하 더미 파일만 사용
[ ] 테스트 후 CloudFront 배포, S3 버킷, Akamai 리소스 삭제 또는 비활성화
```

---

## 3. LocalStack 기본 테스트

### 3.1 실행

```bash
docker run -d \
  -p 4566:4566 \
  -e SERVICES=s3 \
  --name localstack \
  localstack/localstack:4.4.0
```

이미 컨테이너가 있으면:

```bash
docker stop localstack
docker rm localstack
```

버킷 생성:

```bash
awslocal s3api create-bucket \
  --bucket nexuspurge-test \
  --region us-east-1
```

### 3.2 앱 실행

```bash
pnpm install
pnpm run tauri:dev
```

### 3.3 LocalStack 프로파일

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `LocalStack 테스트` |
| Region | `us-east-1` |
| Bucket | `nexuspurge-test` |
| Access Key ID | `test` |
| Secret Access Key | `test` |
| Custom Endpoint | `http://localhost:4566` |
| CDN Provider | `사용 안 함` |

이 프로파일로 연결, 업로드, 다운로드, 삭제, Smart Sync, multipart, Presigned URL을 먼저 검증한다.

---

## 4. 테스트 파일 준비

macOS/Linux:

```bash
mkdir -p /tmp/nexuspurge-test/subdir
printf "Hello NexusPurge\n" > /tmp/nexuspurge-test/hello.txt
printf "Second file\n" > /tmp/nexuspurge-test/subdir/second.txt
dd if=/dev/urandom of=/tmp/nexuspurge-test/large-file.bin bs=1m count=15
```

Windows PowerShell:

```powershell
New-Item -ItemType Directory -Force C:\test-files\subdir
Set-Content C:\test-files\hello.txt "Hello NexusPurge"
Set-Content C:\test-files\subdir\second.txt "Second file"
$bytes = New-Object byte[] (15 * 1024 * 1024)
[System.Random]::new().NextBytes($bytes)
[System.IO.File]::WriteAllBytes("C:\test-files\large-file.bin", $bytes)
```

---

## 5. AWS S3 무료 범위 테스트

### 5.1 AWS 콘솔 확인

AWS Console에서 먼저 확인한다.

```text
[ ] Free plan 또는 Free Tier credit 사용 가능
[ ] Billing 알림 설정
[ ] 테스트 리전 선택: ap-northeast-2 또는 us-east-1
[ ] 테스트 후 삭제할 리소스 목록 기록
```

### 5.2 S3 버킷 생성

S3 Console -> Create bucket:

```text
Bucket name: nexuspurge-free-[고유값]
Region: ap-northeast-2 또는 us-east-1
Block Public Access: On
Versioning: Off
Default encryption: 기본값
```

### 5.3 IAM 권한

테스트용 IAM 사용자 또는 Access Key에 최소 권한을 부여한다.

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Sid": "S3TestAccess",
      "Effect": "Allow",
      "Action": [
        "s3:ListBucket",
        "s3:GetObject",
        "s3:PutObject",
        "s3:DeleteObject",
        "s3:AbortMultipartUpload",
        "s3:ListBucketMultipartUploads",
        "s3:ListMultipartUploadParts"
      ],
      "Resource": [
        "arn:aws:s3:::nexuspurge-free-*",
        "arn:aws:s3:::nexuspurge-free-*/*"
      ]
    }
  ]
}
```

### 5.4 AWS S3 프로파일

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `AWS S3 무료 테스트` |
| Region | 버킷 리전 |
| Bucket | `nexuspurge-free-[고유값]` |
| Access Key ID | AWS Access Key ID |
| Secret Access Key | AWS Secret Access Key |
| Custom Endpoint | 비움 |
| CDN Provider | `사용 안 함` |

검증:

```text
[ ] 연결 테스트 성공
[ ] hello.txt 업로드
[ ] 동일 파일 재업로드 시 스킵
[ ] 수정 후 재업로드 시 덮어쓰기
[ ] large-file.bin multipart 업로드
[ ] 다운로드 후 해시 비교
[ ] 삭제
```

---

## 6. CloudFront Free 플랜 테스트

CloudFront Free 플랜은 CloudFront 배포 단위로 선택하는 $0/month 플랜이다. AWS 문서상 Free 플랜은 월 사용량 허용치가 있고, 일반 invalidation도 월 1,000 path까지 무료 범위가 있다. 콘솔에서 Free 플랜을 선택할 수 없거나 계정 조건이 맞지 않으면 이 테스트는 중단한다.

### 6.1 S3 Origin 준비

5장에서 만든 S3 버킷을 origin으로 사용한다.

CloudFront Console -> Create distribution:

```text
Origin domain: nexuspurge-free-[고유값].s3.[region].amazonaws.com
Origin access: Origin access control (OAC)
Viewer protocol policy: Redirect HTTP to HTTPS
Cache policy: CachingOptimized
Pricing plan: Free 플랜 선택
```

배포 생성 후 기록:

```text
Distribution ID: E...
Domain name: dxxxx.cloudfront.net
```

### 6.2 IAM에 CloudFront 권한 추가

기존 테스트 IAM 정책에 추가한다.

```json
{
  "Sid": "CloudFrontInvalidation",
  "Effect": "Allow",
  "Action": [
    "cloudfront:CreateInvalidation",
    "cloudfront:GetDistribution"
  ],
  "Resource": "*"
}
```

### 6.3 앱 프로파일 CDN 설정

AWS S3 프로파일을 편집한다.

| 필드 | 값 |
|------|----|
| CDN Provider | `AWS CloudFront` |
| Distribution ID | CloudFront Distribution ID |
| CDN Domain | `dxxxx.cloudfront.net` |
| 신규 업로드도 Purge | 기본 Off. 신규 파일도 CDN에 미리 있거나 stale 캐시 제거가 필요하면 On |
| Cache-Control | 자동 또는 `no-cache`, `max-age=31536000, immutable` |
| Content-Type override | 비움. 특정 테스트가 필요하면 `text/plain` 등 입력 |

저장 후 프로파일 편집 화면에서 `CDN 연결 테스트`를 누른다.

기대 결과:

```text
[ ] CloudFront Distribution ID가 유효함
[ ] IAM 권한으로 GetDistribution 호출 가능
[ ] 배포 도메인 dxxxx.cloudfront.net 표시
```

### 6.4 조건부 Purge 검증

1. `hello.txt`를 최초 업로드한다.
2. CloudFront URL로 접근한다.

```bash
curl https://dxxxx.cloudfront.net/hello.txt
```

3. 로컬 `hello.txt` 내용을 수정한다.
4. 같은 remote path로 다시 업로드한다.
5. 전송 큐와 Purge 이력 탭을 확인한다.

기대 결과:

```text
기본 설정: 최초 업로드는 CDN Purge 없음
신규 업로드도 Purge On: 최초 업로드도 CloudFront Invalidation 생성
덮어쓰기 업로드: CloudFront Invalidation 생성
전송 큐: 완료 + CDN
Purge 이력: CDN/Purge 로그 표시
CDN 반영 확인: 업로드 완료 후 작업 로그에 200 응답 또는 경고 표시
```

신규 업로드 Purge 검증:

```text
1. 프로파일에서 신규 업로드도 Purge를 On
2. CDN Provider가 설정된 상태로 새 파일 upload-new.txt 최초 업로드
3. 전송 큐에 + CDN 표시
4. CloudFront Invalidation 목록에서 /upload-new.txt 확인
```

삭제 Purge 검증:

```text
1. CDN Provider가 설정된 프로파일로 hello.txt를 S3에서 삭제
2. 작업 로그에서 S3 삭제 완료 확인
3. 작업 로그에서 삭제 CDN Purge 완료와 Invalidation ID 확인
4. CloudFront Console -> Invalidations에서 /hello.txt 확인
```

UI 검증:

```text
[ ] 삭제 확인 창에 CDN Purge 안내 표시
[ ] 전송 완료 화면에 Purge 대기/진행중/완료/실패 배지 표시
[ ] 업로드 완료 후 CDN URL 목록 표시
[ ] CDN URL 전체 복사 가능
[ ] Remote 파일 우클릭 메뉴에서 CDN URL 복사/열기 가능
[ ] Remote 파일 우클릭 메뉴에서 CDN 반영 확인 가능
[ ] 작업 로그에 CDN 확인 결과 표시
```

Purge 상태 조회는 개발자 도구 콘솔에서 확인할 수 있다.

```javascript
const { invoke } = window.__TAURI__.core;

await invoke("get_purge_status", {
  profileId: "프로파일-ID",
  provider: "cloudfront",
  distributionId: "E...",
  invalidationId: "I..."
});
```

기대 결과:

```text
status: InProgress 또는 Completed
```

---

## 7. Akamai 무료 테스트

Akamai는 두 가지를 구분한다.

```text
Akamai Object Storage: S3-compatible 업로드 대상
Akamai Fast Purge: Akamai CDN 캐시 무효화
```

NexusPurge의 Akamai 어댑터는 Fast Purge API v3 URL invalidation을 호출한다. Akamai Object Storage만 있어서는 Akamai Purge 테스트가 되지 않는다.

### 7.1 Akamai Object Storage Free Trial로 업로드 테스트

Akamai Cloud Manager에서 Free Trial/credit 상태를 확인한다.

```text
[ ] Trial credit 사용 가능
[ ] Object Storage 활성화 조건 확인
[ ] 테스트 후 Object Storage 취소/정리 계획 확인
```

Object Storage Access Key 생성:

```text
Cloud Manager -> Object Storage -> Access Keys -> Create Access Key
Region: 테스트 리전
Permission: Read/Write
```

버킷 생성:

```text
Bucket name: nexuspurge-akamai-free-[고유값]
Region: Access Key와 같은 리전
```

앱 프로파일:

| 필드 | 값 |
|------|----|
| 프로파일 이름 | `Akamai Object Storage 무료 테스트` |
| Region | endpoint에 맞는 값 |
| Bucket | `nexuspurge-akamai-free-[고유값]` |
| Access Key ID | Akamai Object Storage Access Key |
| Secret Access Key | Akamai Object Storage Secret Key |
| Custom Endpoint | `https://[s3-endpoint-hostname]` |
| CDN Provider | `사용 안 함` |

검증:

```text
연결 테스트
업로드
Smart Sync
multipart
다운로드
삭제
```

### 7.2 Akamai Fast Purge 무료/Trial 권한으로 CDN 테스트

필수 조건:

```text
[ ] Akamai CDN으로 서비스되는 테스트 도메인
[ ] CDN 도메인이 AWS S3 또는 Akamai Object Storage origin을 바라봄
[ ] EdgeGrid API Client 생성 가능
[ ] Fast Purge 권한 있음
```

EdgeGrid 값:

```ini
[default]
client_secret = ...
host = akab-xxxx.luna.akamaiapis.net
access_token = akab-...
client_token = akab-...
```

앱 프로파일 CDN 설정:

| 필드 | 값 |
|------|----|
| CDN Provider | `Akamai` |
| EdgeGrid 호스트 | `.edgerc`의 `host` |
| Client Token | `.edgerc`의 `client_token` |
| Access Token | `.edgerc`의 `access_token` |
| Client Secret | `.edgerc`의 `client_secret` |
| CDN 도메인 | Akamai CDN으로 서비스되는 도메인 |
| 신규 업로드도 Purge | 기본 Off. 신규 URL 캐시 제거가 필요하면 On |
| Cache-Control | 자동 또는 직접 입력 |
| Content-Type override | 비움 또는 테스트 MIME type |

저장 후 프로파일 편집 화면에서 `CDN 연결 테스트`를 누른다.

기대 결과:

```text
[ ] EdgeGrid 인증 성공
[ ] Fast Purge 권한 테스트 성공
[ ] CDN 도메인 표시
```

검증:

```text
1. hello.txt 최초 업로드 -> 기본 설정이면 Purge 없음
2. https://[Akamai CDN 도메인]/hello.txt 접근
3. hello.txt 수정
4. 같은 remote path로 재업로드
5. Akamai Fast Purge 호출 확인
6. 전송 큐: 완료 + CDN
7. Purge 이력 탭 확인
8. 작업 로그에서 CDN 반영 확인 또는 미확인 경고 확인
```

신규 업로드 Purge 검증:

```text
1. 프로파일에서 신규 업로드도 Purge를 On
2. CDN Provider가 설정된 상태로 새 파일 upload-new.txt 최초 업로드
3. Akamai Fast Purge 호출 확인
4. 전송 큐에 + CDN 표시
```

삭제 Purge 검증:

```text
1. CDN Provider가 Akamai인 프로파일로 hello.txt를 S3에서 삭제
2. 작업 로그에서 S3 삭제 완료 확인
3. 작업 로그에서 삭제 CDN Purge 완료 확인
4. Akamai Control Center 또는 로그에서 Fast Purge 요청 확인
```

Akamai Purge 상태 조회:

```javascript
const { invoke } = window.__TAURI__.core;

await invoke("get_purge_status", {
  profileId: "프로파일-ID",
  provider: "akamai",
  distributionId: "",
  invalidationId: ""
});
```

기대 결과:

```text
status: Accepted
message: Akamai Fast Purge는 요청 성공 후 별도 Invalidation ID 없이 처리됨
```

---

## 8. 공통 기능 테스트

### 8.1 로컬 파일 기능

```text
[ ] Local 패널에서 테스트 디렉터리 열기
[ ] 새 폴더 생성
[ ] 이름 변경
[ ] 삭제
```

### 8.2 S3 객체 기능

```text
[ ] Remote 목록 탐색
[ ] S3 폴더 생성
[ ] S3 이름 변경
[ ] S3 삭제
[ ] CDN Provider 설정 상태에서 S3 삭제 시 삭제 CDN Purge 로그 표시
```

### 8.3 Smart Sync

```text
[ ] 신규 파일: 업로드
[ ] 동일 파일: 스킵
[ ] 변경 파일: 덮어쓰기
[ ] 기본 설정에서는 덮어쓰기일 때만 CDN Purge
[ ] 신규 업로드도 Purge On이면 신규 파일도 CDN Purge
[ ] CDN Domain이 있으면 업로드 완료 후 CDN 반영 확인 로그 표시
```

### 8.6 업로드 메타데이터

프로파일에서 다음 값을 설정한 뒤 새 파일을 업로드한다.

```text
Cache-Control: no-cache
Content-Type override: text/plain
```

CDN 또는 S3 객체 헤더를 확인한다.

```bash
curl -I https://[CDN-DOMAIN]/hello.txt
```

기대 결과:

```text
Cache-Control: no-cache
Content-Type: text/plain
```

### 8.7 안전장치

프로파일 저장 검증:

```text
[ ] Custom Endpoint에 잘못된 문자열 입력 시 저장 실패
[ ] CloudFront 선택 후 Distribution ID 또는 CDN Domain 누락 시 저장 실패
[ ] Akamai 선택 후 EdgeGrid 필수값 또는 CDN Domain 누락 시 저장 실패
[ ] CDN Provider를 사용 안 함으로 두면 S3-only 프로파일 저장 가능
```

작업 취소:

```text
[ ] 큰 파일 업로드 시작
[ ] Progress Dialog에서 해당 항목의 취소 버튼 클릭
[ ] 전송 상태가 취소됨으로 변경
[ ] multipart 업로드 중 취소 시 원격에 미완료 multipart가 남지 않는지 콘솔에서 확인
[ ] 다운로드 중 취소 시 상태가 취소됨으로 변경
```

재시도:

```text
[ ] 일시적인 408/429/5xx 또는 네트워크 오류에서 S3 요청이 제한적으로 재시도됨
[ ] CloudFront/Akamai Purge 실패 시 최대 3회 retry/backoff 후 실패 로그 표시
```

### 8.8 UI 사용성/접근성

프로파일 모달:

```text
[ ] S3 설정, CDN 설정, Purge 정책이 접이식 섹션으로 표시됨
[ ] 신규 업로드도 Purge 옵션에 설명 문구가 표시됨
[ ] LocalStack endpoint 입력 시 비용 없음 안내 표시
[ ] 실제 AWS/Akamai 연결 테스트 전 확인 창 표시
```

Remote/Local 파일 목록:

```text
[ ] Tab으로 파일 row에 포커스 가능
[ ] Enter로 폴더 열기 또는 파일 선택 가능
[ ] Space로 선택 토글 가능
[ ] Delete/Backspace로 삭제 가능
[ ] F2로 이름 변경 가능
```

단축키:

```text
[ ] Ctrl/Cmd+P: 프로파일 모달 열기
[ ] Ctrl/Cmd+R: 현재 포커스 패널 새로고침
[ ] Ctrl/Cmd+D: Dry-run 미리보기
```

### 8.9 자동/반자동 테스트

CDN mock unit test:

```bash
cargo test --manifest-path src-tauri/Cargo.toml mock --lib
```

LocalStack 통합 스크립트:

```bash
pnpm run test:localstack
```

스크린샷 회귀 체크:

```text
scripts/ui-screenshot-checklist.md 항목대로 프로파일 모달, 전송 큐, Purge 이력, Dry-run UI를 캡처한다.
```

### 8.4 Dry-run

Toolbar에서 `미리보기`를 누른다.

기대 결과:

```text
[ ] Dry-run 패널 표시
[ ] 신규/수정/삭제 후보/스킵 수 표시
[ ] Purge Preview에 덮어쓰기 Purge 대상 표시
[ ] 신규 업로드도 Purge On이면 신규 파일도 Purge 대상으로 표시
[ ] Purge 목록 복사 가능
```

개발자 도구 콘솔로도 확인할 수 있다.

```javascript
const { invoke } = window.__TAURI__.core;

await invoke("sync_preview", {
  profileId: "프로파일-ID",
  localDir: "/tmp/nexuspurge-test",
  remotePrefix: ""
});
```

기대 결과:

```text
new
modified
deleted
unchanged
purgeTargets
```

### 8.4.1 multipart ETag fallback

S3-compatible 스토리지에서 multipart ETag 정책이 AWS와 다르면 프로파일에서 `Multipart ETag fallback`을 켠다.

```text
[ ] 10 MB 이상 파일 업로드
[ ] 같은 파일로 Dry-run 또는 업로드 계획 생성
[ ] 원격 크기가 같고 multipart ETag 형식이면 스킵으로 분류
[ ] fallback Off일 때는 ETag 불일치 시 수정/덮어쓰기 대상으로 분류
```

### 8.5 로그

```text
[ ] 작업 로그 표시
[ ] 전송 큐 표시
[ ] Purge 이력 표시
[ ] 로그 파일 저장
```

---

## 9. 테스트 후 정리

AWS:

```text
[ ] CloudFront 배포 비활성화 후 삭제
[ ] S3 버킷 비우기 후 삭제
[ ] IAM Access Key 삭제
[ ] Billing 대시보드 확인
```

Akamai:

```text
[ ] 테스트 버킷 삭제
[ ] 사용하지 않는 Access Key revoke
[ ] 필요 없으면 Object Storage 취소
[ ] Trial credit/usage 확인
```

---

## 10. 빠른 체크리스트

```text
[ ] LocalStack 기본 테스트 통과
[ ] AWS S3 무료 범위 실제 업로드 통과
[ ] CloudFront Free 플랜 덮어쓰기 Purge 통과
[ ] Akamai Object Storage 무료 업로드 테스트 통과
[ ] Akamai Fast Purge 무료/Trial 테스트 통과
[ ] 기본 설정에서는 신규 업로드 Purge 안 됨
[ ] 신규 업로드도 Purge On이면 신규 업로드 Purge 됨
[ ] 덮어쓰기는 항상 Purge 됨
[ ] 삭제 성공 후 CDN Purge 됨
[ ] CDN 연결 테스트는 S3 연결 테스트와 별도로 성공/실패가 표시됨
[ ] CloudFront Purge 상태 조회에서 InProgress/Completed 확인
[ ] 업로드 후 CDN 반영 확인 로그가 표시됨
[ ] 삭제 확인 다이얼로그에 CDN Purge 안내가 표시됨
[ ] Progress Dialog에 Purge 상태 배지와 업로드 CDN URL 목록이 표시됨
[ ] Remote 우클릭 메뉴에서 CDN URL 복사/열기/확인이 동작함
[ ] Cache-Control과 Content-Type override가 응답 헤더에 반영됨
[ ] Dry-run 미리보기에서 purgeTargets가 표시됨
[ ] Multipart ETag fallback 옵션으로 S3-compatible ETag 차이를 완화할 수 있음
[ ] Smart Sync 배지가 신규/스킵/수정/Purge로 구분됨
[ ] 잘못된 프로파일 설정은 저장 전에 차단됨
[ ] 진행 중 전송을 취소할 수 있음
[ ] 일시 오류에서 S3/CDN 요청이 제한적으로 재시도됨
[ ] 프로파일 모달 섹션 구조와 비용 경고가 표시됨
[ ] 파일 목록 키보드 조작이 동작함
[ ] CDN mock unit test 통과
[ ] LocalStack 통합 스크립트 통과
[ ] UI screenshot checklist 업데이트
[ ] 모든 테스트 리소스 정리
```

---

## 11. 참고 링크

- LocalStack Docker image tags: https://hub.docker.com/r/localstack/localstack
- AWS S3 Pricing / Free Tier: https://aws.amazon.com/s3/pricing/
- CloudFront Free plan: https://aws.amazon.com/cloudfront/pricing/
- CloudFront flat-rate Free plan docs: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/flat-rate-pricing-plan.html
- CloudFront invalidation pricing: https://docs.aws.amazon.com/AmazonCloudFront/latest/DeveloperGuide/PayingForInvalidation.html
- Akamai Object Storage Free Trial: https://www.akamai.com/lp/object-storage
- Akamai Object Storage pricing: https://techdocs.akamai.com/cloud-computing/docs/object-storage-pricing
- Akamai EdgeGrid: https://techdocs.akamai.com/developer/docs/edgegrid
- Akamai purge mechanisms: https://techdocs.akamai.com/purge-cache/docs/purge-mechanisms
