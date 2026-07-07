import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import styles from "./ContextMenu.module.css";

export interface MenuItem {
  label: string;
  action: () => void;
  disabled?: boolean;
  danger?: boolean;
}
export interface MenuDivider { divider: true }
export type MenuEntry = MenuItem | MenuDivider;

interface Props {
  x: number;
  y: number;
  items: MenuEntry[];
  onClose: () => void;
}

export function ContextMenu({ x, y, items, onClose }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ x, y });

  // 뷰포트 경계 보정
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const { width, height } = el.getBoundingClientRect();
    setPos({
      x: Math.min(x, window.innerWidth - width - 4),
      y: Math.min(y, window.innerHeight - height - 4),
    });
  }, [x, y]);

  // 외부 클릭/ESC로 닫기
  // 주의: capture 단계 리스너는 메뉴 내부의 stopPropagation보다 먼저 실행되므로
  // 메뉴 내부 클릭 여부를 target으로 직접 판별해야 한다 (아니면 클릭 즉시 닫혀 액션이 실행되지 않음)
  useEffect(() => {
    const close = (e: MouseEvent | KeyboardEvent) => {
      if (e instanceof KeyboardEvent && e.key !== "Escape") return;
      if (e instanceof MouseEvent && ref.current?.contains(e.target as Node)) return;
      onClose();
    };
    window.addEventListener("mousedown", close, true);
    window.addEventListener("keydown", close, true);
    return () => {
      window.removeEventListener("mousedown", close, true);
      window.removeEventListener("keydown", close, true);
    };
  }, [onClose]);

  return createPortal(
    <div
      ref={ref}
      className={styles.menu}
      style={{ left: pos.x, top: pos.y }}
    >
      {items.map((item, i) =>
        "divider" in item ? (
          <div key={i} className={styles.divider} />
        ) : (
          <button
            key={i}
            className={`${styles.item} ${item.danger ? styles.danger : ""}`}
            disabled={item.disabled}
            onClick={() => { item.action(); onClose(); }}
          >
            {item.label}
          </button>
        )
      )}
    </div>,
    document.body
  );
}
