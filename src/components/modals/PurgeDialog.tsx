import { useState } from "react";
import { readBatchSettings } from "../../utils/batch-settings";
import styles from "./PurgeDialog.module.css";

const PREVIEW_MAX = 8;

interface Props {
  paths: string[];
  mode: "selected" | "all";
  onConfirm: () => Promise<void>;
  onCancel: () => void;
}

export default function PurgeDialog({ paths, mode, onConfirm, onCancel }: Props) {
  const [isPurging, setIsPurging] = useState(false);

  const { purgeWarnThreshold, purgeBatchSize } = readBatchSettings();
  const count = paths.length;
  const isWarn  = count >= purgeWarnThreshold;
  const isLimit = count >= purgeBatchSize;

  const preview = paths.slice(0, PREVIEW_MAX);
  const remainder = count - preview.length;

  const handleConfirm = async () => {
    setIsPurging(true);
    try {
      await onConfirm();
    } finally {
      setIsPurging(false);
    }
  };

  return (
    <div className={styles.overlay}>
      <div className={styles.dialog}>
        <div className={styles.header}>
          <span className={styles.title}>CDN Purge 확인</span>
        </div>

        <div className={styles.body}>
          <p className={styles.summary}>
            {mode === "all"
              ? `현재 경로 전체 (${paths[0]}) 를 CDN에서 무효화합니다.`
              : `선택한 ${count}개 경로를 CDN에서 무효화합니다.`}
          </p>

          {isLimit && (
            <div className={`${styles.alert} ${styles.alertError}`}>
              배치 크기({purgeBatchSize.toLocaleString()}개) 이상 Purge는 CDN 제한에 도달할 수 있습니다.
              분할 실행을 권장합니다.
            </div>
          )}
          {!isLimit && isWarn && (
            <div className={`${styles.alert} ${styles.alertWarn}`}>
              {purgeWarnThreshold.toLocaleString()}개 이상 Purge — 처리 시간이 길어질 수 있습니다.
            </div>
          )}

          {mode === "selected" && (
            <div className={styles.pathList}>
              {preview.map((p) => (
                <div key={p} className={styles.pathItem} title={p}>{p}</div>
              ))}
              {remainder > 0 && (
                <div className={styles.pathMore}>+{remainder}개 더</div>
              )}
            </div>
          )}
        </div>

        <div className={styles.footer}>
          <button className={styles.btnCancel} onClick={onCancel} disabled={isPurging}>
            취소
          </button>
          <button
            className={`${styles.btnConfirm} ${isLimit ? styles.btnDanger : ""}`}
            onClick={handleConfirm}
            disabled={isPurging}
          >
            {isPurging ? "Purge 중..." : `Purge 실행 (${count}개)`}
          </button>
        </div>
      </div>
    </div>
  );
}
