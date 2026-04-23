import { useRef, useEffect } from "react";
import { useAppStore } from "../../store/appStore";
import type { LogEntry } from "../../types";
import styles from "./LogPanel.module.css";

function LogRow({ entry }: { entry: LogEntry }) {
  const time = new Date(entry.timestamp).toLocaleTimeString("ko-KR", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });

  return (
    <div className={`${styles.logRow} ${styles[entry.level]}`}>
      <span className={styles.time}>{time}</span>
      <span className={styles.level}>[{entry.level.toUpperCase()}]</span>
      <span className={styles.message}>{entry.message}</span>
    </div>
  );
}

export default function LogPanel() {
  const { logs, clearLogs } = useAppStore((s) => ({
    logs: s.logs,
    clearLogs: s.clearLogs,
  }));

  const bottomRef = useRef<HTMLDivElement>(null);

  // 새 로그 추가 시 자동 스크롤
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [logs.length]);

  return (
    <div className={styles.panel}>
      <div className={styles.header}>
        <span className={styles.title}>작업 로그</span>
        <button className={styles.clearBtn} onClick={clearLogs} title="로그 지우기">
          지우기
        </button>
      </div>
      <div className={styles.logList}>
        {logs.length === 0 ? (
          <div className={styles.empty}>로그가 없습니다</div>
        ) : (
          logs.map((entry) => <LogRow key={entry.id} entry={entry} />)
        )}
        <div ref={bottomRef} />
      </div>
    </div>
  );
}
