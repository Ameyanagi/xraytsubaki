use plotly::common::Mode;
use plotly::{common::Title, Layout};
use plotly::{Plot, Scatter};

use dioxus::prelude::*;

#[inline_props]
pub fn Chart(cx: Scope) -> Element {
    use_future(&cx, (), |_| async move {
        let id = "plot-div";
        let mut plot = Plot::new();
        let x = (0..1000).into_iter().collect::<Vec<_>>();
        let y = (0..1000).into_iter().map(|i| i * 2).collect::<Vec<_>>();

        let trace = Scatter::new(x, y).mode(Mode::Markers);
        plot.add_trace(trace);
        plot.set_layout(Layout::new().title(Title::new(
            "Distribution of Revenue normalized by Review Velocity",
        )));
        plotly::bindings::new_plot(id, &plot).await;
    });

    cx.render(rsx!( div { id: "plot-div", width: "300px", height: "300px" } ))
}
