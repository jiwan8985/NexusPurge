import { useState } from "react";
import { createPortal } from "react-dom";
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
  // 루트 전체 Purge 감지: "/*" 또는 단일 경로가 와일드카드로 끝나는 경우
  const isRootPurge = mode === "all" && (
    paths[0] === "/*" || paths[0] === "*" || paths[0] === "/"
  );
  const batchCount = Math.ceil(count / purgeBatchSize);

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

  return createPortal(
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

          {isRootPurge && (
            <div className={`${styles.alert} ${styles.alertDanger}`}>
              ⚠️ 루트 전체 Purge — CDN 캐시 전체가 무효화됩니다.<br />
              모든 사용자에게 즉시 영향을 미치며, 트래픽 급증이 발생할 수 있습니다.<br />
              <strong>반드시 테스트 환경에서만 실행하세요.</strong>
            </div>
          )}
          {isLimit && (
            <div className={`${styles.alert} ${styles.alertError}`}>
              {batchCount}개 배치로 분할하여 자동 처리됩니다 (배치당 {purgeBatchSize.toLocaleString()}개).
              완료까지 다소 시간이 걸릴 수 있습니다.
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
            className={`${styles.btnConfirm} ${isLimit || isRootPurge ? styles.btnDanger : ""}`}
            onClick={handleConfirm}
            disabled={isPurging}
          >
            {isPurging
              ? "Purge 중..."
              : isRootPurge
                ? "전체 Purge 실행 (주의)"
                : `Purge 실행 (${count}개)`}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
