import { useEffect, useRef, useState } from "react";
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
  onConfirm,
  onCancel,
}: Props) {
  const [value, setValue] = useState(initialValue);
  const inputRef = useRef<HTMLInputElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  useEffect(() => {
    if (multiline) {
      textareaRef.current?.select();
    } else {
      inputRef.current?.select();
    }
  }, [multiline]);

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (multiline) return; // Prevent enter key form submission for multiline
    const trimmed = value.trim();
    if (!trimmed) return;
    onConfirm(trimmed);
  };

  return (
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
              onChange={(e) => setValue(e.target.value)}
              placeholder={placeholder}
              autoFocus
            />
          )}
        </form>
        <div className={styles.actions}>
          <button type="button" className={styles.cancelBtn} onClick={onCancel}>
            {cancelLabel}
          </button>
          <button
            type="button"
            className={styles.confirmBtn}
            disabled={!value.trim()}
            onClick={() => {
              const trimmed = value.trim();
              if (trimmed) onConfirm(trimmed);
            }}
          >
            {confirmLabel}
          </button>
        </div>
      </div>
    </div>
  );
}
