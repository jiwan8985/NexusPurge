// S3 안전 문자 화이트리스트 — src-tauri/src/utils/validate.rs의 규칙과 동일하게 유지할 것
const SAFE_CHAR_RE = /^[A-Za-z0-9!\-_.*'()]+$/;

/** 유효하면 null, 아니면 사용자에게 보여줄 한글 오류 메시지를 반환한다. */
export function validateS3KeySegment(name: string): string | null {
  const trimmed = name.trim();
  if (!trimmed) {
    return "이름은 비워둘 수 없습니다";
  }
  if (!SAFE_CHAR_RE.test(trimmed)) {
    const badChar = [...trimmed].find((c) => !SAFE_CHAR_RE.test(c));
    return `허용되지 않는 문자 '${badChar}'가 포함되어 있습니다. 영문, 숫자, ! - _ . * ' ( ) 문자만 사용할 수 있습니다.`;
  }
  return null;
}
