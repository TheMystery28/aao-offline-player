"""
Regenerate Android icons to fill the full diameter of the launcher space.

Modern Android (API 26+) uses Adaptive Icons (ic_launcher_foreground.png), 
while legacy devices use ic_launcher_round.png. This script updates both 
so the icon appears as large as possible across all Android versions.
"""

import sys
from pathlib import Path
from PIL import Image, ImageDraw

MIPMAP_SIZES = {
    "mipmap-mdpi": 48,
    "mipmap-hdpi": 72,
    "mipmap-xhdpi": 96,
    "mipmap-xxhdpi": 144,
    "mipmap-xxxhdpi": 192,
}

def make_round_icon(source: Path, size: int) -> Image.Image:
    """Generates the legacy full-diameter round icon."""
    img = Image.open(source).convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)

    # Full-diameter circular mask (touches all 4 edges)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)

    result = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    result.paste(img, (0, 0), mask)
    return result

def make_adaptive_foreground(source: Path, base_size: int) -> Image.Image:
    """Generates the modern adaptive icon foreground without padding."""
    # Adaptive icons have a canvas size of 108dp (2.25x the base size)
    canvas_size = int(base_size * 2.25)
    
    # Scale the source image to fill the ENTIRE adaptive canvas.
    # The OS will apply its own circular mask to this, resulting in a maximum-size icon.
    img = Image.open(source).convert("RGBA").resize((canvas_size, canvas_size), Image.Resampling.LANCZOS)
    return img

def main():
    repo_root = Path(__file__).parent.parent
    source = repo_root / "src-tauri" / "icons" / "icon.png"
    res_dir = repo_root / "src-tauri" / "gen" / "android" / "app" / "src" / "main" / "res"

    if not source.exists():
        sys.exit(f"Source icon not found: {source}")
    if not res_dir.exists():
        sys.exit(f"Android res dir not found: {res_dir} — run 'tauri android init' first")

    for mipmap, size in MIPMAP_SIZES.items():
        out_round = res_dir / mipmap / "ic_launcher_round.png"
        out_foreground = res_dir / mipmap / "ic_launcher_foreground.png"
        
        if not out_round.parent.exists():
            print(f"  skipping {mipmap} (directory missing)")
            continue
            
        # 1. Overwrite legacy round icon
        make_round_icon(source, size).save(out_round)
        
        # 2. Overwrite adaptive foreground icon (Required for Android 8+)
        make_adaptive_foreground(source, size).save(out_foreground)
        
        print(f"  Updated {mipmap} (Legacy: {size}px, Adaptive: {int(size * 2.25)}px)")

    print("Done.")

if __name__ == "__main__":
    main()