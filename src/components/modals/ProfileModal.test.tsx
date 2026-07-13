import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import ProfileModal from "./ProfileModal";
import { useAppStore } from "../../store/appStore";
import type { S3Profile } from "../../types";

vi.mock("../../services/runtime", () => ({
  runtime: {
    invoke: vi.fn().mockResolvedValue([]),
    listen: vi.fn().mockResolvedValue(() => undefined),
    openDirectory: vi.fn(),
  },
}));

const profile: S3Profile = {
  id: "p1",
  name: "고객사 프로필",
  region: "ap-northeast-2",
  bucket: "secret-bucket-name",
  accessKeyId: "AKIASECRETKEY",
  secretAccessKey: "",
  cdnDomain: "cdn.secret-domain.com",
  createdAt: "2026-01-01T00:00:00Z",
  updatedAt: "2026-01-01T00:00:00Z",
};

describe("ProfileModal — 프로필 정보 잠금", () => {
  beforeEach(() => {
    useAppStore.setState({ profiles: [profile] });
  });

  it("목록에 프로필 이름만 보이고 버킷/키/도메인은 노출되지 않는다", () => {
    render(<ProfileModal />);
    expect(screen.getByText("고객사 프로필")).toBeInTheDocument();
    expect(screen.queryByText(/secret-bucket-name/)).not.toBeInTheDocument();
    expect(screen.queryByText(/AKIASECRETKEY/)).not.toBeInTheDocument();
    expect(screen.queryByText(/secret-domain/)).not.toBeInTheDocument();
  });

  it("직접 입력 폼이 없고, 프로필 이름을 클릭해도 아무 정보도 열리지 않는다 (가져오기 전용)", () => {
    render(<ProfileModal />);
    fireEvent.click(screen.getByText("고객사 프로필"));
    // 신규 프로필 직접 입력 폼 자체가 존재하지 않음 (파일 가져오기로만 등록)
    expect(document.querySelector("form")).toBeNull();
    expect(screen.queryByText("새 프로필")).not.toBeInTheDocument();
    expect(screen.queryByText("프로필 편집")).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("secret-bucket-name")).not.toBeInTheDocument();
    expect(screen.queryByDisplayValue("AKIASECRETKEY")).not.toBeInTheDocument();
    // 가져오기 버튼은 존재
    expect(screen.getByRole("button", { name: /가져오기/ })).toBeInTheDocument();
  });

  it("행 액션은 연결/테스트/내보내기/삭제만 제공한다", () => {
    render(<ProfileModal />);
    expect(screen.getByRole("button", { name: "연결" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "테스트" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "내보내기" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "삭제" })).toBeInTheDocument();
  });
});
