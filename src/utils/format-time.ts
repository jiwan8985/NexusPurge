/** 로그 문자열에 쓰는 HH:MM:SS 포맷터 — LogPanel과 동일한 표시 형식 유지 */
export function fmtClockTime(iso: string): string {
  return new Date(iso).toLocaleTimeString("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
