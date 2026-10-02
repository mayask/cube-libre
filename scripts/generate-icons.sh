#!/usr/bin/env bash
# Generate raster bundle assets without editor metadata or workstation paths.
set -euo pipefail
cd "$(dirname "$0")/.."
temp="$(mktemp -d)"
trap 'rm -rf "$temp"' EXIT
magick -background none -density 384 assets/cube-libre.svg -resize 1024x1024 -strip "$temp/icon.png"
python3 - "$temp/icon.png" <<'PY'
import sys
from PIL import Image

with Image.open(sys.argv[1]) as source:
    rgba = source.convert('RGBA')
    rgba.resize((512, 512), Image.Resampling.LANCZOS).save('assets/icon.png')
    rgba.save('assets/icon.ico', sizes=[(s, s) for s in (16, 24, 32, 48, 64, 128, 256)])
    rgba.save('assets/icon.icns')
print('Generated PNG, ICO and ICNS icons without metadata.')
PY
