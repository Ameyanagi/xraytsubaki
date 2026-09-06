"""Convert the selected generated PNG into native icon containers; no image generation."""
from pathlib import Path
from PIL import Image

brand = Path(__file__).resolve().parents[1] / "assets/brand"
with Image.open(brand / "rexafs-icon.png") as source:
    icon = source.convert("RGBA").resize((1024, 1024), Image.Resampling.LANCZOS)
    icon.save(brand / "rexafs.icns", format="ICNS")
    icon.save(brand / "rexafs.ico", format="ICO", sizes=[(n, n) for n in (16, 24, 32, 48, 64, 128, 256)])
print("Created rexafs.icns and rexafs.ico from rexafs-icon.png")
