import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

export default function TitleBar() {
  const appWindow = getCurrentWindow();

  const handleMinimize = () => appWindow.minimize();
  const handleMaximize = () => appWindow.toggleMaximize();
  const handleClose = () => appWindow.close();

  return (
    // data-tauri-drag-region: 드래그 영역 지정 (Tauri frameless window)
    <div className={styles.titlebar} data-tauri-drag-region>
      <div className={styles.left} data-tauri-drag-region>
        <span className={styles.icon}>☁</span>
        <span className={styles.title}>CDN Upload Tool</span>
      </div>

      <div className={styles.controls}>
        <button
          className={styles.controlBtn}
          onClick={handleMinimize}
          aria-label="최소화"
          title="최소화"
        >
          ─
        </button>
        <button
          className={styles.controlBtn}
          onClick={handleMaximize}
          aria-label="최대화"
          title="최대화"
        >
          □
        </button>
        <button
          className={`${styles.controlBtn} ${styles.closeBtn}`}
          onClick={handleClose}
          aria-label="닫기"
          title="닫기"
        >
          ✕
        </button>
      </div>
    </div>
  );
}
