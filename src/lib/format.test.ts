import { describe, expect, it } from "vitest";
import { formatBytes, looksLikeIpv4 } from "./format";

describe("format", () => {
  it("formats bytes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1536)).toBe("1.5 KB");
    expect(formatBytes(5 * 1024 * 1024, "ru")).toBe("5.0 МБ");
  });
  it("validates ipv4 shape", () => {
    expect(looksLikeIpv4("192.168.1.50")).toBe(true);
    for (const bad of ["", "tv.local", "192.168.1", "999.1.1.1", "1.2.3.4.5", "1.2.3.4; ls"]) expect(looksLikeIpv4(bad)).toBe(false);
  });
});
