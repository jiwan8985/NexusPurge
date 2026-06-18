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
