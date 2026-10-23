//! Wavelet is a signal choice in Add trend, with a frozen Transform definition.
//! Only one preview map is resident; workers retain scalar results for all frames.
use super::*;
use crate::app::shell::button;
use gpui::{IntoElement, ParentElement, Styled, div, prelude::*, px};
use ruviz::core::ViewportPoint;

pub(super) struct WaveletPreview {
    map: Arc<rexafs::WaveletMap>,
    definition: WaveletTrend,
    label: String,
}
impl StudioApp {
    pub(super) fn choose_wavelet_trend(&mut self, cx: &mut Context<Self>) {
        self.ensure_wavelet_fields(cx);
        let transform = match self.wavelet_definition(cx) {
            Ok(t) => t,
            Err(e) => {
                self.measurements.message = e;
                cx.notify();
                return;
            }
        };
        let [a, b] = transform.k_range;
        let rmax = transform
            .radii
            .as_ref()
            .and_then(|r| r.last().copied())
            .unwrap_or(transform.rmax);
        let k_range = [a + (b - a) * 0.2, a + (b - a) * 0.8];
        let r_range = [1_f64.min(rmax * 0.25), 3_f64.min(rmax * 0.75)];
        let old = self.measurements.wavelet.as_ref();
        self.measurements.wavelet = Some(WaveletTrend {
            transform,
            statistic: WaveletStatistic::Integral,
            k_range: old
                .filter(|w| w.k_range[0] >= a && w.k_range[1] <= b)
                .map(|w| w.k_range)
                .unwrap_or(k_range),
            r_range: old
                .filter(|w| w.r_range[1] <= rmax)
                .map(|w| w.r_range)
                .unwrap_or(r_range),
        });
        if !matches!(self.measurements.kind, 1..=3) {
            self.measurements.kind = 2;
        }
        self.measurements.selected_recipe = None;
        self.measurements.wavelet_fields.clear();
        self.ensure_wavelet_trend_fields(cx);
        self.measurements.message.clear();
        self.preview_measurement(cx);
    }
    pub(super) fn ensure_wavelet_trend_fields(&mut self, cx: &mut Context<Self>) {
        if !self.measurements.wavelet_fields.is_empty() {
            return;
        }
        let Some(w) = &self.measurements.wavelet else {
            return;
        };
        let values = [w.k_range[0], w.k_range[1], w.r_range[0], w.r_range[1]];
        for (label, value) in ["k from (Å⁻¹)", "k to (Å⁻¹)", "R from (Å)", "R to (Å)"]
            .into_iter()
            .zip(values)
        {
            let f = cx.new(|cx| {
                NumericField::new(
                    label,
                    "required",
                    Some(value),
                    FieldKind::Float,
                    self.theme,
                    cx,
                )
            });
            cx.subscribe(
                &f,
                |app, _, _: &crate::widgets::numeric_field::FieldEvent, cx| {
                    app.update_wavelet_trend_region(cx);
                },
            )
            .detach();
            self.measurements.wavelet_fields.push(f);
        }
    }
    pub(super) fn wavelet_trend_definition(
        &self,
        cx: &Context<Self>,
    ) -> Result<Option<WaveletTrend>, String> {
        let Some(mut w) = self.measurements.wavelet.clone() else {
            return Ok(None);
        };
        w.statistic = match self.measurements.kind {
            1 => WaveletStatistic::Maximum,
            2 => WaveletStatistic::Integral,
            3 => WaveletStatistic::Mean,
            _ => return Err("Choose integral, maximum or mean for a wavelet region".into()),
        };
        if self.measurements.wavelet_fields.len() == 4 {
            let v = self
                .measurements
                .wavelet_fields
                .iter()
                .map(|f| {
                    f.read(cx)
                        .value()
                        .ok_or("Enter all four region bounds".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            w.k_range = [v[0], v[1]];
            w.r_range = [v[2], v[3]];
        }
        w.validate()?;
        Ok(Some(w))
    }
    pub(super) fn update_wavelet_trend_region(&mut self, cx: &mut Context<Self>) {
        let result = self.wavelet_trend_definition(cx);
        match result {
            Ok(Some(w)) => {
                self.measurements.wavelet = Some(w.clone());
                if let Some(p) = &mut self.measurements.wavelet_preview {
                    p.definition = w;
                    self.measurements.preview_label = match p.definition.measure(&p.map) {
                        Ok(v) => format!("{} · {:.6} {}", p.label, v.value, v.unit),
                        Err(e) => e,
                    };
                }
            }
            Err(e) => self.measurements.preview_label = e,
            _ => {}
        }
        cx.notify();
    }
    pub(super) fn wavelet_trend_controls(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        self.ensure_wavelet_trend_fields(cx);
        let t = self.theme;
        let mut rows = div().flex().flex_col().gap_1();
        for fields in self.measurements.wavelet_fields.chunks(2) {
            let mut row = div().flex().flex_wrap().items_center().gap_2();
            for field in fields {
                row = row.child(div().w(px(290.)).child(field.clone()));
            }
            rows = rows.child(row);
        }
        let description = self
            .measurements
            .wavelet
            .as_ref()
            .map(|w| {
                format!(
                    "Transform: k {:.2}–{:.2} Å⁻¹ · weight {} · order {}",
                    w.transform.k_range[0],
                    w.transform.k_range[1],
                    w.transform.kweight,
                    w.transform.order
                )
            })
            .unwrap_or_default();
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(rows)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.text_muted)
                            .child(description),
                    )
                    .child(
                        button(&t, "wavelet-trend-sync", "Use Transform settings", false)
                            .on_click(cx.listener(|app, _, _, cx| app.choose_wavelet_trend(cx))),
                    )
                    .child(self.plot_ranges_button(cx)),
            )
            .into_any_element()
    }
    pub(super) fn queue_wavelet_trend_preview(
        &mut self,
        input: FrameInput,
        definition: MetricDefinition,
        expected: Option<String>,
        cx: &mut Context<Self>,
    ) {
        self.clear_measurement_handles();
        self.measurements.preview_data = None;
        self.measurements.wavelet_preview = None;
        self.measurements.wavelet_drag = None;
        self.measurements.wavelet_armed = None;
        self.measurements.preview_generation += 1;
        let request = self.measurements.preview_generation;
        let generation = self.project_generation;
        self.measurements.preview = None;
        self.measurements.preview_label = format!("Preparing {}…", input.label);
        cx.spawn(async move |this, cx| {
            let result = cx
                .background_spawn(async move {
                    let (sp, _, _) = input.prepare(&definition, expected.as_deref())?;
                    let w = definition.wavelet.ok_or("Wavelet definition missing")?;
                    let map = sp.wavelet(&w.transform).map_err(|e| e.to_string())?;
                    if map.r().len() < 2 {
                        return Err("The preview needs at least two R rows; increase R maximum or reduce R step in Process › Transform".into());
                    }
                    Ok::<_, String>((input.label, w, Arc::new(map)))
                })
                .await;
            this.update(cx, |app, cx| {
                if app.project_generation != generation
                    || app.measurements.preview_generation != request
                {
                    return;
                }
                match result {
                    Ok((label, mut w, map)) => {
                        // Bounds may have been edited while the transform ran.
                        // Retain the latest region, but never reinterpret a saved result.
                        if !app.measurements.results {
                            if let Ok(Some(current)) = app.wavelet_trend_definition(cx) {
                                if current.transform == w.transform {
                                    w = current;
                                }
                            }
                        }
                        let (matrix, extent) = crate::app::shell::wavelet::plots::texture(&map, 0);
                        let plot: Plot = Plot::new()
                            .theme(app.theme.plot_theme())
                            .size_px(780, 360)
                            .heatmap_with(
                                &matrix,
                                ruviz::plots::heatmap::HeatmapConfig::new()
                                    .colorbar(true)
                                    .origin(ruviz::plots::heatmap::HeatmapOrigin::Lower)
                                    .extent(extent[0], extent[1], extent[2], extent[3])
                                    .vmin(0.)
                                    .vmax(crate::app::shell::wavelet::plots::scale(&map, 0).1),
                            )
                            .xlabel("k (Å⁻¹)")
                            .ylabel("R (Å; uncorrected)")
                            .xlim(map.k_support()[0], map.k_support()[1])
                            .ylim(map.r()[0], *map.r().last().unwrap())
                            .into();
                        app.measurements.preview = Some(
                            plot_builder(plot)
                                .interactive()
                                .interaction_options(ruviz_gpui::InteractionOptions {
                                    tooltips: false,
                                    selection: false,
                                    ..Default::default()
                                })
                                .build(cx),
                        );
                        app.measurements.preview_label = match w.measure(&map) {
                            Ok(value) => format!("{label} · {:.6} {}", value.value, value.unit),
                            Err(e) => e,
                        };
                        app.measurements.wavelet_preview = Some(WaveletPreview {
                            map,
                            definition: w,
                            label,
                        });
                    }
                    Err(e) => app.measurements.preview_label = e,
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
        cx.notify();
    }
    fn wavelet_trend_pointer(
        &mut self,
        position: gpui::Point<gpui::Pixels>,
        cx: &mut Context<Self>,
    ) {
        if self.handles.hidden || self.measurements.results || self.measurements.cancel.is_some() {
            self.measurements.wavelet_armed = None;
            self.measurements.wavelet_drag = None;
            return;
        }
        let (Some(p), Some(plot)) = (
            &self.measurements.wavelet_preview,
            &self.measurements.preview,
        ) else {
            return;
        };
        let plot = plot.read(cx);
        if plot.is_context_menu_open() {
            self.measurements.wavelet_armed = None;
            return;
        }
        if let Some(edge) = self.measurements.wavelet_drag {
            if let Ok(Some(point)) = plot.data_at(position) {
                let mut w = p.definition.clone();
                let (range, x, lo, hi) = if edge < 2 {
                    (
                        &mut w.k_range,
                        point.x,
                        p.map.k_support()[0],
                        p.map.k_support()[1],
                    )
                } else {
                    (
                        &mut w.r_range,
                        point.y,
                        p.map.r()[0],
                        *p.map.r().last().unwrap(),
                    )
                };
                let x = ((x * 100.).round() / 100.)
                    .clamp((lo * 100.).ceil() / 100., (hi * 100.).floor() / 100.);
                let i = edge % 2;
                if (i == 0 && x >= range[1]) || (i == 1 && x <= range[0]) {
                    return;
                }
                range[i] = x;
                self.measurements.wavelet_fields[edge].update(cx, |f, cx| f.set_value(Some(x), cx));
                self.update_wavelet_trend_region(cx);
            }
            return;
        }
        let Some(rect) = region_rect(plot, &p.definition) else {
            return;
        };
        let Ok(view) = plot.interactive_session().viewport_snapshot() else {
            return;
        };
        let b = view.visible_bounds;
        let visible = [
            p.definition.k_range[0] >= b.min.x,
            p.definition.k_range[1] <= b.max.x,
            p.definition.r_range[0] >= b.min.y,
            p.definition.r_range[1] <= b.max.y,
        ];
        let x = f32::from(position.x);
        let y = f32::from(position.y);
        let distances = [
            (x - rect[0]).abs(),
            (x - rect[1]).abs(),
            (y - rect[3]).abs(),
            (y - rect[2]).abs(),
        ];
        let armed = distances
            .into_iter()
            .enumerate()
            .filter(|(i, d)| {
                visible[*i]
                    && *d < 8.
                    && if *i < 2 {
                        y >= rect[2] - 8. && y <= rect[3] + 8.
                    } else {
                        x >= rect[0] - 8. && x <= rect[1] + 8.
                    }
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(i, _)| i);
        if armed != self.measurements.wavelet_armed {
            self.measurements.wavelet_armed = armed;
            cx.notify();
        }
    }
    pub(super) fn wavelet_trend_plot(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let Some(plot) = self.measurements.preview.clone() else {
            return div().into_any_element();
        };
        let region = self
            .measurements
            .wavelet_preview
            .as_ref()
            .map(|p| p.definition.clone());
        let hidden = self.handles.hidden;
        let color: gpui::Hsla = self.theme.accent.into();
        let entity = plot.clone();
        let overlay = gpui::canvas(
            move |_, _, cx| {
                if entity.read(cx).is_context_menu_open() {
                    return None;
                }
                region
                    .as_ref()
                    .and_then(|w| region_rect(entity.read(cx), w))
            },
            move |_, rect, window, _| {
                if hidden {
                    return;
                }
                let Some([left, right, top, bottom]) = rect else {
                    return;
                };
                use gpui::{Bounds, fill, point, size};
                let mut shade = color;
                shade.a = 0.1;
                window.paint_quad(fill(
                    Bounds::new(
                        point(px(left), px(top)),
                        size(px(right - left), px(bottom - top)),
                    ),
                    shade,
                ));
                for (x, y, w, h) in [
                    (left, top, right - left, 1.5),
                    (left, bottom, right - left, 1.5),
                    (left, top, 1.5, bottom - top),
                    (right, top, 1.5, bottom - top),
                ] {
                    window.paint_quad(fill(
                        Bounds::new(point(px(x), px(y)), size(px(w), px(h))),
                        color,
                    ));
                }
                for (x, y) in [
                    (left, (top + bottom) / 2.),
                    (right, (top + bottom) / 2.),
                    ((left + right) / 2., top),
                    ((left + right) / 2., bottom),
                ] {
                    window.paint_quad(fill(
                        Bounds::new(point(px(x - 3.), px(y - 3.)), size(px(6.), px(6.))),
                        color,
                    ));
                }
            },
        )
        .absolute()
        .inset_0();
        let active = !hidden
            && !self.measurements.results
            && self.measurements.cancel.is_none()
            && (self.measurements.wavelet_armed.is_some()
                || self.measurements.wavelet_drag.is_some());
        div()
            .id("wavelet-trend-preview")
            .h(px(360.))
            .min_h(px(360.))
            .w_full()
            .relative()
            .on_mouse_move(cx.listener(|app, e: &gpui::MouseMoveEvent, _, cx| {
                app.wavelet_trend_pointer(e.position, cx)
            }))
            .capture_any_mouse_down(cx.listener(|app, e: &gpui::MouseDownEvent, _, cx| {
                if e.button == gpui::MouseButton::Left {
                    app.wavelet_trend_pointer(e.position, cx);
                    if app.measurements.wavelet_armed.is_some() {
                        app.measurements.wavelet_drag = app.measurements.wavelet_armed;
                        cx.stop_propagation();
                        cx.notify();
                    }
                } else {
                    app.measurements.wavelet_armed = None;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                gpui::MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    app.measurements.wavelet_drag = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                gpui::MouseButton::Left,
                cx.listener(|app, _, _, cx| {
                    app.measurements.wavelet_drag = None;
                    cx.notify();
                }),
            )
            .child(plot)
            .child(overlay)
            .when(active, |d| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .cursor(
                            if self
                                .measurements
                                .wavelet_drag
                                .or(self.measurements.wavelet_armed)
                                .unwrap_or(0)
                                < 2
                            {
                                gpui::CursorStyle::ResizeLeftRight
                            } else {
                                gpui::CursorStyle::ResizeUpDown
                            },
                        )
                        .on_mouse_down(
                            gpui::MouseButton::Left,
                            cx.listener(|_, _, _, cx| cx.stop_propagation()),
                        ),
                )
            })
            .into_any_element()
    }
}
fn region_rect(plot: &RuvizPlot, w: &WaveletTrend) -> Option<[f32; 4]> {
    let b = plot
        .interactive_session()
        .viewport_snapshot()
        .ok()?
        .visible_bounds;
    let [ka, kb] = w.k_range;
    let [ra, rb] = w.r_range;
    if kb < b.min.x || ka > b.max.x || rb < b.min.y || ra > b.max.y {
        return None;
    }
    let a = plot
        .screen_at(ViewportPoint {
            x: ka.clamp(b.min.x + 1e-9, b.max.x - 1e-9),
            y: ra.clamp(b.min.y + 1e-9, b.max.y - 1e-9),
        })
        .ok()??;
    let z = plot
        .screen_at(ViewportPoint {
            x: kb.clamp(b.min.x + 1e-9, b.max.x - 1e-9),
            y: rb.clamp(b.min.y + 1e-9, b.max.y - 1e-9),
        })
        .ok()??;
    Some([
        f32::from(a.x),
        f32::from(z.x),
        f32::from(z.y),
        f32::from(a.y),
    ])
}
