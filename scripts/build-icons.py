"""Generate native desktop icon containers from the SVG's rendered PNG.

Requires Pillow. See assets/icon/README.md for the preceding SVG render step.
"""

from pathlib import Path

from PIL import Image


icon_dir = Path(__file__).resolve().parents[1] / "assets" / "icon"
with Image.open(icon_dir / "json-editor.png") as source:
    icon = source.convert("RGBA")
    if icon.size != (1024, 1024):
        raise ValueError("Expected a 1024 × 1024 icon source")
    icon.save(
        icon_dir / "json-editor.ico",
        sizes=[(size, size) for size in (16, 24, 32, 48, 64, 128, 256)],
    )
    icon.save(
        icon_dir / "json-editor.icns",
        sizes=[(size, size) for size in (16, 32, 64, 128, 256, 512, 1024)],
    )
