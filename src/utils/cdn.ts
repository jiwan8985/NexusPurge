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

export function defaultCacheControlFor(key: string): string {
  if (/\.(html?)$/i.test(key)) return "no-cache";
  if (/\.[a-f0-9]{8,}\./i.test(key)) return "max-age=31536000, immutable";
  return "";
}
