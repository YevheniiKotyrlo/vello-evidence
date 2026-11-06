//! How close vello's analytic blurred rounded rectangle is to the exact Gaussian blur of standard deviation σ.
//!
//! The approximation is vello_common's `EncodeExt for BlurredRoundedRectangle` and vello_cpu's fine stage, written
//! in f64 with vello's `compute_erf7`. It is evaluated in four parametrizations:
//!
//! - `base`: every term takes σ, and the painted extent is 2.5·σ (vello main at the time of writing);
//! - `std_dev_inv alone`: `std_dev_inv` takes 1 / (σ·√2), so `scale`, `delta`'s exponentials and the fine stage's `erf`
//!   follow it, while `r0`, `r1` and `delta`'s factor 1.25·σ keep σ, and the extent is 2.5·σ;
//! - `std_dev_inv alone + extent`: the same with the extent 3·σ;
//! - `head`: every term takes s = σ·√2, the scale blurrr fitted the approximation to, and the extent is 3·σ.
//!
//! The exact blur integrates the rounded rectangle's horizontal slices, each blurred exactly by `erf`, against the
//! Gaussian of standard deviation σ by composite Simpson quadrature.
//!
//! `cargo run --release` prints the tables; `cargo run --release -- <width> <height> <radius> <σ>` prints one case,
//! with where each variant's largest difference sits, measured from the rectangle's centre.

use core::f64::consts::{FRAC_2_SQRT_PI, PI, SQRT_2};

#[derive(Clone, Copy)]
enum Variant {
    Base,
    StdDevInvAlone,
    StdDevInvAloneAndExtent,
    Head,
}

impl Variant {
    const ALL: [Self; 4] = [Self::Base, Self::StdDevInvAlone, Self::StdDevInvAloneAndExtent, Self::Head];

    fn name(self) -> &'static str {
        match self {
            Self::Base => "base",
            Self::StdDevInvAlone => "std_dev_inv alone",
            Self::StdDevInvAloneAndExtent => "std_dev_inv alone + extent",
            Self::Head => "head",
        }
    }

    /// The scale of the shape terms (`r0`, `r1`, `delta`'s factor) and of the `erf` terms, for a standard deviation.
    fn scales(self, standard_deviation: f64) -> (f64, f64) {
        let fitted = standard_deviation * SQRT_2;
        match self {
            Self::Base => (standard_deviation, standard_deviation),
            Self::StdDevInvAlone | Self::StdDevInvAloneAndExtent => (standard_deviation, fitted),
            Self::Head => (fitted, fitted),
        }
    }

    fn extent(self, standard_deviation: f64) -> f64 {
        match self {
            Self::Base | Self::StdDevInvAlone => 2.5 * standard_deviation,
            Self::StdDevInvAloneAndExtent | Self::Head => 3.0 * standard_deviation,
        }
    }
}

#[derive(Clone, Copy)]
struct Case {
    width: f64,
    height: f64,
    radius: f64,
    standard_deviation: f64,
}

struct Encoded {
    exponent: f64,
    recip_exponent: f64,
    scale: f64,
    erf_scale_inverse: f64,
    min_edge: f64,
    w: f64,
    h: f64,
    r1: f64,
}

fn compute_erf7(x: f64) -> f64 {
    let x = x.clamp(-10.0, 10.0) * FRAC_2_SQRT_PI;
    let xx = x * x;
    let x = x + (0.24295 + (0.03395 + 0.0104 * xx) * xx) * (x * xx);
    x / (1.0 + x * x).sqrt()
}

fn encode(case: Case, variant: Variant) -> Encoded {
    let radius = case.radius.min(0.5 * case.width.min(case.height));
    let (shape_scale, erf_scale) = variant.scales(case.standard_deviation.max(1e-6));
    let min_edge = case.width.min(case.height);
    let rmax = 0.5 * min_edge;
    let r0 = radius.hypot(shape_scale * 1.15).min(rmax);
    let r1 = radius.hypot(shape_scale * 2.0).min(rmax);
    let exponent = 2.0 * r1 / r0;
    let erf_scale_inverse = erf_scale.recip();
    let delta = 1.25
        * shape_scale
        * ((-(0.5 * erf_scale_inverse * case.width).powi(2)).exp()
            - (-(0.5 * erf_scale_inverse * case.height).powi(2)).exp());
    let w = case.width + delta.min(0.0);
    let h = case.height - delta.max(0.0);
    let scale = 0.5 * compute_erf7(erf_scale_inverse * 0.5 * (w.max(h) - 0.5 * radius));
    Encoded {
        exponent,
        recip_exponent: exponent.recip(),
        scale,
        erf_scale_inverse,
        min_edge,
        w,
        h,
        r1,
    }
}

/// The fine stage's coverage at `(x, y)`, measured from the rectangle's centre.
fn approximate(encoded: &Encoded, x: f64, y: f64) -> f64 {
    let y0 = encoded.r1 + y.abs() - 0.5 * encoded.h;
    let y1 = y0.max(0.0);
    let x0 = encoded.r1 + x.abs() - 0.5 * encoded.w;
    let x1 = x0.max(0.0);
    let d_pos = (x1.powf(encoded.exponent) + y1.powf(encoded.exponent)).powf(encoded.recip_exponent);
    let d_neg = x0.max(y0).min(0.0);
    let d = d_pos + d_neg - encoded.r1;
    encoded.scale
        * (compute_erf7(encoded.erf_scale_inverse * (encoded.min_edge + d))
            - compute_erf7(encoded.erf_scale_inverse * d))
}

/// The exact Gaussian blur's coverage at `(x, y)`, measured from the rectangle's centre.
fn exact(case: Case, x: f64, y: f64) -> f64 {
    let sigma = case.standard_deviation;
    let half_width = 0.5 * case.width;
    let half_height = 0.5 * case.height;
    let radius = case.radius.min(half_width.min(half_height));
    let low = (-half_height).max(y - 9.0 * sigma);
    let high = half_height.min(y + 9.0 * sigma);
    if low >= high {
        return 0.0;
    }
    let slice = |v: f64| {
        let corner = v.abs() - (half_height - radius);
        let half_extent = if corner <= 0.0 {
            half_width
        } else {
            half_width - radius + (radius * radius - corner * corner).max(0.0).sqrt()
        };
        let erf_scale = sigma * SQRT_2;
        0.5 * (libm::erf((x + half_extent) / erf_scale) - libm::erf((x - half_extent) / erf_scale))
    };
    let gaussian = |t: f64| (-(t * t) / (2.0 * sigma * sigma)).exp() / (sigma * (2.0 * PI).sqrt());
    let steps = 4096;
    let step = (high - low) / f64::from(steps);
    let mut sum = 0.0;
    for index in 0..=steps {
        let v = low + step * f64::from(index);
        let weight = if index == 0 || index == steps {
            1.0
        } else if index % 2 == 1 {
            4.0
        } else {
            2.0
        };
        sum += weight * slice(v) * gaussian(y - v);
    }
    sum * step / 3.0
}

struct Error {
    max: f64,
    root_mean_square: f64,
    at: (f64, f64),
}

/// The drawn coverage against the exact blur over every pixel centre within 3.6·σ of the rectangle, in 1/255 units.
fn error(case: Case, variant: Variant) -> Error {
    let encoded = encode(case, variant);
    let extent = variant.extent(case.standard_deviation);
    let margin = (3.6 * case.standard_deviation).ceil() + 1.0;
    let half_width = 0.5 * case.width;
    let half_height = 0.5 * case.height;
    let mut max: f64 = 0.0;
    let mut at = (0.0, 0.0);
    let mut squares = 0.0;
    let mut count = 0.0;
    let mut y = -half_height - margin + 0.5;
    while y < half_height + margin {
        let mut x = -half_width - margin + 0.5;
        while x < half_width + margin {
            let painted = x.abs() <= half_width + extent && y.abs() <= half_height + extent;
            let drawn = if painted { approximate(&encoded, x, y) } else { 0.0 };
            let difference = (drawn - exact(case, x, y)).abs() * 255.0;
            if difference > max {
                max = difference;
                at = (x, y);
            }
            squares += difference * difference;
            count += 1.0;
            x += 1.0;
        }
        y += 1.0;
    }
    Error {
        max,
        root_mean_square: (squares / count).sqrt(),
        at,
    }
}

fn main() {
    let arguments: Vec<f64> = std::env::args().skip(1).map(|argument| argument.parse().expect("a number")).collect();
    if let [width, height, radius, standard_deviation] = arguments[..] {
        let case = Case { width, height, radius, standard_deviation };
        print!("| {width} × {height}, {radius}, {standard_deviation} |");
        for variant in Variant::ALL {
            let error = error(case, variant);
            print!(" {:.2} / {:.3} at ({}, {}) |", error.max, error.root_mean_square, error.at.0, error.at.1);
        }
        println!();
        return;
    }
    let cases = [
        // vello_tests' blurred_rounded_rect snapshots: a 60×60 rectangle.
        Case { width: 60.0, height: 60.0, radius: 0.0, standard_deviation: 5.0 },
        Case { width: 60.0, height: 60.0, radius: 0.0, standard_deviation: 10.0 },
        Case { width: 60.0, height: 60.0, radius: 0.0, standard_deviation: 20.0 },
        Case { width: 60.0, height: 60.0, radius: 10.0, standard_deviation: 10.0 },
        Case { width: 60.0, height: 60.0, radius: 30.0, standard_deviation: 10.0 },
        // linebender/vello#2043's reproduction.
        Case { width: 100.0, height: 100.0, radius: 0.0, standard_deviation: 8.0 },
        // A long, thin card shadow, a rectangle smaller than its blur, and a large one with a small blur.
        Case { width: 200.0, height: 40.0, radius: 8.0, standard_deviation: 6.0 },
        Case { width: 20.0, height: 20.0, radius: 4.0, standard_deviation: 10.0 },
        Case { width: 300.0, height: 300.0, radius: 50.0, standard_deviation: 3.0 },
        // vello_common's probe cell.
        Case { width: 8.0, height: 8.0, radius: 2.0, standard_deviation: 0.75 },
    ];

    println!("Drawn coverage against the exact Gaussian blur of standard deviation σ, in 1/255: max / RMS over every");
    println!("pixel centre within 3.6·σ of the rectangle. Each variant paints its own extent (see the module doc).");
    println!();
    print!("| w × h, radius, σ |");
    for variant in Variant::ALL {
        print!(" {} |", variant.name());
    }
    println!();
    println!("| --- | --- | --- | --- | --- |");
    for case in cases {
        print!(
            "| {} × {}, {}, {} |",
            case.width, case.height, case.radius, case.standard_deviation
        );
        for variant in Variant::ALL {
            let error = error(case, variant);
            print!(" {:.2} / {:.3} at ({}, {}) |", error.max, error.root_mean_square, error.at.0, error.at.1);
        }
        println!();
    }

    println!();
    println!("Where the extent cuts the blur: the exact coverage at the middle of a side, at each extent, in 1/255.");
    println!("The inverse paints 1 − coverage, so the same number is its step against a caller's fill beyond the extent.");
    println!();
    println!("| σ | at 2.5·σ | at 3·σ | at 2.5·√2·σ |");
    println!("| --- | --- | --- | --- |");
    for standard_deviation in [1.0, 5.0, 10.0, 20.0] {
        let case = Case { width: 400.0, height: 400.0, radius: 0.0, standard_deviation };
        let at = |distance: f64| exact(case, 200.0 + distance, 0.0) * 255.0;
        println!(
            "| {} | {:.3} | {:.3} | {:.4} |",
            standard_deviation,
            at(2.5 * standard_deviation),
            at(3.0 * standard_deviation),
            at(2.5 * SQRT_2 * standard_deviation)
        );
    }

    println!();
    println!("linebender/vello#2043's row: 100×100 square, σ = 8, the pixel d px outside its right edge, at its centre row.");
    println!();
    println!("| d | exact | base | head |");
    println!("| --- | --- | --- | --- |");
    let case = Case { width: 100.0, height: 100.0, radius: 0.0, standard_deviation: 8.0 };
    let base = encode(case, Variant::Base);
    let head = encode(case, Variant::Head);
    for distance in [0.0, 4.0, 8.0, 12.0, 16.0] {
        let x = 50.0 + distance + 0.5;
        println!(
            "| {} | {:.3} | {:.3} | {:.3} |",
            distance,
            exact(case, x, 0.5),
            approximate(&base, x, 0.5),
            approximate(&head, x, 0.5)
        );
    }
}
