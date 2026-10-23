//! Compact wavelet controls; display choices never change retained numerical maps.
use super::*;
use crate::app::series_display::HeatmapPalette;
impl StudioApp {
    pub(crate) fn wavelet_toolbar_actions(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                super::super::controls::icon_button(
                    &t,
                    "wavelet-refresh",
                    crate::icons::Icon::Refresh,
                    "Refresh wavelet from source",
                    false,
                )
                .on_click(cx.listener(|app, _, _, cx| app.schedule_wavelet(cx))),
            )
            .child(
                button(&t, "wavelet-colors", "Colors ▾", self.wavelet.colors_open).on_click(
                    cx.listener(|app, event: &gpui::ClickEvent, window, cx| {
                        app.wavelet.colors_open = !app.wavelet.colors_open;
                        app.wavelet.menu_position = event.position();
                        let focus = cx.focus_handle();
                        focus.focus(window, cx);
                        app.wavelet.menu_focus = Some(focus);
                        cx.notify();
                    }),
                ),
            )
            .child(self.plot_export_button(
                "wavelet-export",
                super::super::plot_export::Target::Wavelet,
                cx,
            ))
    }

    pub(crate) fn wavelet_view_selector(&self, cx: &mut Context<Self>) -> gpui::Div {
        let t = self.theme;
        let mut views = segmented(&t).flex_none();
        for (i, label) in ["Magnitude", "Real", "Imaginary", "Phase"]
            .into_iter()
            .enumerate()
        {
            views = views.child(
                segment(
                    &t,
                    format!("wavelet-view-{i}"),
                    label,
                    self.wavelet.view == i,
                    i == 0,
                )
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.wavelet.view = i;
                    app.wavelet.lock_scale = None;
                    app.wavelet.lock_basis = None;
                    app.rebuild_wavelet_plots(cx);
                    cx.notify();
                })),
            );
        }
        views
    }

    pub(crate) fn wavelet_center(&mut self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let mut body = div()
            .flex_1()
            .min_w_0()
            .min_h_0()
            .p_3()
            .flex()
            .flex_col()
            .gap_2();
        let mut linked = div().flex().flex_none().min_w_0().items_center().gap_1();
        for (slices, label) in [(false, "Spectra"), (true, "Slices")] {
            linked = linked.child(
                button(
                    &t,
                    format!("wavelet-linked-{slices}"),
                    label,
                    self.wavelet.slices == slices,
                )
                .flex_none()
                .w(px(78.))
                .justify_center()
                .on_click(cx.listener(move |app, _, _, cx| {
                    if app.wavelet.slices != slices {
                        app.wavelet.plot_k = None;
                        app.wavelet.plot_r = None;
                    }
                    app.wavelet.slices = slices;
                    app.rebuild_wavelet_slices(cx);
                    cx.notify();
                })),
            );
        }
        let caption = if !self.wavelet.message.is_empty() {
            self.wavelet.message.clone()
        } else if self.wavelet.slices {
            format!(
                "R = {:.2} Å · k = {:.2} Å⁻¹ · |W| = {:.4}",
                self.wavelet.cursor[1],
                self.wavelet.cursor[0],
                self.wavelet_cursor_value().unwrap_or(0.)
            )
        } else {
            self.wavelet
                .record
                .as_ref()
                .and_then(|r| r.fourier.as_ref())
                .map(|f| {
                    format!(
                        "Original χ · Fourier {} window",
                        crate::params::window_label(f.window)
                    )
                })
                .unwrap_or_else(|| "Click the map to inspect slices".into())
        };
        linked = linked.child(
            div()
                .id("wavelet-status")
                .tooltip({
                    let label: SharedString = caption.clone().into();
                    move |_, cx| {
                        cx.new(|_| super::super::controls::Tooltip {
                            label: label.clone(),
                            theme: t,
                        })
                        .into()
                    }
                })
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_size(px(12.))
                .text_color(t.text_muted)
                .child(caption),
        );
        body = body.child(linked).child(self.wavelet_joint_plots(cx));
        body.into_any_element()
    }
    pub(super) fn wavelet_plot_card(
        &mut self,
        plot: usize,
        cx: &mut Context<Self>,
    ) -> gpui::AnyElement {
        let _ = cx;
        div()
            .id(format!("wavelet-linked-plot-{plot}"))
            .relative()
            .flex_1()
            .min_w_0()
            .h_full()
            .children(self.plot_entity(plot))
            .into_any_element()
    }
    pub(crate) fn wavelet_inspector(&self, cx: &mut Context<Self>) -> gpui::AnyElement {
        let t = self.theme;
        let mut body = div().flex().flex_col().gap_2();
        for field in self.wavelet.fields.iter().take(4) {
            body = body.child(field.clone());
        }
        body = body.child(
            button(&t, "wavelet-advanced", "Advanced ▾", self.wavelet.advanced).on_click(
                cx.listener(|app, _, _, cx| {
                    app.wavelet.advanced = !app.wavelet.advanced;
                    cx.notify();
                }),
            ),
        );
        if self.wavelet.advanced {
            for field in self.wavelet.fields.iter().skip(4) {
                body = body.child(field.clone());
            }
        }
        if let Some(record) = &self.wavelet.record {
            body = body.child(
                div()
                    .text_size(px(11.))
                    .text_color(t.text_muted)
                    .child("R is not phase-corrected. Color changes affect only the display."),
            );
            if self.wavelet.view == 3 {
                body = body.child(
                    div()
                        .text_size(px(11.))
                        .text_color(t.text_muted)
                        .child("Phase: radians; values below 1% of peak magnitude are hidden."),
                );
            }
            for warning in record
                .map
                .warnings()
                .iter()
                .chain(record.fourier_error.iter())
            {
                if self.wavelet.advanced || record.fourier_error.as_ref() == Some(warning) {
                    body = body.child(
                        div()
                            .text_size(px(11.))
                            .text_color(t.warn)
                            .child(warning.clone()),
                    );
                }
            }
        }
        body.into_any_element()
    }
    pub(crate) fn wavelet_appearance_overlay(
        &self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.wavelet.colors_open || !self.wavelet.open || self.stage != Stage::Process {
            return None;
        }
        let focus = self.wavelet.menu_focus.as_ref()?;
        let t = self.theme;
        let mut menu = div()
            .id("wavelet-color-menu")
            .w(px(190.))
            .max_h(px(430.))
            .overflow_y_scroll()
            .p_1()
            .flex()
            .flex_col()
            .gap_1()
            .rounded_lg()
            .bg(t.surface)
            .border_1()
            .border_color(t.border)
            .shadow_lg()
            .on_any_mouse_down(|_, _, cx| cx.stop_propagation());
        menu = menu.child(
            button(
                &t,
                "wavelet-color-auto",
                "Auto",
                self.wavelet.palette.is_none(),
            )
            .w_full()
            .on_click(cx.listener(|app, _, _, cx| {
                app.wavelet.palette = None;
                app.wavelet.colors_open = false;
                app.rebuild_wavelet_plots(cx);
                cx.notify();
            })),
        );
        for palette in HeatmapPalette::ALL {
            menu = menu.child(
                button(
                    &t,
                    format!("wavelet-color-{}", palette.label()),
                    palette.label(),
                    self.wavelet.palette == Some(palette),
                )
                .w_full()
                .on_click(cx.listener(move |app, _, _, cx| {
                    app.wavelet.palette = Some(palette);
                    app.wavelet.colors_open = false;
                    app.rebuild_wavelet_plots(cx);
                    cx.notify();
                })),
            );
        }
        menu = menu.child(
            button(
                &t,
                "wavelet-color-reverse",
                "Reverse",
                self.wavelet.reversed,
            )
            .w_full()
            .on_click(cx.listener(|app, _, _, cx| {
                app.wavelet.reversed = !app.wavelet.reversed;
                app.rebuild_wavelet_plots(cx);
                cx.notify();
            })),
        );
        menu = menu.child(
            button(
                &t,
                "wavelet-lock",
                "Lock scale",
                self.wavelet.lock_scale.is_some(),
            )
            .w_full()
            .on_click(cx.listener(|app, _, _, cx| {
                app.wavelet.lock_scale = if app.wavelet.lock_scale.is_some() {
                    None
                } else {
                    app.wavelet
                        .record
                        .as_ref()
                        .map(|r| plots::scale(&r.map, app.wavelet.view))
                };
                app.wavelet.lock_basis = if app.wavelet.lock_scale.is_some() {
                    app.wavelet
                        .record
                        .as_ref()
                        .map(|r| (r.settings.clone(), r.map.settings().clone()))
                } else {
                    None
                };
                app.rebuild_wavelet_plots(cx);
                cx.notify();
            })),
        );
        Some(
            div()
                .id("wavelet-color-dismiss")
                .absolute()
                .inset_0()
                .occlude()
                .track_focus(focus)
                .on_key_down(cx.listener(|app, event: &gpui::KeyDownEvent, _, cx| {
                    if event.keystroke.key == "escape" {
                        app.wavelet.colors_open = false;
                        cx.stop_propagation();
                        cx.notify();
                    }
                }))
                .on_any_mouse_down(cx.listener(|app, _, _, cx| {
                    app.wavelet.colors_open = false;
                    cx.stop_propagation();
                    cx.notify();
                }))
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    gpui::anchored()
                        .position(self.wavelet.menu_position)
                        .offset(gpui::point(px(0.), px(15.)))
                        .snap_to_window()
                        .child(menu),
                )
                .into_any_element(),
        )
    }
}
