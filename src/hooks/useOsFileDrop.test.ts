import { describe, it, expect, vi } from "vitest";
import { physicalToLogical, registerAllListeners } from "./useOsFileDrop";

describe("physicalToLogical", () => {
  it("물리 좌표를 devicePixelRatio로 나눠 논리 좌표로 변환한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 1.5)).toEqual({ x: 200, y: 100 });
  });

  it("scale이 0 이하이면 1로 취급한다", () => {
    expect(physicalToLogical({ x: 300, y: 150 }, 0)).toEqual({ x: 300, y: 150 });
  });
});

describe("registerAllListeners", () => {
  it("모두 성공하면 각 unlisten을 순서대로 반환한다", async () => {
    const unA = vi.fn();
    const unB = vi.fn();
    const fns = await registerAllListeners([
      () => Promise.resolve(unA),
      () => Promise.resolve(unB),
    ]);
    expect(fns).toEqual([unA, unB]);
    expect(unA).not.toHaveBeenCalled();
    expect(unB).not.toHaveBeenCalled();
  });

  it("일부만 실패하면 이미 성공한 리스너를 되감아 해제하고 에러를 던진다", async () => {
    const unA = vi.fn();
    let resolveB!: (fn: () => void) => void;
    const bPromise = new Promise<() => void>((resolve) => {
      resolveB = resolve;
    });

    const registration = registerAllListeners([
      () => Promise.resolve(unA), // 즉시 성공
      () => Promise.reject(new Error("등록 실패")), // 즉시 실패
      () => bPromise, // 나중에(catch 이후) 성공
    ]);

    await expect(registration).rejects.toThrow("등록 실패");
    // Promise.all catch 시점에 이미 resolve된 unA는 즉시 되감아 해제된다.
    expect(unA).toHaveBeenCalledTimes(1);

    // catch가 실행된 이후 뒤늦게 resolve되는 리스너도 failed 플래그를 보고 스스로 해제된다.
    const unB = vi.fn();
    resolveB(unB);
    await new Promise((r) => setTimeout(r, 0));
    expect(unB).toHaveBeenCalledTimes(1);
  });
});
