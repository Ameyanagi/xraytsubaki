#![allow(non_snake_case)]

use std::borrow::BorrowMut;

// Import dioxus
use dioxus::{
    html::{canvas, label, legend, script},
    prelude::*,
};
use dioxus_router::prelude::*;
use log::LevelFilter;

// Import local components]
mod top_menu;

mod menu;
use menu::Navibar;

mod footer;
use footer::Footer;

#[cfg(target_arch = "wasm32")]
const TOP_DIR: &'static str = "./";

#[cfg(not(target_arch = "wasm32"))]
const TOP_DIR: &'static str = "./public/";

// Plotly related components
// This will be imported for wasm support.
#[cfg(target_arch = "wasm32")]
mod plotly_temp;
#[cfg(target_arch = "wasm32")]
use plotly_temp::Chart;

// Plotting related components
#[cfg(not(target_arch = "wasm32"))]
mod plotters_chart;
#[cfg(not(target_arch = "wasm32"))]
use plotters_chart::{LineChartBmp, PlotData};

fn main() {
    // Init debug
    dioxus_logger::init(LevelFilter::Info).expect("failed to init logger");
    console_error_panic_hook::set_once();

    log::info!("starting app");

    #[cfg(not(target_arch = "wasm32"))]
    dioxus_desktop::launch_cfg(
        app,
        dioxus_desktop::Config::new().with_custom_head(
            r#"<link rel="stylesheet" href="./public/tailwind.css">"#.to_string(),
        ),
    );
    // dioxus_desktop::launch(app);

    #[cfg(target_arch = "wasm32")]
    dioxus_web::launch(app);

}

fn app(cx: Scope) -> Element {
    render! { Router::<Route> {} }
}

#[cfg(not(target_arch = "wasm32"))]
async fn run_xas_batch_async(repetitions: usize) -> Result<String, String> {
    use futures_channel::oneshot;
    use xraytsubaki::xafs::io;
    use xraytsubaki::xafs::xasgroup::XASGroup;

    let (tx, rx) = oneshot::channel::<Result<String, String>>();
    std::thread::spawn(move || {
        let result = (|| -> Result<String, String> {
            let mut group = XASGroup::new();
            let sample_path = "../crates/xraytsubaki/tests/testfiles/Ru_QAS.dat";
            for _ in 0..repetitions {
                let spectrum = io::load_spectrum_QAS_trans(sample_path).map_err(|e| e.to_string())?;
                group.add_spectrum(spectrum);
            }
            group.find_e0().map_err(|e| e.to_string())?;
            group.normalize().map_err(|e| e.to_string())?;
            group.calc_background().map_err(|e| e.to_string())?;
            group.fft().map_err(|e| e.to_string())?;
            Ok(format!("Processed {} spectra in async worker", group.len()))
        })();
        let _ = tx.send(result);
    });

    rx.await
        .map_err(|_| "async worker dropped before sending result".to_string())?
}

#[cfg(target_arch = "wasm32")]
async fn run_xas_batch_async(_repetitions: usize) -> Result<String, String> {
    Err("core batch processing in a worker thread is not enabled on wasm builds".to_string())
}

#[derive(Clone, Routable, Debug, PartialEq)]
pub enum Route {
    #[route("/")]
    Home {},
    #[route("/xas")]
    XAS {},
    #[route("/:..route")]
    PageNotFound { route: Vec<String> },
}

#[inline_props]
fn Home(cx: Scope) -> Element {
    let mut count = use_state(cx, || 0);
    let x = (0..=1000)
        .map(|x| x as f64 / 1000.0 * (**count as f64))
        .collect::<Vec<_>>();
    let y = x.iter().map(|x| x * x).collect::<Vec<_>>();
    let x = vec![ x.clone(), x ];
    let y = vec![ y.clone(), y.iter().map(|x| x * x).collect::<Vec<_>>() ];

    cx.render(rsx! {
        div { class: "min-h-screen bg-slate-100",

            top_menu::TopMenu {}

            main {
                h1 { "High-Five counter: {count}" }
                button { onclick: move |_| count += 1, "Up high!" }
                button { onclick: move |_| count -= 1, "Down low!" }
                button {
                    onclick: move |event| {
                        println!("event: {:?}", event);
                    },
                    "test"
                }
            }

            Footer {}
        }
    })
}

fn XAS(cx: Scope) -> Element {
    let status = use_state(cx, || "Idle".to_string());

    use_future(&cx, (), |_| {
        let status = status.clone();
        async move {
            status.set("Running large batch in async core worker...".to_string());
            match run_xas_batch_async(64).await {
                Ok(message) => status.set(message),
                Err(err) => status.set(format!("Core worker failed: {err}")),
            }
        }
    });

    cx.render(rsx! {
        div { class: "bg-[#fafafa]",

            Navibar {}

            div {
                h1 { "XAS" }
                p { "{status}" }
            }

            Footer {}
        }
    })
}

// EXAFS analysis
// main window
// normalization
// FFT
// RFFT
// Plotting parameter

// calibrate data

// align data

// rebin data

// deglitch data

// smooth data

// convolute and add noise

// self absorption correction

// multi-electron exitation

// Linear combination fitting
// standards
// fit results
// combinatorics
// sequence

// PCA

#[inline_props]
fn PageNotFound(cx: Scope, route: Vec<String>) -> Element {
    render! {
        h1 { "Page not found" }
        p { "We are terribly sorry, but we couldn't find the page you were looking for." }
        pre { color: "red", "log:\nattempted to navigate to: {route:?}" }
    }
}
