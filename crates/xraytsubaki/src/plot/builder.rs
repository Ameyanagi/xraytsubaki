use crate::plot::config::{FitPlotOptions, GroupPlotOptions, PanelKind, PanelSpec, PlotConfig};
use crate::plot::errors::PlotError;
use crate::plot::{fitting_plots, group_plots, panels, spectrum_plots};
use crate::xafs::fitting::types::FeffFitResult;
use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;
use ruviz::core::Plot;
use std::path::Path;

pub(crate) enum PlotSource<'a> {
    Spectrum(&'a mut XASSpectrum),
    Group(&'a mut XASGroup),
    Fit(&'a mut FeffFitResult),
}

pub struct XASPlotBuilder<'a> {
    source: PlotSource<'a>,
    panels: Vec<PanelSpec>,
    config: PlotConfig,
    group: GroupPlotOptions,
    fit: FitPlotOptions,
    pending_error: Option<PlotError>,
}

impl<'a> XASPlotBuilder<'a> {
    pub(crate) fn for_spectrum(spectrum: &'a mut XASSpectrum) -> Self {
        Self {
            source: PlotSource::Spectrum(spectrum),
            panels: Vec::new(),
            config: PlotConfig::default(),
            group: GroupPlotOptions::default(),
            fit: FitPlotOptions::default(),
            pending_error: None,
        }
    }

    pub(crate) fn for_group(group: &'a mut XASGroup) -> Self {
        Self {
            source: PlotSource::Group(group),
            panels: Vec::new(),
            config: PlotConfig::default(),
            group: GroupPlotOptions::default(),
            fit: FitPlotOptions::default(),
            pending_error: None,
        }
    }

    pub(crate) fn for_fit(fit: &'a mut FeffFitResult) -> Self {
        Self {
            source: PlotSource::Fit(fit),
            panels: Vec::new(),
            config: PlotConfig::default(),
            group: GroupPlotOptions::default(),
            fit: FitPlotOptions::default(),
            pending_error: None,
        }
    }

    fn set_error(&mut self, error: PlotError) {
        if self.pending_error.is_none() {
            self.pending_error = Some(error);
        }
    }

    fn has_panels(&self) -> bool {
        !self.panels.is_empty()
    }

    fn is_group_source(&self) -> bool {
        matches!(self.source, PlotSource::Group(_))
    }

    fn is_fit_source(&self) -> bool {
        matches!(self.source, PlotSource::Fit(_))
    }

    fn last_panel_kind(&self) -> Option<PanelKind> {
        self.panels.last().map(|panel| panel.kind)
    }

    fn require_panel_for_option(&mut self, option_name: &str) -> bool {
        if self.has_panels() {
            true
        } else {
            self.set_error(PlotError::invalid_option(format!(
                "{option_name}() requires a panel selector before it"
            )));
            false
        }
    }

    fn apply_panel_option<F>(&mut self, option_name: &str, mut f: F)
    where
        F: FnMut(&mut PanelSpec) -> Result<(), PlotError>,
    {
        if !self.require_panel_for_option(option_name) {
            return;
        }

        if let Some(panel) = self.panels.last_mut() {
            if let Err(error) = f(panel) {
                self.set_error(error);
            }
        }
    }

    fn build_plots(&mut self) -> Result<Vec<Plot>, PlotError> {
        if let Some(error) = self.pending_error.take() {
            return Err(error);
        }
        if self.panels.is_empty() {
            return Err(PlotError::EmptySelection);
        }

        let panel_specs = self.panels.clone();
        let show_legend = self.config.show_legend;
        let mut plots = Vec::with_capacity(panel_specs.len());

        match &mut self.source {
            PlotSource::Spectrum(spectrum) => {
                for panel in panel_specs {
                    let data = spectrum_plots::extract_spectrum_panel_data(spectrum, &panel)?;
                    let plot = panels::render_panel(data, show_legend)?;
                    plots.push(plot);
                }
            }
            PlotSource::Group(group) => {
                for panel in panel_specs {
                    let data = group_plots::extract_group_panel_data(group, &panel, &self.group)?;
                    let plot = panels::render_panel(data, show_legend)?;
                    plots.push(plot);
                }
            }
            PlotSource::Fit(fit) => {
                for panel in panel_specs {
                    let data = fitting_plots::extract_fit_panel_data(fit, &panel, &self.fit)?;
                    let plot = panels::render_panel(data, show_legend)?;
                    plots.push(plot);
                }
            }
        }

        Ok(plots)
    }

    fn apply_single_config(&self, mut plot: Plot) -> Plot {
        plot = plot.size_px(self.config.width, self.config.height);
        plot = plot.dpi(self.config.dpi);
        if let Some(title) = &self.config.title {
            plot = plot.title(title.clone());
        }
        plot
    }

    pub fn mu(mut self) -> Self {
        self.panels.push(PanelSpec::new(PanelKind::Mu));
        self
    }

    pub fn norm(mut self) -> Self {
        self.panels.push(PanelSpec::new(PanelKind::Norm));
        self
    }

    pub fn k(mut self) -> Self {
        self.panels.push(PanelSpec::new(PanelKind::K));
        self
    }

    pub fn r(mut self) -> Self {
        self.panels.push(PanelSpec::new(PanelKind::R));
        self
    }

    pub fn kweight(mut self, weight: f64) -> Self {
        self.apply_panel_option("kweight", |panel| {
            if panel.kind != PanelKind::K {
                return Err(PlotError::invalid_option(
                    "kweight() is only valid for k() panels",
                ));
            }
            panel.kweight = Some(weight);
            Ok(())
        });
        self
    }

    pub fn components(mut self, show: bool) -> Self {
        self.apply_panel_option("components", |panel| {
            if panel.kind != PanelKind::R {
                return Err(PlotError::invalid_option(
                    "components() is only valid for r() panels",
                ));
            }
            if show {
                panel.r_mag = Some(true);
                panel.r_real = true;
                panel.r_imag = true;
            } else {
                panel.r_mag = Some(true);
                panel.r_real = false;
                panel.r_imag = false;
            }
            Ok(())
        });
        self
    }

    pub fn mag(mut self) -> Self {
        self.apply_panel_option("mag", |panel| {
            if panel.kind != PanelKind::R {
                return Err(PlotError::invalid_option(
                    "mag() is only valid for r() panels",
                ));
            }
            panel.r_mag = Some(true);
            Ok(())
        });
        self
    }

    pub fn real(mut self) -> Self {
        self.apply_panel_option("real", |panel| {
            if panel.kind != PanelKind::R {
                return Err(PlotError::invalid_option(
                    "real() is only valid for r() panels",
                ));
            }
            if panel.r_mag.is_none() {
                panel.r_mag = Some(false);
            }
            panel.r_real = true;
            Ok(())
        });
        self
    }

    pub fn imag(mut self) -> Self {
        self.apply_panel_option("imag", |panel| {
            if panel.kind != PanelKind::R {
                return Err(PlotError::invalid_option(
                    "imag() is only valid for r() panels",
                ));
            }
            if panel.r_mag.is_none() {
                panel.r_mag = Some(false);
            }
            panel.r_imag = true;
            Ok(())
        });
        self
    }

    pub fn edges(mut self, show: bool) -> Self {
        self.apply_panel_option("edges", |panel| {
            if panel.kind != PanelKind::Norm {
                return Err(PlotError::invalid_option(
                    "edges() is only valid for norm() panels",
                ));
            }
            panel.edges = show;
            Ok(())
        });
        self
    }

    pub fn window(mut self, show: bool) -> Self {
        self.apply_panel_option("window", |panel| {
            if panel.kind != PanelKind::K {
                return Err(PlotError::invalid_option(
                    "window() is only valid for k() panels",
                ));
            }
            panel.window_fn = show;
            panel.window_box = show;
            Ok(())
        });
        self
    }

    pub fn window_fn(mut self, show: bool) -> Self {
        self.apply_panel_option("window_fn", |panel| {
            if panel.kind != PanelKind::K {
                return Err(PlotError::invalid_option(
                    "window_fn() is only valid for k() panels",
                ));
            }
            panel.window_fn = show;
            Ok(())
        });
        self
    }

    pub fn window_box(mut self, show: bool) -> Self {
        let fit_source = self.is_fit_source();
        self.apply_panel_option("window_box", |panel| match panel.kind {
            PanelKind::K => {
                panel.window_box = show;
                Ok(())
            }
            PanelKind::R if fit_source => {
                panel.window_box = show;
                Ok(())
            }
            PanelKind::R => Err(PlotError::invalid_option(
                "window_box() on r() panels is only valid for FeffFitResult sources",
            )),
            _ => Err(PlotError::invalid_option(
                "window_box() is only valid for k() panels, or r() panels on FeffFitResult",
            )),
        });
        self
    }

    pub fn stacked(mut self, offset: f64) -> Self {
        if !self.require_panel_for_option("stacked") {
            return self;
        }
        if !self.is_group_source() {
            self.set_error(PlotError::invalid_option(
                "stacked() is only valid for XASGroup sources",
            ));
            return self;
        }
        self.group.stacked = Some(offset);
        self
    }

    pub fn select(mut self, indices: &[usize]) -> Self {
        if !self.require_panel_for_option("select") {
            return self;
        }
        if !self.is_group_source() {
            self.set_error(PlotError::invalid_option(
                "select() is only valid for XASGroup sources",
            ));
            return self;
        }
        self.group.selected = Some(indices.to_vec());
        self
    }

    pub fn dataset(mut self, index: usize) -> Self {
        if !self.require_panel_for_option("dataset") {
            return self;
        }
        if !self.is_fit_source() {
            self.set_error(PlotError::invalid_option(
                "dataset() is only valid for FeffFitResult sources",
            ));
            return self;
        }
        self.fit.dataset = Some(index);
        self
    }

    pub fn paths(mut self, show: bool) -> Self {
        if !self.require_panel_for_option("paths") {
            return self;
        }
        if !self.is_fit_source() {
            self.set_error(PlotError::invalid_option(
                "paths() is only valid for FeffFitResult sources",
            ));
            return self;
        }
        if self.last_panel_kind() != Some(PanelKind::K) {
            self.set_error(PlotError::invalid_option(
                "paths() is only valid when the last selected panel is k()",
            ));
            return self;
        }
        self.fit.paths = show;
        self
    }

    pub fn title(mut self, title: &str) -> Self {
        self.config.title = Some(title.to_string());
        self
    }

    pub fn width(mut self, width: u32) -> Self {
        self.config.width = width.max(1);
        self
    }

    pub fn height(mut self, height: u32) -> Self {
        self.config.height = height.max(1);
        self
    }

    pub fn legend(mut self, show: bool) -> Self {
        self.config.show_legend = show;
        self
    }

    pub fn save_png<P: AsRef<Path>>(mut self, path: P) -> Result<(), PlotError> {
        let mut plots = self.build_plots()?;
        if plots.len() == 1 {
            let plot = self.apply_single_config(plots.remove(0));
            plot.save(path)?;
            return Ok(());
        }

        let figure = panels::build_subplot_figure(
            plots,
            self.config.width,
            self.config.height,
            self.config.title.as_deref(),
        )?;
        figure.save_with_dpi(path, self.config.dpi as f32)?;
        Ok(())
    }

    pub fn render_plot(mut self) -> Result<Plot, PlotError> {
        let mut plots = self.build_plots()?;
        if plots.len() != 1 {
            return Err(PlotError::MultiPanelRenderUnsupported);
        }
        Ok(self.apply_single_config(plots.remove(0)))
    }

    pub fn to_svg(self) -> Result<String, PlotError> {
        let plot = self.render_plot()?;
        Ok(plot.render_to_svg()?)
    }

    pub fn to_svg_panels(mut self) -> Result<Vec<String>, PlotError> {
        let plots = self.build_plots()?;
        let mut svg_panels = Vec::with_capacity(plots.len());

        for plot in plots {
            let panel = plot.size_px(self.config.width, self.config.height);
            svg_panels.push(panel.render_to_svg()?);
        }

        Ok(svg_panels)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plot::traits::PlotXAS;
    use crate::xafs::fitting::types::FeffFitResult;
    use nalgebra::DVector;

    #[test]
    fn option_before_panel_is_rejected() {
        let mut spectrum = XASSpectrum::new();
        let error = spectrum
            .plot()
            .kweight(2.0)
            .mu()
            .render_plot()
            .expect_err("must fail");
        assert!(matches!(error, PlotError::InvalidOption { .. }));
    }

    #[test]
    fn invalid_panel_option_combination_is_rejected() {
        let mut spectrum = XASSpectrum::new();
        let error = spectrum
            .plot()
            .mu()
            .components(true)
            .render_plot()
            .expect_err("must fail");
        assert!(matches!(error, PlotError::InvalidOption { .. }));
    }

    #[test]
    fn window_alias_enables_fn_and_box_for_k_panel() {
        let mut spectrum = XASSpectrum::new();
        let builder = spectrum.plot().k().window(true);
        assert!(builder.pending_error.is_none());
        assert!(builder.panels[0].window_fn);
        assert!(builder.panels[0].window_box);
    }

    #[test]
    fn window_fn_is_rejected_for_r_panel() {
        let mut spectrum = XASSpectrum::new();
        let builder = spectrum.plot().r().window_fn(true);
        assert!(matches!(
            builder.pending_error,
            Some(PlotError::InvalidOption { .. })
        ));
    }

    #[test]
    fn window_box_on_r_panel_is_fit_only() {
        let mut spectrum = XASSpectrum::new();
        let spectrum_builder = spectrum.plot().r().window_box(true);
        assert!(matches!(
            spectrum_builder.pending_error,
            Some(PlotError::InvalidOption { .. })
        ));

        let mut fit = FeffFitResult::default();
        let fit_builder = fit.plot().r().window_box(true);
        assert!(fit_builder.pending_error.is_none());
        assert!(fit_builder.panels[0].window_box);
    }

    #[test]
    fn dataset_index_validation_propagates() {
        let mut fit = FeffFitResult::default();
        let error = fit
            .plot()
            .k()
            .dataset(3)
            .save_png("/tmp/ignore.png")
            .expect_err("must fail");
        assert!(matches!(error, PlotError::IndexOutOfRange { .. }));
    }

    #[test]
    fn multi_panel_single_output_methods_are_rejected() {
        let mut spectrum = XASSpectrum::new();
        spectrum.set_spectrum(
            DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0]),
            DVector::from_vec(vec![1.0, 1.5, 2.0, 2.5]),
        );

        let render_error = spectrum
            .plot()
            .mu()
            .mu()
            .render_plot()
            .expect_err("multi-panel render_plot must fail");
        assert!(matches!(
            render_error,
            PlotError::MultiPanelRenderUnsupported
        ));

        let svg_error = spectrum
            .plot()
            .mu()
            .mu()
            .to_svg()
            .expect_err("multi-panel to_svg must fail");
        assert!(matches!(svg_error, PlotError::MultiPanelRenderUnsupported));

        let panels = spectrum
            .plot()
            .mu()
            .mu()
            .to_svg_panels()
            .expect("multi-panel svg panels should succeed");
        assert_eq!(panels.len(), 2);
    }
}
