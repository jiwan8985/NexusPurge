import { useState, useEffect, useRef, useCallback } from "react";

export const ITEM_H = 22; // px — Panel.module.css의 .row height와 동기화

const BUFFER = 14; // 뷰포트 위아래로 미리 렌더링할 행 수

export function useVirtualList<T>(items: T[]) {
  const containerRef = useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = useState(0);
  const [viewportH, setViewportH] = useState(500);

  useEffect(() => {
    const el = containerRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => {
      setViewportH(el.clientHeight);
    });
    ro.observe(el);
    setViewportH(el.clientHeight);
    return () => ro.disconnect();
  }, []);

  const onScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  }, []);

  const start = Math.max(0, Math.floor(scrollTop / ITEM_H) - BUFFER);
  const end = Math.min(
    items.length,
    Math.ceil((scrollTop + viewportH) / ITEM_H) + BUFFER
  );

  return {
    containerRef,
    onScroll,
    visibleItems: items.slice(start, end) as T[],
    startIndex: start,
    totalHeight: items.length * ITEM_H,
    offsetTop: start * ITEM_H,
    offsetBottom: Math.max(0, (items.length - end) * ITEM_H),
  };
}
