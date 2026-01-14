#!/usr/bin/env python3
from __future__ import annotations

import argparse
import math
from pathlib import Path
from PIL import Image

def build_grid_png(
    input_dir: Path,
    columns: int,
    output_path: Path,
    padding: int = 0,
    bg: tuple[int, int, int, int] = (0, 0, 0, 0)
) -> None:
    if columns <= 0:
        raise ValueError("columns must be >= 1")

    pngs = sorted(input_dir.glob("*.png"))
    if not pngs:
        raise FileNotFoundError(f"No PNG files found in: {input_dir}")

    # Open first image to get dimensions and mode
    first = Image.open(pngs[0]).convert("RGBA")
    w, h = first.size

    # Validate sizes and load all images
    images: list[Image.Image] = [first]
    for p in pngs[1:]:
        im = Image.open(p).convert("RGBA")
        if im.size != (w, h):
            raise ValueError(f"Size mismatch: {p.name} is {im.size}, expected {(w, h)}")
        images.append(im)

    n = len(images)
    rows = math.ceil(n / columns)

    out_w = columns * w + (columns - 1) * padding
    out_h = rows * h + (rows - 1) * padding

    canvas = Image.new("RGBA", (out_w, out_h), bg)

    for i, im in enumerate(images):
        r = i // columns
        c = i % columns
        x = c * (w + padding)
        y = r * (h + padding)
        canvas.paste(im, (x, y))

    output_path.parent.mkdir(parents=True, exist_ok=True)
    canvas.save(output_path, format="PNG")
    print(f"Wrote: {output_path}  ({out_w}x{out_h}, {rows} rows x {columns} cols, {n} images)")


def main() -> None:
    ap = argparse.ArgumentParser(description="Combine PNGs into a grid PNG.")
    ap.add_argument("input_dir", type=Path, help="Directory containing PNG files")
    ap.add_argument("columns", type=int, help="Number of columns in the grid")
    ap.add_argument("-o", "--output", type=Path, default=Path("grid.png"), help="Output PNG path")
    ap.add_argument("--padding", type=int, default=0, help="Padding (pixels) between cells")
    args = ap.parse_args()

    build_grid_png(args.input_dir, args.columns, args.output, args.padding)


if __name__ == "__main__":
    main()

