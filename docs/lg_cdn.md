# LG U+ CDN 3.0 OpenAPI v3 공식 명세서

## 1. 개요 (Introduction)
Solbox CDN 3.0 API v3는 외부 시스템과 Solbox CDN의 연동을 위한 표준 API 규약입니다. 
- **목적:** CDN 오브젝트의 생성, 수정, 삭제 및 관리.
- **대상:** HTTP, RESTful API, 서버 및 네트워크, 보안에 대한 이해도가 있는 개발자.
- **테스트:** [Run in Postman](https://v3-api-docs.lgucdn.com) 버튼을 통해 Postman 환경에서 직접 테스트 가능.

## 2. 기술적 제약 및 규칙 (Limitation & Overview)
- **통신:** HTTPS를 통한 보안 통신 필수 (HTTP 요청 시 301 리디렉션).
- **응답 형식:** 모든 응답은 JSON 형식으로 전송.
- **프로토콜:** HTTP 1.1 지원.
- **인코딩:** - URI: ASCII Character Set만 지원.
    - 그 외: 특별한 언급이 없으면 UTF-8 기본 사용.
- **버전 정책:** 상위 버전 출시 시 이전 버전은 6개월간 유지 후 폐기.
- **서비스 명명:** `serviceName`은 영문 소문자와 숫자 조합으로 최대 30글자 제한.

## 3. 인증 및 헤더 (Authentication)
모든 API는 HTTPS 기반이며 토큰 인증을 사용합니다.
- **Base URL:** [https://api.lgucdn.com](https://api.lgucdn.com)
- **공통 헤더:**
    - `Authorization`: `Bearer {token}` (토큰 생성 API 제외 필수)

### [POST] 토큰 생성
- **Endpoint:** `https://api.lgucdn.com/v3/auth/tokens`
- **Body Parameter:**
    - `username` (String, 필수): 사용자 ID
    - `password` (String, 필수): 사용자 비밀번호
    - `expiresIn` (String, 선택): 유효 기간 (1y, 5d, 2h, 1m 등)
        - 기본값: 1 days
        - 최대값: 1 years

## 4. 관리 API (Management)
서비스 또는 볼륨 단위로 CDN 동작을 제어합니다. (모든 관리 API는 Bearer Token 인증 필요)

| Operation | Method | Endpoint |
| :--- | :--- | :--- |
| **Status of Transaction** | GET | `/v3/management/transaction/{transactionId}` |
| **Purge by Service** | POST | `/v3/management/service/{serviceName}/purge` |
| **Purge by Volume** | POST | `/v3/management/service/{serviceName}/volume/{volumeName}/purge` |
| **Purge by Domain** | POST | `/v3/management/service/{serviceName}/domain/{domain}/purge` |
| **Preload by Service** | POST | `/v3/management/service/{serviceName}/preload` |
| **Preload by Volume** | POST | `/v3/management/service/{serviceName}/volume/{volumeName}/preload` |

## 5. 트랜잭션 (Transaction)
오래 걸리는 작업은 비동기로 처리됩니다.
- **작동 방식:** 작업을 요청하면 트랜잭션 ID만 응답하고 연결이 종료됩니다.
- **조회:** 반환된 트랜잭션 ID를 사용하여 `/v3/management/transaction/{transactionId}`를 통해 작업 진행 상황을 조회합니다.

## 6. 통계 및 에러 (Statistics & Error Code)
- **통계 API:** 서비스, 볼륨, 도메인별 통계 데이터 조회 가능.
- **Error Code:** API 상세 오류 정의 섹션 참조.