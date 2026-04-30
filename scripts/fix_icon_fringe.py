#!/usr/bin/env python3
"""
Fix white/light fringe on the Colima UI icon - v2.
Uses vectorized numpy operations for speed and more aggressive targeting.
"""

import os
import shutil
from PIL import Image, ImageDraw
import numpy as np
from scipy import ndimage


def fix_icon_fringe(input_path, output_path):
    """Fix the white fringe using distance-based edge detection + vectorized ops."""
    img = Image.open(input_path).convert('RGBA')
    data = np.array(img, dtype=np.float64)
    
    r, g, b, a = data[:,:,0], data[:,:,1], data[:,:,2], data[:,:,3]
    
    # Dark background color (#1c2733)
    dark_bg = np.array([28.0, 39.0, 51.0])
    
    # === PHASE 1: Fix transparent pixels with white RGB ===
    fully_transparent = a == 0
    data[fully_transparent, 0:3] = 0
    
    # === PHASE 2: Fix semi-transparent pixels ===
    semi_transparent = (a > 0) & (a < 128)
    brightness_semi = 0.299 * r + 0.587 * g + 0.114 * b
    bright_semi = semi_transparent & (brightness_semi > 80)
    data[bright_semi, 0] = dark_bg[0]
    data[bright_semi, 1] = dark_bg[1]
    data[bright_semi, 2] = dark_bg[2]
    
    # === PHASE 3: Fix opaque bright edge pixels ===
    opaque = a > 200
    
    # Distance from nearest transparent pixel
    dist_to_edge = ndimage.distance_transform_edt(opaque)
    
    # Brightness (perceived luminance)
    brightness = 0.299 * r + 0.587 * g + 0.114 * b
    
    # The fringe pixels are: opaque, bright, and near the edge
    # We use different parameters for different edge depths:
    
    # Layer 1: Very edge (1-4px) - aggressive (brightness > 60)
    mask_1 = opaque & (dist_to_edge >= 1) & (dist_to_edge <= 4) & (brightness > 60)
    
    # Layer 2: Near edge (4-8px) - moderate (brightness > 100)
    mask_2 = opaque & (dist_to_edge > 4) & (dist_to_edge <= 8) & (brightness > 100)
    
    # Layer 3: Edge periphery (8-14px) - light (brightness > 140)
    mask_3 = opaque & (dist_to_edge > 8) & (dist_to_edge <= 14) & (brightness > 140)
    
    # But we need to EXCLUDE the interior crystal/cube (which is also bright but NOT at the edge)
    # The crystal is deep inside (distance > 20px from edge), so our masks won't touch it
    
    # Apply darkening with distance-based blending
    for mask, max_dist, min_dist in [(mask_1, 4, 1), (mask_2, 8, 4), (mask_3, 14, 8)]:
        if not np.any(mask):
            continue
            
        ys, xs = np.where(mask)
        for i in range(len(ys)):
            y, x = ys[i], xs[i]
            dist = dist_to_edge[y, x]
            
            # Blend strength: stronger closer to edge
            t = 1.0 - ((dist - min_dist) / (max_dist - min_dist))
            t = max(0.0, min(1.0, t))
            
            # Stronger blend for brighter pixels
            b_val = brightness[y, x]
            if b_val > 180:
                t = min(1.0, t * 1.5)
            
            for c in range(3):
                data[y, x, c] = data[y, x, c] * (1 - t) + dark_bg[c] * t
    
    # === PHASE 4: Final cleanup - erode alpha slightly ===
    # This catches any remaining sub-pixel fringe
    alpha_binary = a > 128
    eroded = ndimage.binary_erosion(alpha_binary, iterations=1)
    
    # Only zero out alpha for pixels that were eroded AND are bright
    eroded_pixels = alpha_binary & ~eroded
    bright_eroded = eroded_pixels & (brightness > 50)
    data[bright_eroded, 3] = 0
    
    result = Image.fromarray(data.astype(np.uint8), 'RGBA')
    result.save(output_path, 'PNG')
    print(f"  ✓ Fixed and saved to {os.path.basename(output_path)}")
    return result


def generate_all_icons(fixed_img, project_dir):
    """Generate all platform icon variants from the fixed source."""
    icons_dir = os.path.join(project_dir, 'src-tauri', 'icons')
    
    # Main icons
    for name, size in [('icon.png', 512), ('32x32.png', 32), ('64x64.png', 64),
                       ('128x128.png', 128), ('128x128@2x.png', 256)]:
        fixed_img.resize((size, size), Image.LANCZOS).save(
            os.path.join(icons_dir, name), 'PNG')
        print(f"  ✓ {name} ({size}x{size})")
    
    # Windows Store
    for name, size in [('Square30x30Logo', 30), ('Square44x44Logo', 44),
                       ('Square71x71Logo', 71), ('Square89x89Logo', 89),
                       ('Square107x107Logo', 107), ('Square142x142Logo', 142),
                       ('Square150x150Logo', 150), ('Square284x284Logo', 284),
                       ('Square310x310Logo', 310), ('StoreLogo', 50)]:
        fixed_img.resize((size, size), Image.LANCZOS).save(
            os.path.join(icons_dir, f'{name}.png'), 'PNG')
        print(f"  ✓ {name}.png ({size}x{size})")
    
    # Android
    for dirname, size in [('mipmap-mdpi', 48), ('mipmap-hdpi', 72),
                          ('mipmap-xhdpi', 96), ('mipmap-xxhdpi', 144),
                          ('mipmap-xxxhdpi', 192)]:
        dirpath = os.path.join(icons_dir, 'android', dirname)
        os.makedirs(dirpath, exist_ok=True)
        resized = fixed_img.resize((size, size), Image.LANCZOS)
        resized.save(os.path.join(dirpath, 'ic_launcher.png'), 'PNG')
        
        # Round
        mask = Image.new('L', (size, size), 0)
        ImageDraw.Draw(mask).ellipse([0, 0, size-1, size-1], fill=255)
        round_img = Image.new('RGBA', (size, size), (0, 0, 0, 0))
        round_img.paste(resized, mask=mask)
        round_img.save(os.path.join(dirpath, 'ic_launcher_round.png'), 'PNG')
        
        fg_size = int(size * 108 / 72)
        fg = Image.new('RGBA', (fg_size, fg_size), (0, 0, 0, 0))
        fg.paste(resized, ((fg_size - size) // 2, (fg_size - size) // 2))
        fg.save(os.path.join(dirpath, 'ic_launcher_foreground.png'), 'PNG')
        print(f"  ✓ android/{dirname} ({size}x{size})")
    
    # iOS
    ios_dir = os.path.join(icons_dir, 'ios')
    os.makedirs(ios_dir, exist_ok=True)
    for name, size in [('AppIcon-20x20@1x', 20), ('AppIcon-20x20@2x', 40),
                       ('AppIcon-20x20@2x-1', 40), ('AppIcon-20x20@3x', 60),
                       ('AppIcon-29x29@1x', 29), ('AppIcon-29x29@2x', 58),
                       ('AppIcon-29x29@2x-1', 58), ('AppIcon-29x29@3x', 87),
                       ('AppIcon-40x40@2x', 80), ('AppIcon-40x40@2x-1', 80),
                       ('AppIcon-40x40@3x', 120), ('AppIcon-76x76@2x', 152),
                       ('AppIcon-512@2x', 1024)]:
        fixed_img.resize((size, size), Image.LANCZOS).save(
            os.path.join(ios_dir, f'{name}.png'), 'PNG')
        print(f"  ✓ ios/{name}.png ({size}x{size})")
    
    # ICO
    ico_sizes = [16, 32, 48, 64, 128, 256]
    imgs = [fixed_img.resize((s, s), Image.LANCZOS) for s in ico_sizes]
    ico_path = os.path.join(icons_dir, 'icon.ico')
    imgs[0].save(ico_path, format='ICO', sizes=[(s, s) for s in ico_sizes], append_images=imgs[1:])
    shutil.copy2(ico_path, os.path.join(project_dir, 'public', 'favicon.ico'))
    print(f"  ✓ icon.ico + favicon.ico")
    
    # ICNS via ImageMagick
    src = os.path.join(project_dir, 'public', 'colima_icon.png')
    dst = os.path.join(icons_dir, 'icon.icns')
    os.system(f'magick "{src}" -resize 512x512 "{dst}"')
    print(f"  ✓ icon.icns")


def main():
    project_dir = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    backup = os.path.join(project_dir, 'src-tauri', 'icons', '_backup', 'colima_icon.png')
    output = os.path.join(project_dir, 'public', 'colima_icon.png')
    
    print("=== Colima UI Icon Fringe Fix v2 ===\n")
    print("Step 1: Fixing white fringe...")
    fixed = fix_icon_fringe(backup, output)
    
    print("\nStep 2: Generating all platform icons...")
    generate_all_icons(fixed, project_dir)
    
    print("\n=== Done! ===")


if __name__ == '__main__':
    main()
