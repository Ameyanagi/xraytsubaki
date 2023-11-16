// Import dioxus
use dioxus::prelude::*;

// Import Plotters
use plotters::style::Palette100;
use plotters::{backend::RGBPixel, prelude::*};
use plotters_svg::SVGBackend;

// Crates related to base64 encoding the image
use base64::alphabet;
use base64::engine::{self, general_purpose};
use base64::Engine;

use image::{DynamicImage, ImageBuffer, ImageOutputFormat, Rgb};
use std::io::Cursor;

#[derive(dioxus::prelude::Props, PartialEq)]
pub struct PlotData {
    #[props(default = vec![vec![0.0, 1.0, 2.0, 3.0, 4.0]])]
    pub x: Vec<Vec<f64>>,

    #[props(default = vec![vec![0.0, 1.0, 2.0, 3.0, 4.0]])]
    pub y: Vec<Vec<f64>>,

    #[props(default = None)]
    pub labels: Option<Vec<String>>,

    #[props(default = "test".to_string())]
    pub title: String,

    #[props(default = "x".to_string())]
    pub x_label: String,

    #[props(default = "y".to_string())]
    pub y_label: String,

    #[props(default = 0.0)]
    pub x_min: f64,

    #[props(default = 1.0)]
    pub x_max: f64,

    #[props(default = 0.0)]
    pub y_min: f64,

    #[props(default = 1.0)]
    pub y_max: f64,

    #[props(default = 480)]
    pub width: usize,

    #[props(default = 480)]
    pub height: usize,
}

const CUSTOM_ENGINE: engine::GeneralPurpose =
    engine::GeneralPurpose::new(&alphabet::STANDARD, general_purpose::NO_PAD);

fn image_to_base64(img: &DynamicImage) -> String {
    let mut image_data: Vec<u8> = Vec::new();
    img.write_to(&mut Cursor::new(&mut image_data), ImageOutputFormat::Png)
        .unwrap();
    let res_base64 = general_purpose::STANDARD.encode(image_data);
    format!("data:image/png;base64,{}", res_base64)
}

pub fn LineChartBmp<'a>(cx: Scope<'a, PlotData>) -> Element<'a> {
    let mut buffer: Vec<u8> = vec![0; cx.props.width * cx.props.height * 3];

    // let path = "test.png";
    let root = BitMapBackend::<RGBPixel>::with_buffer_and_format(
        &mut buffer,
        (cx.props.width as u32, cx.props.height as u32),
    )
    .unwrap()
    .into_drawing_area();

    root.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root)
        .caption(&cx.props.title, ("sans-serif", 25).into_font())
        .margin(2)
        .x_label_area_size(30)
        .y_label_area_size(30)
        .build_cartesian_2d(
            cx.props.x_min as f32..cx.props.x_max as f32,
            cx.props.y_min as f32..cx.props.y_max as f32,
        )
        .unwrap();

    chart
        .configure_mesh()
        .y_desc(&cx.props.y_label)
        .x_desc(&cx.props.x_label)
        .draw()
        .unwrap();

    let labels = if let Some(labels) = &cx.props.labels {
        if labels.len() < cx.props.x.len().min(cx.props.y.len()) {
            labels.to_owned().extend(
                std::iter::repeat("".to_string())
                    .take(cx.props.x.len().min(cx.props.y.len()) - labels.len())
                    .collect::<Vec<String>>(),
            );
        }

        labels.to_owned()
    } else {
        std::iter::repeat("".to_string())
            .take(cx.props.x.len().min(cx.props.y.len()))
            .collect::<Vec<String>>()
    };

    _ = cx
        .props
        .x
        .iter()
        .zip(cx.props.y.iter())
        .zip(labels.iter())
        .enumerate()
        .for_each(|(i, ((x, y), lab))| {
            let series = chart
                .draw_series(LineSeries::new(
                    x.iter().zip(y).map(|(x, y)| (*x as f32, *y as f32)),
                    &Palette100::pick(i.clone()),
                ))
                .unwrap();

            if lab != "" {
                series.label(lab).legend(move |(x, y)| {
                    PathElement::new(vec![(x, y), (x + 20, y)], &Palette100::pick(i.clone()))
                });
            } else {
            }
        });

    chart
        .configure_series_labels()
        .background_style(&WHITE.mix(0.8))
        .border_style(&BLACK)
        .draw()
        .unwrap();

    drop(chart);
    drop(root);

    let borrowed_image = ImageBuffer::<Rgb<u8>, Vec<u8>>::from_raw(
        cx.props.width as u32,
        cx.props.height as u32,
        buffer,
    )
    .unwrap();

    let image = format!(
        "<img src=\"{}\" draggable=\"false\" />",
        image_to_base64(&DynamicImage::ImageRgb8(borrowed_image))
    );

    render!(
        rsx!(
            div {
        onclick: move |event| {
            println!("event: {:?}", event);
        },
        ondragstart: move |event| {
            println!("event: {:?}", event);
        },
        ondragend: move |event| {
            println!("event: {:?}", event);
        },
        dangerous_inner_html: "{image}"
                }
            )
        )
}
