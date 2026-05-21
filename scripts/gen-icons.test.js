/**
 * 图标生成验证测试
 *
 * 验证所有 PNG、ICO、ICNS 文件存在且魔法字节正确
 */

import { describe, it, expect } from "bun:test";
import { readFileSync, existsSync, statSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const ICONS_DIR = join(ROOT, "assets", "icons");

const PNG_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024];

describe("PNG 套装", () => {
  for (const size of PNG_SIZES) {
    it(`issh-${size}.png 存在且为有效 PNG`, () => {
      const path = join(ICONS_DIR, `issh-${size}.png`);
      expect(existsSync(path)).toBe(true);

      const buf = readFileSync(path);
      // PNG 魔法字节: 89 50 4E 47 (0x89 "PNG")
      expect(buf[0]).toBe(0x89);
      expect(buf[1]).toBe(0x50); // P
      expect(buf[2]).toBe(0x4e); // N
      expect(buf[3]).toBe(0x47); // G

      // 文件不应为空
      expect(statSync(path).size).toBeGreaterThan(100);
    });
  }
});

describe("ICO (Windows)", () => {
  it("issh.ico 存在且魔法字节正确", () => {
    const path = join(ROOT, "assets", "issh.ico");
    expect(existsSync(path)).toBe(true);

    const buf = readFileSync(path);
    // ICO 魔法字节: 00 00 01 00
    expect(buf[0]).toBe(0x00);
    expect(buf[1]).toBe(0x00);
    expect(buf[2]).toBe(0x01);
    expect(buf[3]).toBe(0x00);

    expect(statSync(path).size).toBeGreaterThan(1000);
  });
});

describe("ICNS (macOS)", () => {
  it("issh.icns 存在且魔法字节正确", () => {
    const path = join(ROOT, "assets", "issh.icns");
    expect(existsSync(path)).toBe(true);

    const buf = readFileSync(path);
    // ICNS 魔法字节: "icns" (69 63 6E 73)
    const magic = buf.toString("ascii", 0, 4);
    expect(magic).toBe("icns");

    expect(statSync(path).size).toBeGreaterThan(1000);
  });
});

describe("SVG 源文件", () => {
  it("issh.svg 存在", () => {
    const path = join(ICONS_DIR, "issh.svg");
    expect(existsSync(path)).toBe(true);
  });
});
