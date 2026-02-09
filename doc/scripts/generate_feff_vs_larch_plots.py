#!/usr/bin/env python3
from __future__ import annotations

import csv
import math
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List


ROOT = Path(__file__).resolve().parents[2]
DATA_DIR = ROOT / "doc" / "plots" / "feff_vs_larch_data"
PLOT_DIR = ROOT / "doc" / "plots"

CONTRIB_COLORS = [
    "#7b61ff",
    "#f4a259",
    "#00a6a6",
    "#9b5de5",
    "#43aa8b",
    "#ef476f",
    "#8d99ae",
    "#ff7f50",
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
            raise ValueError(f"unsupported schema for {path.name}: {fields}")

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


def line(points: List[tuple[float, float]], color: str, width: float = 1.6, dash: str | None = None) -> str:
    pts = " ".join(f"{x:.2f},{y:.2f}" for x, y in points)
    dash_attr = f' stroke-dasharray="{dash}"' if dash else ""
    return f'<polyline fill="none" stroke="{color}" stroke-width="{width}"{dash_attr} points="{pts}" />'


def map_xy(xs: List[float], ys: List[float], x0: float, y0: float, w: float, h: float, xmin: float, xmax: float, ymin: float, ymax: float) -> List[tuple[float, float]]:
    out = []
    xr = xmax - xmin if xmax > xmin else 1.0
    yr = ymax - ymin if ymax > ymin else 1.0
    for x, y in zip(xs, ys):
        px = x0 + (x - xmin) / xr * w
        py = y0 + h - (y - ymin) / yr * h
        out.append((px, py))
    return out


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


def axis_labels(x_name: str) -> tuple[str, str]:
    if x_name.lower() == "r":
        return "R (\u00c5)", "|\u03c7(R)|"
    return "k (\u00c5\u207b\u00b9)", "\u03c7(k)"


def write_plot(series: Series) -> None:
    width, height = 1400, 840
    left, right = 90, 40
    top, mid_gap, bottom = 52, 60, 56
    plot_h = 500
    resid_h = 170
    plot_w = width - left - right

    x_min, x_max = min(series.x), max(series.x)
    y_min = min(min(series.model), min(series.larch))
    y_max = max(max(series.model), max(series.larch))
    for vals in series.contributions.values():
        y_min = min(y_min, min(vals))
        y_max = max(y_max, max(vals))
    if abs(y_max - y_min) < 1e-12:
        y_max += 1e-6
        y_min -= 1e-6

    d_min, d_max = min(series.diff), max(series.diff)
    dm = max(abs(d_min), abs(d_max))
    d_min, d_max = (-dm, dm) if dm > 0 else (-1e-8, 1e-8)

    p_model = map_xy(series.x, series.model, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
    p_larch = map_xy(series.x, series.larch, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
    p_diff = map_xy(series.x, series.diff, left, top + plot_h + mid_gap, plot_w, resid_h, x_min, x_max, d_min, d_max)
    p_contrib = {
        name: map_xy(series.x, vals, left, top, plot_w, plot_h, x_min, x_max, y_min, y_max)
        for name, vals in series.contributions.items()
    }

    rms = math.sqrt(sum(v * v for v in series.diff) / max(1, len(series.diff)))
    max_abs = max(abs(v) for v in series.diff) if series.diff else 0.0
    x_ticks = ticks(x_min, x_max)
    y_ticks = ticks(y_min, y_max)
    d_ticks = ticks(d_min, d_max)
    x_label, y_label = axis_labels(series.x_name)

    parts = []
    parts.append(f'<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">')
    parts.append('<style>text{font-family:Menlo,Monaco,Consolas,monospace;fill:#222}.small{font-size:12px}.axis{font-size:13px}.title{font-size:18px;font-weight:600}.sub{font-size:12px;fill:#444}</style>')
    parts.append('<rect x="0" y="0" width="100%" height="100%" fill="#fff"/>')
    subtitle = f"RMS(diff)={rms:.3e}, Max|diff|={max_abs:.3e}, contributions={len(series.contributions)}"
    parts.append(f'<text class="title" x="{left}" y="26">{series.name}: xraytsubaki vs xraylarch</text>')
    parts.append(f'<text class="sub" x="{left}" y="42">{subtitle}</text>')

    for xt in x_ticks:
        px = left + (xt - x_min) / (x_max - x_min) * plot_w
        parts.append(f'<line x1="{px:.2f}" y1="{top}" x2="{px:.2f}" y2="{top + plot_h}" stroke="#efefef"/>')
        parts.append(f'<line x1="{px:.2f}" y1="{top + plot_h + mid_gap}" x2="{px:.2f}" y2="{top + plot_h + mid_gap + resid_h}" stroke="#efefef"/>')
        parts.append(f'<text class="small" x="{px:.2f}" y="{height - 18}" text-anchor="middle">{fmt(xt)}</text>')

    for yt in y_ticks:
        py = top + plot_h - (yt - y_min) / (y_max - y_min) * plot_h
        parts.append(f'<line x1="{left}" y1="{py:.2f}" x2="{left + plot_w}" y2="{py:.2f}" stroke="#f3f3f3"/>')
        parts.append(f'<text class="small" x="{left - 8}" y="{py + 4:.2f}" text-anchor="end">{fmt(yt)}</text>')

    for dt in d_ticks:
        py = top + plot_h + mid_gap + resid_h - (dt - d_min) / (d_max - d_min) * resid_h
        parts.append(f'<line x1="{left}" y1="{py:.2f}" x2="{left + plot_w}" y2="{py:.2f}" stroke="#f3f3f3"/>')
        parts.append(f'<text class="small" x="{left - 8}" y="{py + 4:.2f}" text-anchor="end">{fmt(dt)}</text>')

    parts.append(f'<rect x="{left}" y="{top}" width="{plot_w}" height="{plot_h}" fill="none" stroke="#bcbcbc"/>')
    parts.append(f'<rect x="{left}" y="{top + plot_h + mid_gap}" width="{plot_w}" height="{resid_h}" fill="none" stroke="#bcbcbc"/>')

    for idx, (name, points) in enumerate(p_contrib.items()):
        color = CONTRIB_COLORS[idx % len(CONTRIB_COLORS)]
        parts.append(line(points, color, width=1.1))

    parts.append(line(p_larch, "#2e6fdb", width=1.8, dash="5,4"))
    parts.append(line(p_model, "#cc3f3f", width=1.8))
    parts.append(line(p_diff, "#2a9d5b", width=1.4))

    parts.append(f'<text class="axis" x="{left + plot_w + 8}" y="{top + 14}">{y_label}</text>')
    parts.append(f'<text class="axis" x="{left + plot_w + 8}" y="{top + plot_h + mid_gap + 14}">diff</text>')
    parts.append(f'<text class="axis" x="{left + plot_w / 2}" y="{height - 4}" text-anchor="middle">{x_label}</text>')

    legend_x = left + 8
    legend_y = top + 18
    parts.append(f'<line x1="{legend_x}" y1="{legend_y}" x2="{legend_x + 26}" y2="{legend_y}" stroke="#cc3f3f" stroke-width="1.8"/>')
    parts.append(f'<text class="small" x="{legend_x + 32}" y="{legend_y + 4}">xraytsubaki total</text>')
    parts.append(f'<line x1="{legend_x + 170}" y1="{legend_y}" x2="{legend_x + 196}" y2="{legend_y}" stroke="#2e6fdb" stroke-width="1.8" stroke-dasharray="5,4"/>')
    parts.append(f'<text class="small" x="{legend_x + 202}" y="{legend_y + 4}">xraylarch total</text>')
    parts.append(f'<line x1="{legend_x + 330}" y1="{legend_y}" x2="{legend_x + 356}" y2="{legend_y}" stroke="#2a9d5b" stroke-width="1.4"/>')
    parts.append(f'<text class="small" x="{legend_x + 362}" y="{legend_y + 4}">diff</text>')

    if series.contributions:
        start_y = legend_y + 20
        for idx, name in enumerate(series.contributions.keys()):
            color = CONTRIB_COLORS[idx % len(CONTRIB_COLORS)]
            y = start_y + idx * 16
            parts.append(f'<line x1="{legend_x}" y1="{y}" x2="{legend_x + 18}" y2="{y}" stroke="{color}" stroke-width="1.2"/>')
            parts.append(f'<text class="small" x="{legend_x + 24}" y="{y + 4}">{name}</text>')

    parts.append("</svg>")

    out_path = PLOT_DIR / f"feff_vs_larch_{series.name}.svg"
    out_path.write_text("\n".join(parts))


def write_index(series_list: List[Series]) -> None:
    lines = ["# FEFF: xraytsubaki vs xraylarch", "", "Generated comparison plots:"]
    for s in series_list:
        lines.append(
            f"- `{s.name}`: `doc/plots/feff_vs_larch_{s.name}.svg`, `doc/plots/feff_vs_larch_{s.name}.png`"
        )
    (PLOT_DIR / "feff_vs_larch_index.md").write_text("\n".join(lines) + "\n")


def main() -> None:
    if not DATA_DIR.exists():
        raise SystemExit(f"missing data directory: {DATA_DIR}")
    os.makedirs(PLOT_DIR, exist_ok=True)
    series_paths = sorted(DATA_DIR.glob("*.csv"))
    if not series_paths:
        raise SystemExit("no comparison csv files found")
    series_list = [read_series(p) for p in series_paths]
    for series in series_list:
        write_plot(series)
    write_index(series_list)
    print(f"Generated {len(series_list)} plot(s) under {PLOT_DIR}")


if __name__ == "__main__":
    main()
