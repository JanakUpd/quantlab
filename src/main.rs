use chrono::NaiveDate;
use clap::{Parser, Subcommand};
use quantlab::bonds::{Bond, BondError, BondFrequency};
use quantlab::options::analytic::black_scholes;
use quantlab::options::monte_carlo::*;
use quantlab::options::option::EuropeanOption;
use std::num::NonZeroU16;
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
        /// Annual coupon rate; omit for a zero-coupon bond
        #[arg(long)]
        coupon: Option<f64>,
        /// Issue date, YYYY-MM-DD
        #[arg(long)]
        issue: NaiveDate,
        /// Maturity date, YYYY-MM-DD
        #[arg(long)]
        maturity: NaiveDate,
        /// Valuation (settlement) date, YYYY-MM-DD; defaults to the issue date
        #[arg(long)]
        settlement: Option<NaiveDate>,
        /// Coupons per year: 1, 2, 4 or 12
        #[arg(long, default_value_t = 1, conflicts_with = "days")]
        freq: u32,
        /// Coupon period in days (e.g. 182 for OFZ) instead of --freq
        #[arg(long)]
        days: Option<NonZeroU16>,
        #[arg(
            long = "yield",
            conflicts_with = "price",
            required_unless_present = "price"
        )]
        yld: Option<f64>,
        /// Clean (quoted) price, in the same units as --face
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

fn fail(msg: String) -> ! {
    eprintln!("error: {msg}");
    std::process::exit(1);
}

fn run_bond(
    bond: Bond,
    settlement: NaiveDate,
    yld: Option<f64>,
    price: Option<f64>,
    shifts: &[f64],
) -> Result<(), BondError> {
    if settlement < bond.get_issue_date() {
        fail(format!(
            "settlement {settlement} is before issue date {}",
            bond.get_issue_date()
        ));
    }

    // --price is a clean (quoted) price, in the same units as --face
    let yld = match (yld, price) {
        (Some(y), _) => y,
        (None, Some(p)) => bond.ytm_from_clean(p, settlement)?,
        (None, None) => unreachable!("clap requires --yield or --price"),
    };

    let an = bond.analytics(yld, settlement)?;

    println!(
        "Bond: face {}, issued {}, matures {}, {:?}",
        bond.get_face(),
        bond.get_issue_date(),
        bond.get_maturity(),
        bond.get_nature()
    );
    println!("Settlement: {settlement}");
    println!("{:<20}{:>9.4}%", "Yield:", yld * 100.0);
    println!(
        "{:<20}{:>10.4}",
        "Clean price:",
        bond.clean_price(yld, settlement)?
    );
    println!(
        "{:<20}{:>10.4}",
        "Accrued interest:",
        bond.accrued(settlement)?
    );
    println!("{:<20}{:>10.4}", "Dirty price:", an.price);
    println!("{:<20}{:>10.4}", "Macaulay duration:", an.macaulay_duration);
    println!("{:<20}{:>10.4}", "Modified duration:", an.modified_duration);
    println!("{:<20}{:>10.4}", "DV01:", an.dv01);
    println!("{:<20}{:>10.4}", "Convexity:", an.convexity);
    println!();
    println!("Shift table (dirty prices):");
    println!(
        "{:>10} {:>10} {:>10} {:>10} {:>9} {:>9}",
        "Shift(bp)", "Exact", "Duration", "Dur+Conv", "Err(D)", "Err(D+C)"
    );
    for x in bond.shift_table(yld, shifts, settlement)? {
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
    Ok(())
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
            issue,
            maturity,
            settlement,
            freq,
            days,
            yld,
            price,
            shifts,
        } => {
            let bond = match (coupon, days) {
                (None, _) => Bond::zero_coupon(issue, face, maturity),
                (Some(rate), Some(d)) => Bond::date_based(issue, face, maturity, d, rate),
                (Some(rate), None) => {
                    let frequency = match freq {
                        1 => BondFrequency::Annual,
                        2 => BondFrequency::SemiAnnual,
                        4 => BondFrequency::Quarterly,
                        12 => BondFrequency::Monthly,
                        other => fail(format!("--freq must be 1, 2, 4 or 12, got {other}")),
                    };
                    Bond::frequent(issue, face, maturity, frequency, rate)
                }
            };
            if let Err(e) = run_bond(bond, settlement.unwrap_or(issue), yld, price, &shifts) {
                fail(e.to_string());
            }
        }
    }
}
