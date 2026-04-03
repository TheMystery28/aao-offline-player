"""
Regenerate Android ic_launcher_round.png icons with a full-diameter circle.

`tauri icon` generates round icons with a small margin (~8px on each side),
making the circle diameter smaller than the canvas. This script replaces them
with versions where the circle fills the entire canvas (diameter = canvas size),
so the icon appears as large as possible in Android round-icon launchers.
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
    img = Image.open(source).convert("RGBA").resize((size, size), Image.Resampling.LANCZOS)

    # Full-diameter circular mask (touches all 4 edges)
    mask = Image.new("L", (size, size), 0)
    ImageDraw.Draw(mask).ellipse((0, 0, size - 1, size - 1), fill=255)

    result = Image.new("RGBA", (size, size), (0, 0, 0, 0))
    result.paste(img, (0, 0), mask)
    return result


def main():
    repo_root = Path(__file__).parent.parent
    source = repo_root / "src-tauri" / "icons" / "icon.png"
    res_dir = repo_root / "src-tauri" / "gen" / "android" / "app" / "src" / "main" / "res"

    if not source.exists():
        sys.exit(f"Source icon not found: {source}")
    if not res_dir.exists():
        sys.exit(f"Android res dir not found: {res_dir} — run 'tauri android init' first")

    for mipmap, size in MIPMAP_SIZES.items():
        out = res_dir / mipmap / "ic_launcher_round.png"
        if not out.parent.exists():
            print(f"  skipping {mipmap} (directory missing)")
            continue
        make_round_icon(source, size).save(out)
        print(f"  {mipmap}/ic_launcher_round.png  ({size}x{size})")

    print("Done.")


if __name__ == "__main__":
    main()
