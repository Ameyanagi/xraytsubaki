//! Independently framed inset plots, composited above a panel's existing data.
//!
//! ruviz 0.15.2 does not attach its `axes::InsetAxes` configuration to `Plot`.
//! Rendering a separate plot also keeps inset limits independent of the main
//! axes and preserves editable SVG text when the two documents are nested.

use super::{FigureData, FigureOptions, Frame, Grid, InsetPlacement, blend, blit, layout};
use crate::publication::report::html;
use ruviz::core::Image;
use ruviz::prelude::{Color, Plot};
use ruviz::render::typst_text::{TypstRasterOutput, literal_text_snippet, render_raster};

/// Copied display data and limits resolved independently of the main panel.
pub(crate) struct PreparedInset {
    pub data: FigureData,
    pub options: FigureOptions,
    pub placement: InsetPlacement,
    /// Literal quantity name placed inside the axes, such as `μ(E)` or `Δμ`.
    pub quantity: &'static str,
    pub caption: String,
    /// Shifts in source-series order, with exact ordinates before stacking.
    pub offsets: Vec<Option<super::DisplayOffset>>,
}

/// The outer inset canvas, including the entire row of energy tick labels.
#[derive(Clone, Copy, Debug)]
struct InsetBox {
    origin: (u32, u32),
    size: (u32, u32),
}

/// Draw an inset wholly inside the main plot frame. All measurements use the
/// panel's output DPI; the inset has 80% of its font size and 70% of its curve
/// width. The source arrays, main axes and existing panel pixels outside the
/// inset canvas are unchanged.
pub(super) fn render(
    image: &mut Image,
    svg: &mut String,
    inset: &PreparedInset,
    frame: Frame,
    grid: &Grid,
    font_size: f64,
    line_width: Option<f64>,
) -> Result<(), String> {
    let bounds = placement_box((image.width, image.height), frame, grid, inset.placement)?;
    let scale = grid.dpi / 72.;
    let font_pt = (font_size * 0.8).max(4.);
    let font_px = font_pt * scale;
    let default_width = Plot::new().get_config().lines.data_width as f64;
    // ruviz's public Plot renderer requires at least 100 pixels on each
    // side. Keep the physical geometry at low preview/export DPI, rendering
    // only these small canvases at a higher integer multiple of the DPI.
    let render_factor = 100u32.div_ceil(bounds.size.0.min(bounds.size.1)).max(1);
    // Build at valid single-figure dimensions first. The renderer's fixed
    // canvas below can be smaller than FigureOptions' one-inch minimum.
    let options = FigureOptions {
        width: Some(1.),
        height: Some(1.),
        dpi: Some(grid.dpi * render_factor as f64),
        font_size: Some(font_pt),
        line_width: None,
        title: None,
        xlabel: Some(String::new()),
        ylabel: Some(String::new()),
        legend: false,
        guides: false,
        ..inset.options.clone()
    };
    let (xmin, xmax) = inset
        .data
        .xlim(&options)
        .filter(|(lo, hi)| lo.is_finite() && hi.is_finite() && lo < hi)
        .ok_or("The inset needs finite, increasing energy limits.")?;
    let values = ruviz::axes::generate_ticks(xmin, xmax, 3);
    let labels = ruviz::axes::format_tick_labels(&values);
    let mut ticks = values
        .into_iter()
        .zip(labels)
        .map(|(value, label)| {
            Ok(Tick {
                fraction: (value - xmin) / (xmax - xmin),
                text: Text::new(label, font_px)?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let quantity = Text::new(inset.quantity.to_owned(), font_px)?;
    let padding = 2.5 * scale;
    let widest_tick = ticks
        .iter()
        .map(|tick| tick.text.width())
        .fold(0., f64::max);
    let tallest_tick = ticks
        .iter()
        .map(|tick| tick.text.height())
        .fold(0., f64::max);
    let margins = [
        widest_tick / 2. + padding,
        widest_tick / 2. + padding,
        padding,
        tallest_tick + 2. * padding,
    ];
    let plot_width = bounds.size.0 as f64 - margins[0] - margins[1];
    let plot_height = bounds.size.1 as f64 - margins[2] - margins[3];
    if plot_width < (quantity.width() + 2. * padding).max(3. * font_px)
        || plot_height < (quantity.height() + 2. * padding).max(2. * font_px)
    {
        return Err("The inset has too little plot area for its labels. Widen the figure, use fewer columns, increase the panel height or reduce the font size.".into());
    }
    // Three short labels are preferred. If the middle label collides, retain
    // the two outer labels; never clip text or silently shrink its font.
    if ticks.windows(2).any(|pair| {
        (pair[1].fraction - pair[0].fraction) * plot_width
            < (pair[0].text.width() + pair[1].text.width()) / 2. + padding
    }) {
        if ticks.len() > 2 {
            let last = ticks.pop().expect("At least three ticks were checked.");
            ticks.truncate(1);
            ticks.push(last);
        }
        if ticks.windows(2).any(|pair| {
            (pair[1].fraction - pair[0].fraction) * plot_width
                < (pair[0].text.width() + pair[1].text.width()) / 2. + padding
        }) {
            return Err("The inset's energy tick labels do not fit. Widen the figure, use fewer columns or reduce the font size.".into());
        }
    }

    // There is no independent y-tick-label switch in this ruviz release.
    // Disable its tick row and draw the measured x row below; a zero y-tick
    // count would still draw the two endpoint labels.
    let plot = inset.data.panel(&options, false)?.ticks(false);
    let mut config = plot.get_config().clone();
    // The inset may be thinner than the main editor's 0.1 pt minimum.
    config.lines.data_width = (line_width.unwrap_or(default_width) * 0.7) as f32;
    config.figure.width = (bounds.size.0 as f64 / grid.dpi) as f32;
    let plot = layout::panel_frame(
        plot.plot_config(config),
        bounds.size.1 as f64 / grid.dpi,
        margins.map(|px| px / grid.dpi),
    );
    let rendered = plot.render().map_err(|error| error.to_string())?;
    let native_svg = plot.render_to_svg().map_err(|error| error.to_string())?;
    let render_size = (bounds.size.0 * render_factor, bounds.size.1 * render_factor);
    if (rendered.width, rendered.height) != render_size {
        return Err("The inset rendered at an unexpected canvas size.".into());
    }
    let mut rendered = downsample(rendered, render_factor);
    let bottom = rendered.height as f64 - margins[3];
    let mut markup = format!(
        "<desc>{}</desc><g class=\"composer-inset-labels\">",
        html(&inset.caption)
    );
    for tick in &ticks {
        let x = margins[0] + tick.fraction * plot_width;
        let tick_length = 2.5 * scale;
        let tick_width = 0.5 * scale;
        solid_rect(
            &mut rendered,
            (x - tick_width / 2., bottom - tick_length),
            (tick_width, tick_length),
            0,
        );
        markup.push_str(&format!(
            r##"<line class="inset-x-tick" x1="{x:.3}" x2="{x:.3}" y1="{:.3}" y2="{bottom:.3}" stroke="#000000" stroke-width="{tick_width:.3}"/>"##,
            bottom - tick_length
        ));
        tick.text.draw(
            &mut rendered,
            &mut markup,
            (x - tick.text.width() / 2., bottom + padding),
            font_px,
            "inset-x-tick-label",
        );
    }
    let quantity_origin = (margins[0] + padding, margins[2] + padding);
    quantity.draw_boxed(&mut rendered, &mut markup, quantity_origin, font_px, scale);
    markup.push_str("</g>");
    let start = native_svg
        .find("<svg")
        .ok_or("The inset SVG has no opening element.")?;
    let inset_svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" class="composer-inset" width="{}" height="{}"><g transform="scale({:.12})">{}</g>{markup}</svg>"#,
        bounds.size.0,
        bounds.size.1,
        1. / render_factor as f64,
        &native_svg[start..]
    );
    let nested = layout::nest_svgs(
        &[
            (svg.as_str(), "main", 0, 0),
            (&inset_svg, "inset", bounds.origin.0, bounds.origin.1),
        ],
        "",
        (image.width, image.height),
    );
    blit(
        &mut image.pixels,
        image.width,
        &rendered,
        bounds.origin,
        bounds.size,
    );
    *svg = nested;
    Ok(())
}

/// Average each integer-sized pixel block of the opaque high-resolution plot.
/// Text added afterwards is measured and drawn directly at the requested DPI.
fn downsample(image: Image, factor: u32) -> Image {
    if factor == 1 {
        return image;
    }
    let (width, height) = (image.width / factor, image.height / factor);
    let count = factor * factor;
    let mut pixels = vec![0; width as usize * height as usize * 4];
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            for dy in 0..factor {
                for dx in 0..factor {
                    let source = (((y * factor + dy) * image.width + x * factor + dx) * 4) as usize;
                    for (channel, total) in sum.iter_mut().enumerate() {
                        *total += image.pixels[source + channel] as u32;
                    }
                }
            }
            let target = ((y * width + x) * 4) as usize;
            for (channel, total) in sum.into_iter().enumerate() {
                pixels[target + channel] = ((total + count / 2) / count) as u8;
            }
        }
    }
    Image::new(width, height, pixels)
}

/// Use the main axes' interior, with a four-point gap on every occupied edge.
/// The 44% width and 40% height are rexafs display choices, including ticks.
fn placement_box(
    (width, height): (u32, u32),
    [left, right, top, bottom]: Frame,
    grid: &Grid,
    placement: InsetPlacement,
) -> Result<InsetBox, String> {
    let invalid = || {
        "The inset needs more plot area. Widen the figure, use fewer columns, increase the panel height or reduce the font size.".to_owned()
    };
    if !grid.dpi.is_finite()
        || grid.dpi <= 0.
        || [left, right, top, bottom]
            .iter()
            .any(|value| !value.is_finite() || *value < 0.)
    {
        return Err(invalid());
    }
    let plot_left = (left * grid.dpi).ceil();
    let plot_right = (width as f64 - right * grid.dpi).floor();
    let plot_top = (top * grid.dpi).ceil();
    let plot_bottom = (height as f64 - bottom * grid.dpi).floor();
    let plot_width = plot_right - plot_left;
    let plot_height = plot_bottom - plot_top;
    let gap = (4. * grid.dpi / 72.).ceil();
    let inset_width = (0.44 * plot_width).floor();
    let inset_height = (0.40 * plot_height).floor();
    if inset_width < 1.
        || inset_height < 1.
        || inset_width + 2. * gap > plot_width
        || inset_height + 2. * gap > plot_height
    {
        return Err(invalid());
    }
    let x = match placement {
        InsetPlacement::UpperLeft => plot_left + gap,
        InsetPlacement::LowerRight | InsetPlacement::CenterRight => plot_right - gap - inset_width,
    };
    let y = match placement {
        InsetPlacement::UpperLeft => plot_top + gap,
        InsetPlacement::LowerRight => plot_bottom - gap - inset_height,
        InsetPlacement::CenterRight => plot_top + ((plot_height - inset_height) / 2.).floor(),
    };
    Ok(InsetBox {
        origin: (x as u32, y as u32),
        size: (inset_width as u32, inset_height as u32),
    })
}

struct Tick {
    fraction: f64,
    text: Text,
}

/// The same literal string supplies measured raster ink and live SVG text.
struct Text {
    value: String,
    raster: TypstRasterOutput,
}

impl Text {
    fn new(value: String, font_px: f64) -> Result<Self, String> {
        let raster = render_raster(
            &literal_text_snippet(&value),
            font_px as f32,
            Color::BLACK,
            0.,
            "inset label",
        )
        .map_err(|error| error.to_string())?;
        Ok(Self { value, raster })
    }

    fn width(&self) -> f64 {
        self.raster.pixmap.width() as f64
    }

    fn height(&self) -> f64 {
        self.raster.pixmap.height() as f64
    }

    /// Keep the quantity readable above curves, with one point of white
    /// padding and the same measured box in raster and vector output.
    fn draw_boxed(
        &self,
        image: &mut Image,
        markup: &mut String,
        (x, y): (f64, f64),
        font_px: f64,
        padding: f64,
    ) {
        let origin = (x - padding, y - padding);
        let size = (
            self.width() + 2. * padding,
            self.height().max(font_px) + 2. * padding,
        );
        solid_rect(image, origin, size, 255);
        markup.push_str(&format!(
            r##"<rect class="inset-quantity-background" x="{:.3}" y="{:.3}" width="{:.3}" height="{:.3}" fill="#ffffff"/>"##,
            origin.0, origin.1, size.0, size.1
        ));
        self.draw(image, markup, (x, y), font_px, "inset-quantity-label");
    }

    fn draw(
        &self,
        image: &mut Image,
        markup: &mut String,
        (x, y): (f64, f64),
        font_px: f64,
        class: &str,
    ) {
        let pixmap = &self.raster.pixmap;
        blend(
            &mut image.pixels,
            (image.width, image.height),
            (pixmap.data(), pixmap.width(), pixmap.height()),
            (x, y),
        );
        markup.push_str(&format!(
            r##"<text class="{class}" x="{x:.3}" y="{:.3}" font-family="Arial, Helvetica, sans-serif" font-size="{font_px:.3}" fill="#000000">{}</text>"##,
            y + 0.8 * font_px,
            html(&self.value)
        ));
    }
}

/// Draw an antialiased grayscale rectangle on opaque RGBA.
fn solid_rect(image: &mut Image, (x, y): (f64, f64), (width, height): (f64, f64), gray: u8) {
    for py in y.floor().max(0.) as u32..((y + height).ceil() as u32).min(image.height) {
        for px in x.floor().max(0.) as u32..((x + width).ceil() as u32).min(image.width) {
            let coverage_x = (x + width).min(px as f64 + 1.) - x.max(px as f64);
            let coverage_y = (y + height).min(py as f64 + 1.) - y.max(py as f64);
            let coverage = (coverage_x * coverage_y).clamp(0., 1.);
            let start = ((py * image.width + px) * 4) as usize;
            for channel in &mut image.pixels[start..start + 3] {
                *channel =
                    (*channel as f64 * (1. - coverage) + gray as f64 * coverage).round() as u8;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantity_tags_cover_curves_with_padded_white_boxes_and_live_svg_text() {
        for value in ["μ(E)", "Δμ"] {
            let quantity = Text::new(value.into(), 16.).unwrap();
            let size = (
                quantity.width() as u32 + 12,
                quantity.height().max(16.) as u32 + 12,
            );
            let mut image = Image::new(
                size.0,
                size.1,
                [200, 0, 0, 255].repeat((size.0 * size.1) as usize),
            );
            let mut markup = String::new();
            quantity.draw_boxed(&mut image, &mut markup, (4., 4.), 16., 2.);
            let pixel = |x, y| {
                let index = ((y * size.0 + x) * 4) as usize;
                &image.pixels[index..index + 4]
            };
            assert_eq!(pixel(1, 1), [200, 0, 0, 255]);
            assert_eq!(pixel(2, 2), [255, 255, 255, 255]);
            assert_eq!(pixel(3, 3), [255, 255, 255, 255]);
            for y in 4..(4. + quantity.height().max(16.)) as u32 {
                for x in 4..(4. + quantity.width()) as u32 {
                    let pixel = pixel(x, y);
                    // Antialiased black text stays grayscale: none of the
                    // coloured curve underneath can bleed through the tag.
                    assert_eq!(pixel[0], pixel[1]);
                    assert_eq!(pixel[1], pixel[2]);
                    assert_eq!(pixel[3], 255);
                }
            }
            assert!(markup.starts_with("<rect class=\"inset-quantity-background\""));
            assert!(markup.contains("fill=\"#ffffff\"/><text class=\"inset-quantity-label\""));
            assert!(markup.ends_with(&format!(">{value}</text>")));
            assert!(!markup.contains("<image"));
        }
    }

    #[test]
    fn every_placement_keeps_the_whole_canvas_inside_the_main_axes() {
        let grid = Grid {
            columns: 1,
            rows: 1,
            cell_px: (1000, 750),
            dpi: 300.,
        };
        let frame = [0.5, 0.2, 0.3, 0.4];
        let placements = [
            InsetPlacement::LowerRight,
            InsetPlacement::UpperLeft,
            InsetPlacement::CenterRight,
        ];
        let boxes = placements
            .map(|placement| placement_box(grid.cell_px, frame, &grid, placement).unwrap());
        for bounds in boxes {
            assert_eq!(bounds.size, (347, 216));
            assert!(bounds.origin.0 >= 150 + 17);
            assert!(bounds.origin.1 >= 90 + 17);
            assert!(bounds.origin.0 + bounds.size.0 <= 940 - 17);
            assert!(bounds.origin.1 + bounds.size.1 <= 630 - 17);
        }
        assert_eq!(boxes[0].origin, (576, 397));
        assert_eq!(boxes[1].origin, (167, 107));
        assert_eq!(boxes[2].origin, (576, 252));
    }

    #[test]
    fn an_exhausted_frame_reports_a_geometry_error() {
        let grid = Grid {
            columns: 1,
            rows: 1,
            cell_px: (100, 100),
            dpi: 100.,
        };
        let error = placement_box(
            grid.cell_px,
            [0.5, 0.5, 0., 0.],
            &grid,
            InsetPlacement::LowerRight,
        )
        .unwrap_err();
        assert!(error.contains("more plot area"));
    }

    #[test]
    fn low_dpi_raster_blocks_are_averaged_without_changing_opacity() {
        let image = Image::new(
            2,
            2,
            vec![
                0, 0, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255, 255, 255, 255, 255,
            ],
        );
        let result = downsample(image, 2);
        assert_eq!((result.width, result.height), (1, 1));
        assert_eq!(result.pixels, [128, 128, 64, 255]);
    }
}
