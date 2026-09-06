//! Ripple-strip thumbnails: tiny sparkline canvases of the current group
//! (norm μ(E) → k^wχ(k) → windowed k^wχ(k) → |χ(R)|). Drawn with GPUI paths
//! rather than full ruviz plots so the curve fills the card edge to edge.

use std::sync::Arc;

use gpui::{Bounds, IntoElement, Pixels, Styled, canvas, fill, point, px};
use rexafs::prelude::XASSpectrum;

use crate::theme::Theme;

/// One polyline in data coordinates.
pub struct ThumbSeries {
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub muted: bool,
}

pub struct ThumbData {
    pub series: Vec<ThumbSeries>,
    pub xlim: Option<(f64, f64)>,
    /// Shaded x-range (e.g. the fit R-range on |χ(R)|).
    pub span: Option<(f64, f64)>,
}

fn vecs(v: &nalgebra::DVector<f64>) -> Vec<f64> {
    v.iter().copied().collect()
}

/// Thumbnail data for the four downstream views of one spectrum.
pub fn thumb_data(sp: &XASSpectrum, _fit_r: (f64, f64)) -> [ThumbData; 4] {
    let energy = sp.energy.as_ref().map(vecs).unwrap_or_default();
    let norm = sp
        .norm()
        .or_else(|| sp.flat())
        .map(|v| vecs(&v))
        .unwrap_or_default();
    let k = sp
        .k()
        .map(nalgebra::DVector::from_column_slice)
        .map(|v| vecs(&v))
        .unwrap_or_default();
    let chi = sp.chi_kweighted().map(|v| vecs(&v)).unwrap_or_default();
    let r = sp.r().map(|v| vecs(&v)).unwrap_or_default();
    let mag = sp.chir_mag().map(|v| vecs(&v)).unwrap_or_default();
    let mut win_series = vec![ThumbSeries {
        x: k.clone(),
        y: chi.clone(),
        muted: false,
    }];
    if let Some(kwin) = sp.kwin() {
        let peak = chi.iter().fold(0.0f64, |m, v| m.max(v.abs())).max(1e-12);
        let n = k.len().min(kwin.len());
        win_series.push(ThumbSeries {
            x: k[..n].to_vec(),
            y: kwin.iter().take(n).map(|w| w * peak).collect(),
            muted: true,
        });
    }
    [
        ThumbData {
            series: vec![ThumbSeries {
                x: energy,
                y: norm,
                muted: false,
            }],
            xlim: None,
            span: None,
        },
        ThumbData {
            series: vec![ThumbSeries {
                x: k.clone(),
                y: chi.clone(),
                muted: false,
            }],
            xlim: None,
            span: None,
        },
        ThumbData {
            series: win_series,
            xlim: None,
            span: None,
        },
        ThumbData {
            series: vec![ThumbSeries {
                x: r,
                y: mag,
                muted: false,
            }],
            xlim: None,
            span: None,
        },
    ]
}

/// A sparkline canvas that fills its container.
pub fn sparkline(all: Arc<[ThumbData; 4]>, index: usize, theme: Theme) -> impl IntoElement {
    canvas(
        move |bounds, _window, _cx| bounds,
        move |bounds: Bounds<Pixels>, _state, window, _cx| {
            let data = &all[index];
            let pad = px(6.);
            let x0 = bounds.origin.x + pad;
            let y0 = bounds.origin.y + pad;
            let w = (bounds.size.width - pad * 2.).max(px(1.));
            let h = (bounds.size.height - pad * 2.).max(px(1.));
            // Data extent.
            let (xmin, xmax) = data.xlim.unwrap_or_else(|| {
                let mut lo = f64::INFINITY;
                let mut hi = f64::NEG_INFINITY;
                for s in &data.series {
                    for v in &s.x {
                        lo = lo.min(*v);
                        hi = hi.max(*v);
                    }
                }
                (lo, hi)
            });
            let mut ymin = f64::INFINITY;
            let mut ymax = f64::NEG_INFINITY;
            for s in &data.series {
                for (x, y) in s.x.iter().zip(&s.y) {
                    if *x >= xmin && *x <= xmax && y.is_finite() {
                        ymin = ymin.min(*y);
                        ymax = ymax.max(*y);
                    }
                }
            }
            if xmax <= xmin || ymax <= ymin {
                return;
            }
            let ypad = (ymax - ymin) * 0.06;
            let (ymin, ymax) = (ymin - ypad, ymax + ypad);
            let sx = |x: f64| x0 + w * (((x - xmin) / (xmax - xmin)) as f32);
            let sy = |y: f64| y0 + h * ((1.0 - (y - ymin) / (ymax - ymin)) as f32);
            if let Some((a, b)) = data.span {
                let (a, b) = (a.max(xmin), b.min(xmax));
                if b > a {
                    let left = sx(a);
                    let right = sx(b);
                    window.paint_quad(fill(
                        Bounds::from_corners(point(left, y0), point(right, y0 + h)),
                        gpui::Rgba {
                            a: 0.14,
                            ..theme.accent
                        },
                    ));
                }
            }
            for s in &data.series {
                let mut builder = gpui::PathBuilder::stroke(px(if s.muted { 1.0 } else { 1.3 }));
                let mut started = false;
                for (x, y) in s.x.iter().zip(&s.y) {
                    if *x < xmin || *x > xmax || !y.is_finite() {
                        continue;
                    }
                    let p = point(sx(*x), sy(*y));
                    if started {
                        builder.line_to(p);
                    } else {
                        builder.move_to(p);
                        started = true;
                    }
                }
                if let Ok(path) = builder.build() {
                    let color = if s.muted {
                        theme.text_muted
                    } else {
                        crate::plotting::trace_rgba(&theme, 0)
                    };
                    window.paint_path(path, color);
                }
            }
        },
    )
    .size_full()
}
