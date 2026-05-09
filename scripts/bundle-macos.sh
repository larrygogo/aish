#!/usr/bin/env bash
# 将 aish 打包为 macOS .app bundle
set -euo pipefail

BUNDLE="aish.app"
BIN="target/release/aish"

# 从 cargo metadata + glob 定位 OUT_DIR 中的 aish.icns
OUT_DIR=$(cargo metadata --no-deps --format-version 1 \
  | python3 -c "
import json, sys, glob
meta = json.load(sys.stdin)
target = meta['target_directory']
matches = glob.glob(f'{target}/aish-app/build/*/out/icons/aish.icns')
print(matches[0] if matches else '')
")

if [[ -z "$OUT_DIR" ]]; then
  echo "❌ 找不到 aish.icns，请先运行 cargo build --release"
  exit 1
fi

mkdir -p "$BUNDLE/Contents/MacOS"
mkdir -p "$BUNDLE/Contents/Resources"

cp "$BIN"     "$BUNDLE/Contents/MacOS/aish"
cp "$OUT_DIR" "$BUNDLE/Contents/Resources/aish.icns"

cat > "$BUNDLE/Contents/Info.plist" << 'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key>             <string>aish</string>
  <key>CFBundleIdentifier</key>       <string>com.larrygogo.aish</string>
  <key>CFBundleVersion</key>          <string>0.1.0</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleExecutable</key>       <string>aish</string>
  <key>CFBundleIconFile</key>         <string>aish</string>
  <key>CFBundlePackageType</key>      <string>APPL</string>
  <key>NSHighResolutionCapable</key>  <true/>
  <key>LSMinimumSystemVersion</key>   <string>13.0</string>
</dict>
</plist>
EOF

echo "✅ Bundle 创建完成：$BUNDLE"
