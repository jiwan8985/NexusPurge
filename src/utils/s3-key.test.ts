import { describe, expect, it } from "vitest";
import { validateS3KeySegment } from "./s3-key";

describe("validateS3KeySegment", () => {
  it("allows S3-safe characters", () => {
    expect(validateS3KeySegment("my-file_v1.2(final)!'*.txt")).toBeNull();
  });

  it("rejects Korean characters", () => {
    expect(validateS3KeySegment("한글파일.txt")).not.toBeNull();
  });

  it("rejects spaces", () => {
    expect(validateS3KeySegment("my file.txt")).not.toBeNull();
  });

  it("rejects Windows-reserved characters", () => {
    // `*`는 S3 안전 문자로 허용되므로 제외
    for (const ch of ["\\", ":", "?", '"', "<", ">", "|"]) {
      expect(validateS3KeySegment(`bad${ch}name`)).not.toBeNull();
    }
  });

  it("rejects empty names", () => {
    expect(validateS3KeySegment("")).not.toBeNull();
    expect(validateS3KeySegment("   ")).not.toBeNull();
  });
});
