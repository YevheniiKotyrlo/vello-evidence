//! The before/after pair: a 140×100 black rounded rectangle, corner radius 20, blurred with a standard deviation of 12,
//! drawn on white by `fill_blurred_rounded_rect` on the left and through a `GaussianBlur` filter layer on the right.
//!
//! Build it against `main` and against the pull request's branch, as Cargo.toml says; each run writes `<label>.png` into
//! the folder it is given and prints the largest difference in alpha between the left half and the right half.
//!
//!   cargo run --release -- <label> <output folder>

use vello_cpu::color::palette::css;
use vello_cpu::filter_effects::{EdgeMode, Filter, FilterPrimitive};
use vello_cpu::kurbo::{Rect, RoundedRect, Shape};
use vello_cpu::{Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources};

const WIDTH: u16 = 480;
const HEIGHT: u16 = 240;
const HALF: f64 = 240.0;
const RADIUS: f64 = 20.0;
const STD_DEV: f32 = 12.0;

fn shape(offset: f64) -> Rect {
    Rect::new(50.0 + offset, 70.0, 190.0 + offset, 170.0)
}

fn render(draw_white: bool) -> Pixmap {
    let settings = RenderSettings { num_threads: 0, ..Default::default() };
    let mut context = RenderContext::new_with(WIDTH, HEIGHT, settings);
    if draw_white {
        context.set_paint(css::WHITE);
        context.fill_rect(&Rect::new(0.0, 0.0, f64::from(WIDTH), f64::from(HEIGHT)));
    }
    context.set_paint(css::BLACK);
    context.fill_blurred_rounded_rect(&shape(0.0), RADIUS as f32, STD_DEV);
    context.push_filter_layer(Filter::from_primitive(FilterPrimitive::gaussian_blur(STD_DEV, EdgeMode::None)));
    context.fill_path(&RoundedRect::from_rect(shape(HALF), RADIUS).to_path(0.1));
    context.pop_layer();
    context.flush();
    let mut pixmap = Pixmap::new(WIDTH, HEIGHT);
    context.render_with(
        &mut pixmap,
        &mut Resources::new(),
        RasterizerSettings { render_mode: RenderMode::OptimizeQuality, ..Default::default() },
    );
    pixmap
}

fn main() {
    let mut arguments = std::env::args().skip(1);
    let label = arguments.next().expect("a label");
    let folder = std::path::PathBuf::from(arguments.next().expect("an output folder"));

    let coverage = render(false);
    let data = coverage.data();
    let mut largest = (0_u8, 0_u16, 0_u16);
    for y in 0..HEIGHT {
        for x in 0..WIDTH / 2 {
            let left = data[usize::from(y) * usize::from(WIDTH) + usize::from(x)].a;
            let right = data[usize::from(y) * usize::from(WIDTH) + usize::from(x + WIDTH / 2)].a;
            let difference = left.abs_diff(right);
            if difference > largest.0 {
                largest = (difference, x, y);
            }
        }
    }
    println!(
        "{label}: largest alpha difference between fill_blurred_rounded_rect and the GaussianBlur filter: {} at ({}, {})",
        largest.0, largest.1, largest.2
    );

    let png = render(true).into_png().expect("the pixmap encodes");
    std::fs::write(folder.join(format!("{label}.png")), png).expect("the image is written");
}
