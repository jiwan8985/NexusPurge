import { describe, it, expect } from "vitest";
import { physicalToLogical } from "./useOsFileDrop";

describe("physicalToLogical", () => {
  it("물리 좌표를 devicePixelRatio로 나눠 논리 좌표로 변환한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 1.5)).toEqual({ x: 200, y: 100 });
  });

  it("scale이 0 이하이면 1로 취급한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 0)).toEqual({ x: 300, y: 150 });
  });
});
