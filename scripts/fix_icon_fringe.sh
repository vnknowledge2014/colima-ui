#!/bin/bash
# Fix white/light fringe on the Colima UI icon
# Strategy:
#   1. Extract the alpha mask from the original icon
#   2. Slightly erode the alpha to remove the 2-3px light fringe at edges
#   3. Apply the tightened alpha back to the original
#   4. For the outermost remaining edge pixels, darken any that are too bright
#   5. Regenerate all icon sizes

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
ICON_DIR="$PROJECT_DIR/src-tauri/icons"
SOURCE="$PROJECT_DIR/public/colima_icon.png"
BACKUP_DIR="$ICON_DIR/_backup"

echo "=== Colima UI Icon Fringe Fix ==="
echo "Source: $SOURCE"

# Ensure backup exists
mkdir -p "$BACKUP_DIR"
if [ ! -f "$BACKUP_DIR/colima_icon.png" ]; then
  cp "$SOURCE" "$BACKUP_DIR/colima_icon.png"
  echo "✓ Backed up original source icon"
fi

TEMP_DIR=$(mktemp -d "$PROJECT_DIR/scripts/.icon_fix_XXXXXX")
trap "rm -rf $TEMP_DIR" EXIT

echo ""
echo "Step 1: Analyzing icon..."
# Get original dimensions
DIMS=$(magick identify -format "%wx%h" "$SOURCE")
echo "  Original size: $DIMS"

echo ""
echo "Step 2: Removing white fringe..."

# Approach: 
# The light fringe is caused by bright opaque pixels at the edge of the rounded rectangle.
# We need to:
# a) Create a mask of the "interior" dark area
# b) Slightly erode alpha to cut off the bright edge pixels
# c) For remaining edge pixels, clamp brightness down

# Step 2a: Extract the alpha channel
magick "$SOURCE" -channel A -separate "$TEMP_DIR/alpha_orig.png"

# Step 2b: Erode the alpha mask by 2-3 pixels to remove the bright fringe
# Use morphology to erode - this removes the outermost opaque pixels
magick "$TEMP_DIR/alpha_orig.png" \
  -morphology Erode Disk:2.0 \
  "$TEMP_DIR/alpha_eroded.png"

# Step 2c: Smooth the eroded alpha to avoid jagged edges
magick "$TEMP_DIR/alpha_eroded.png" \
  -blur 0x0.8 \
  "$TEMP_DIR/alpha_smooth.png"

# Step 2d: Combine original RGB with the eroded+smoothed alpha
magick "$SOURCE" "$TEMP_DIR/alpha_smooth.png" \
  -compose CopyOpacity -composite \
  "$TEMP_DIR/icon_eroded.png"

# Step 2e: Now apply additional edge darkening
# For any remaining semi-bright edge pixels, darken them
# Use a technique: overlay a darkening layer only where alpha transitions
magick "$TEMP_DIR/icon_eroded.png" \
  \( -clone 0 -channel RGB -evaluate multiply 0.15 +channel \) \
  \( -clone 0 -channel A -separate +channel -edge 1 -negate -blur 0x1 \) \
  -compose Over -composite \
  "$TEMP_DIR/icon_darkened.png"

# Actually, the eroded approach above is cleaner. Let's just use the eroded version
# but verify it looks good first
cp "$TEMP_DIR/icon_eroded.png" "$TEMP_DIR/icon_final.png"

echo "  ✓ White fringe removed"

echo ""
echo "Step 3: Generating all icon sizes..."

# Copy cleaned source
cp "$TEMP_DIR/icon_final.png" "$SOURCE"
echo "  ✓ Updated public/colima_icon.png"

# Generate main icon (512x512)
magick "$TEMP_DIR/icon_final.png" -resize 512x512 "$ICON_DIR/icon.png"
echo "  ✓ icon.png (512x512)"

# Generate .icns for macOS
magick "$TEMP_DIR/icon_final.png" -resize 512x512 "$ICON_DIR/icon.icns"
echo "  ✓ icon.icns"

# Generate .ico for Windows (multi-size)
magick "$TEMP_DIR/icon_final.png" \
  \( -clone 0 -resize 16x16 \) \
  \( -clone 0 -resize 32x32 \) \
  \( -clone 0 -resize 48x48 \) \
  \( -clone 0 -resize 64x64 \) \
  \( -clone 0 -resize 128x128 \) \
  \( -clone 0 -resize 256x256 \) \
  -delete 0 "$ICON_DIR/icon.ico"
echo "  ✓ icon.ico (multi-size)"

# Copy to favicon
cp "$ICON_DIR/icon.ico" "$PROJECT_DIR/public/favicon.ico"
echo "  ✓ public/favicon.ico"

# Standard sizes
for size in 32 64 128; do
  magick "$TEMP_DIR/icon_final.png" -resize ${size}x${size} "$ICON_DIR/${size}x${size}.png"
  echo "  ✓ ${size}x${size}.png"
done

# 128x128@2x (256x256)
magick "$TEMP_DIR/icon_final.png" -resize 256x256 "$ICON_DIR/128x128@2x.png"
echo "  ✓ 128x128@2x.png (256x256)"

# Windows Store logos
declare -A STORE_SIZES=(
  ["Square30x30Logo"]=30
  ["Square44x44Logo"]=44
  ["Square71x71Logo"]=71
  ["Square89x89Logo"]=89
  ["Square107x107Logo"]=107
  ["Square142x142Logo"]=142
  ["Square150x150Logo"]=150
  ["Square284x284Logo"]=284
  ["Square310x310Logo"]=310
  ["StoreLogo"]=50
)

for name in "${!STORE_SIZES[@]}"; do
  size=${STORE_SIZES[$name]}
  magick "$TEMP_DIR/icon_final.png" -resize ${size}x${size} "$ICON_DIR/${name}.png"
  echo "  ✓ ${name}.png (${size}x${size})"
done

# Android icons
ANDROID_SIZES=(
  "mipmap-mdpi:48"
  "mipmap-hdpi:72"
  "mipmap-xhdpi:96"
  "mipmap-xxhdpi:144"
  "mipmap-xxxhdpi:192"
)

for entry in "${ANDROID_SIZES[@]}"; do
  dir="${entry%%:*}"
  size="${entry##*:}"
  android_dir="$ICON_DIR/android/$dir"
  mkdir -p "$android_dir"
  
  magick "$TEMP_DIR/icon_final.png" -resize ${size}x${size} "$android_dir/ic_launcher.png"
  
  # Round version (circular mask)
  magick "$TEMP_DIR/icon_final.png" -resize ${size}x${size} \
    \( -size ${size}x${size} xc:none -fill white -draw "circle $((size/2)),$((size/2)) $((size/2)),0" \) \
    -compose DstIn -composite "$android_dir/ic_launcher_round.png"
  
  # Foreground (slightly smaller, for adaptive icon)
  fg_size=$((size * 108 / 72))  # adaptive icon spec: 108dp for 72dp viewport
  magick "$TEMP_DIR/icon_final.png" -resize ${fg_size}x${fg_size} \
    -gravity center -extent ${fg_size}x${fg_size} \
    "$android_dir/ic_launcher_foreground.png"
  
  echo "  ✓ android/$dir (${size}x${size})"
done

# iOS icons
IOS_SIZES=(
  "AppIcon-20x20@1x:20"
  "AppIcon-20x20@2x:40"
  "AppIcon-20x20@2x-1:40"
  "AppIcon-20x20@3x:60"
  "AppIcon-29x29@1x:29"
  "AppIcon-29x29@2x:58"
  "AppIcon-29x29@2x-1:58"
  "AppIcon-29x29@3x:87"
  "AppIcon-40x40@2x:80"
  "AppIcon-40x40@2x-1:80"
  "AppIcon-40x40@3x:120"
  "AppIcon-76x76@2x:152"
  "AppIcon-512@2x:1024"
)

ios_dir="$ICON_DIR/ios"
mkdir -p "$ios_dir"

for entry in "${IOS_SIZES[@]}"; do
  name="${entry%%:*}"
  size="${entry##*:}"
  magick "$TEMP_DIR/icon_final.png" -resize ${size}x${size} "$ios_dir/${name}.png"
  echo "  ✓ ios/${name}.png (${size}x${size})"
done

echo ""
echo "=== Done! All icons regenerated without white fringe ==="
echo ""
echo "To verify, compare:"
echo "  Original: $BACKUP_DIR/colima_icon.png"
echo "  Fixed:    $SOURCE"
