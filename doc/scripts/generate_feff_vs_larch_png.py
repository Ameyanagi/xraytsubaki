#!/usr/bin/env python3
from __future__ import annotations

import csv
import math
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Tuple

from PIL import Image, ImageDraw, ImageFont


ROOT = Path(__file__).resolve().parents[2]
DATA_DIR = ROOT / "doc" / "plots" / "feff_vs_larch_data"
PLOT_DIR = ROOT / "doc" / "plots"

CONTRIB_COLORS = [
    (123, 97, 255),
    (244, 162, 89),
    (0, 166, 166),
    (155, 93, 229),
    (67, 170, 139),
    (239, 71, 111),
    (141, 153, 174),
    (255, 127, 80),
]


@dataclass
class Series:
    name: str
    x_name: str
    x: List[float]
    model: List[float]
    larch: List[float]
    diff: List[float]
    contributions: Dict[str, List[float]]


def read_series(path: Path) -> Series:
    with path.open() as f:
        reader = csv.DictReader(f)
        fields = reader.fieldnames or []
        if not fields:
            raise ValueError(f"no headers in {path}")
        x_name = fields[0]
        contributions: Dict[str, List[float]] = {}

        if "model" in fields and "larch" in fields:
            model_key = "model"
            larch_key = "larch"
            contrib_keys: List[str] = []
        elif "total_model" in fields and "total_larch" in fields:
            model_key = "total_model"
            larch_key = "total_larch"
            contrib_keys = fields[4:]
            for key in contrib_keys:
                contributions[key] = []
        else:
            raise ValueError(f"unsupported schema in {path.name}: {fields}")

        x, model, larch, diff = [], [], [], []
        for row in reader:
            x.append(float(row[x_name]))
            mv = float(row[model_key])
            lv = float(row[larch_key])
            model.append(mv)
            larch.append(lv)
            if "diff" in row and row["diff"] is not None and row["diff"] != "":
                diff.append(float(row["diff"]))
            else:
                diff.append(mv - lv)
            for key in contrib_keys:
                contributions[key].append(float(row[key]))

    return Series(path.stem, x_name, x, model, larch, diff, contributions)


def ticks(minv: float, maxv: float, n: int = 6) -> List[float]:
    if maxv <= minv:
        return [minv]
    step = (maxv - minv) / (n - 1)
    return [minv + i * step for i in range(n)]


def fmt(v: float) -> str:
    av = abs(v)
    if av >= 1000 or (av > 0 and av < 1e-3):
        return f"{v:.2e}"
    return f"{v:.4f}"


def axis_labels(x_name: str) -> Tuple[str, str]:
    if x_name.lower() == "r":
        return "R (A)", "|chi(R)|"
    return "k (A^-1)", "chi(k)"


def map_points(
    xs: List[float],
    ys: List[float],
    x0: int,
    y0: int,
    w: int,
    h: int,
    xmin: float,
    xmax: float,
    ymin: float,
    ymax: float,
) -> List[Tuple[int, int]]:
    out = []
    xr = xmax - xmin if xmax > xmin else 1.0
    yr = ymax - ymin if ymax > ymin else 1.0
    for x, y in zip(xs, ys):
        px = int(round(x0 + (x - xmin) / xr * w))
        py = int(round(y0 + h - (y - ymin) / yr * h))
        out.append((px, py))
    return out


def draw_plot(series: Series) -> None:
    width, height = 1900, 1180
    left, right = 130, 70
    top, mid_gap, bottom = 90, 100, 80
    plot_h = 680
    resid_h = 220
    plot_w = width - left - right

    x_min, x_max = min(series.x), max(series.x)
    y_min = min(min(series.model), min(series.larch))
    y_max = max(max(series.model), max(series.larch))
    for vals in series.contributions.values():
        y_min = min(y_min, min(vals))
        y_max = max(y_max, max(vals))
    if abs(y_max - y_min) < 1.0e-12:
        y_max += 1.0e-6
        y_min -= 1.0e-6

    d_min, d_max = min(series.diff), max(series.diff)
    dm = max(abs(d_min), abs(d_max))
    d_min, d_max = (-dm, dm) if dm > 0 else (-1.0e-8, 1.0e-8)

    img = Image.new("RGB", (width, height), "white")
    draw = ImageDraw.Draw(img)
    font = ImageFont.load_default()

    p_model = map_points(series.x, series.model, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
    p_larch = map_points(series.x, series.larch, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
    p_diff = map_points(
        series.x,
        series.diff,
        left,
        top + plot_h + mid_gap,
        plot_w,
        resid_h,
        x_min,
        x_max,
        d_min,
        d_max,
    )
    p_contrib = {
        name: map_points(series.x, vals, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
        for name, vals in series.contributions.items()
    }

    x_ticks = ticks(x_min, x_max)
    y_ticks = ticks(y_min, y_max)
    d_ticks = ticks(d_min, d_max)

    grid_col = (236, 236, 236)
    axis_col = (170, 170, 170)
    text_col = (30, 30, 30)

    for xt in x_ticks:
        px = int(round(left + (xt - x_min) / (x_max - x_min) * plot_w))
        draw.line([(px, top), (px, top + plot_h)], fill=grid_col, width=1)
        draw.line([(px, top + plot_h + mid_gap), (px, top + plot_h + mid_gap + resid_h)], fill=grid_col, width=1)
        t = fmt(xt)
        tw, th = draw.textbbox((0, 0), t, font=font)[2:]
        draw.text((px - tw // 2, height - bottom + 14), t, fill=text_col, font=font)

    for yt in y_ticks:
        py = int(round(top + plot_h - (yt - y_min) / (y_max - y_min) * plot_h))
        draw.line([(left, py), (left + plot_w, py)], fill=grid_col, width=1)
        t = fmt(yt)
        tw, th = draw.textbbox((0, 0), t, font=font)[2:]
        draw.text((left - 14 - tw, py - th // 2), t, fill=text_col, font=font)

    for dt in d_ticks:
        py = int(round(top + plot_h + mid_gap + resid_h - (dt - d_min) / (d_max - d_min) * resid_h))
        draw.line([(left, py), (left + plot_w, py)], fill=grid_col, width=1)
        t = fmt(dt)
        tw, th = draw.textbbox((0, 0), t, font=font)[2:]
        draw.text((left - 14 - tw, py - th // 2), t, fill=text_col, font=font)

    draw.rectangle([left, top, left + plot_w, top + plot_h], outline=axis_col, width=2)
    draw.rectangle(
        [left, top + plot_h + mid_gap, left + plot_w, top + plot_h + mid_gap + resid_h],
        outline=axis_col,
        width=2,
    )

    for idx, (_name, pts) in enumerate(p_contrib.items()):
        draw.line(pts, fill=CONTRIB_COLORS[idx % len(CONTRIB_COLORS)], width=2)
    draw.line(p_larch, fill=(45, 99, 214), width=4)
    draw.line(p_model, fill=(204, 63, 63), width=4)
    draw.line(p_diff, fill=(40, 157, 90), width=3)

    rms = math.sqrt(sum(v * v for v in series.diff) / max(1, len(series.diff)))
    max_abs = max(abs(v) for v in series.diff) if series.diff else 0.0
    x_label, y_label = axis_labels(series.x_name)
    title = f"{series.name}: rexafs vs xraylarch"
    subtitle = f"RMS(diff)={rms:.3e}, Max|diff|={max_abs:.3e}, contributions={len(series.contributions)}"
    draw.text((left, 22), title, fill=text_col, font=font)
    draw.text((left, 46), subtitle, fill=(70, 70, 70), font=font)

    draw.text((left + plot_w + 12, top + 8), y_label, fill=text_col, font=font)
    draw.text((left + plot_w + 12, top + plot_h + mid_gap + 8), "diff", fill=text_col, font=font)
    draw.text((left + plot_w // 2 - 30, height - 18), x_label, fill=text_col, font=font)

    legend_y = top + 20
    lx = left + 8
    draw.line([(lx, legend_y), (lx + 40, legend_y)], fill=(204, 63, 63), width=4)
    draw.text((lx + 48, legend_y - 8), "rexafs total", fill=text_col, font=font)
    lx += 250
    draw.line([(lx, legend_y), (lx + 40, legend_y)], fill=(45, 99, 214), width=4)
    draw.text((lx + 48, legend_y - 8), "xraylarch total", fill=text_col, font=font)
    lx += 220
    draw.line([(lx, legend_y), (lx + 40, legend_y)], fill=(40, 157, 90), width=3)
    draw.text((lx + 48, legend_y - 8), "diff", fill=text_col, font=font)

    if series.contributions:
        y = legend_y + 22
        for idx, name in enumerate(series.contributions.keys()):
            draw.line([(left + 8, y), (left + 26, y)], fill=CONTRIB_COLORS[idx % len(CONTRIB_COLORS)], width=2)
            draw.text((left + 32, y - 8), name, fill=text_col, font=font)
            y += 16

    out_path = PLOT_DIR / f"feff_vs_larch_{series.name}.png"
    img.save(out_path, "PNG", optimize=True)


def main() -> None:
    if not DATA_DIR.exists():
        raise SystemExit(f"missing data directory: {DATA_DIR}")
    series_paths = sorted(DATA_DIR.glob("*.csv"))
    if not series_paths:
        raise SystemExit("no comparison csv files found")
    for path in series_paths:
        draw_plot(read_series(path))
    print(f"Generated {len(series_paths)} PNG plot(s) under {PLOT_DIR}")


if __name__ == "__main__":
    main()
