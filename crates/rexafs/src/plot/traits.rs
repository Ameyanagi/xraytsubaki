use crate::plot::builder::XASPlotBuilder;
use crate::xafs::fitting::types::FeffFitResult;
use crate::xafs::xasgroup::XASGroup;
use crate::xafs::xasspectrum::XASSpectrum;

pub trait PlotXAS {
    fn plot(&mut self) -> XASPlotBuilder<'_>;
}

impl PlotXAS for XASSpectrum {
    fn plot(&mut self) -> XASPlotBuilder<'_> {
        XASPlotBuilder::for_spectrum(self)
    }
}

impl PlotXAS for XASGroup {
    fn plot(&mut self) -> XASPlotBuilder<'_> {
        XASPlotBuilder::for_group(self)
    }
}

impl PlotXAS for FeffFitResult {
    fn plot(&mut self) -> XASPlotBuilder<'_> {
        XASPlotBuilder::for_fit(self)
    }
}
