import { describe, it, expect, vi, afterEach } from "vitest";
import { resolvePanelAt } from "./usePanelDrag";

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
