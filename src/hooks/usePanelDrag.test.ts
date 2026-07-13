import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { act, renderHook } from "@testing-library/react";
import { resolvePanelAt, usePanelDrag } from "./usePanelDrag";
import { useAppStore } from "../store/appStore";

describe("resolvePanelAt", () => {
  afterEach(() => {
    document.body.innerHTML = "";
    vi.restoreAllMocks();
  });

  it("data-panel 조상을 가진 요소 위면 해당 패널을 반환한다", () => {
    document.body.innerHTML = `<div data-panel="remote"><div id="row">file.txt</div></div>`;
    vi.spyOn(document, "elementFromPoint").mockReturnValue(
      document.getElementById("row")
    );
    expect(resolvePanelAt(10, 10)).toBe("remote");
  });

  it("패널 밖 요소면 null을 반환한다", () => {
    document.body.innerHTML = `<div id="outside">x</div>`;
    vi.spyOn(document, "elementFromPoint").mockReturnValue(
      document.getElementById("outside")
    );
    expect(resolvePanelAt(10, 10)).toBeNull();
  });

  it("요소가 없으면(창 밖) null을 반환한다", () => {
    vi.spyOn(document, "elementFromPoint").mockReturnValue(null);
    expect(resolvePanelAt(-1, -1)).toBeNull();
  });
});

describe("usePanelDrag", () => {
  const ensureSelected = vi.fn();
  const onDropToOpposite = vi.fn();
  const ghostLabel = vi.fn(() => "1개 항목");

  const pointerDown = (
    result: { current: ReturnType<typeof usePanelDrag> },
    x: number,
    y: number,
    path = "/local/file.txt"
  ) => {
    act(() => {
      result.current.onRowPointerDown(
        { button: 0, clientX: x, clientY: y } as unknown as React.PointerEvent,
        path
      );
    });
  };

  const move = (x: number, y: number) => {
    act(() => {
      window.dispatchEvent(
        new MouseEvent("pointermove", { clientX: x, clientY: y, bubbles: true })
      );
    });
  };

  const up = (x: number, y: number) => {
    act(() => {
      window.dispatchEvent(
        new MouseEvent("pointerup", { clientX: x, clientY: y, bubbles: true })
      );
    });
  };

  const pressEscape = () => {
    act(() => {
      window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
    });
  };

  beforeEach(() => {
    ensureSelected.mockClear();
    onDropToOpposite.mockClear();
    ghostLabel.mockClear();
    act(() => {
      useAppStore.setState({ panelDrag: null });
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("임계값(5px) 미만 이동이면 드래그가 시작되지 않는다", () => {
    const { result } = renderHook(() =>
      usePanelDrag({ side: "local", ensureSelected, onDropToOpposite, ghostLabel })
    );

    pointerDown(result, 0, 0);
    move(2, 2);

    expect(ensureSelected).not.toHaveBeenCalled();
    expect(document.querySelector(".panel-drag-ghost")).toBeNull();
  });

  it("임계값 초과 이동이면 드래그가 시작되고 고스트/body 클래스가 나타난다", () => {
    vi.spyOn(document, "elementFromPoint").mockReturnValue(null);
    const { result } = renderHook(() =>
      usePanelDrag({ side: "local", ensureSelected, onDropToOpposite, ghostLabel })
    );

    pointerDown(result, 0, 0);
    move(20, 20);

    expect(ensureSelected).toHaveBeenCalledWith("/local/file.txt");
    expect(document.querySelector(".panel-drag-ghost")).not.toBeNull();
    expect(document.body.classList.contains("panel-dragging")).toBe(true);
  });

  it("반대 패널 위에서 pointerup하면 onDropToOpposite가 호출되고 고스트/클래스가 정리된다", () => {
    const remoteEl = document.createElement("div");
    remoteEl.setAttribute("data-panel", "remote");
    vi.spyOn(document, "elementFromPoint").mockReturnValue(remoteEl);

    const { result } = renderHook(() =>
      usePanelDrag({ side: "local", ensureSelected, onDropToOpposite, ghostLabel })
    );

    pointerDown(result, 0, 0);
    move(20, 20);
    up(20, 20);

    expect(onDropToOpposite).toHaveBeenCalledTimes(1);
    expect(document.querySelector(".panel-drag-ghost")).toBeNull();
    expect(document.body.classList.contains("panel-dragging")).toBe(false);
  });

  it("Escape로 드래그를 취소하면 고스트가 제거되고 이후 pointerup에서 onDropToOpposite가 호출되지 않는다", () => {
    const remoteEl = document.createElement("div");
    remoteEl.setAttribute("data-panel", "remote");
    vi.spyOn(document, "elementFromPoint").mockReturnValue(remoteEl);

    const { result } = renderHook(() =>
      usePanelDrag({ side: "local", ensureSelected, onDropToOpposite, ghostLabel })
    );

    pointerDown(result, 0, 0);
    move(20, 20);
    expect(document.querySelector(".panel-drag-ghost")).not.toBeNull();

    pressEscape();
    expect(document.querySelector(".panel-drag-ghost")).toBeNull();

    up(20, 20);
    expect(onDropToOpposite).not.toHaveBeenCalled();
  });
});
