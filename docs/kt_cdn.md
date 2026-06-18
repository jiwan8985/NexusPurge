# KT CDN3.0 OpenAPI v3 전체 기술 명세

## 1. 개요 (Introduction)
* **목적**: 외부 시스템과 Solbox CDN의 연동을 위한 규약으로, CDN 오브젝트 생성, 수정, 삭제 및 통계 조회 기능을 제공.
* **대상**: CDN 서비스를 개발하는 개발자 (HTTP, RESTful, 네트워크, 보안 기본 지식 필요)

## 2. 개요 및 기술 제약 (Overview & Limitation)
* **통신**: HTTPS 기반 통신만 응답 (HTTP 요청 시 301 리디렉션)
* **데이터 포맷**: 모든 응답은 JSON 형식
* **프로토콜**: HTTP 1.1 지원
* **문자셋**: 
    * URI: ASCII만 지원
    * 기타: 특별한 언급이 없는 경우 UTF-8 기본
* **버전 정책**: 업데이트 시 이전 버전은 6개월간 유지 후 폐기
* **서비스명(serviceName)**: 최대 30자의 영어 소문자와 숫자 조합만 사용 가능

## 3. 공통 헤더 및 인증 (Authentication)
* **공통 헤더**: 모든 요청(토큰 생성 API 제외)에 `Authorization: Bearer <Token>` 포함 필수
* **토큰 생성**: `POST /v3/auth/tokens`
    * **필수 파라미터**: `username`, `password`
    * **선택 파라미터**: `expiresIn` (입력 가이드: [ms format](https://github.com/zeit/ms#ms))
        * 가능한 값: 1y, 5d, 2h, 1m
        * 기본값: 1 days / 최대값: 1 years

## 4. 관리 API (Management)
서비스 또는 볼륨 단위의 CDN 제어 API.

| Operation | Method | Endpoint |
| :--- | :--- | :--- |
| Status of Transaction | GET | `/v3/management/transaction/{transactionId}` |
| Purge by Service | POST | `/v3/management/service/{serviceName}/purge` |
| Purge by Volume | POST | `/v3/management/service/{serviceName}/volume/{volumeName}/purge` |
| Purge by Domain | POST | `/v3/management/service/{serviceName}/domain/{domain}/purge` |
| Preload by Service | POST | `/v3/management/service/{serviceName}/preload` |
| Preload by Volume | POST | `/v3/management/service/{serviceName}/volume/{volumeName}/preload` |

* **Transaction 처리**: 비동기 작업 시 트랜잭션 ID만 응답하며, 이를 통해 진행 상황 조회 가능.

## 5. 통계 API (Statistics) 상세
### 5.1 시간 입력 규격
* **ISO 8601 준수**: 한국 시간(+09:00) 기준(생략 가능).
* **URL 인코딩**: '+' 기호는 반드시 `%2B`로 인코딩.
* **Interval/데이터 제한 규칙**:
    1. `(now-190일) <= start < end <= (now-20분)`
    2. 조회 구간:
        * 5분 간격: 최대 24시간
        * 1시간 간격: 최대 31일
        * 1일 간격: 최대 190일
    3. 1분 간격은 Network 통계에서만 제공하며 데이터 지연으로 인해 오차 발생 가능.

### 5.2 통계 항목 정의
| 통계 구분 | 주요 항목 | 상세 정의 |
| :--- | :--- | :--- |
| **Network** | outBytes, outBps, activeConnections | 전송량, 트래픽, 동시 접속자 수 |
| **OriginNetwork** | inBytes, inBps | 원본 유입량 및 트래픽 |
| **Response** | responses | HTTP 상태 코드별 접속자 수 |
| **Contents** | path, hit, downHit, outBps, outBytes, rank | 파일별 상세 성능 및 순위 |
| **Directory** | path, hit, downHit, outBps, outBytes, rank | 디렉토리별 성능 및 순위 |
| **Xcache** | HIT, MISS, TRAFFIC_OUT | 캐시 적중률 및 효율 분석 |
| **Visitor** | os, ispName, tnohit, rank | OS/ISP별 사용자 통계 |

> **참고**: Contents 통계에서 `sort` 파라미터 미입력 시 1일 단위, hit 기준으로 rank 산출.