# NexusPurge: 사용자 매뉴얼 및 문제 해결 가이드 (상세 운영 설명서)

본 매뉴얼은 **NexusPurge**의 구체적인 설정 사양 테이블, 조작 흐름 및 발생할 수 있는 주요 문제 진단 가이드를 제공합니다.

---

## 1. 세부 프로필 설정 참조 정보

프로필을 생성하거나 수정할 때, 사용 중인 스토리지 및 CDN 시스템 구조에 따라 아래 필드 정보를 입력해야 합니다.

### 1.1 S3 스토리지 설정 항목
| 설정 필드명 | 필수 여부 | 상세 설명 및 형식 | 예시 입력값 |
| :--- | :--- | :--- | :--- |
| **프로필 이름** | 필수 | 프로필을 구분하기 위한 임의의 식별 레이블 | `Nexon CDN Dev Profile` |
| **Region** | 필수 | 대상 S3 버킷이 위치한 AWS 리전 코드 | `ap-northeast-2` (서울) |
| **Bucket** | 필수 | 대상 S3 버킷의 공식 명칭 | `sklb-test-test-kr-online-nexoncdn` |
| **Access Key ID** | 필수 | S3 버킷 읽기/쓰기 권한이 부여된 IAM Access Key ID | `AKIAIOSFODNN7EXAMPLE` |
| **Secret Access Key**| 필수 | IAM Secret Key (시스템 Keyring에 안전하게 암호화 보관) | `wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY` |
| **API Endpoint** | 선택 | S3 호환 독자 스토리지(MinIO 등)를 연동할 경우 입력 | `https://s3.ap-northeast-2.amazonaws.com` |

---

### 1.2 CDN 공급자 설정 항목

#### AWS CloudFront
- **Distribution ID**: 필수 (예: `E2M7X1EXAMPLEID`). 캐시 무효화(Invalidation) 요청을 전송할 대상 배포판 식별자.
- **CDN Domain**: 필수 (예: `d2f1611qj8oisv.cloudfront.net`). Purge 완료 상태를 실제 네트워크에서 교차 검증할 때 사용할 도메인 주소.

#### Akamai
- **EdgeGrid 호스트**: 필수 (예: `akab-xxxx.luna.akamaiapis.net`). Akamai API 게이트웨이 주소.
- **Client Token**: 필수 (예: `akab-client-token-xxx`).
- **Access Token**: 필수 (예: `akab-access-token-xxx`).
- **Client Secret**: 필수 (Keyring 안전 저장).
- **CDN 도메인**: 필수 (예: `sklb-test.dn.nexoncdn.co.kr.edgesuite.net`). Purge 대상 URL을 조합하기 위해 필수적으로 사용되는 서비스 FQDN 도메인.

#### KT CDN 및 LG U+ CDN (Solbox v3 기반)
- **Username / Password**: 필수. Solbox API 인증용 사용자 계정 명과 비밀번호(Keyring 저장).
- **Service Name**: 필수. CDN 서비스 관리명 (명세상 영어 소문자 및 숫자의 조합으로 최대 30자 제한).
- **Volume Name**: 선택. 무효화 대상을 특정 스토리지 볼륨 영역으로 좁혀 정의하고자 할 때 기재.
- **CDN 도메인**: 필수 (예: `sklb-test.dn.nexoncdn.co.kr`). 무효화 대상 URL 도메인 주소.
- **API Endpoint**: 선택 (미입력 시 기본 공식 게이트웨이 제공). 
  - KT CDN 기본값: `https://api.ktcdn.co.kr`
  - LG U+ CDN 기본값: `https://api.lgucdn.com`

#### 효성 ITX CDN
- **API Key (Principal)**: 필수. 효성 ITX 캐시 무효화 API 요청 시 전송할 Principal 키.
- **API Secret**: 필수. 효성 ITX Secret Key (Keyring 안전 저장).
- **Service ID (Distribution ID)**: 필수 (예: `TID_18656`). Purge 대상 효성 CDN 서비스 아이디.
- **CDN 도메인**: 필수 (예: `sklb-test.dn.nexoncdn.co.kr`). URL 구성을 위한 전송용 FQDN 도메인.
- **API 엔드포인트**: 선택 (미입력 시 기본 게이트웨이 제공).
  - 효성 ITX CDN 기본값: `https://api.xtrmcdn.co.kr:28091`

---

## 2. CDN Base Path: S3-CDN 경로 라우팅 매핑

S3 버킷 내의 원본 경로 구조와 CDN에서 호스팅하는 도메인 경로 구조가 서로 일치하지 않는 경우, **CDN Base Path** 속성을 활용하여 무효화 대상 URL을 올바르게 매핑할 수 있습니다.

### 설정 시나리오 예시
- **S3 원본 파일 키**: `sklb-test-test-kr-online-nexoncdn/contents/main/index.html`
- **사용자 요청 CDN URL**: `http://sklb-test.dn.nexoncdn.co.kr/main/index.html`
- **현상 분석**: S3 버킷 내의 `/contents/` 폴더 전체가 CDN 도메인의 최상위 루트 경로(`/`)에 직접 일대일 매핑되어 배포 중인 상태입니다.
- **설정 조치 방법**: 해당 프로필의 **CDN Base Path** 항목에 `contents/`를 기재합니다.
- **처리 프로세스**:
  1. 사용자가 `contents/main/index.html` 파일을 업로드하거나 선택합니다.
  2. 전송이 완료된 후, 백엔드에서 CDN API로 Purge를 요청할 때 접두사인 `contents/`를 잘라내어 정규화된 경로인 `/main/index.html`로 변환하여 요청을 보냅니다.
  3. CDN 캐시가 올바르게 갱신되어 배포 버전 불일치 현상이 완벽히 해결됩니다.

---

## 3. 사용자 화면 구성 및 기능 설명

NexusPurge는 이중 패널 탐색 화면을 통해 안전하고 신속한 수동/자동 배포 제어를 지원합니다.

```
+-------------------------------------------------------------------------------+
|  [프로필 선택 및 전환]                          [자동 Purge 토글]  [설정 열기]  |
+-------------------------------------------------------------------------------+
|  로컬 디렉토리 패널 (Local)        |  S3 원격 버킷 패널 (Remote)              |
|  - 로컬 파일 트리 조회             |  - S3 접두사(Prefix) 트리 실시간 렌더링  |
|  - 마우스로 끌어다 놓아 업로드     |  - 우클릭 컨텍스트 단축 메뉴:             |
|                                    |    * URL 복사 (Presigned / CDN URL)      |
|                                    |    * 선택한 파일/디렉토리 CDN Purge      |
+-------------------------------------------------------------------------------+
|  파일 전송 진행 큐 (대기 중 -> 전송 중 -> 전송 완료 및 Purge 추적 상태 표시)   |
+-------------------------------------------------------------------------------+
|  작업 시스템 로그 탭 (종합 로그 / 파일 전송 로그 / CDN 작업 로그 필터 탑재)   |
+-------------------------------------------------------------------------------+
```

### 3.1 CDN 캐시 무효화(Purge) 실행 방법
1. **자동 Purge 활성화**: 상단 툴바의 "자동 Purge" 토글을 켜두면, 스마트 싱크에 의해 파일 덮어쓰기(`toOverwrite`) 및 신규 전송이 감지될 때마다 백엔드가 해당 파일 경로를 자동으로 CDN Purge 대기열에 추가하여 처리합니다.
2. **선택 Purge**: S3 탐색 패널에서 마우스 드래그 또는 키보드 조작으로 다중 선택한 항목들에 한해 상단 `선택 Purge` 버튼을 누르면 즉시 무효화를 수행합니다.
3. **전체 Purge**: 현재 탐색 중인 S3 디렉토리 및 그 하위 구조 전체(예: `/assets/js/*`)에 대해 일괄 무효화를 처리합니다.
4. **직접 Purge**: 툴바의 `직접 Purge` 버튼을 클릭하면 뜨는 팝업 창에 와일드카드 문자나 임의의 경로(예: `/images/banner/*`, `/index.html`)를 줄바꿈 또는 쉼표로 구분하여 직접 다중 입력해 Purge할 수 있습니다.

---

## 4. 문제 진단 및 에러 해결 가이드 (Troubleshooting)

작업 도중 오류가 발생하면 시스템 로그 창의 오류 코드 및 설명을 참고하여 다음과 같이 진단 및 조치하십시오.

### 4.1 "401 Unauthorized" (인증 권한 만료/오류)
- **증상**: 연결 테스트 진행 시 또는 CDN Purge API 호출 시 즉시 401 오류 코드를 응답하며 취소됨.
- **원인**: 
  - AWS IAM 자격증명이 잘못되었거나, CDN 포털 계정 자격정보(아이디/패스워드, Akamai EdgeGrid 인증정보, 효성 API Secret) 입력 오류.
  - OS 보안 정책으로 인해 로컬 Keyring(자격 증명 관리자) 비밀키 읽기 권한이 제한됨.
- **조치 요령**:
  - 프로필 관리 창을 열고, 대상 비밀번호 및 시크릿 키 등을 다시 확인하여 덮어쓰기 저장하십시오. 이를 통해 Keyring 내부 캐시 정보가 올바르게 초기화됩니다.
  - OS 상에서 보안 잠금 해제 알림이 표시된다면, NexusPurge 실행 파일의 로컬 자격증명 보관함 액세스를 승인해 주셔야 합니다.

### 4.2 "400 Bad Request" / "Invalid volumeName/serviceName" (잘못된 API 매개변수)
- **증상**: KT CDN 또는 LG U+ CDN Purge 요청 시 요청이 즉시 거부되고 400 에러 기록됨.
- **원인**: 
  - 입력한 서비스 이름(`serviceName`)에 대문자나 특수문자가 섞여 있거나 30자를 초과한 경우. (솔박스 v3 명세상 반드시 영어 소문자와 숫자 조합이어야 함)
- **조치 요령**:
  - 관리 콘솔에 정의된 CDN 서비스 식별 영문 소문자 명칭을 정확히 파악하여 프로필 내 설정을 변경한 뒤 재시도하십시오.

### 4.3 "Hyosung CDN Service ID가 필요합니다"
- **증상**: 효성 ITX CDN 연결 테스트 또는 Purge 요청 즉시 실패.
- **원인**: 효성 ITX CDN 인터페이스는 배포판 번호인 서비스 ID(예: `TID_18656`)가 필수적입니다.
- **조치 요령**:
  - 프로필 설정 중 `Service ID (Distribution ID)` 필드에 효성 ITX 전용 서비스 코드 값을 입력하고 저장했는지 확인하십시오.

### 4.4 "Multipart ETag Mismatch" (대용량 파일 중복 전송 및 불필요한 Purge 유발)
- **증상**: 10MB 이상의 대용량 파일을 업로드할 때 이미 S3 버킷 내의 파일과 동일함에도 불구하고 매번 변경 파일(`toOverwrite`)로 분류되어 매 요청마다 CDN Purge를 무익하게 유발함.
- **원인**: S3 멀티파트 업로드 특성상 업로드 완료 후 ETag 뒤에 파트 수 접미사(예: `-3`)가 붙어 로컬 파일 단일 MD5 해시값과 다르게 판정됩니다.
- **조치 요령**:
  - 프로필 수정 다이얼로그를 통해 **`multipartEtagFallback`** 설정을 활성화하십시오. 이 옵션을 켜두면, ETag로 직접 해시 비교가 어려운 대용량 파일의 경우 로컬 파일 크기와 마지막 수정 시각 정보를 교차 점검하여 중복 전송을 지능적으로 건너뜁니다.

