import type { CdnProvider, S3Profile } from "../types";

export const CDN_LABELS: Record<CdnProvider, string> = {
  cloudfront: "CloudFront",
  akamai:     "Akamai",
  kt:         "KT CDN",
  lguplus:    "LG U+ CDN",
  hyosung:    "효성 ITX",
};

/** 프로필에서 사용 가능한 CDN 목록 (멀티 CDN 프로필은 cdnProviders, 단일 프로필은 자격증명 존재 여부로 판별) */
export function availableCdns(profile: S3Profile | null | undefined): CdnProvider[] {
  if (!profile) return [];
  const fromList = (profile.cdnProviders ?? [])
    .filter((c) => c.enabled !== false)
    .map((c) => c.provider);
  if (fromList.length > 0) return fromList;

  const out: CdnProvider[] = [];
  if (profile.cdnProvider === "cloudfront" || profile.cdnDistributionId) out.push("cloudfront");
  if (profile.akamaiHost && profile.akamaiClientToken) out.push("akamai");
  if (profile.ktUsername && profile.ktServiceName) out.push("kt");
  if (profile.lguplusUsername && profile.lguplusServiceName) out.push("lguplus");
  if (profile.hyosungApiKey) out.push("hyosung");
  if (out.length === 0 && profile.cdnProvider) out.push(profile.cdnProvider);
  return [...new Set(out)];
}

/** provider별 CDN 도메인 — cdnProviders 항목 우선, 없으면 공용 cdnDomain */
export function cdnDomainFor(
  profile: S3Profile | null | undefined,
  provider: CdnProvider | null | undefined,
): string | undefined {
  if (!profile) return undefined;
  if (provider) {
    const entry = profile.cdnProviders?.find((c) => c.provider === provider);
    if (entry?.domain?.trim()) return entry.domain;
  }
  return profile.cdnDomain;
}

/** provider별 Distribution ID (CloudFront) */
export function cdnDistributionIdFor(
  profile: S3Profile | null | undefined,
  provider: CdnProvider | null | undefined,
): string | undefined {
  if (!profile) return undefined;
  if (provider) {
    const entry = profile.cdnProviders?.find((c) => c.provider === provider);
    if (entry?.distributionId?.trim()) return entry.distributionId;
  }
  return profile.cdnDistributionId;
}

// cdnBasePath: S3 키에서 제거할 CDN 경로 접두사
// 예) S3 키 "contents/file.txt" + cdnBasePath "contents/" → CDN 경로 "file.txt"
export function buildCdnUrl(
  cdnDomain: string | undefined,
  key: string,
  cdnBasePath?: string,
): string | null {
  if (!cdnDomain?.trim()) return null;
  const domain = cdnDomain
    .trim()
    .replace(/^https?:\/\//, "")
    .replace(/\/+$/, "");
  let normalizedKey = key.replace(/^\/+/, "");
  if (cdnBasePath) {
    const base = cdnBasePath.replace(/^\/+/, "").replace(/\/+$/, "") + "/";
    if (normalizedKey.startsWith(base)) {
      normalizedKey = normalizedKey.slice(base.length);
    }
  }
  return `https://${domain}/${normalizedKey}`;
}

// 효성 엣지 별칭({서비스도메인}.{그룹}.hscdn.net, CNAME 대상)에서 서비스 도메인 복원.
// Rust adapters/cdn/hyosung.rs::service_domain과 동일 규칙 — 엣지 별칭은 실서비스
// vhost가 아니라서(엣지 노드 실측 404) 확인/Purge 모두 서비스 도메인을 써야 한다.
function hyosungServiceDomain(domain: string): string {
  const m = domain.match(/^(.+\..+)\.[^.]+\.hscdn\.net$/);
  return m ? m[1] : domain;
}

// 속성 다이얼로그 "실시간 확인"용 공개 URL.
// Purge 대상 URL(buildCdnUrl)과 달리, cdnBasePath가 비어 있으면 basePrefix(S3 탐색 루트)를
// 제거한다 — CDN 원본 경로가 basePrefix를 가리키는 구성(예: contents/)에서 실제 응답을
// 확인하려면 공개 URL로 요청해야 하기 때문. Purge 경로에는 영향 없음.
// 효성은 서비스 도메인 + 기본 http 스킴(도메인에 https:// 명시 시 https) 사용.
export function buildInspectUrl(
  provider: CdnProvider,
  cdnDomain: string | undefined,
  key: string,
  cdnBasePath?: string,
  basePrefix?: string,
): string | null {
  const stripPrefix = cdnBasePath?.trim() ? cdnBasePath : basePrefix;
  const url = buildCdnUrl(cdnDomain, key, stripPrefix);
  if (!url || provider !== "hyosung") return url;

  const scheme = cdnDomain!.trim().startsWith("https://") ? "https" : "http";
  const rest = url.slice("https://".length); // "{host}/{path}"
  const slash = rest.indexOf("/");
  const host = slash === -1 ? rest : rest.slice(0, slash);
  const path = slash === -1 ? "" : rest.slice(slash);
  return `${scheme}://${hyosungServiceDomain(host)}${path}`;
}

export function defaultCacheControlFor(key: string): string {
  if (/\.(html?)$/i.test(key)) return "no-cache";
  if (/\.[a-f0-9]{8,}\./i.test(key)) return "max-age=31536000, immutable";
  return "";
}

/**
 * Purge 요청이 실제로 호출하는 CDN API 엔드포인트를 설명 문자열로 반환 (정보 표시용, 실제 호출 아님).
 * Rust 쪽 commands/cdn.rs::describe_cdn_endpoint와 동일한 로직 — 속성 다이얼로그에서
 * 실제 IPC 호출 없이 "이 CDN을 Purge하면 어떤 엔드포인트로 요청이 가는지"를 바로 보여주기 위함.
 */
export function describeCdnEndpoint(
  profile: S3Profile | null | undefined,
  provider: CdnProvider,
): string | null {
  if (!profile) return null;
  const distributionId = cdnDistributionIdFor(profile, provider);
  switch (provider) {
    case "cloudfront":
      return distributionId
        ? `POST https://cloudfront.amazonaws.com/2020-05-31/distribution/${distributionId}/invalidation`
        : null;
    case "akamai":
      return profile.akamaiHost
        ? `POST https://${profile.akamaiHost}/ccu/v3/invalidate/url/production (폴더/전체 Purge는 .../invalidate/cpcode/production)`
        : null;
    case "lguplus": {
      const endpoint = profile.lguplusEndpoint || "https://api.lgucdn.com";
      return profile.lguplusServiceName
        ? `POST ${endpoint}/v3/management/service/${profile.lguplusServiceName}/volume/{volumeName}/purge (Volume Name 미설정 시 .../domain/{domain}/purge)`
        : null;
    }
    case "kt": {
      const endpoint = profile.ktEndpoint || "https://api.ktcdn.co.kr";
      return profile.ktServiceName
        ? `POST ${endpoint}/v3/management/service/${profile.ktServiceName}/volume/{volumeName}/purge (Volume Name 미설정 시 .../domain/{domain}/purge)`
        : null;
    }
    case "hyosung": {
      const endpoint = profile.hyosungEndpoint || "https://api.xtrmcdn.co.kr:28091";
      return distributionId ? `POST ${endpoint}/api/v1/purge/${distributionId}` : null;
    }
    default:
      return null;
  }
}
