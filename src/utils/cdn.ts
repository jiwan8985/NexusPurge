export function buildCdnUrl(cdnDomain: string | undefined, key: string): string | null {
  if (!cdnDomain?.trim()) return null;
  const domain = cdnDomain
    .trim()
    .replace(/^https?:\/\//, "")
    .replace(/\/+$/, "");
  const normalizedKey = key.replace(/^\/+/, "");
  return `https://${domain}/${normalizedKey}`;
}

export function defaultCacheControlFor(key: string): string {
  if (/\.(html?)$/i.test(key)) return "no-cache";
  if (/\.[a-f0-9]{8,}\./i.test(key)) return "max-age=31536000, immutable";
  return "";
}
