//! Display resampling is bounded and independent of native-grid region integrals.
use super::*;
use crate::app::series_display::HeatmapPalette;
use crate::plot_ranges::plot_builder;
use ruviz::{
    plots::heatmap::{HeatmapConfig, HeatmapOrigin},
    prelude::*,
};

/// Fixed physical margins align data rectangles, not just the outer widgets.
/// Reserve the same right margin below the map as its colorbar occupies above.
/// The rotated |χ(R)| marginal is only as wide as the map's left gutter, so
/// its x axis keeps a smaller font and fewer ticks to stop labels colliding.
pub(super) fn panel_plot(theme: &crate::theme::Theme, size: (u32, u32), rotated: bool) -> Plot {
    let plot = Plot::new()
        .theme(theme.plot_theme())
        .size_px(size.0, size.1)
        .font_size(if rotated { 8.5 } else { 10. });
    let plot = if rotated { plot.major_ticks_x(4) } else { plot };
    let mut config = plot.get_config().clone();
    config.margins = ruviz::core::config::MarginConfig::fixed(
        0.48,
        if rotated { 0.12 } else { 0.85 },
        0.12,
        0.42,
    );
    plot.plot_config(config)
}

/// These panels have static, resolved data bounds. Restore their viewport now,
/// before wiring the shared axes; waiting for an asynchronous first render can
/// let a fresh marginal reset a peer's zoom in the meantime.
fn replace_panel(plot_view: &mut RuvizPlot, plot: Plot, cx: &mut Context<RuvizPlot>) {
    let old = plot_view.interactive_session().view_bounds_snapshot();
    plot_view.set_plot_keep_view(plot, cx);
    if old.visible_bounds != old.base_bounds {
        plot_view
            .interactive_session()
            .restore_visible_bounds(old.visible_bounds);
    }
}

pub(crate) fn scale(map: &rexafs::WaveletMap, view: usize) -> (f64, f64) {
    if view == 3 {
        return (-std::f64::consts::PI, std::f64::consts::PI);
    }
    let maximum = map
        .real()
        .iter()
        .zip(map.imaginary())
        .map(|(&a, &b)| match view {
            1 => a.abs(),
            2 => b.abs(),
            _ => a.hypot(b),
        })
        .fold(0., f64::max);
    let maximum = if maximum == 0. { 1. } else { maximum };
    (if view == 0 { 0. } else { -maximum }, maximum)
}
fn bracket(axis: &[f64], x: f64) -> (usize, f64) {
    let i = axis
        .partition_point(|v| *v <= x)
        .saturating_sub(1)
        .min(axis.len() - 2);
    (i, ((x - axis[i]) / (axis[i + 1] - axis[i])).clamp(0., 1.))
}
/// Uniform physical centers, with bilinear interpolation on the scientific grid.
/// This also respects maps with deliberately nonuniform reference R coordinates.
pub(crate) fn texture(map: &rexafs::WaveletMap, view: usize) -> (Vec<Vec<f64>>, [f64; 4]) {
    let nx = map.k().len().clamp(2, 512);
    let ny = map.r().len().clamp(2, 256);
    let [ka, kb] = map.k_support();
    let ra = map.r()[0];
    let rb = *map.r().last().unwrap();
    let dx = (kb - ka) / (nx - 1) as f64;
    let dy = (rb - ra) / (ny - 1) as f64;
    let max = scale(map, 0).1;
    let nk = map.k().len();
    let rows = (0..ny)
        .map(|y| {
            let (r, b) = bracket(map.r(), ra + y as f64 * dy);
            (0..nx)
                .map(|x| {
                    let (k, a) = bracket(map.k(), ka + x as f64 * dx);
                    let samples = [
                        (r * nk + k, (1. - a) * (1. - b)),
                        (r * nk + k + 1, a * (1. - b)),
                        ((r + 1) * nk + k, (1. - a) * b),
                        ((r + 1) * nk + k + 1, a * b),
                    ];
                    let blend = |component: usize| {
                        samples
                            .iter()
                            .map(|&(i, w)| {
                                w * match component {
                                    1 => map.real()[i],
                                    2 => map.imaginary()[i],
                                    _ => map.real()[i].hypot(map.imaginary()[i]),
                                }
                            })
                            .sum::<f64>()
                    };
                    if view == 3 {
                        let re = blend(1);
                        let im = blend(2);
                        if re.hypot(im) < max * 0.01 || re.hypot(im) == 0. {
                            f64::NAN
                        } else {
                            im.atan2(re)
                        }
                    } else {
                        blend(view)
                    }
                })
                .collect()
        })
        .collect();
    (
        rows,
        [ka - dx / 2., kb + dx / 2., ra - dy / 2., rb + dy / 2.],
    )
}
impl StudioApp {
    pub(super) fn rebuild_wavelet_plots(&mut self, cx: &mut Context<Self>) {
        let Some(record) = self.wavelet.record.clone() else {
            return;
        };
        self.wavelet.view_links.clear();
        let layout = layout::JointLayout::new(self.wavelet.layout_size.unwrap_or((680, 500)));
        let map = &record.map;
        let (matrix, extent) = texture(map, self.wavelet.view);
        let (lo, hi) = self
            .wavelet
            .lock_scale
            .unwrap_or_else(|| scale(map, self.wavelet.view));
        let palette = self.wavelet.palette.unwrap_or(if self.wavelet.view == 0 {
            HeatmapPalette::Viridis
        } else {
            HeatmapPalette::Coolwarm
        });
        let config = HeatmapConfig::new()
            .colorbar(true)
            .cmap(palette.map(self.wavelet.reversed))
            .origin(HeatmapOrigin::Lower)
            .extent(extent[0], extent[1], extent[2], extent[3])
            .vmin(lo)
            .vmax(hi);
        let plot: Plot = if matrix.iter().flatten().any(|v| v.is_finite()) {
            panel_plot(&self.theme, layout.map, false)
                .ticks(false)
                .heatmap_with(&matrix, config)
                .xlim(map.k_support()[0], map.k_support()[1])
                .ylim(map.r()[0], *map.r().last().unwrap())
                .into()
        } else {
            // All phase samples can be masked. Draw empty physical axes,
            // never fabricate a phase value just to satisfy heatmap bounds.
            panel_plot(&self.theme, layout.map, false)
                .ticks(false)
                .line(&map.k_support(), &[map.r()[0], *map.r().last().unwrap()])
                .color(Color::TRANSPARENT)
                .xlim(map.k_support()[0], map.k_support()[1])
                .ylim(map.r()[0], *map.r().last().unwrap())
                .into()
        };
        if let Some(entity) = &self.wavelet.plot_map {
            entity.update(cx, |p, cx| replace_panel(p, plot, cx));
        } else {
            let entity = plot_builder(plot)
                .interactive()
                .interaction_options(ruviz_gpui::InteractionOptions {
                    tooltips: false,
                    selection: false,
                    ..Default::default()
                })
                .build(cx);
            cx.subscribe(
                &entity,
                |app, _, _: &crate::plot_ranges::RangesChanged, cx| app.link_wavelet_axes(0, cx),
            )
            .detach();
            self.wavelet.subscription = Some(cx.subscribe(
                &entity,
                |app, _, event: &PlotPointerEvent, cx| {
                    if event.kind == PlotPointerEventKind::Click
                        && event.mouse_button == Some(gpui::MouseButton::Left)
                        && let Some(position) = event.data_position
                    {
                        let (k, r) = (position.x, position.y);
                        let Some(record) = &app.wavelet.record else {
                            return;
                        };
                        let bounds = record.map.k_support();
                        app.wavelet.cursor = [
                            k.clamp(bounds[0], bounds[1]),
                            r.clamp(record.map.r()[0], *record.map.r().last().unwrap()),
                        ];
                        if !app.wavelet.slices {
                            app.wavelet.plot_k = None;
                            app.wavelet.plot_r = None;
                        }
                        app.wavelet.slices = true;
                        app.rebuild_wavelet_slices(cx);
                        cx.notify();
                    }
                },
            ));
            self.wavelet.plot_map = Some(entity);
        }
        self.rebuild_wavelet_slices(cx);
    }
    pub(super) fn wavelet_cursor_value(&self) -> Option<f64> {
        let record = self.wavelet.record.as_ref()?;
        let values = record.map.slice_at_r(self.wavelet.cursor[1]).ok()?;
        let (i, a) = bracket(record.map.k(), self.wavelet.cursor[0]);
        Some(values[i] * (1. - a) + values[i + 1] * a)
    }
    /// Slice cursor follows the physical viewport; region measurements live in Series.
    pub(super) fn wavelet_map_overlay(&self) -> Option<gpui::AnyElement> {
        if !self.wavelet.slices {
            return None;
        }
        let entity = self.wavelet.plot_map.clone()?;
        let cursor = self.wavelet.cursor;
        Some(
            gpui::canvas(
                move |_, _, cx| {
                    entity
                        .read(cx)
                        .screen_at(ruviz::core::ViewportPoint {
                            x: cursor[0],
                            y: cursor[1],
                        })
                        .ok()
                        .flatten()
                },
                move |_, p, window, _| {
                    if let Some(p) = p {
                        use gpui::{Bounds, fill, point, size};
                        for bounds in [
                            Bounds::new(point(p.x - px(5.), p.y - px(0.7)), size(px(10.), px(1.4))),
                            Bounds::new(point(p.x - px(0.7), p.y - px(5.)), size(px(1.4), px(10.))),
                        ] {
                            window.paint_quad(fill(bounds, gpui::white()));
                        }
                    }
                },
            )
            .absolute()
            .inset_0()
            .into_any_element(),
        )
    }
    pub(super) fn rebuild_wavelet_slices(&mut self, cx: &mut Context<Self>) {
        self.wavelet.view_links.clear();
        let Some(record) = &self.wavelet.record else {
            return;
        };
        let layout = layout::JointLayout::new(self.wavelet.layout_size.unwrap_or((680, 500)));
        let map = &record.map;
        let (kx, ky, rx, ry) = if self.wavelet.slices {
            (
                map.k().to_vec(),
                map.slice_at_r(self.wavelet.cursor[1]).unwrap_or_default(),
                map.r().to_vec(),
                map.slice_at_k(self.wavelet.cursor[0]).unwrap_or_default(),
            )
        } else {
            let (r, y) = record
                .fourier
                .as_ref()
                .and_then(|f| f.r.as_ref().zip(f.chir_mag.as_ref()))
                .map(|(r, y)| (r.as_slice().to_vec(), y.as_slice().to_vec()))
                .unwrap_or_default();
            (
                map.input_k().to_vec(),
                map.input_k()
                    .iter()
                    .zip(map.input_chi())
                    .map(|(k, y)| k.powi(map.settings().kweight as i32) * y)
                    .collect(),
                r,
                y,
            )
        };
        let mut kp: Plot = panel_plot(&self.theme, (layout.map.0, layout.bottom), false)
            .line(&kx, &ky)
            .color(crate::plotting::trace_color(&self.theme, 0))
            .xlabel("k (Å⁻¹)")
            .ylabel(if self.wavelet.slices {
                "|W|"
            } else {
                "k-weighted χ"
            })
            .xlim(map.k_support()[0], map.k_support()[1])
            .into();
        if !self.wavelet.slices {
            let (lo, hi) = crate::plotting::symmetric_y_limits(ky.iter().copied());
            kp = kp.ylim(lo, hi);
        }
        let amplitude = ry
            .iter()
            .copied()
            .filter(|v| v.is_finite())
            .fold(0., f64::max)
            .max(1e-12);
        let rp: Plot = panel_plot(&self.theme, (layout.left, layout.map.1), true)
            .line(&ry, &rx)
            .color(crate::plotting::trace_color(&self.theme, 0))
            .ylabel("R (Å)")
            .xlabel(if self.wavelet.slices {
                "|W|"
            } else {
                "|χ(R)|"
            })
            .xlim(amplitude * 1.05, 0.)
            .ylim(map.r()[0], *map.r().last().unwrap())
            .into();
        for (source, (slot, plot)) in [
            (&mut self.wavelet.plot_k, kp),
            (&mut self.wavelet.plot_r, rp),
        ]
        .into_iter()
        .enumerate()
        {
            if let Some(entity) = slot {
                entity.update(cx, |p, cx| replace_panel(p, plot, cx));
            } else {
                let entity = plot_builder(plot).interactive().build(cx);
                cx.subscribe(
                    &entity,
                    move |app, _, _: &crate::plot_ranges::RangesChanged, cx| {
                        app.link_wavelet_axes(source + 1, cx)
                    },
                )
                .detach();
                *slot = Some(entity);
            }
        }
        self.link_wavelet_axes(0, cx);
        cx.notify();
    }

    fn link_wavelet_axes(&mut self, source: usize, cx: &mut Context<Self>) {
        self.wavelet.view_links.clear();
        if let (Some(map), Some(k), Some(r)) = (
            &self.wavelet.plot_map,
            &self.wavelet.plot_k,
            &self.wavelet.plot_r,
        ) {
            self.wavelet.view_links = layout::link_views(
                map.read(cx).interactive_session().clone(),
                k.read(cx).interactive_session().clone(),
                r.read(cx).interactive_session().clone(),
                source,
            );
        }
        cx.notify();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn display_resampling_uses_physical_centers_and_keeps_native_integral() {
        let k = (0..401).map(|i| i as f64 * 0.025).collect::<Vec<_>>();
        let chi = k
            .iter()
            .map(|v| (4.6 * v).sin() * (-((v - 6.) / 2.).powi(2)).exp())
            .collect::<Vec<_>>();
        let map = rexafs::Wavelet::new(1. ..=9.)
            .kstep(0.025)
            .radii(vec![0.1, 0.2, 1., 2., 2.3, 3., 5.])
            .calculate(&k, &chi)
            .unwrap();
        let expected = map.integral(2. ..=8., 1. ..=3.).unwrap();
        let (image, extent) = texture(&map, 0);
        assert_eq!(image.len(), 7);
        assert_eq!(image[0].len(), 401);
        let dx = (extent[1] - extent[0]) / image[0].len() as f64;
        let dy = (extent[3] - extent[2]) / image.len() as f64;
        assert!((extent[0] + dx / 2. - 1.).abs() < 1e-12);
        assert!((extent[2] + dy / 2. - 0.1).abs() < 1e-12);
        let at_min = map.slice_at_r(0.1).unwrap();
        assert_eq!(image[0][0], at_min[40]);
        for view in 0..4 {
            let _ = texture(&map, view);
        }
        assert_eq!(map.integral(2. ..=8., 1. ..=3.).unwrap(), expected);
        let (lo, hi) = scale(&map, 1);
        assert_eq!(lo, -hi);
    }
}
