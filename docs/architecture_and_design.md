# NexusPurge: 시스템 아키텍처 및 설계 정의서 (상세 기술 명세)

본 문서는 S3 호환 저장소 탐색 및 멀티 CDN 캐시 무효화(Purge) 기능을 통합 처리하는 데스크톱 전용 도구인 **NexusPurge**의 내부 설계, 스레드 병렬성, 자격증명 보안, 그리고 캐시 무효화 정규화 흐름을 설명합니다.

---

## 1. 스레드 모델 및 비동기 동시성 아키텍처

NexusPurge는 리액트(TS) 프론트엔드와 러스트 백엔드(Tauri 2 코어)가 비동기식 IPC(Inter-Process Communication)로 밀접하게 결합되어 구동됩니다.

```
                  +-----------------------------------+
                  |  Tauri main thread (V8 루프)      |
                  +-----------------------------------+
                                    |
                                    | Tauri Command 호출 (IPC)
                                    v
                  +-----------------------------------+
                  |  Tokio 다중 스레드 Threadpool      |
                  +-----------------------------------+
                      /             |             \
      Spawn (JoinSet)              Spawn         Spawn
                    v               v             v
             [S3 병렬 전송]    [MD5 해시 계산]   [CDN API Purge]
```

### 1.1 동시성 상태 및 스레드 안정성
Tauri는 러스트의 관리형 상태(`tauri::State<'_, T>`) 시스템을 통해 전역 관리 구조체를 스레드 안전하게 주입합니다.
- **`ProfileStore`**: `tokio::sync::RwLock<Vec<ProfileConfig>>`로 구현되어 병렬 S3 작업 시에는 리드 락(`read()`)을, 프로필 생성/삭제 시에는 라이트 락(`write()`)을 획득하여 공유 데이터 레이스를 방지합니다.
- **`AdapterCache`**: 중복 S3 커넥션을 최소화하기 위해 S3 클라이언트 인스턴스 캐시를 원자적 뮤텍스 구조로 공유하며, TCP/TLS 풀을 효과적으로 재사용합니다.
- **`TransferControl`**: 업로드 일시 중단 및 취소 이벤트를 수신하여 전송 중인 청크 스트림을 원자적 플래그(AtomicBool)로 즉각 차단합니다.

### 1.2 `tokio::task::JoinSet`을 통한 병렬 스마트 싱크
- **해시 계산**: CPU 사용량이 높은 파일 MD5 체킹은 독립된 워커 스레드로 분할 처리됩니다.
- **S3 I/O 스케줄링**: 대량 파일 업로드 시 네트워크 대역폭 고갈을 방지하기 위해 `tokio::sync::Semaphore`를 활용하여 동시 전송 개수를 최대 8개로 엄격하게 제어합니다.
- **JoinSet 관리**: 비동기로 분할 동작하는 각 파일 전송 태스크를 JoinSet에 수집하고 완료 및 에러 상태를 프론트엔드 전송 큐로 브로드캐스트합니다.

---

## 2. 보안 자격증명 레이아웃 (Keyring 연동 구조)

NexusPurge는 보안 안전성 확보를 위해 일반 메타데이터와 중요 비밀키(Secret)를 분리 관리하는 하이브리드 구조를 사용합니다.

### 2.1 물리 저장 매체 분리
- **일반 설정 정보**: 로컬 시스템 폴더 아래에 `profiles.json` 텍스트 형태로 저장됩니다. (버킷명, 리전, CDN 배포 ID, 도메인, 활성화 여부 등)
- **보안 암호 키**: 운영체제 네이티브 자격증명 보관함에 `keyring` 러스트 라이브러리를 통해 암호화 저장됩니다.
  - **Windows**: Windows 자격 증명 관리자 (Credential Manager)
  - **macOS**: 키체인 (Keychain Access)
  - **Linux**: Secret Service API / libsecret

### 2.2 키 보관 매핑 정책 (Mapping Scheme)
보안 자격증명은 프로필별 고유 UUID를 기준으로 생성되며, 운영체제 보관함에 아래와 같이 매핑되어 격리 보관됩니다.

| 대상 비밀키 정보 | Keyring 서비스명 | 자격증명 계정 키 (Account Key) |
| :--- | :--- | :--- |
| S3 Secret Access Key | `cdn-upload-tool` | `{profile_id}` |
| Akamai Client Secret | `cdn-upload-tool` | `{profile_id}_akamai` |
| LG U+ CDN Password | `cdn-upload-tool` | `{profile_id}_lguplus` |
| KT CDN Password | `cdn-upload-tool` | `{profile_id}_kt` |
| 효성 ITX API Secret | `cdn-upload-tool` | `{profile_id}_hyosung` |

---

## 3. 스마트 동기화(Smart Sync) 판정 로직

원격 버킷의 ETag와 로컬 파일의 MD5 해시를 비교하여 전송 대상을 선별합니다.

### 3.1 동기화 판정 흐름
```
            [동기화 대상 파일 분석 시작]
                         |
               로컬 파일 크기 측정
                         |
               로컬 파일 MD5 해시 계산
                         |
              S3 Object Head 요청 메타데이터 조회
                         |
           /---------------------------------\
          /  S3 ETag가 로컬 MD5와 일치하는가?  \
          \                                   /
           \---------------------------------/
                    /                 \
                 (일치)             (불일치)
                  /                     \
         [toSkip 그룹 분류]       [toOverwrite 그룹 분류]
         - S3 업로드 스킵         - S3 덮어쓰기 업로드 실행
         - CDN Purge 대상 아님    - 업로드 성공 후 CDN Purge 등록
```

### 3.2 멀티파트 ETag 폴백 처리
10MB 이상의 대용량 파일은 S3 업로드 시 ETag 형식 뒤에 파트 개수(예: `-2`)가 따라붙어 직접 해시 대조가 불가능합니다.
- **폴백 로직**: ETag에 하이픈 `-`이 포함된 경우, `multipartEtagFallback` 옵션이 켜져 있으면 강제로 파일 크기 및 최신 수정 시간 교차 비교로 전환하여 중복 업로드를 방지하고 네트워크 리소스를 보존합니다.

---

## 4. 멀티 CDN Purge 어댑터 아키텍처

캐시 무효화 모듈은 `CdnCredentials` 열거형 형식을 매커니즘 삼아 각 CDN의 네트워크 헤더 및 통신 패킷 구조를 캡슐화합니다.

### 4.1 KT & LG U+ CDN (Solbox 3.0 v3 API 연동)
동일한 솔박스 OpenAPI v3 인터페이스 엔진을 탑재하였으나 인증 주소와 Purge 엔드포인트 도메인은 별도 격리 운영됩니다.
- **KT API Base**: `https://api.ktcdn.co.kr` (기본값)
- **LG U+ API Base**: `https://api.lgucdn.com` (기본값)

#### 인증 및 토큰 교환 흐름
- **토큰 엔드포인트**: `POST /v3/auth/tokens`
- **전송 본문**: `{"username": "...", "password": "...", "expiresIn": "1h"}`
- **응답 반환**: `{"token": "<JWT_Bearer_Token>"}`

#### 비동기 Purge 요청
- **Purge 엔드포인트**: `POST /v3/management/service/{serviceName}/volume/{volumeName}/purge`
- **인증 헤더**: `Authorization: Bearer <JWT_Bearer_Token>`
- **전송 본문**: `{"paths": ["/index.html", "/assets/app.js"]}`
- **반환 데이터**: `{"transactionId": "tx_solbox_xxxx"}`

#### 트랜잭션 진행 상황 갱신
- **상태 엔드포인트**: `GET /v3/management/transaction/{transactionId}`
- **동작 방식**: 반환된 트랜잭션 ID를 큐에 등록하고, 백엔드에서 지속적으로 조회를 날려 비동기 완료 유무를 실시간 체킹합니다.

---

### 4.2 효성 ITX CDN (X-ITX API 연동)
JWT 세션 토큰 대신 고정된 시크릿 키 쌍을 매 요청 헤더에 직접 전송하는 인증 체계를 따릅니다.

#### 필수 연동 헤더
- `X-ITX-Security-Principal`: 효성 API Key
- `X-ITX-Security-Secret`: 효성 API Secret (Keyring 관리)
- `Content-Type`: application/json

#### 엔드포인트 및 응답 데이터 이중 파싱
- **일괄 Purge**: `POST /api/v1/purge/{serviceId}`
  - **전송 본문**: `{"filelist": ["https://cdn.domain.com/index.html", ...]}`
  - **응답 Envelope**: `{"meta": { "statusCode": 200, "status": "ok", "transactionId": "..." }, "data": "{\"successCount\":1,...}"}`
  - *특이사항: 효성 API의 `data` 노드는 단순 JSON 객체가 아닌 JSON 문자열을 포함하므로, 백엔드 Rust에서 2차 Deserialize를 수행해 `successCount`를 검출합니다.*

---

### 4.3 S3-CDN 매핑 경로 보정 (Stripping Logic)
S3 상의 특정 루트 디렉토리가 CDN 최상위 도메인과 다이렉트로 결합되어 있을 때(예: S3의 `contents/app.js` -> CDN의 `/app.js`), S3 키 경로의 앞 접두사를 자르는 보정 함수가 작동합니다.
```rust
fn normalize_path(path: &str, cdn_base_path: Option<&str>) -> String {
    if let Some(base) = cdn_base_path.filter(|b| !b.trim().is_empty()) {
        let base_stripped = base.trim_start_matches('/').trim_end_matches('/');
        let prefix = format!("{}/", base_stripped);
        let key_stripped = path.trim_start_matches('/');
        if key_stripped.starts_with(&prefix) {
            format!("/{}", &key_stripped[prefix.len()..])
        } else {
            format!("/{}", key_stripped)
        }
    } else {
        let key_stripped = path.trim_start_matches('/');
        format!("/{}", key_stripped)
    }
}
```
본 로직은 사용자가 S3 탭에서 수동 파일 Purge를 실행할 때뿐만 아니라, 업로드 완료 시 발생하는 백엔드 자동 Purge 루틴에서도 동일하게 가동되어 캐시가 어긋나는 오동작을 원천 방지합니다.
