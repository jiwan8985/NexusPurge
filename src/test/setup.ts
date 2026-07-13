import "@testing-library/jest-dom";

// jsdom은 elementFromPoint를 구현하지 않음 — pointer 드래그 훅 테스트에서 vi.spyOn 대상으로 필요
if (!document.elementFromPoint) {
  document.elementFromPoint = () => null;
}
