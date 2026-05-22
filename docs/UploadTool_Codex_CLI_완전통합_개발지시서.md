# UploadTool Codex CLI 완전 통합 개발 지시서

> 이 문서는 Codex CLI에 그대로 넣어 개발을 진행시키기 위한 **단일 통합 Markdown 파일**이다.  
> 사용자가 정리한 UploadTool 프로젝트 개요, 요구사항, 데이터 모델, 고객사 기술 미팅 질문, TODO, 개발 시 주의사항, 아키텍처, Phase 계획을 모두 반영한다.  
> Codex CLI는 이 문서를 기준으로 현재 로컬 Repository를 분석하고, 기존 구조를 유지하면서 필요한 개발을 수행해야 한다.

---

# 0. Codex CLI 역할

너는 현재 로컬 Repository에서 실행 중인 Codex CLI 개발 에이전트다.

너의 역할은 다음과 같다.

- 현재 Repository 구조 분석
- 기존 S3 / CloudFront / Akamai 구현 확인
- 기존 기능 훼손 없이 UploadTool 요구사항 반영
- LG U+ CDN / 효성 CDN / KT CDN Stub Adapter 추가
- 외부 인증 모듈 연동 준비 구조 추가
- 작업 로그/결과 처리 구조 추가
- 문서 정리
- 가능한 검증 명령 실행
- 작업 결과 보고

절대 전체 프로젝트를 새로 갈아엎지 말고, 기존 구조를 보존하면서 최소 침습 방식으로 수정하라.

---

# 1. 프로젝트 정의

**UploadTool**은 기존 개발사가 개발 및 운영하던 **CDN 업로드/Purge 툴을 대체하기 위한 신규 구현 프로젝트**다.

기존 고객사는 기존 개발사와 함께 **AWS S3 업로드 및 CDN Purge 자동화 툴**을 개발하여 운영해 왔다.  
그러나 기존 개발사와의 계약 만료로 인해 기존 툴을 더 이상 사용할 수 없게 되었고, 이에 따라 신규 구현이 필요하다.

따라서 본 프로젝트의 1차 목표는 완전히 새로운 기능 중심의 CDN 관리 도구를 만드는 것이 아니라, 기존 고객사가 사용하던 **CDN 업로드/Purge 툴의 기능을 동일하거나 유사하게 재현**하는 것이다.

---

# 2. 프로젝트 목적

| 구분 | 내용 |
| --- | --- |
| 기존 기능 재현 | 기존 CDN 업로드/Purge 툴의 기능 재현 |
| 스토리지 관리 | AWS S3 기반 파일 업로드 및 관리 기능 제공 |
| CDN Purge | CDN Purge 자동화 기능 제공 |
| 지원 CDN | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN 연동 구조 제공 |
| URL 매핑 | S3 Key와 CDN URL 매핑 구조 제공 |
| 변경 감지 | 파일 변경 감지 후 필요한 대상만 업로드 및 Purge |
| 인증 연동 | 외부 인증 모듈 연동을 고려한 인증 구조 준비 |
| 결과 확인 | 업로드/Purge 결과 확인 및 작업 로그 저장 구조 제공 |
| 확장성 | 고객사 확인 사항에 따라 CDN별 API, Purge 정책, 권한 정책 확장 가능 구조 제공 |

---

# 3. 프로젝트 배경

| 항목 | 내용 |
| --- | --- |
| 기존 운영 방식 | 고객사와 기존 개발사가 CDN 업로드/Purge 툴을 공동 개발 및 운영 |
| 기존 툴 용도 | 고객사 내부 운영용 AWS S3 업로드 및 CDN Purge 자동화 |
| 변경 사유 | 기존 개발사와의 계약 만료로 기존 툴 지속 사용 불가 |
| 고객사 요청 | 기존 툴과 동일한 기능을 제공하는 신규 툴 개발 |
| 개발 방향 | 신규 제품 기획이 아닌 기존 툴 대체 및 기능 재현 |

---

# 4. 핵심 개발 방향

| 구분 | 방향 |
| --- | --- |
| 개발 목표 | 신규 기능 확장이 아니라 기존 툴 기능 재현 우선 |
| 스토리지 | AWS S3만 1차 지원 |
| CDN | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN 지원 |
| LG U+ / 효성 CDN / KT CDN | API 문서 수령 전까지 Stub Adapter 구조만 구현 |
| 인증/로그인 | 자체 구현하지 않고 별도 인증 모듈 연동 구조 준비 |
| 작업 결과 | 업로드/Purge 결과 및 기본 로그 구조 포함 |
| 고객사 기술 미팅 | 인증 및 로그 상세 항목 제외, S3/CDN 기술 스펙 중심 확인 |
| 실제 개발 범위 | 인증 연동 준비 구조와 작업 로그/결과 처리 구조 포함 |

---

# 5. 확정 범위

| 영역 | 확정 내용 |
| --- | --- |
| 스토리지 | AWS S3 |
| CDN | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN |
| 인증 | 외부 인증 모듈 연동 |
| 로그/결과 처리 | 기본 구조 포함 |

---

# 6. 제외 범위

| 제외 항목 | 비고 |
| --- | --- |
| Tencent COS 1차 지원 | 1차 범위 제외 |
| S3-compatible storage 1차 지원 | 1차 범위 제외 |
| NexusPurge 자체 계정/비밀번호 로그인 구현 | 외부 인증 모듈 연동 예정 |
| 자체 사용자 DB 구현 | 외부 인증 모듈 연동 예정 |
| LG U+ CDN 실제 API 추측 구현 | API 문서 수령 후 구현 |
| 효성 CDN 실제 API 추측 구현 | API 문서 수령 후 구현 |
| KT CDN 실제 API 추측 구현 | API 문서 수령 후 구현 |
| 고객사 확인 전 Query String Purge 구현 | 고객사 확인 후 반영 |
| 고객사 확인 전 와일드카드 Purge 구현 | 고객사 확인 후 반영 |
| 고객사 확인 전 삭제 파일 Purge 확정 구현 | 고객사 확인 후 반영 |
| 고객사 확인 전 CDN별 Prefix 차이 확정 구현 | 고객사 확인 후 반영 |
| 고도화된 감사 로그/관리자 리포트 | 후속 고도화 |
| CSV Export 정식 구현 | 후속 고도화 |

---

# 7. 개발 절대 원칙

Codex는 아래 원칙을 반드시 지켜야 한다.

## 7.1 범위 관련 주의사항

| 구분 | 주의사항 |
| --- | --- |
| 스토리지 범위 | Tencent COS 지원을 1차 범위처럼 문서화하지 않음 |
| 스토리지 범위 | S3-compatible storage를 1차 핵심 범위처럼 강조하지 않음 |
| 인증 | 인증/로그인을 NexusPurge 자체 계정 DB 방식으로 구현하지 않음 |
| 인증 | 자체 로그인 화면을 새로 만들지 않음 |
| CDN | LG U+ CDN 실제 API 호출을 추측으로 구현하지 않음 |
| CDN | 효성 CDN 실제 API 호출을 추측으로 구현하지 않음 |
| CDN | KT CDN 실제 API 호출을 추측으로 구현하지 않음 |
| 기존 기능 | CloudFront 기존 구현을 깨지 않음 |
| 기존 기능 | Akamai 기존 구현을 삭제하지 않음 |
| 타입/설정 | 기존 타입/설정 필드를 임의로 제거하지 않음 |
| 고객사 확인 | 고객사 확인이 필요한 항목을 확정된 것처럼 실제 구현하지 않음 |
| 미팅 문서 | 미팅 질문 리스트에 인증/로그 상세 항목을 다시 포함하지 않음 |
| 실제 개발 | 실제 개발 구조에는 인증 연동 준비와 로그/결과 처리를 포함함 |

## 7.2 CDN 관련 주의사항

| CDN | 처리 기준 |
| --- | --- |
| CloudFront | 기존 구현 유지 |
| Akamai | 기존 구현 유지, URL Purge / CP Code Purge 방식은 고객사 확인 후 확정 |
| LG U+ CDN | API 문서 수령 전까지 Stub Adapter까지만 구현 |
| 효성 CDN | API 문서 수령 전까지 Stub Adapter까지만 구현 |
| KT CDN | API 문서 수령 전까지 Stub Adapter까지만 구현 |

## 7.3 CDN 구현 시 금지 사항

| 항목 | 설명 |
| --- | --- |
| API 추측 구현 금지 | 고객사 문서 없이 Endpoint, Payload, 인증 방식을 임의 구현하지 않음 |
| 인증 방식 추측 금지 | API Key, Token, Signature 방식 등을 임의 가정하지 않음 |
| Rate Limit 추측 금지 | CDN별 호출 제한을 임의로 설정하지 않음 |
| Purge 방식 확정 금지 | URL Purge, Path Purge, CP Code Purge, Wildcard Purge 여부를 문서 없이 확정하지 않음 |
| 기존 구현 훼손 금지 | CloudFront/Akamai 기존 동작을 신규 Provider 추가 과정에서 깨지 않음 |

---

# 8. Stub Adapter 처리 기준

LG U+ CDN, 효성 CDN, KT CDN은 실제 API 문서 수령 전까지 **NotImplemented 에러를 반환하는 Stub Adapter**로만 구현한다.

| CDN | NotImplemented 메시지 |
| --- | --- |
| LG U+ CDN | `LG U+ CDN purge API is not implemented yet. API specification is required.` |
| 효성 CDN | `Hyosung CDN purge API is not implemented yet. API specification is required.` |
| KT CDN | `KT CDN purge API is not implemented yet. API specification is required.` |

Stub Adapter는 다음 조건을 만족해야 한다.

- Provider 타입에 포함된다.
- Profile UI에서 선택 가능하다.
- Profile UI에서 Credential 입력 가능하다.
- Rust Credential 구조가 존재한다.
- Credential 조회 분기가 존재한다.
- CDN Purge Dispatch 분기가 존재한다.
- 실제 HTTP API 호출은 하지 않는다.
- 명확한 NotImplemented 에러를 반환한다.
- 작업 로그에 실패 또는 NotImplemented 결과로 저장 가능해야 한다.
- 고객사 API 문서 수령 후 실제 구현 가능하도록 구조를 분리한다.

---

# 9. AWS S3 기능 요구사항

| 구분 | 요구사항 |
| --- | --- |
| Profile 선택 | S3 Bucket/Profile 선택 기능 제공 |
| Prefix 탐색 | Prefix 기반 파일 탐색 지원 |
| 로컬 탐색 | 로컬 파일 탐색 지원 |
| S3 목록 조회 | S3 파일 목록 조회 기능 제공 |
| 업로드 | 로컬 파일을 S3로 업로드 |
| 다운로드 | S3 파일을 로컬로 다운로드 |
| 삭제 | S3 파일 삭제 |
| 폴더 생성 | S3 Prefix 기반 폴더 생성 |
| 이름 변경 | S3 Object 이름 변경 |
| Presigned URL | Presigned URL 생성 여부 확인 후 반영 |
| Content-Type | 업로드 시 Content-Type 자동 설정 |
| Cache-Control | 업로드 시 Cache-Control 자동 설정 |
| 동일 파일 판단 | MD5/ETag 기반 동일 파일 판단 |
| 변경 감지 | 로컬/S3 파일 변경 여부 감지 |
| 파일 분류 | 신규/변경/동일 파일 분류 |
| 사전 확인 | 업로드 전 대상 미리보기 제공 |

---

# 10. 동일 파일 판단 기준

| 항목 | 기준 |
| --- | --- |
| 기본 판단 방식 | MD5/ETag 비교 |
| 적용 목적 | 동일 파일 재업로드 방지 |
| 예외 케이스 | Multipart Upload의 ETag는 일반 MD5와 다를 수 있음 |
| 대응 방향 | 고객사 파일 크기와 업로드 방식 확인 후 fallback 정책 적용 가능 구조 유지 |

Codex는 Multipart ETag 문제를 고려하되, 고객사 확인 전 fallback 정책을 확정 구현하지 말고 구조와 TODO로 남겨야 한다.

---

# 11. CDN Purge 기능 요구사항

| 구분 | 요구사항 |
| --- | --- |
| 연계 방식 | S3 업로드와 CDN Purge 연계 |
| 기본 Purge 조건 | 기존 파일 덮어쓰기 시 Purge |
| 신규 업로드 Purge | 옵션으로 유지 |
| 삭제 파일 Purge | 고객사 확인 후 반영 |
| 와일드카드 Purge | CDN별 지원 여부 확인 후 반영 |
| Query String Purge | 고객사 확인 후 반영 |
| Purge Preview | Purge 실행 전 대상 URL 목록 확인 구조 제공 |
| 완료 상태 확인 | Purge 완료 상태 Polling 여부는 고객사 확인 후 반영 |

---

# 12. 지원 CDN

| CDN | 처리 방향 |
| --- | --- |
| CloudFront | Distribution ID + Object Path 기준 Invalidation 수행 |
| Akamai | 고객사 확인 전까지 URL Purge 기준 유지, CP Code Purge 가능성은 TODO 처리 |
| LG U+ CDN | API 문서 수령 전까지 Stub Adapter 구성 |
| 효성 CDN | API 문서 수령 전까지 Stub Adapter 구성 |
| KT CDN | API 문서 수령 전까지 Stub Adapter 구성 |

---

# 13. S3 Key와 CDN URL 매핑 요구사항

S3 Key와 CDN URL 매핑은 UploadTool의 핵심 로직 중 하나다.

| 항목 | 내용 |
| --- | --- |
| S3 Key 예시 | `prod/assets/app.js` |
| CDN URL 예시 | `https://cdn.example.com/prod/assets/app.js` |
| 처리 방식 | 공통 함수로 S3 Key 또는 Object Path를 CDN URL로 변환 |
| 기본 규칙 | CDN Domain의 trailing slash 제거 |
| 기본 규칙 | Object Path의 leading slash 제거 |
| 결과 형태 | `domain/path` 형태로 조합 |
| CDN별 Prefix 차이 | 고객사 확인 후 반영 |
| Query String 포함 여부 | 고객사 확인 후 반영 |

CloudFront는 CDN URL이 아니라 Object Path 기준으로 Invalidation한다.

```text
S3 Key:
prod/assets/app.js

CloudFront Path:
/prod/assets/app.js
```

Akamai / LG U+ / 효성 / KT는 URL Purge 기반 구조로 준비하되, LG U+ / 효성 / KT 실제 API 호출은 Stub 처리한다.

---

# 14. Profile 관리 요구사항

Profile은 S3 및 CDN 설정을 포함한다.

| 구분 | 필드 |
| --- | --- |
| 기본 정보 | Profile ID |
| 기본 정보 | Profile Name |
| AWS 설정 | AWS Region |
| AWS 설정 | AWS S3 Bucket |
| AWS 설정 | Base Prefix |
| AWS 인증 | AWS Access Key ID |
| AWS 인증 | AWS Secret Access Key |
| CDN 설정 | CDN Provider |
| CDN 설정 | CDN Domain |
| CloudFront | CloudFront Distribution ID |
| Akamai | Akamai Client Token |
| Akamai | Akamai Client Secret |
| Akamai | Akamai Access Token |
| Akamai | Akamai Host |
| LG U+ CDN | LG U+ CDN API Key |
| LG U+ CDN | LG U+ CDN API Secret |
| LG U+ CDN | LG U+ CDN Endpoint |
| 효성 CDN | 효성 CDN API Key |
| 효성 CDN | 효성 CDN API Secret |
| 효성 CDN | 효성 CDN Endpoint |
| KT CDN | KT CDN API Key |
| KT CDN | KT CDN API Secret |
| KT CDN | KT CDN Endpoint |
| 정책 | Purge on New Upload 여부 |
| 정책 | Default Cache-Control |
| 정책 | Content-Type Override |

Secret 정보는 평문 저장을 피하고, 기존 프로젝트의 보안 저장 방식 또는 OS Keyring 방식을 따른다.

---

# 15. TypeScript 타입 기준

기존 타입 구조가 있다면 기존 구조를 유지하면서 필요한 필드를 추가하라.

```ts
export type CdnProvider =
  | "cloudfront"
  | "akamai"
  | "lguplus"
  | "hyosung"
  | "kt";

export interface S3Profile {
  id: string;
  name: string;

  region: string;
  bucket: string;
  basePrefix?: string;

  accessKeyId: string;
  secretAccessKey: string;

  cdnProvider: CdnProvider;
  cdnDomain?: string;

  cdnDistributionId?: string;

  akamaiClientToken?: string;
  akamaiClientSecret?: string;
  akamaiAccessToken?: string;
  akamaiHost?: string;

  lguplusApiKey?: string;
  lguplusApiSecret?: string;
  lguplusEndpoint?: string;

  hyosungApiKey?: string;
  hyosungApiSecret?: string;
  hyosungEndpoint?: string;

  ktApiKey?: string;
  ktApiSecret?: string;
  ktEndpoint?: string;

  purgeOnNewUpload?: boolean;
  defaultCacheControl?: string;
  contentTypeOverride?: string;
}
```

---

# 16. 데이터 모델

## 16.1 CdnProvider

| 값 | 설명 |
| --- | --- |
| `cloudfront` | AWS CloudFront |
| `akamai` | Akamai CDN |
| `lguplus` | LG U+ CDN |
| `hyosung` | 효성 CDN |
| `kt` | KT CDN |

## 16.2 AuthUser

| 필드 | 설명 |
| --- | --- |
| `id` | 사용자 고유 ID |
| `name` | 사용자 이름 |
| `email` | 사용자 이메일 |
| `organization` | 사용자 조직 또는 부서 |
| `roles` | 사용자 Role 목록 |

```ts
export interface AuthUser {
  id: string;
  name: string;
  email: string;
  organization?: string;
  roles: string[];
}
```

## 16.3 AuthSession

| 필드 | 설명 |
| --- | --- |
| `user` | 인증된 사용자 정보 |
| `accessToken` | Access Token |
| `refreshToken` | Refresh Token |
| `expiresAt` | Access Token 만료 시각 |

```ts
export interface AuthSession {
  user: AuthUser;
  accessToken: string;
  refreshToken?: string;
  expiresAt?: string;
}
```

## 16.4 AuthAdapter

| 메서드 | 설명 |
| --- | --- |
| `login` | 외부 인증 모듈을 통한 로그인 |
| `logout` | 로그아웃 처리 |
| `refreshToken` | Access Token 갱신 |
| `getCurrentSession` | 현재 인증 세션 조회 |

```ts
export interface AuthAdapter {
  login(): Promise<AuthSession>;
  logout(): Promise<void>;
  refreshToken(refreshToken: string): Promise<AuthSession>;
  getCurrentSession(): Promise<AuthSession | null>;
}
```

## 16.5 OperationType

| 값 | 설명 |
| --- | --- |
| `upload` | 파일 업로드 |
| `download` | 파일 다운로드 |
| `delete` | 파일 삭제 |
| `mkdir` | 폴더 생성 |
| `rename` | 이름 변경 |
| `purge` | CDN Purge |
| `sync` | 동기화 작업 |

```ts
export type OperationType =
  | "upload"
  | "download"
  | "delete"
  | "mkdir"
  | "rename"
  | "purge"
  | "sync";
```

## 16.6 OperationStatus

| 값 | 설명 |
| --- | --- |
| `pending` | 대기 중 |
| `running` | 실행 중 |
| `success` | 성공 |
| `failed` | 실패 |
| `partial` | 일부 성공 / 일부 실패 |

```ts
export type OperationStatus =
  | "pending"
  | "running"
  | "success"
  | "failed"
  | "partial";
```

## 16.7 FileOperationResult

| 필드 | 설명 |
| --- | --- |
| `path` | 대상 파일 경로 |
| `operation` | 작업 유형 |
| `status` | 작업 상태 |
| `message` | 처리 메시지 |
| `error` | 실패 사유 |
| `startedAt` | 작업 시작 시각 |
| `finishedAt` | 작업 종료 시각 |

```ts
export interface FileOperationResult {
  path: string;
  operation: OperationType;
  status: OperationStatus;
  message?: string;
  error?: string;
  startedAt?: string;
  finishedAt?: string;
}
```

## 16.8 CdnPurgeResult

| 필드 | 설명 |
| --- | --- |
| `provider` | CDN Provider |
| `urls` | Purge 대상 URL 목록 |
| `status` | Purge 상태 |
| `requestId` | CDN API Request ID |
| `taskId` | CDN Purge Task ID |
| `error` | 실패 사유 |
| `startedAt` | Purge 시작 시각 |
| `finishedAt` | Purge 종료 시각 |

```ts
export interface CdnPurgeResult {
  provider: CdnProvider;
  urls: string[];
  status: OperationStatus;
  requestId?: string;
  taskId?: string;
  error?: string;
  startedAt?: string;
  finishedAt?: string;
}
```

## 16.9 OperationLog

| 필드 | 설명 |
| --- | --- |
| `id` | 작업 로그 ID |
| `profileId` | 사용한 Profile ID |
| `operation` | 대표 작업 유형 |
| `status` | 전체 작업 상태 |
| `bucket` | 대상 S3 Bucket |
| `prefix` | 대상 S3 Prefix |
| `files` | 파일별 작업 결과 목록 |
| `purgeResults` | CDN별 Purge 결과 목록 |
| `startedAt` | 작업 시작 시각 |
| `finishedAt` | 작업 종료 시각 |

```ts
export interface OperationLog {
  id: string;
  profileId: string;
  operation: OperationType;
  status: OperationStatus;
  bucket: string;
  prefix?: string;
  files: FileOperationResult[];
  purgeResults: CdnPurgeResult[];
  startedAt: string;
  finishedAt?: string;
}
```

---

# 17. Backend Credential 구조

Rust Backend에서 CDN API 호출 시 사용하는 Credential 모델이다.

```rust
pub enum CdnCredentials {
    CloudFront(CloudFrontCredentials),
    Akamai(AkamaiCredentials),
    Lguplus(LguplusCredentials),
    Hyosung(HyosungCredentials),
    Kt(KtCredentials),
}
```

## 17.1 LG U+ CDN Credential

| 필드 | 설명 |
| --- | --- |
| `api_key` | LG U+ CDN API Key |
| `api_secret` | LG U+ CDN API Secret |
| `endpoint` | LG U+ CDN API Endpoint |
| `cdn_domain` | LG U+ CDN 도메인 |

```rust
pub struct LguplusCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub endpoint: String,
    pub cdn_domain: String,
}
```

## 17.2 효성 CDN Credential

| 필드 | 설명 |
| --- | --- |
| `api_key` | 효성 CDN API Key |
| `api_secret` | 효성 CDN API Secret |
| `endpoint` | 효성 CDN API Endpoint |
| `cdn_domain` | 효성 CDN 도메인 |

```rust
pub struct HyosungCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub endpoint: String,
    pub cdn_domain: String,
}
```

## 17.3 KT CDN Credential

| 필드 | 설명 |
| --- | --- |
| `api_key` | KT CDN API Key |
| `api_secret` | KT CDN API Secret |
| `endpoint` | KT CDN API Endpoint |
| `cdn_domain` | KT CDN 도메인 |

```rust
pub struct KtCredentials {
    pub api_key: String,
    pub api_secret: String,
    pub endpoint: String,
    pub cdn_domain: String,
}
```

---

# 18. 인증 연동 요구사항

| 구분 | 내용 |
| --- | --- |
| 미팅 범위 | 고객사 기술 미팅에서는 인증 상세 제외 |
| 개발 범위 | 실제 개발에는 외부 인증 모듈 연동 준비 구조 포함 |
| 구현 방식 | NexusPurge 자체 계정/비밀번호 방식으로 구현하지 않음 |
| 연동 방식 | 다른 툴 개발업체가 제공하는 인증 모듈 연동 |
| 실제 API 호출 | 별도 인증 업체 문서 수령 후 구현 |

## 18.1 준비할 구조

| 항목 | 내용 |
| --- | --- |
| AuthAdapter | 외부 인증 모듈 연동 Interface |
| ExternalAuthAdapter | 외부 인증 모듈 Stub |
| AuthUser | 사용자 정보 타입 |
| AuthSession | 인증 세션 타입 |
| Access Token | Access Token 수신 구조 |
| Refresh Token | Refresh Token 수신 구조 |
| Token 만료 | Token 만료 처리 Hook |
| Token 갱신 | Token 갱신 Hook |
| Logout | Logout Hook |
| Role | 사용자 Role 구조 |
| 권한 제어 | Role 기반 기능 제한 구조 |
| Profile 권한 | 사용자 Role과 S3/CDN Profile 접근 권한 매핑 구조 |

---

# 19. 로그 및 결과 처리 요구사항

| 구분 | 내용 |
| --- | --- |
| 미팅 범위 | 고객사 기술 미팅에서는 로그/결과 상세 질문 제외 |
| 개발 범위 | 실제 개발에는 기본 로그/결과 처리 구조 포함 |
| 고도화 항목 | CSV Export, 감사 로그, 관리자 리포트는 후속 고도화 |

## 19.1 기본 로그/결과 처리 범위

| 항목 | 내용 |
| --- | --- |
| 업로드 결과 | 업로드 작업 결과 표시 |
| 다운로드 결과 | 다운로드 작업 결과 표시 |
| 삭제 결과 | 삭제 작업 결과 표시 |
| 폴더 생성 결과 | 폴더 생성 결과 표시 |
| 이름 변경 결과 | 이름 변경 결과 표시 |
| Purge 결과 | Purge 결과 표시 |
| 파일별 상태 | 파일별 성공/실패 상태 표시 |
| CDN별 결과 | CDN별 Purge 결과 표시 |
| Request ID | Request ID 저장 가능 구조 |
| Task ID | Task ID 저장 가능 구조 |
| 실패 사유 | 실패 사유 저장 |
| 재시도 | 실패 항목 재시도 가능 구조 |
| 작업 이력 | 작업 이력 로컬 JSON 저장 |
| 최근 로그 | 최근 작업 로그 조회 |
| 상세 로그 | 특정 작업 로그 상세 조회 |

---

# 20. 고객사 기술 미팅 질문

## 20.1 추가 고려사항

- PC / WEB 버전 동시 가능하게 고려
- CPU / MEM 사용량 고려
- Nexon 느낌으로 UI 개발 필요

## 20.2 미팅 범위

| 구분 | 내용 |
| --- | --- |
| 미팅 대상 | 고객사 기술팀 |
| 미팅 목적 | 기존 CDN 업로드/Purge 툴 기능 재현을 위한 기술 스펙 확인 |
| 포함 범위 | AWS S3 구조, CDN URL 매핑, Purge 정책, CDN별 API 연동 방식 |
| 제외 범위 | 인증 상세, 로그/결과 처리 상세 |

## 20.3 기존 툴 기능 범위 확인

| 확인 항목 | 질문 |
| --- | --- |
| 필수 기능 | 기존 툴의 필수 기능은 AWS S3 업로드 + CDN Purge 자동화가 맞는가 |
| 지원 CDN | 지원 CDN은 CloudFront, Akamai, LG U+, 효성 CDN, KT 5개가 맞는가 |
| 업로드 | 기존 툴에서 업로드 기능을 실제 사용했는가 |
| 다운로드 | 기존 툴에서 다운로드 기능을 실제 사용했는가 |
| 삭제 | 기존 툴에서 삭제 기능을 실제 사용했는가 |
| 폴더 생성 | 기존 툴에서 폴더 생성 기능을 실제 사용했는가 |
| 이름 변경 | 기존 툴에서 이름 변경 기능을 실제 사용했는가 |
| Presigned URL | 기존 툴에서 Presigned URL 생성 기능을 실제 사용했는가 |
| Purge | 기존 툴에서 Purge 기능을 실제 사용했는가 |

## 20.4 AWS S3 구조 확인

| 확인 항목 | 질문 |
| --- | --- |
| Bucket 구조 | S3 Bucket은 단일 Bucket인가, 환경별로 분리되어 있는가 |
| 환경 구분 | dev / stage / prod 등 환경별 구분이 있는가 |
| Prefix 규칙 | S3 Prefix 규칙은 어떻게 되는가 |
| Prefix 예시 | `dev/`, `stage/`, `prod/`, `assets/` 같은 구조를 사용하는가 |
| 동일 파일 판단 | 동일 파일 판단은 MD5/ETag 비교 기준이면 되는가 |
| Content-Type | 업로드 시 Content-Type 자동 설정이 필요한가 |
| Cache-Control | 업로드 시 Cache-Control 자동 설정이 필요한가 |

## 20.5 S3 ↔ CDN URL 매핑 확인

| 확인 항목 | 질문 |
| --- | --- |
| 기본 매핑 | S3 Key와 CDN URL 매핑 규칙은 어떻게 되는가 |
| 매핑 예시 | `s3://bucket/prod/a.js` → `https://cdn.example.com/prod/a.js` 형태가 맞는가 |
| CDN별 도메인 | CDN별로 도메인이 다를 수 있는가 |
| CDN별 Prefix | CDN별로 Prefix가 다를 수 있는가 |
| 다중 CDN 연결 | 하나의 S3 파일이 여러 CDN 도메인에 동시에 연결될 수 있는가 |
| Query String | Query String이 붙은 URL도 Purge 대상에 포함해야 하는가 |

## 20.6 Purge 정책 확인

| 확인 항목 | 질문 |
| --- | --- |
| 기본 Purge 조건 | Purge는 기존 파일 덮어쓰기 시에만 자동 실행하면 되는가 |
| 신규 업로드 | 신규 업로드 파일도 Purge 대상인가 |
| 삭제 파일 | 삭제 파일도 Purge 대상인가 |
| 파일 단위 Purge | 파일 단위 Purge만 필요한가 |
| 폴더/와일드카드 Purge | `/assets/*` 같은 폴더/와일드카드 Purge도 필요한가 |
| Purge Preview | Purge 실행 전 대상 URL 목록을 미리 보여줘야 하는가 |
| 요청 성공 기준 | Purge 요청 성공까지만 확인하면 되는가 |
| 완료 상태 확인 | Purge 요청 후 완료 상태까지 Polling해야 하는가 |

## 20.7 CDN별 API 확인

| CDN | 확인 질문 |
| --- | --- |
| CloudFront | Distribution ID + Object Path 기준으로 처리하면 되는가 |
| Akamai | URL Purge 방식인가, CP Code Purge 방식인가 |
| LG U+ CDN | Purge API 문서와 인증 방식을 공유받을 수 있는가 |
| 효성 CDN | Purge API 문서와 인증 방식을 공유받을 수 있는가 |
| KT CDN | Purge API 문서와 인증 방식을 공유받을 수 있는가 |

## 20.8 CDN 공통 제한 사항 확인

| 확인 항목 | 질문 |
| --- | --- |
| 1회 요청 제한 | CDN별 1회 Purge URL 개수 제한이 있는가 |
| Rate Limit | CDN별 API Rate Limit이 있는가 |
| 요청 간격 | CDN별 요청 간격 제한이 있는가 |
| 완료 확인 방식 | CDN별 Purge 완료 상태 조회 API가 있는가 |
| 실패 응답 | CDN별 실패 응답 코드와 재시도 기준이 있는가 |

## 20.9 미팅에서 전달할 핵심 메시지

```text
이번 개발은 기존 CDN 툴을 대체하기 위한 것으로 이해하고 있습니다.

인증은 별도 업체 인증 모듈을 연동하고, 당사 개발 범위는 AWS S3 업로드/관리 기능과 CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN Purge 기능 재현입니다.

금일 미팅에서는 기존 툴 기준의 기능 범위, S3 구조, CDN URL 매핑, Purge 정책, CDN별 API 연동 방식을 확인하고자 합니다.
```

## 20.10 미팅 후 고객사로부터 수령 필요 자료

| 자료 | 목적 |
| --- | --- |
| 기존 툴 화면 캡처 | UI/동작 방식 재현 |
| 기존 설정 파일 또는 Profile 샘플 | S3/CDN 설정 구조 확인 |
| S3 Bucket / Prefix 구조 | 업로드 경로 및 환경 구분 확인 |
| S3 Key ↔ CDN URL 매핑 규칙 | Purge URL 생성 로직 구현 |
| CloudFront 설정 정보 | Distribution ID / Path 기준 확인 |
| Akamai API 문서 | URL Purge / CP Code Purge 방식 확인 |
| LG U+ CDN API 문서 | Purge API 구현 |
| 효성 CDN API 문서 | Purge API 구현 |
| KT CDN API 문서 | Purge API 구현 |
| CDN별 인증 방식 | API Key / Token / 기타 인증 방식 확인 |
| CDN별 호출 제한 정보 | Rate Limit / 요청 간격 / 1회 URL 개수 제한 확인 |

---

# 21. UploadTool TODO

## 21.1 고객사 확인 필요

| 구분 | TODO | 목적 |
| --- | --- | --- |
| CDN API | LG U+ CDN Purge API 문서 수령 | LG U+ CDN Purge 실제 구현 |
| CDN API | 효성 CDN Purge API 문서 수령 | 효성 CDN Purge 실제 구현 |
| CDN API | KT CDN Purge API 문서 수령 | KT CDN Purge 실제 구현 |
| CDN API | Akamai URL Purge / CP Code Purge 방식 확인 | Akamai Purge 구현 방식 확정 |
| S3 구조 | S3 Bucket / Prefix 구조 확인 | 업로드 경로 및 환경 구분 반영 |
| URL 매핑 | S3 Key ↔ CDN URL 매핑 규칙 확인 | Purge URL 생성 로직 구현 |
| Purge 정책 | 삭제 파일 Purge 정책 확인 | 삭제 파일 처리 기준 확정 |
| Purge 정책 | 와일드카드 Purge 지원 여부 확인 | 폴더/패턴 Purge 지원 여부 확정 |
| Purge 정책 | Query String Purge 처리 여부 확인 | Query String 포함 URL 처리 기준 확정 |
| Purge 정책 | Purge 완료 상태 Polling 여부 확인 | 요청 성공 기준 또는 완료 상태 확인 기준 확정 |
| 기존 툴 | 기존 툴 화면 캡처 수령 | UI/동작 방식 재현 |
| 기존 툴 | 기존 설정 파일 또는 Profile 샘플 수령 | 설정 구조 및 Profile 항목 확인 |
| API 제한 | CDN별 1회 Purge URL 개수 제한 확인 | Batch 처리 기준 수립 |
| API 제한 | CDN별 Rate Limit 확인 | 호출 제한 및 재시도 정책 수립 |
| API 제한 | CDN별 요청 간격 제한 확인 | 호출 간격 제어 로직 반영 |

## 21.2 1차 개발 TODO

| 구분 | TODO | 목적 |
| --- | --- | --- |
| 문서 | README.md 프로젝트 방향 수정 | 기존 툴 대체/기능 재현 방향 반영 |
| 문서 | PROJECT_ANALYSIS.md 프로젝트 방향 수정 | 현재 코드 분석 기준 재정리 |
| 문서 | MEETING_SUMMARY.md 미팅 목적 수정 | 고객사 기술 미팅 확인사항 중심 정리 |
| 문서 | TODO.md Phase 기준 정리 | 개발 우선순위 및 후속 항목 정리 |
| TypeScript | CdnProvider에 `lguplus` 추가 | LG U+ CDN Provider 지원 |
| TypeScript | CdnProvider에 `hyosung` 추가 | 효성 CDN Provider 지원 |
| TypeScript | CdnProvider에 `kt` 추가 | KT CDN Provider 지원 |
| Frontend | Profile UI에 LG U+ CDN 추가 | UI에서 LG U+ CDN 선택 가능 |
| Frontend | Profile UI에 효성 CDN 추가 | UI에서 효성 CDN 선택 가능 |
| Frontend | Profile UI에 KT CDN 추가 | UI에서 KT CDN 선택 가능 |
| Frontend | LG U+ CDN 설정 필드 추가 | API Key / Secret / Endpoint / Domain 입력 |
| Frontend | 효성 CDN 설정 필드 추가 | API Key / Secret / Endpoint / Domain 입력 |
| Frontend | KT CDN 설정 필드 추가 | API Key / Secret / Endpoint / Domain 입력 |
| Backend | Rust CdnCredentials에 Lguplus 추가 | LG U+ CDN Credential 구조 추가 |
| Backend | Rust CdnCredentials에 Hyosung 추가 | 효성 CDN Credential 구조 추가 |
| Backend | Rust CdnCredentials에 Kt 추가 | KT CDN Credential 구조 추가 |
| Backend | get_cdn_credentials 분기 추가 | 신규 CDN Provider Credential 조회 |
| Backend | LG U+ CDN Stub Adapter 추가 | API 문서 수령 전 구조 준비 |
| Backend | 효성 CDN Stub Adapter 추가 | API 문서 수령 전 구조 준비 |
| Backend | KT CDN Stub Adapter 추가 | API 문서 수령 전 구조 준비 |
| Backend | CDN Purge Dispatch 분기 추가 | Provider별 Purge 분기 처리 |
| Backend | S3 Key → CDN URL 매핑 공통 함수 추가 | Purge URL 생성 공통화 |
| 인증 | AuthAdapter Interface 추가 | 외부 인증 모듈 연동 구조 |
| 인증 | ExternalAuthAdapter Stub 추가 | 실제 인증 API 연동 전 Stub |
| 인증 | AuthUser 타입 추가 | 사용자 정보 모델 |
| 인증 | AuthSession 타입 추가 | 인증 세션 모델 |
| 로그 | OperationLog 타입 추가 | 작업 단위 로그 모델 |
| 로그 | FileOperationResult 타입 추가 | 파일별 작업 결과 모델 |
| 로그 | CdnPurgeResult 타입 추가 | CDN Purge 결과 모델 |
| 로그 | 작업 로그 JSON 저장 구조 추가 | 로컬 작업 이력 저장 |
| 로그 | 최근 작업 로그 조회 구조 추가 | 운영자 작업 이력 확인 |
| 로그 | 실패 항목 재시도 데이터 구조 추가 | 실패 파일/Purge 재시도 기반 |

## 21.3 후속 고도화 TODO

| 구분 | TODO | 목적 |
| --- | --- | --- |
| CDN API | LG U+ CDN Purge API 실제 구현 | API 문서 수령 후 구현 |
| CDN API | 효성 CDN Purge API 실제 구현 | API 문서 수령 후 구현 |
| CDN API | KT CDN Purge API 실제 구현 | API 문서 수령 후 구현 |
| CDN API | Akamai CP Code Purge 지원 여부 반영 | 고객사 방식 확정 후 반영 |
| Purge 정책 | 삭제 파일 Purge 구현 | 삭제 파일 캐시 제거 |
| Purge 정책 | 와일드카드 Purge 구현 | 폴더/패턴 단위 Purge |
| Purge 정책 | Query String Purge 구현 | Query String 포함 URL Purge |
| Purge 정책 | Purge 완료 상태 Polling 구현 | CDN별 완료 상태 확인 |
| 로그 | CSV Export | 작업 이력 외부 공유 |
| 로그 | 상세 작업 리포트 | 작업 결과 상세 확인 |
| 로그 | 감사 로그 | 운영 감사 대응 |
| 관리 | 관리자용 작업 이력 조회 | 관리자 관점 운영 이력 확인 |
| 관리 | 재시도 Dashboard | 실패 항목 일괄 재시도 |
| 권한 | 사용자 Role별 Profile 접근 제어 | 사용자 권한 기반 Profile 제한 |
| 문서 | 운영자 가이드 문서 작성 | 실제 운영자 사용 가이드 |
| 배포 | 배포 패키징 정리 | Windows/macOS/Linux 배포 정리 |

## 21.4 우선순위 정리

| 우선순위 | 항목 |
| --- | --- |
| 1순위 | 기존 툴 대체 방향 문서화 |
| 1순위 | AWS S3 중심 구조 정리 |
| 1순위 | CloudFront / Akamai 기존 기능 유지 |
| 1순위 | LG U+ CDN / 효성 CDN / KT Stub 구조 추가 |
| 1순위 | Profile UI에 전체 CDN Provider 선택 구조 반영 |
| 1순위 | S3 Key → CDN URL 매핑 공통 함수 추가 |
| 1순위 | 외부 인증 모듈 연동 준비 |
| 1순위 | 작업 로그/결과 처리 기본 구조 추가 |
| 2순위 | 고객사 확인 후 LG U+ CDN 실제 API 구현 |
| 2순위 | 고객사 확인 후 효성 CDN 실제 API 구현 |
| 2순위 | 고객사 확인 후 KT CDN 실제 API 구현 |
| 2순위 | 고객사 확인 후 Akamai Purge 방식 확정 |
| 2순위 | 고객사 확인 후 Purge 정책 세부 반영 |
| 3순위 | CSV Export |
| 3순위 | 감사 로그 |
| 3순위 | 관리자 리포트 |
| 3순위 | 재시도 Dashboard |
| 3순위 | Role 기반 상세 권한 제어 |

---

# 22. UI/UX 방향

고객사 요청 및 미팅 메모를 반영하여 UI 방향은 다음과 같다.

- PC / WEB 버전 동시 가능하게 고려
- CPU / MEM 사용량 고려
- Nexon 느낌의 내부 운영툴 스타일
- 단순 개발자 샘플 UI가 아닌 운영자가 쓰기 쉬운 화면
- FTP 스타일 듀얼 패널 고려
- 로컬 파일 패널 / S3 파일 패널 구분
- Profile 선택 영역 명확화
- CDN Provider 선택 영역 명확화
- 업로드 전 Preview 제공
- Purge 대상 URL Preview 제공
- 성공/실패 결과를 파일별로 명확히 표시
- 최근 작업 로그를 운영자가 확인 가능하게 표시
- NotImplemented CDN은 사용자에게 명확한 메시지 표시

---

# 23. Codex CLI 작업 방식

Codex CLI는 다음 방식으로 작업하라.

```text
1. Repository 분석
2. 변경 계획 수립
3. 작은 단위로 코드 수정
4. 기존 기능 영향 확인
5. typecheck/lint/cargo check 실행
6. 결과 보고
```

전체를 한 번에 대규모 수정하지 말고, 기존 구조를 유지하면서 최소 단위로 수정하라.

---

# 24. 검증 기준

작업 완료 후 가능한 범위에서 아래 명령을 실행한다.

| 명령 | 목적 |
| --- | --- |
| `pnpm typecheck` | TypeScript 타입 검증 |
| `pnpm lint` | Frontend Lint 검증 |
| `cargo check` | Rust Backend 컴파일 검증 |

프로젝트에 해당 스크립트가 없으면 가능한 대체 명령을 사용한다.

예:

```bash
pnpm build
npm run build
cargo check
cargo test
```

실패 시 실패 이유를 요약하고, 수정 가능한 범위는 수정한다.

---

# 25. 작업 완료 보고 형식

작업 완료 후 반드시 아래 형식으로 보고하라.

```md
## 작업 요약

- 변경한 주요 파일:
- 추가/수정한 타입:
- 추가한 CDN Provider:
- 추가한 Stub Adapter:
- 유지한 기존 기능:
- 추가한 인증 Stub:
- 추가한 로그/결과 구조:
- 추가한 TODO:
- 검증 결과:

## 미구현 / 고객사 확인 필요

- LG U+ CDN 실제 API:
- 효성 CDN 실제 API:
- KT CDN 실제 API:
- Akamai Purge 방식:
- Query String Purge:
- Wildcard Purge:
- 삭제 파일 Purge:
- Purge Polling:
- Rate Limit:
- Presigned URL:
- Multipart ETag fallback:

## 주의사항

- 고객사 API 문서 수령 전이라 실제 구현하지 않은 부분:
- 기존 CloudFront / Akamai 동작 유지 여부:
- 추가 검증 필요 사항:
```

---

# 26. 절대 금지 사항

아래는 절대 하지 마라.

- 기존 프로젝트를 새로 갈아엎지 마라.
- CloudFront 기존 구현을 깨지 마라.
- Akamai 기존 구현을 삭제하지 마라.
- 기존 S3 구현을 임의로 삭제하지 마라.
- LG U+ CDN API를 추측 구현하지 마라.
- 효성 CDN API를 추측 구현하지 마라.
- KT CDN API를 추측 구현하지 마라.
- 임의 Endpoint를 만들지 마라.
- 임의 Payload를 만들지 마라.
- 임의 Signature 인증을 만들지 마라.
- 자체 로그인 화면을 만들지 마라.
- 자체 사용자 DB를 만들지 마라.
- NexusPurge 자체 계정/비밀번호 인증을 만들지 마라.
- Tencent COS를 1차 범위처럼 넣지 마라.
- S3-compatible storage를 핵심 범위처럼 강조하지 마라.
- 고객사 확인 전 Query String Purge를 구현 완료 처리하지 마라.
- 고객사 확인 전 와일드카드 Purge를 구현 완료 처리하지 마라.
- 고객사 확인 전 삭제 파일 Purge를 구현 완료 처리하지 마라.
- 고객사 확인 전 Purge 완료 Polling을 확정 구현하지 마라.
- 고객사 확인 전 CDN별 Rate Limit을 확정하지 마라.
- 테스트 없이 완료됐다고 말하지 마라.

---

# 27. 최종 목표 문장

```text
UploadTool은 기존 CDN 업로드/Purge 툴을 대체하기 위한 프로젝트다.

1차 목표는 신규 기능 기획이 아니라 기존 툴 기능 재현이다.

스토리지는 AWS S3만 1차 지원한다.

CloudFront와 Akamai 기존 기능은 유지한다.

LG U+ CDN, 효성 CDN, KT CDN은 API 문서 수령 전까지 Stub Adapter로만 구성한다.

인증은 자체 구현하지 않고 외부 인증 모듈 연동 구조만 준비한다.

작업 로그와 결과 처리는 1차 개발 구조에 포함한다.

고객사 확인 전 추측 구현은 금지한다.
```

위 기준을 지키면서 현재 Repository를 분석하고, 최소 침습 방식으로 필요한 타입, UI, Backend Adapter, Stub, 로그 구조, TODO 문서를 반영하라.

---

# 부록 A. UploadTool 아키텍처 원문 반영

아래 내용은 사용자가 제공한 UploadTool 아키텍처 문서를 그대로 반영한 참고 원문이다.  
Codex는 위 개발 지시와 함께 이 아키텍처 기준을 참고해야 한다.

# UploadTool 아키텍처

## 아키텍처 개요

**UploadTool**은 **Tauri 기반 데스크톱 애플리케이션** 구조로 개발합니다.

Frontend는 **React / TypeScript** 기반으로 구성하고, Backend는 **Rust / Tauri Command** 기반으로 구성

전체 구조는 **S3 작업**, **CDN Purge**, **Profile 관리**, **작업 로그 저장**, **외부 인증 모듈 연동 Stub**을 각각 모듈화하는 방향입니다.

---

## 전체 구성

| 영역 | 기술 / 역할 |
| --- | --- |
| Desktop App | Tauri |
| Frontend | React, TypeScript |
| Backend | Rust, Tauri Command |
| Storage | AWS S3 |
| CDN | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN |
| 인증 | 외부 인증 모듈 연동 구조 준비 |
| 로그 | 로컬 JSON 기반 작업 로그 |
| 설정 | Profile 기반 S3/CDN 설정 관리 |

---

## 전체 처리 흐름

| 단계 | 처리 내용 |
| --- | --- |
| 1 | 사용자가 Profile 선택 |
| 2 | 로컬 파일 패널과 S3 파일 패널 표시 |
| 3 | 사용자가 업로드/다운로드/삭제/폴더 생성/이름 변경 작업 수행 |
| 4 | 업로드 대상 파일의 신규/변경/동일 여부 판단 |
| 5 | 덮어쓰기 또는 Purge 대상 파일 계산 |
| 6 | S3 Key를 CDN URL 또는 CloudFront Path로 변환 |
| 7 | CDN Provider별 Adapter를 통해 Purge 실행 |
| 8 | 작업 결과 표시 |
| 9 | 작업 로그를 로컬 JSON으로 저장 |

---

## Frontend 아키텍처

Frontend는 사용자가 직접 조작하는 UI 영역입니다.

주요 역할은 다음과 같습니다.

| 구분 | 역할 |
| --- | --- |
| 로컬 파일 패널 | 로컬 파일/폴더 목록 표시 |
| S3 파일 패널 | S3 Bucket/Prefix 기준 파일 목록 표시 |
| Profile 설정 | S3 및 CDN Provider 설정 관리 |
| CDN 설정 | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN 설정 입력 |
| 파일 작업 | 업로드, 다운로드, 삭제, 폴더 생성, 이름 변경 실행 |
| Purge Preview | Purge 대상 URL 또는 Path 미리보기 |
| 작업 결과 | 업로드/Purge 성공/실패 결과 표시 |
| 작업 로그 | 최근 작업 이력 표시 |
| 인증 상태 | 외부 인증 모듈 연동 상태 표시 구조 준비 |

---

## Frontend 권장 구조

| 경로 | 역할 |
| --- | --- |
| `src/types/index.ts` | 공통 타입 정의 |
| `src/components/modals/ProfileModal.tsx` | Profile 설정 UI |
| `src/services/auth/auth-types.ts` | 인증 관련 타입 |
| `src/services/auth/auth-adapter.ts` | 인증 Adapter Interface |
| `src/services/auth/external-auth-adapter.ts` | 외부 인증 Stub Adapter |
| `src/services/operation-log/operation-log-types.ts` | 작업 로그 타입 |
| `src/services/operation-log/operation-log-store.ts` | 작업 로그 상태/저장 구조 |

---

## Backend 아키텍처

Backend는 Tauri Command를 통해 실제 파일 처리, S3 API 호출, CDN Purge 호출, 로그 저장을 담당합니다.

주요 역할은 다음과 같습니다.

| 구분 | 역할 |
| --- | --- |
| AWS S3 API | 파일 목록 조회, 업로드, 다운로드, 삭제, 폴더 생성, 이름 변경 |
| CloudFront Purge | Distribution ID + Object Path 기반 Invalidation |
| Akamai Purge | URL Purge 또는 CP Code Purge 구조 준비 |
| LG U+ CDN | API 문서 수령 전까지 Stub Adapter 제공 |
| KT CDN | API 문서 수령 전까지 Stub Adapter 제공 |
| 효성 CDN | API 문서 수령 전까지 Stub Adapter 제공 |
| URL 매핑 | S3 Key → CDN URL 변환 |
| Profile Credential | CDN/S3 인증 정보 관리 |
| 작업 로그 | 로컬 JSON 저장 |
| Tauri Command | Frontend와 Backend 연결 |

---

## Backend 권장 구조

| 경로 | 역할 |
| --- | --- |
| `src-tauri/src/utils/config.rs` | Profile 및 Credential 관리 |
| `src-tauri/src/adapters/cdn/mod.rs` | CDN Adapter 공통 Dispatch |
| `src-tauri/src/adapters/cdn/cloudfront.rs` | CloudFront Purge Adapter |
| `src-tauri/src/adapters/cdn/akamai.rs` | Akamai Purge Adapter |
| `src-tauri/src/adapters/cdn/lguplus.rs` | LG U+ CDN Stub Adapter |
| `src-tauri/src/adapters/cdn/hyosung.rs` | 효성 CDN Stub Adapter |
| `src-tauri/src/commands/cdn.rs` | CDN 관련 Tauri Command |
| `src-tauri/src/services/operation_log.rs` | 작업 로그 저장/조회 서비스 |
| `src-tauri/src/adapters/cdn/kt.rs` | KT CDN Stub Adapter |

---

## CDN Adapter 구조

CDN Adapter는 CDN Provider별 구현 차이를 분리하기 위한 구조입니다.

| CDN | 처리 방식 |
| --- | --- |
| CloudFront | Object Path 기반 Invalidation |
| Akamai | URL Purge 기준, CP Code Purge 가능성 TODO |
| LG U+ CDN | API 문서 수령 전까지 NotImplemented Stub |
| 효성 CDN | API 문서 수령 전까지 NotImplemented Stub |
| KT CDN | API 문서 수령 전까지 NotImplemented Stub |

---

## CDN Adapter 처리 원칙

| 항목 | 원칙 |
| --- | --- |
| 공통 구조 | Provider별 Adapter 분리 |
| 입력값 | Purge 대상 Path 또는 URL 목록 |
| CloudFront | 기존 Object Path 방식 유지 |
| Akamai | URL Purge 기반 구조 유지 |
| LG U+ CDN | 실제 API 추측 구현 금지 |
| 효성 CDN | 실제 API 추측 구현 금지 |
| KT CDN | 실제 API 추측 구현 금지 |
| 미구현 Provider | 명확한 NotImplemented 에러 반환 |

---

## CDN Purge Dispatch 흐름

| 단계 | 처리 내용 |
| --- | --- |
| 1 | Profile에서 CDN Provider 확인 |
| 2 | Provider별 Credential 조회 |
| 3 | S3 Key를 Purge 대상 Path 또는 URL로 변환 |
| 4 | Provider별 Adapter 선택 |
| 5 | Purge API 호출 또는 Stub 에러 반환 |
| 6 | Purge 결과를 작업 결과/로그 구조로 전달 |

---

## 인증 Adapter 구조

인증은 UploadTool 자체에서 구현하지 않습니다.

외부 툴 개발업체가 제공하는 인증 모듈을 연동하기 위한 구조만 준비합니다.

| 구성 요소 | 역할 |
| --- | --- |
| AuthAdapter | 외부 인증 모듈 연동 Interface |
| ExternalAuthAdapter | 실제 연동 전 Stub 구현체 |
| AuthUser | 사용자 정보 타입 |
| AuthSession | 인증 세션 타입 |
| Token 갱신 Hook | Access Token 만료 대응 구조 |
| Logout Hook | 로그아웃 처리 구조 |
| Role 구조 | 사용자 권한 제어 기반 |
| Role 기반 제한 | 기능/Profile 접근 제어 구조 |

---

## 인증 처리 원칙

| 항목 | 원칙 |
| --- | --- |
| 자체 로그인 | 구현하지 않음 |
| 자체 계정 DB | 구현하지 않음 |
| 비밀번호 인증 | 구현하지 않음 |
| 외부 인증 API 호출 | 문서 수령 후 구현 |
| 현재 개발 범위 | Adapter / Interface / Stub 구조 준비 |

---

## 작업 로그 구조

작업 로그는 Frontend 타입과 Backend 저장 구조를 함께 둡니다.

기본 저장 방식은 **로컬 JSON 파일**입니다.

| 기능 | 설명 |
| --- | --- |
| 작업 로그 저장 | 업로드/Purge 등 작업 이력 저장 |
| 최근 로그 조회 | 최근 작업 이력 목록 조회 |
| 상세 로그 조회 | 특정 작업의 상세 결과 조회 |
| 실패 항목 보존 | 재시도에 필요한 데이터 보존 |
| CSV Export | 후속 TODO 처리 |

---

## 작업 로그 대상

| 작업 유형 | 기록 여부 |
| --- | --- |
| 업로드 | 기록 |
| 다운로드 | 기록 |
| 삭제 | 기록 |
| 폴더 생성 | 기록 |
| 이름 변경 | 기록 |
| Purge | 기록 |
| Sync | 구조 준비 |

---

## S3 Key → CDN URL 매핑 구조

S3 Key와 CDN URL 매핑은 CDN Purge 실행 전 필요한 핵심 로직입니다.

기본 매핑 함수는 다음 역할을 합니다.

| 처리 | 설명 |
| --- | --- |
| CDN Domain 정리 | trailing slash 제거 |
| Object Path 정리 | leading slash 제거 |
| URL 생성 | `domain/path` 형태로 조합 |

---

## 매핑 예시

| 항목 | 값 |
| --- | --- |
| 입력 CDN Domain | `https://cdn.example.com/` |
| 입력 Object Path | `/prod/assets/app.js` |
| 결과 URL | `https://cdn.example.com/prod/assets/app.js` |

---

## CDN별 매핑 기준

| CDN | 매핑 기준 |
| --- | --- |
| CloudFront | Object Path 기반 기존 방식 유지 |
| Akamai | URL Purge 기반 구조 |
| LG U+ CDN | URL Purge 기반 구조 준비 |
| 효성 CDN | URL Purge 기반 구조 준비 |

CDN별 Prefix 차이와 Query String Purge 여부는 고객사 확인 후 반영합니다.

---

## 아키텍처 정리

> UploadTool은 **React/TypeScript Frontend + Rust/Tauri Backend** 구조의 데스크톱 애플리케이션입니다.
> 
> 
> 핵심 모듈은 **S3 파일 관리**, **CDN Purge Adapter**, **Profile/Credential 관리**, **외부 인증 모듈 연동 Stub**, **작업 로그 저장 구조**로 분리합니다.
> 
> CloudFront와 Akamai는 기존 기능을 유지하고, LG U+ CDN과 효성 CDN은 API 문서 수령 전까지 Stub Adapter로 구성합니다.
>

---

# 부록 B. UploadTool Phase 계획 원문 반영

아래 내용은 사용자가 제공한 UploadTool Phase 계획 문서를 그대로 반영한 참고 원문이다.  
Codex는 위 개발 지시와 함께 이 Phase 기준을 참고해야 한다.

# UploadTool Phase 계획

## 개요

UploadTool의 Phase 계획은 **기존 툴 대체 기반 정리 → S3 기능 재현 → CDN 구조 확장 → 인증/로그 구조 준비 → 고객사 확인 후 실제 API 구현 → 후속 고도화** 순서로 진행합니다.

전체 개발은 신규 기능 확장이 아니라 **기존 CDN 업로드/Purge 툴의 기능 재현**을 1차 목표로 합니다.

---

## Phase 요약

| Phase | 구분 | 목표 |
| --- | --- | --- |
| Phase 1 | 기존 툴 대체 기반 정리 | 프로젝트 방향을 기존 툴 대체 및 기능 재현으로 정리 |
| Phase 2 | S3 기능 정리 및 기존 기능 재현 | 기존 툴의 S3 파일 관리 기능 재현 |
| Phase 3 | CDN Provider 구조 확장 | CloudFront, Akamai, LG U+ CDN, 효성 CDN, KT CDN 선택 구조 확장 |
| Phase 4 | CDN Adapter 및 Purge Dispatch 구성 | CDN별 Purge Adapter 구조 정리 |
| Phase 5 | 인증 연동 준비 구조 | 외부 인증 모듈 연동을 위한 구조 준비 |
| Phase 6 | 작업 로그 및 결과 처리 구조 | 작업 결과 확인 및 실패 항목 재시도 구조 제공 |
| Phase 7 | 고객사 확인 후 실제 API 구현 | 고객사 확인사항과 API 문서 기반 미구현 기능 완성 |
| Phase 8 | 후속 고도화 | 운영 편의성과 관리 기능 강화 |

---

## Phase 1. 기존 툴 대체 기반 정리

| 항목 | 내용 |
| --- | --- |
| 목표 | 프로젝트 방향을 기존 툴 대체 및 기능 재현으로 명확히 정리 |
| 산출물 | README, PROJECT_ANALYSIS, MEETING_SUMMARY, TODO 정리 |
| 핵심 기준 | 신규 기능 개발이 아니라 기존 툴 기능 재현 중심 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| README.md 수정 | 프로젝트 목적과 범위를 기존 툴 대체 기준으로 수정 |
| PROJECT_ANALYSIS.md 수정 | 현재 코드 분석 내용을 기존 기능 재현 관점으로 정리 |
| MEETING_SUMMARY.md 수정 | 고객사 기술 미팅 확인사항 중심으로 수정 |
| TODO.md 수정 | Phase 및 우선순위 기준으로 TODO 재정리 |
| 범위 정리 | Tencent COS 및 S3-compatible storage 1차 범위 표현 제거 |
| AWS S3 중심 정리 | 스토리지는 AWS S3만 1차 지원으로 정리 |
| CDN 범위 명시 | CloudFront, Akamai, LG U+ CDN, 효성 CDN 지원 명시 |
| 인증 방향 정리 | 외부 인증 모듈 연동 방식으로 정리 |
| 로그/결과 정리 | 기본 작업 로그 및 결과 처리 구조 포함 명시 |

---

## Phase 2. S3 기능 정리 및 기존 기능 재현

| 항목 | 내용 |
| --- | --- |
| 목표 | 기존 툴에서 사용하던 S3 파일 관리 기능 재현 |
| 핵심 기능 | 탐색, 업로드, 다운로드, 삭제, 폴더 생성, 이름 변경 |
| 확인 필요 | Presigned URL 사용 여부, MD5/ETag 기준, Cache-Control 정책 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| S3 파일 탐색 | Bucket/Prefix 기준 파일 목록 조회 |
| 파일 업로드 | 로컬 파일을 S3로 업로드 |
| 파일 다운로드 | S3 파일을 로컬로 다운로드 |
| 파일 삭제 | S3 Object 삭제 |
| 폴더 생성 | Prefix 기반 폴더 생성 |
| 이름 변경 | S3 Object rename 처리 |
| Presigned URL | 기존 툴 사용 여부 확인 후 반영 |
| Content-Type 자동 설정 | 확장자 기반 Content-Type 설정 |
| Cache-Control 자동 설정 | Profile 또는 확장자 기준 Cache-Control 설정 |
| 동일 파일 판단 | MD5/ETag 기반 동일 파일 판단 |
| 변경 파일 미리보기 | 업로드 전 신규/변경/동일 파일 목록 표시 |

---

## Phase 3. CDN Provider 구조 확장

| 항목 | 내용 |
| --- | --- |
| 목표 | CloudFront, Akamai, LG U+ CDN, 효성 CDN을 모두 선택 가능하도록 구조 확장 |
| 핵심 작업 | Provider 타입, Profile UI, Credential 구조 확장 |
| 주의사항 | LG U+ CDN과 효성 CDN, KT CDN 은 실제 API 추측 구현 금지 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| CdnProvider 타입 확장 | `cloudfront`, `akamai`, `lguplus`, `hyosung` 추가 |
| Profile UI 확장 | LG U+ CDN 선택 옵션 추가 |
| Profile UI 확장 | 효성 CDN 선택 옵션 추가 |
| LG U+ 설정 필드 | API Key, API Secret, Endpoint, CDN Domain 추가 |
| 효성 설정 필드 | API Key, API Secret, Endpoint, CDN Domain 추가 |
| Rust Credential 확장 | `CdnCredentials::Lguplus` 추가 |
| Rust Credential 확장 | `CdnCredentials::Hyosung` 추가 |
| Credential 조회 분기 | `get_cdn_credentials()`에 LG U+ / 효성 처리 추가 |
| Validation 추가 | Provider별 필수 Credential 누락 시 명확한 에러 반환 |

---

## Phase 4. CDN Adapter 및 Purge Dispatch 구성

| 항목 | 내용 |
| --- | --- |
| 목표 | CDN별 Purge Adapter 구조 정리 |
| 핵심 방향 | Provider별 Adapter 분리 |
| 주의사항 | 기존 CloudFront/Akamai 기능 유지 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| CloudFront 유지 | 기존 Object Path 기반 Invalidation 유지 |
| Akamai 유지 | 기존 Akamai Purge 구조 유지 |
| LG U+ Stub 추가 | API 문서 수령 전까지 NotImplemented Stub 구현 |
| 효성 Stub 추가 | API 문서 수령 전까지 NotImplemented Stub 구현 |
| Dispatch 분기 추가 | Provider별 Purge Adapter 선택 구조 추가 |
| URL 매핑 함수 추가 | S3 Key → CDN URL 공통 변환 함수 추가 |
| URL Purge 구조 준비 | Akamai/LG U+/효성 URL Purge 기반 구조 준비 |
| CloudFront Path 유지 | CloudFront는 CDN URL이 아닌 Object Path 기준 유지 |

### Stub Adapter 원칙

| Provider | 처리 방식 |
| --- | --- |
| LG U+ CDN | API 문서 수령 전까지 NotImplemented 에러 반환 |
| 효성 CDN | API 문서 수령 전까지 NotImplemented 에러 반환 |
| KT CDN | API 문서 수령 전까지 NotImplemented 에러 반환 |

예시 에러 메시지:

| Provider | 메시지 |
| --- | --- |
| LG U+ CDN | `LG U+ CDN purge API is not implemented yet. API specification is required.` |
| 효성 CDN | `Hyosung CDN purge API is not implemented yet. API specification is required.` |
| KT CDN | `kt CDN purge API is not implemented yet. API specification is required.` |

---

## Phase 5. 인증 연동 준비 구조

| 항목 | 내용 |
| --- | --- |
| 목표 | 외부 인증 모듈 연동을 위한 구조 준비 |
| 구현 범위 | Interface / Stub / 타입 구조 |
| 제외 범위 | 자체 로그인, 자체 계정 DB, 실제 인증 API 호출 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| AuthUser 타입 추가 | 사용자 ID, 이름, 이메일, 조직, Role 구조 |
| AuthSession 타입 추가 | User, Access Token, Refresh Token, 만료 시각 |
| AuthAdapter Interface 추가 | 외부 인증 모듈 연동 Interface |
| ExternalAuthAdapter Stub 추가 | 실제 인증 모듈 연동 전 Stub 구현 |
| Login 구조 준비 | 로그인 메서드 구조만 준비 |
| Logout 구조 준비 | 로그아웃 메서드 구조만 준비 |
| Token Refresh 구조 준비 | 토큰 갱신 메서드 구조 준비 |
| Current Session 구조 준비 | 현재 세션 조회 메서드 구조 준비 |
| Role 구조 준비 | 사용자 Role 기반 권한 제어 준비 |
| 실제 API 호출 | 구현하지 않고 TODO 처리 |

---

## Phase 6. 작업 로그 및 결과 처리 구조

| 항목 | 내용 |
| --- | --- |
| 목표 | 운영자가 작업 결과를 확인하고 실패 항목을 재시도할 수 있는 구조 제공 |
| 저장 방식 | 로컬 JSON 저장 |
| 후속 확장 | CSV Export, 감사 로그, 리포트 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| OperationLog 타입 추가 | 작업 단위 로그 모델 |
| FileOperationResult 타입 추가 | 파일별 작업 결과 모델 |
| CdnPurgeResult 타입 추가 | CDN Purge 결과 모델 |
| 작업 로그 Store 추가 | Frontend 작업 로그 상태/조회 구조 |
| OperationLogService 추가 | Rust Backend 로그 저장/조회 서비스 |
| 로컬 JSON 저장 | 작업 이력 로컬 파일 저장 |
| 최근 로그 조회 | 최근 작업 목록 조회 |
| 상세 로그 조회 | 특정 작업 상세 조회 |
| 실패 데이터 보존 | 실패 항목 재시도에 필요한 데이터 보존 |
| CSV Export | 이번 범위에서는 TODO 처리 |

---

## Phase 7. 고객사 확인 후 실제 API 구현

| 항목 | 내용 |
| --- | --- |
| 목표 | 고객사 기술 미팅 결과와 API 문서 기반 미구현 기능 완성 |
| 선행 조건 | 고객사 API 문서 및 기존 툴 동작 방식 확인 |
| 핵심 대상 | LG U+ CDN, 효성 CDN, Akamai 방식 확정, Purge 정책 확정 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| LG U+ CDN API 구현 | Purge API 문서 수령 후 실제 구현 |
| 효성 CDN API 구현 | Purge API 문서 수령 후 실제 구현 |
| Akamai 방식 확정 | URL Purge 또는 CP Code Purge 방식 반영 |
| S3 구조 반영 | Bucket / Prefix 규칙 반영 |
| URL 매핑 반영 | S3 Key ↔ CDN URL 세부 규칙 반영 |
| 삭제 파일 Purge | 고객사 정책 확인 후 반영 |
| 와일드카드 Purge | CDN별 지원 여부 확인 후 반영 |
| Query String Purge | 고객사 필요 여부 확인 후 반영 |
| Polling 구현 | Purge 완료 상태 확인 필요 시 반영 |

---

## Phase 8. 후속 고도화

| 항목 | 내용 |
| --- | --- |
| 목표 | 운영 편의성과 관리 기능 강화 |
| 성격 | 필수 기능 재현 이후 고도화 |
| 대상 | 리포트, 감사 로그, 재시도 Dashboard, 권한 제어 |

### 주요 작업

| 작업 | 설명 |
| --- | --- |
| CSV Export | 작업 로그 CSV 내보내기 |
| 상세 작업 리포트 | 작업별 상세 결과 리포트 |
| 감사 로그 | 운영 감사 목적의 상세 로그 |
| 관리자용 작업 이력 조회 | 관리자 관점의 작업 이력 확인 |
| 재시도 Dashboard | 실패 항목 일괄 재시도 UI |
| Role별 Profile 접근 제어 | 사용자 Role에 따른 Profile 접근 제한 |
| 환경별 Profile 관리 | dev/stage/prod Profile 분리 관리 |
| 운영자 가이드 | 사용자 운영 가이드 문서 작성 |
| 배포 패키징 | Windows/macOS/Linux 패키징 정리 |

---

## 전체 우선순위 정리

| 우선순위 | 작업 |
| --- | --- |
| 1순위 | 기존 툴 대체 목적 문서화 |
| 1순위 | AWS S3 중심 구조 정리 |
| 1순위 | CloudFront / Akamai 기존 기능 유지 |
| 1순위 | LG U+ CDN / 효성 CDN Stub 구조 추가 |
| 1순위 | Profile UI에 4개 CDN 선택 구조 반영 |
| 1순위 | S3 Key → CDN URL 매핑 공통 함수 추가 |
| 1순위 | 외부 인증 모듈 연동 준비 |
| 1순위 | 작업 로그/결과 처리 기본 구조 추가 |
| 2순위 | 고객사 확인 후 LG U+ CDN 실제 API 구현 |
| 2순위 | 고객사 확인 후 효성 CDN 실제 API 구현 |
| 2순위 | 고객사 확인 후 Akamai Purge 방식 확정 |
| 2순위 | 고객사 확인 후 Purge 정책 세부 반영 |
| 3순위 | CSV Export |
| 3순위 | 감사 로그 |
| 3순위 | 관리자 리포트 |
| 3순위 | 재시도 Dashboard |
| 3순위 | Role 기반 상세 권한 제어 |

---

## Phase 계획 정리

> UploadTool의 Phase는 **기존 툴 기능 재현을 위한 기반 정리**에서 시작해, **S3 기능 재현**, **CDN Provider 확장**, **Adapter 구조 구성**, **인증/로그 구조 준비** 순서로 진행합니다.
> 
> 
> 이후 고객사 기술 미팅과 API 문서 수령 결과에 따라 LG U+ CDN, 효성 CDN, Akamai Purge 방식, 세부 Purge 정책을 실제 구현하고, 마지막으로 운영 편의 기능을 고도화합니다.
>
