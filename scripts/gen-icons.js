/**
 * issh 图标生成脚本
 *
 * 单一 SVG 真相来源 → PNG 套装 + ICO (Windows) + ICNS (macOS)
 *
 * 用法: bun run gen-icons.js
 */

import { readFileSync, writeFileSync, mkdirSync } from "fs";
import { join, dirname } from "path";
import { fileURLToPath } from "url";
import { Resvg } from "@resvg/resvg-js";
import pngToIco from "png-to-ico";
import { Icns, IcnsImage } from "@fiahfy/icns";

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, "..");
const ICONS_DIR = join(ROOT, "assets", "icons");
const SVG_PATH = join(ICONS_DIR, "issh.svg");

// 所有需要生成的 PNG 尺寸
const PNG_SIZES = [16, 32, 48, 64, 128, 256, 512, 1024];

// ICO 包含的尺寸（Windows 标准）
const ICO_SIZES = [16, 32, 48, 64, 128, 256];

// ICNS osType 映射（macOS 标准）
const ICNS_TYPES = [
  { size: 16, osType: "icp4" },
  { size: 32, osType: "icp5" },
  { size: 64, osType: "icp6" },
  { size: 128, osType: "ic07" },
  { size: 256, osType: "ic08" },
  { size: 512, osType: "ic09" },
  { size: 1024, osType: "ic10" },
];

/**
 * SVG → PNG（指定尺寸）
 */
function renderPng(svgData, size) {
  const resvg = new Resvg(svgData, {
    fitTo: { mode: "width", value: size },
    background: "rgba(0, 0, 0, 0)",
  });
  const rendered = resvg.render();
  return rendered.asPng();
}

async function main() {
  console.log("📦 issh 图标生成开始...\n");

  const svgData = readFileSync(SVG_PATH, "utf-8");

  // 确保输出目录存在
  mkdirSync(ICONS_DIR, { recursive: true });

  // 1. 生成所有 PNG
  const pngBuffers = new Map();
  for (const size of PNG_SIZES) {
    const png = renderPng(svgData, size);
    const outPath = join(ICONS_DIR, `issh-${size}.png`);
    writeFileSync(outPath, png);
    pngBuffers.set(size, png);
    console.log(`  ✅ PNG ${size}×${size} → ${outPath}`);
  }

  // 2. 生成 ICO（Windows）
  const icoPngs = ICO_SIZES.map((size) => {
    const path = join(ICONS_DIR, `issh-${size}.png`);
    return path;
  });
  const icoBuffer = await pngToIco(icoPngs);
  const icoPath = join(ROOT, "assets", "issh.ico");
  writeFileSync(icoPath, icoBuffer);
  console.log(`  ✅ ICO → ${icoPath}`);

  // 3. 生成 ICNS（macOS）
  const icns = new Icns();
  for (const { size, osType } of ICNS_TYPES) {
    const pngBuf = pngBuffers.get(size);
    const image = IcnsImage.fromPNG(pngBuf, osType);
    icns.append(image);
  }
  const icnsPath = join(ROOT, "assets", "issh.icns");
  writeFileSync(icnsPath, icns.data);
  console.log(`  ✅ ICNS → ${icnsPath}`);

  console.log("\n🎉 图标生成完成！");
}

main().catch((err) => {
  console.error("❌ 图标生成失败:", err);
  process.exit(1);
});
