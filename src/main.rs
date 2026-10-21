use clap::{Parser, Subcommand};
use quantlab::bonds::Bond;
use quantlab::options::analytic::black_scholes;
use quantlab::options::monte_carlo::*;
use quantlab::options::option::EuropeanOption;
use std::time::Instant;

#[derive(Parser)]
#[command(name = "quantlab", version, about = "Option and bond analytics")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Option {
        #[arg(long, default_value_t = 100.0)]
        spot: f64,
        #[arg(long, default_value_t = 100.0)]
        strike: f64,
        #[arg(long, default_value_t = 1.0)]
        time: f64,
        #[arg(long, default_value_t = 0.05)]
        rate: f64,
        #[arg(long, default_value_t = 0.2)]
        vol: f64,
        #[arg(long, default_value_t = 1_000_000)]
        paths: usize,
    },
    Bond {
        #[arg(long, default_value_t = 100.0)]
        face: f64,
        #[arg(long)]
        coupon: f64,
        #[arg(long)]
        maturity: f64,
        #[arg(long, default_value_t = 1)]
        freq: u32,
        #[arg(
            long = "yield",
            conflicts_with = "price",
            required_unless_present = "price"
        )]
        yld: Option<f64>,
        #[arg(long)]
        price: Option<f64>,
        #[arg(
            long,
            value_delimiter = ',',
            allow_negative_numbers = true,
            default_value = "-300,-100,-10,10,100,300"
        )]
        shifts: Vec<f64>,
    },
}

fn run_option(p: EuropeanOption, paths: usize) {
    let (c, put) = black_scholes(&p);
    println!("Black-Scholes:  call = {c:.4}   put = {put:.4}");

    let start = Instant::now();
    let (c, se) = monte_carlo_call(&p, paths, 42);
    let single = start.elapsed();
    println!("MC single:      call = {c:.4} ± {se:.4}   ({single:.2?})");

    let start = Instant::now();
    let (c, se) = monte_carlo_call_multithread(&p, paths, 42);
    let multi = start.elapsed();
    println!("MC multi:       call = {c:.4} ± {se:.4}   ({multi:.2?})");

    println!(
        "Speedup: {:.2}x",
        single.as_secs_f64() / multi.as_secs_f64()
    );
}

fn run_bond(bond: Bond, yld: Option<f64>, price: Option<f64>, shifts: &[f64]) {
    let yld = match (yld, price) {
        (Some(y), _) => y,
        (None, Some(p)) => bond.ytm(p).unwrap_or_else(|| {
            eprintln!("error: no yield in [-99.9%, 500%] matches price {p}");
            std::process::exit(1);
        }),
        (None, None) => unreachable!("clap requires --yield or --price"),
    };
    println!(
        "Bond: face {}, coupon {:.2}%, maturity {:.1}y, freq {}",
        bond.face,
        bond.coupon_rate * 100.0,
        bond.maturity,
        bond.freq
    );
    println!("{:<20}{:>9.4}%", "Yield:", yld * 100.0);
    println!("{:<20}{:>10.4}", "Price:", bond.price(yld));
    println!(
        "{:<20}{:>10.4}",
        "Macaulay duration:",
        bond.macaulay_duration(yld)
    );
    println!(
        "{:<20}{:>10.4}",
        "Modified duration:",
        bond.modified_duration(yld)
    );
    println!("{:<20}{:>10.4}", "DV01:", bond.dv01(yld));
    println!("{:<20}{:>10.4}", "Convexity:", bond.convexity(yld));
    println!();
    println!(
        "{:>10} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "Shift(bp)", "Exact", "Duration", "Dur+Conv", "Err(D)", "Err(D+C)"
    );
    for x in bond.shift_table(yld, shifts) {
        println!(
            "{:>+10.0} {:>10.4} {:>10.4} {:>10.4} {:>+9.4} {:>+9.4}",
            x.shift_bp,
            x.exact,
            x.duration_est,
            x.convexity_est,
            x.duration_est - x.exact,
            x.convexity_est - x.exact
        );
    }
}

fn main() {
    match Cli::parse().command {
        Command::Option {
            spot,
            strike,
            time,
            rate,
            vol,
            paths,
        } => run_option(
            EuropeanOption {
                spot,
                strike,
                time,
                rate,
                vol,
            },
            paths,
        ),
        Command::Bond {
            face,
            coupon,
            maturity,
            freq,
            yld,
            price,
            shifts,
        } => run_bond(Bond::new(face, coupon, maturity, freq), yld, price, &shifts),
    }
}
