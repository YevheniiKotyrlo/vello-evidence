//! The cycles vello_cpu's calling thread runs to render a 1024×1024 scene of 48 blurred rounded rectangles,
//! 100×60 with a corner radius of 8 and a standard deviation of 8, in each pipeline: the thread's own cycle count
//! (`QueryThreadCycleTime`) rather than wall-clock time, so other work on the machine moves the numbers little.
//!
//! Build it against `main` and against the pull request's branch, as Cargo.toml says, and run
//! `cargo run --release -- <label>` on Windows. Each line printed is the label, the pipeline, then the median and the
//! 10th and 90th percentiles of the cycles per render, in millions.

use windows_sys::Win32::System::Threading::GetCurrentThread;
use windows_sys::Win32::System::WindowsProgramming::QueryThreadCycleTime;
use vello_cpu::color::palette::css;
use vello_cpu::kurbo::Rect;
use vello_cpu::{Pixmap, RasterizerSettings, RenderContext, RenderMode, RenderSettings, Resources};

const SIZE: u16 = 1024;
const WARM_UP: usize = 20;
const SAMPLES: usize = 400;

fn scene(context: &mut RenderContext) {
    context.set_paint(css::BLACK.with_alpha(0.5));
    for row in 0..8 {
        for column in 0..6 {
            let x = 30.0 + 165.0 * f64::from(column);
            let y = 30.0 + 125.0 * f64::from(row);
            context.fill_blurred_rounded_rect(&Rect::new(x, y, x + 100.0, y + 60.0), 8.0, 8.0);
        }
    }
}

fn main() {
    let label = std::env::args().nth(1).unwrap_or_default();
    for (pipeline, render_mode) in [("u8", RenderMode::OptimizeSpeed), ("f32", RenderMode::OptimizeQuality)] {
        let settings = RenderSettings { num_threads: 0, ..Default::default() };
        let mut context = RenderContext::new_with(SIZE, SIZE, settings);
        let mut pixmap = Pixmap::new(SIZE, SIZE);
        let mut resources = Resources::new();
        let mut samples = Vec::with_capacity(SAMPLES);
        for iteration in 0..WARM_UP + SAMPLES {
            context.reset();
            let start = thread_cycles();
            scene(&mut context);
            context.flush();
            context.render_with(
                &mut pixmap,
                &mut resources,
                RasterizerSettings { render_mode, ..Default::default() },
            );
            let elapsed = (thread_cycles() - start) as f64 / 1e6;
            if iteration >= WARM_UP {
                samples.push(elapsed);
            }
        }
        samples.sort_by(f64::total_cmp);
        let quantile = |q: f64| samples[((samples.len() - 1) as f64 * q).round() as usize];
        println!("{label}\t{pipeline}\t{:.1}\t{:.1}\t{:.1}", quantile(0.5), quantile(0.1), quantile(0.9));
    }
}

fn thread_cycles() -> u64 {
    let mut cycles = 0;
    // SAFETY: the pseudo-handle of the calling thread, and a valid pointer to a u64.
    let succeeded = unsafe { QueryThreadCycleTime(GetCurrentThread(), &mut cycles) };
    assert_ne!(succeeded, 0, "QueryThreadCycleTime failed");
    cycles
}
