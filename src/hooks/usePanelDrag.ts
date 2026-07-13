import { useCallback, useEffect, useRef } from "react";
import { useAppStore } from "../store/appStore";

export type PanelSide = "local" | "remote";

const DRAG_THRESHOLD_PX = 5;

/** 화면 좌표의 요소에서 가장 가까운 data-panel 패널을 찾는다 */
export function resolvePanelAt(x: number, y: number): PanelSide | null {
  const el = document.elementFromPoint(x, y);
  const panel = el?.closest?.("[data-panel]");
  const side = panel?.getAttribute("data-panel");
  return side === "local" || side === "remote" ? side : null;
}

interface Options {
  side: PanelSide;
  /** 드래그 확정 시 미선택 행이면 단일 선택으로 교체 (스토어 직접 접근으로 stale closure 방지) */
  ensureSelected: (path: string) => void;
  /** 반대편 패널에 드랍 완료 시 호출 (업로드/다운로드) */
  onDropToOpposite: () => void | Promise<void>;
  /** 드래그 고스트에 표시할 텍스트 (예: "3개 항목") */
  ghostLabel: () => string;
}

/**
 * 패널 간 파일 이동용 pointer 이벤트 드래그.
 * HTML5 DnD는 dragDropEnabled: true(탐색기 드랍 수신에 필요)와 공존할 수 없어 직접 구현한다.
 */
export function usePanelDrag({ side, ensureSelected, onDropToOpposite, ghostLabel }: Options) {
  const panelDrag = useAppStore((s) => s.panelDrag);
  const setPanelDrag = useAppStore((s) => s.setPanelDrag);

  const dragRef = useRef<{
    startX: number;
    startY: number;
    path: string;
    started: boolean;
    ghost: HTMLDivElement | null;
  } | null>(null);

  const cleanup = useCallback(() => {
    dragRef.current?.ghost?.remove();
    dragRef.current = null;
    document.body.classList.remove("panel-dragging");
    if (useAppStore.getState().panelDrag?.source === side) setPanelDrag(null);
  }, [side, setPanelDrag]);

  useEffect(() => {
    const onMove = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      if (!d.started) {
        if (Math.hypot(e.clientX - d.startX, e.clientY - d.startY) < DRAG_THRESHOLD_PX) return;
        d.started = true;
        ensureSelected(d.path);
        document.body.classList.add("panel-dragging");
        const ghost = document.createElement("div");
        ghost.className = "panel-drag-ghost";
        ghost.textContent = ghostLabel();
        document.body.appendChild(ghost);
        d.ghost = ghost;
      }
      if (d.ghost) {
        d.ghost.style.left = `${e.clientX + 14}px`;
        d.ghost.style.top = `${e.clientY + 14}px`;
      }
      const over = resolvePanelAt(e.clientX, e.clientY);
      const cur = useAppStore.getState().panelDrag;
      if (!cur || cur.source !== side || cur.over !== over) {
        setPanelDrag({ source: side, over });
      }
    };

    const onUp = (e: PointerEvent) => {
      const d = dragRef.current;
      if (!d) return;
      const started = d.started;
      const target = started ? resolvePanelAt(e.clientX, e.clientY) : null;
      cleanup();
      if (started) {
        // 드래그 종료 직후 발생하는 click이 선택 상태를 덮어쓰지 않도록 1회 차단
        window.addEventListener(
          "click",
          (ce) => {
            ce.stopPropagation();
            ce.preventDefault();
          },
          { capture: true, once: true }
        );
        if (target && target !== side) void onDropToOpposite();
      }
    };

    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") cleanup();
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("keydown", onKey);
    };
  }, [side, ensureSelected, onDropToOpposite, ghostLabel, setPanelDrag, cleanup]);

  /** 파일 행의 onPointerDown에 연결 */
  const onRowPointerDown = useCallback((e: React.PointerEvent, path: string) => {
    if (e.button !== 0) return; // 좌클릭만 드래그 시작
    dragRef.current = { startX: e.clientX, startY: e.clientY, path, started: false, ghost: null };
  }, []);

  const isDropTarget =
    panelDrag !== null && panelDrag.source !== side && panelDrag.over === side;

  return { onRowPointerDown, isDropTarget };
}
