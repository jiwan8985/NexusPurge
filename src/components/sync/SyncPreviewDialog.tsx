import { useAppStore } from "../../store/appStore";
import type { SyncPreviewEntry } from "../../types";
import styles from "./SyncPreviewDialog.module.css";

function fmtSize(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1048576) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1073741824) return `${(bytes / 1048576).toFixed(1)} MB`;
  return `${(bytes / 1073741824).toFixed(2)} GB`;
}

function PreviewRows({ label, state, items }: { label: string; state: string; items: SyncPreviewEntry[] }) {
  if (items.length === 0) return null;
  return (
    <section className={styles.section}>
      <div className={styles.sectionTitle}>{label}</div>
      {items.map((item) => (
        <div className={styles.row} key={`${state}:${item.remoteKey}:${item.localPath ?? ""}`}>
          <span className={styles.state}>{state}</span>
          <span className={styles.path} title={item.localPath ?? item.remoteKey}>
            {item.localPath ?? item.remoteKey}
          </span>
          <span className={styles.size}>{fmtSize(item.size)}</span>
        </div>
      ))}
    </section>
  );
}

export default function SyncPreviewDialog() {
  const { syncPreview, setShowSyncPreview } = useAppStore((s) => ({
    syncPreview: s.syncPreview,
    setShowSyncPreview: s.setShowSyncPreview,
  }));

  if (!syncPreview) return null;

  const total =
    syncPreview.new.length +
    syncPreview.modified.length +
    syncPreview.deleted.length +
    syncPreview.unchanged.length;

  return (
    <div className={styles.overlay} onClick={(event) => event.target === event.currentTarget && setShowSyncPreview(false)}>
      <div className={styles.dialog}>
        <div className={styles.header}>
          <div>
            <div className={styles.title}>Dry-run 미리보기</div>
            <div className={styles.summary}>
              <span className={styles.badge}>전체 {total}</span>
              <span className={styles.badge}>신규 {syncPreview.new.length}</span>
              <span className={styles.badge}>수정 {syncPreview.modified.length}</span>
              <span className={styles.badge}>삭제 후보 {syncPreview.deleted.length}</span>
              <span className={styles.badge}>Purge {syncPreview.purgeTargets.length}</span>
            </div>
          </div>
          <button type="button" className={styles.btn} onClick={() => setShowSyncPreview(false)}>닫기</button>
        </div>

        <div className={styles.body}>
          {total === 0 ? (
            <div className={styles.empty}>변경 사항이 없습니다.</div>
          ) : (
            <>
              <PreviewRows label="업로드 신규" state="new" items={syncPreview.new} />
              <PreviewRows label="덮어쓰기" state="modified" items={syncPreview.modified} />
              <PreviewRows label="원격 삭제 후보" state="deleted" items={syncPreview.deleted} />
              <PreviewRows label="스킵" state="skip" items={syncPreview.unchanged} />

              <section className={styles.section}>
                <div className={styles.sectionTitle}>Purge Preview</div>
                {syncPreview.purgeTargets.length === 0 ? (
                  <div className={styles.empty}>Purge 대상이 없습니다.</div>
                ) : (
                  syncPreview.purgeTargets.map((key) => (
                    <div className={styles.row} key={`purge:${key}`}>
                      <span className={styles.state}>purge</span>
                      <span className={styles.path} title={key}>{key}</span>
                      <span className={styles.size}>-</span>
                    </div>
                  ))
                )}
              </section>
            </>
          )}
        </div>

        <div className={styles.footer}>
          <span className={styles.badge}>실제 업로드/삭제는 실행되지 않았습니다.</span>
          <button
            type="button"
            className={styles.btn}
            onClick={() => navigator.clipboard.writeText(syncPreview.purgeTargets.join("\n"))}
            disabled={syncPreview.purgeTargets.length === 0}
          >
            Purge 목록 복사
          </button>
        </div>
      </div>
    </div>
  );
}
