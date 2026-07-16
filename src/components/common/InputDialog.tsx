import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import styles from "./ConfirmDialog.module.css";
import iStyles from "./InputDialog.module.css";

interface Props {
  title: string;
  label?: string;
  initialValue?: string;
  placeholder?: string;
  confirmLabel?: string;
  cancelLabel?: string;
  multiline?: boolean;
  /** 입력값 검증 — 오류 메시지를 반환하면 인라인 표시 후 확인을 차단, null이면 통과 */
  validate?: (value: string) => string | null;
  onConfirm: (value: string) => void;
  onCancel: () => void;
}

export default function InputDialog({
  title,
  label,
  initialValue = "",
  placeholder = "",
  confirmLabel = "확인",
  cancelLabel = "취소",
  multiline = false,
  validate,
  onConfirm,
  onCancel,
}: Props) {
  const [value, setValue] = useState(initialValue);
  const [error, setError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (multiline) {
      textareaRef.current?.select();
    } else {
      inputRef.current?.select();
    }
  }, [multiline]);

  const tryConfirm = (raw: string) => {
    const trimmed = raw.trim();
    if (!trimmed) return;
    const invalid = validate?.(trimmed) ?? null;
    if (invalid) {
      setError(invalid);
      return;
    }
    onConfirm(trimmed);
  };

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (multiline) return; // Prevent enter key form submission for multiline
    tryConfirm(value);
  };

  return createPortal(
    <div
      className={styles.overlay}
      onClick={(e) => e.target === e.currentTarget && onCancel()}
    >
      <div className={styles.dialog} role="dialog" aria-modal="true">
        <div className={styles.header}>
          <span className={styles.title}>{title}</span>
        </div>
        <form className={styles.body} onSubmit={handleSubmit}>
          {label && <p className={iStyles.label}>{label}</p>}
          {multiline ? (
            <textarea
              ref={textareaRef}
              className={iStyles.textarea}
              value={value}
              onChange={(e) => setValue(e.target.value)}
              placeholder={placeholder}
              rows={5}
              autoFocus
            />
          ) : (
            <input
              ref={inputRef}
              className={iStyles.input}
              value={value}
              onChange={(e) => {
                setValue(e.target.value);
                if (error) setError(null);
              }}
              placeholder={placeholder}
              autoFocus
            />
          )}
          {error && <p className={iStyles.error}>{error}</p>}
        </form>
        <div className={styles.actions}>
          <button type="button" className={styles.cancelBtn} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className={styles.confirmBtn}
            disabled={!value.trim()}
            onClick={() => tryConfirm(value)}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>,
    document.body
  );
}
