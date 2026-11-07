# quantlab

![CI](https://github.com/JanakUpd/quantlab/actions/workflows/ci.yml/badge.svg?branch=main)

A quantitative finance toolkit in Rust: European option pricing (closed-form Black–Scholes cross-checked by parallel Monte Carlo) and date-based bond analytics (cash-flow schedules, valuation on any date, accrued interest, clean and dirty price, yield, duration, DV01, convexity).

## Features

- **Black–Scholes** closed-form prices for European calls and puts
- **Monte Carlo** pricing with a reported standard error
- **Parallel simulation** with [rayon](https://github.com/rayon-rs/rayon), deterministic for a given seed regardless of thread count
- **Bonds with real dates**: issue and maturity dates, coupon schedules built by calendar months (annual, semi-annual, quarterly, monthly) or by a fixed number of days, and zero-coupon bonds
- **Valuation on any date**: price a bond at its issue or on any later settlement date, with accrued interest (НКД) and clean/dirty price
- **Bond analytics**: price, yield to maturity (from a dirty or a clean price), Macaulay and modified duration, DV01, convexity
- **Rate-shift sensitivity table** comparing exact repricing with duration and duration + convexity estimates
- **Explicit errors**: every public bond method returns `Result<_, BondError>`; invalid dates, rates and prices are reported, not panicked on
- **Optional cash-flow cache** ([moka](https://github.com/moka-rs/moka)) for repeated valuations of the same bond on the same dates; off by default
- **Tests** against independently computed reference values, put–call parity, numerical derivatives, statistical agreement between methods and two real bonds checked against a broker app
- **Benchmarks** with criterion

## Options

```bash
cargo run --release -- option --spot 100 --strike 100 --time 1 --rate 0.05 --vol 0.2 --paths 10000000000
```

```
Black-Scholes:  call = 10.4506   put = 5.5735
MC single:      call = 10.4507 ± 0.0001   (80.01s)
MC multi:       call = 10.4508 ± 0.0001   (9.91s)
Speedup: 8.08x
```

The simulation agrees with the analytic price within its standard error, and the parallel version is about 8× faster than the single-threaded one.

| Flag | Meaning | Default |
|------|---------|--------:|
| `--spot` | Current price of the underlying, S | `100` |
| `--strike` | Strike price, K | `100` |
| `--time` | Time to expiry in years, T | `1` |
| `--rate` | Risk-free rate, r | `0.05` |
| `--vol` | Volatility, σ | `0.2` |
| `--paths` | Monte Carlo paths | `1000000` |

### How it works

**Black–Scholes**

$$C = S\,N(d_1) - K e^{-rT} N(d_2), \qquad P = K e^{-rT} N(-d_2) - S\,N(-d_1)$$

$$d_1 = \frac{\ln(S/K) + (r + \sigma^2/2)\cdot T}{\sigma\sqrt{T}}, \qquad d_2 = d_1 - \sigma\sqrt{T}$$

where $N$ is the standard normal CDF.

**Monte Carlo**

Terminal prices are sampled under the risk-neutral measure:

$$S_T = S \exp\left(\left(r - \tfrac{\sigma^2}{2}\right)\cdot T + \sigma\cdot\sqrt{T} \cdot Z\right), \qquad Z \sim N(0, 1)$$

The price is the discounted mean payoff, $e^{-rT}\,\overline{\max(S_T - K, 0)}$, and the standard error is $e^{-rT}\,s/\sqrt{n}$, where $s$ is the sample standard deviation of the payoffs. Mean and variance are accumulated as running sums, so memory use does not grow with the number of paths.

**Parallelism**

Paths are split into chunks of 65 536. Each chunk gets its own RNG seeded with `seed + chunk_index`, so chunks are independent and the result does not depend on how rayon schedules them. Partial sums are collected in chunk order and added sequentially, which keeps the floating-point result reproducible.

### Convergence

The standard error shrinks as $1/\sqrt{n}$: every 100× more paths gives one more correct decimal place.

| Paths | Monte Carlo call | Std. error | Black–Scholes |
|------:|-----------------:|-----------:|--------------:|
| 10 000 000 | 10.4538 | 0.0047 | 10.4506 |
| 1 000 000 000 | 10.4505 | 0.0005 | 10.4506 |
| 10 000 000 000 | 10.4508 | 0.0001 | 10.4506 |

## Bonds

A bond is described by its issue date, maturity date, face value and a coupon rule:

| Rule | Meaning |
|------|---------|
| `Frequent` | Coupons every 12, 6, 3 or 1 calendar months (annual, semi-annual, quarterly, monthly) |
| `DateBased` | Coupons every fixed number of days, e.g. 182 |
| `ZeroCoupon` | A single payment of the face value at maturity |

```bash
cargo run --release -- bond --coupon 0.05 --issue 2025-06-15 --maturity 2030-06-15 --settlement 2027-09-01 --yield 0.05
```

```
Bond: face 100, issued 2025-06-15, matures 2030-06-15, Frequent { frequency: Annual, rate: 0.05 }
Settlement: 2027-09-01
Yield:                 5.0000%
Clean price:           99.9690
Accrued interest:       1.0656
Dirty price:          101.0346
Macaulay duration:      2.6485
Modified duration:      2.5223
DV01:                   0.0255
Convexity:              8.9604

Shift table (dirty prices):
 Shift(bp)      Exact   Duration   Dur+Conv    Err(D)  Err(D+C)
      -300   109.1065   108.6799   109.0873   -0.4266   -0.0192
      -100   103.6290   103.5830   103.6283   -0.0460   -0.0007
       -10   101.2899   101.2894   101.2899   -0.0005   -0.0000
       +10   100.7802   100.7797   100.7802   -0.0005   +0.0000
      +100    98.5307    98.4862    98.5314   -0.0446   +0.0007
      +300    93.7790    93.3893    93.7967   -0.3897   +0.0177
```

`--settlement` is the valuation date (default: the issue date). Only payments strictly after it are counted, so a coupon paid on the settlement date belongs to the seller. The dirty price is what the buyer pays; the clean price, dirty minus accrued interest, is what markets quote.

With the yield equal to the coupon the clean price is close to par (99.97 here) but not exactly 100: payment times are measured in actual days, and the leap day in 2028 shifts the later payments slightly.

The duration estimate always undershoots the true price, in both directions: price is convex in yield, so the tangent line lies below the curve. Adding the convexity term removes most of the error.

Pass `--price` instead of `--yield` to solve for yield to maturity. The price is a **clean** (quoted) price, in the same units as `--face`:

```bash
cargo run --release -- bond --coupon 0.05 --issue 2025-06-15 --maturity 2030-06-15 --settlement 2027-09-01 --price 99.9690   # Yield: 5.0000%
```

Coupons every fixed number of days, and a zero-coupon bond (no `--coupon`):

```bash
cargo run --release -- bond --coupon 0.071 --issue 2025-01-01 --maturity 2026-12-30 --days 182 --yield 0.08
cargo run --release -- bond --issue 2025-01-01 --maturity 2028-01-01 --yield 0.08
```

Invalid input (for example a settlement date at or after maturity, or a price no yield can produce) is reported as an error message and a non-zero exit code.

| Flag | Meaning | Default |
|------|---------|--------:|
| `--face` | Face value | `100` |
| `--coupon` | Annual coupon rate; omit for a zero-coupon bond | — |
| `--issue` | Issue date, `YYYY-MM-DD` | — |
| `--maturity` | Maturity date, `YYYY-MM-DD` | — |
| `--settlement` | Valuation date, `YYYY-MM-DD`; must be in `[issue, maturity)` | issue date |
| `--freq` | Coupons per year: 1, 2, 4 or 12 | `1` |
| `--days` | Coupon period in days (instead of `--freq`) | — |
| `--yield` / `--price` | Annual effective yield, or clean market price to solve it from | — |
| `--shifts` | Yield shifts in basis points for the sensitivity table | `-300,-100,-10,10,100,300` |

### How it works

**Cash flows.** Coupon dates are generated backwards from the maturity date. For calendar-month coupons the $k$-th date is computed as "maturity minus $k \cdot$ step months" directly from the maturity, not from the previous coupon date, so a coupon on the 31st returns to the 31st after a short month instead of drifting to the 28th. For day-based coupons it is "maturity minus $k \cdot$ interval days". Dates are generated while they fall strictly after the issue date; the last payment is the final coupon plus the face value.

The coupon amount is

$$\text{Frequent: } \frac{F\cdot r}{m}, \qquad \text{DateBased: } \frac{F\cdot r\cdot d}{365},$$

where $F$ is the face value, $r$ the annual rate, $m$ the number of coupons per year and $d$ the interval in days.

**Price.** Payment times are measured in actual days divided by 365 from the settlement date; payments on or before it are dropped:

$$P(y) = \sum_k \frac{CF_k}{(1 + y)^{t_k}}, \qquad t_k = \frac{\text{days}(\text{settlement}, \text{date}_k)}{365}$$

where $y$ is an annual effective yield. This is the dirty price.

- **Yield to maturity** is found by Newton's method, $y_{n+1} = y_n + \frac{P(y_n) - P_{market}}{D_{mod}\cdot P(y_n)}$, starting from the coupon rate. Price is strictly decreasing and convex in yield, so the root is unique and Newton converges in a few steps; bisection is kept as a fallback if it fails to converge.
- **Macaulay duration** is the present-value-weighted average time of the cash flows; **modified duration** $D_{mod} = D_{mac}/(1 + y)$ is the price sensitivity, $dP/dy = -D_{mod} P$.
- **DV01** is the price change for a 1 bp move: $D_{mod} \cdot P \cdot 10^{-4}$.
- **Convexity** is the normalized second derivative of price:
  $$C = \frac{1}{P}\sum_k \frac{t_k\cdot (t_k + 1)\cdot CF_k}{(1 + y)^{t_k + 2}}$$
- Together they give a second-order Taylor expansion:
  $$\frac{\Delta P}{P} \approx -D_{mod}\cdot \Delta y + \tfrac12 C\cdot (\Delta y)^2$$

**Accrued interest.** With $p$ and $n$ the previous and next coupon dates around the settlement date $s$:

$$\text{Frequent: } \frac{F\cdot r}{m}\cdot\frac{s - p}{n - p}, \qquad \text{DateBased: } F\cdot r\cdot\frac{s - p}{365}$$

The frequent-coupon formula is actual/actual by period, so accrued interest reaches exactly one coupon on the payment date; the day-based formula is actual/365, as for Russian federal bonds (ОФЗ). Accrued interest is 0 on a coupon date, on or before the issue date and for zero-coupon bonds. For the first period $p$ is the issue date.

**Clean and dirty price.** $P_{clean} = P_{dirty} - \text{accrued}$. The dirty price drops by a coupon on every payment date, the clean price does not, which is why markets quote it. `ytm_from_clean` converts a quote to a dirty price before solving for yield, so a quoted price is not mistaken for a full one.

**Cash-flow cache.** By default every pricing call computes the payment schedule from scratch. Methods that need the schedule several times (yield, durations, the shift table) compute it once per call and pass it down as a slice. For workloads that value the same bond on the same dates many times, `Bond::with_cache(capacity)` adds a bounded cache of discounted-time cash flows keyed by settlement date. Results are identical with and without it; see [Performance](#performance) for when it pays off.

**Errors.** Public methods return `Result<_, BondError>`:

| Error | When |
|-------|------|
| `MaturityBeforeIssue` | maturity is not after the issue date |
| `MaturityBeforeSettlement` | settlement date is not before maturity |
| `InvalidRate` | yield is `NaN`, infinite or at or below −100% |
| `InvalidMarketPrice` | price is `NaN`, non-positive, or outside what any yield in [−99.9%, 500%] can produce |
| `IndefinitePriceCalculated` | the price overflowed to a non-finite value |
| `NoConvergence` | the bisection solver did not converge |
| `DateOutOfRange` | a coupon date falls outside the range `chrono` supports |

The `*_from_flow` methods are low-level building blocks that work on a precomputed schedule and do not validate their input.

**Current limitations.** The day count is actual/365 (actual/actual by period for accrued interest on frequent coupons), there are no holiday calendars, and the settlement date is taken as given: there is no business-day or T+1 convention. A first coupon period shorter than the regular one still pays a full coupon.

## As a library

```rust
use chrono::NaiveDate;
use quantlab::bonds::{Bond, BondError, BondFrequency};
use quantlab::options::analytic::black_scholes;
use quantlab::options::monte_carlo::monte_carlo_call_multithread;
use quantlab::options::option::EuropeanOption;

let opt = EuropeanOption { spot: 100.0, strike: 100.0, time: 1.0, rate: 0.05, vol: 0.2 };
let (call, put) = black_scholes(&opt);
let (mc_call, std_err) = monte_carlo_call_multithread(&opt, 1_000_000, 42);

let issue = NaiveDate::from_ymd_opt(2025, 6, 15).unwrap();
let maturity = NaiveDate::from_ymd_opt(2030, 6, 15).unwrap();
let bond = Bond::frequent(issue, 100.0, maturity, BondFrequency::Annual, 0.05);
let cashflows = bond.cashflows().unwrap(); // Vec<(NaiveDate, f64)>
let settlement = NaiveDate::from_ymd_opt(2027, 9, 1).unwrap();

// every bond method returns Result<_, BondError>
let dirty = bond.price(0.05, settlement).unwrap();
let accrued = bond.accrued(settlement).unwrap();
let clean = bond.clean_price(0.05, settlement).unwrap();
let ytm = bond.ytm_from_clean(99.969, settlement).unwrap();
let duration = bond.modified_duration(0.05, settlement).unwrap();

// invalid input is an error, not a panic
assert_eq!(bond.price(0.05, maturity), Err(BondError::MaturityBeforeSettlement));

// Optional: cache cash flows per settlement date (up to 1024 dates)
let cached = bond.clone().with_cache(1024);
```

`Bond::date_based` and `Bond::zero_coupon` construct the other two kinds.

## Project structure

```
src/
├── lib.rs
├── main.rs              # CLI with `option` and `bond` subcommands
├── options/
│   ├── mod.rs
│   ├── option.rs        # EuropeanOption
│   ├── analytic.rs      # Black–Scholes
│   └── monte_carlo.rs   # single-threaded and parallel simulation
└── bonds/
    ├── mod.rs
    └── bond.rs          # Bond: coupon schedule, price, YTM, duration, convexity
tests/
├── pricing.rs
└── bonds.rs
benches/
├── pricing.rs
└── bonds.rs
```

## Performance

Measured with [criterion](https://github.com/bheisler/criterion.rs).

Numbers below come from a desktop Ryzen 7 9700X; run-to-run noise is around ±10%, so small differences should not be over-read. On an Apple M4 laptop the same benchmarks run roughly 20–30% faster, with the same relative effects (cache gain, one pass versus two, Newton versus bisection).

Options, ATM call, `cargo bench --bench pricing` (measured on Windows):

| Benchmark | Time | Throughput |
|-----------|-----:|-----------:|
| Black–Scholes (closed form) | 34 ns | — |
| Monte Carlo, 1M paths, single thread | 7.8 ms | 128 M paths/s |
| Monte Carlo, 1M paths, parallel | 1.0 ms | 975 M paths/s |
| Monte Carlo, 10M paths, single thread | 78.5 ms | 127 M paths/s |
| Monte Carlo, 10M paths, parallel | 9.9 ms | 1.0 G paths/s |

The closed-form formula is about 225 000× faster than a 1M-path simulation, which is why Monte Carlo is reserved for payoffs without an analytic solution.

Bonds, `cargo bench --bench bonds` (measured on Fedora Linux, settlement on the issue date unless noted). The bond is built once and reused, with the cash-flow cache off and on:

| Benchmark | No cache | Cached |
|-----------|---------:|-------:|
| Cash-flow schedule, 30y semi-annual (60 payments) | 440 ns | — |
| Price, 5y annual (5 payments) | 152 ns | 113 ns |
| Price, 30y semi-annual (60 payments) | 1.09 µs | 443 ns |
| Price, 30y, mid-life settlement | 909 ns | 262 ns |
| Modified duration, 30y | 1.47 µs | 813 ns |
| Convexity, 30y | 1.46 µs | 811 ns |
| Analytics (price, durations, convexity in one pass), 30y | 1.14 µs | 486 ns |
| Shift table, 6 shifts, 30y | 4.06 µs | 2.77 µs |

The shift table computes price, duration and convexity once in a single pass (`Bond::analytics`), then reprices only the shifted yields from one shared schedule.

Yield solvers, same bond priced at 7% (no cache):

| Bond | Payments | Bisection | Newton | Speedup |
|------|---------:|----------:|-------:|--------:|
| 5y annual | 5 | 2.13 µs | 557 ns | 3.8× |
| 30y semi-annual | 60 | 17.8 µs | 5.22 µs | 3.4× |
| 5000y semi-annual | 10 000 | 2.98 ms | 947 µs | 3.1× |

Bisection takes about 45 iterations; Newton needs far fewer. The solvers build the cash flows once and then iterate over a slice, so a cache saves only about 0.6 µs per call for the 30-year bond (5.22 → 4.51 µs); almost all of the time is the `powf` in the iterations.

**When the cache pays off.** Creating a cache costs about 3.3 µs and dropping it more, and the first call is a miss that costs extra:

| Cold benchmark (fresh bond per call) | No cache | Cached |
|--------------------------------------|---------:|-------:|
| Construct, 30y semi-annual | 6 ns | 3.3 µs |
| First price, 30y semi-annual | 1.10 µs | 1.78 µs |
| Construct + price + drop, 30y semi-annual | 1.11 µs | 10–16 µs |

Against a saving of roughly 0.6 µs per price or yield call (about 1.3 µs for a shift table), the cache breaks even after a few tens of repeated valuations of the same bond on the same date, and sooner for the heavier methods. For a single valuation, leave it off.

These numbers are for a date-aware model that discounts each payment with `powf`. An earlier version that assumed equally spaced payments in whole years was roughly an order of magnitude faster, because it needed no dates and no allocation; it was a special case, not a comparable model.

## Testing

```bash
cargo test
```

Options:

- prices against known reference values, including a non-unit maturity
- put–call parity, $C - P = S - K e^{-rT}$, on arbitrary parameters
- that Monte Carlo lands within three standard errors of the formula, for both versions
- that the parallel version is reproducible for a fixed seed

Bonds:

- cash-flow schedules: payment dates strictly increase and never fall on the issue date, end-of-month dates do not drift, a short first period when issued mid-period, exact day intervals for day-based coupons, a single payment for zero-coupon bonds, and an error when maturity is not after issue
- prices, durations and the shift table against reference values computed independently in Python (annual and semi-annual)
- valuation on a settlement date inside the bond's life: a coupon on the settlement date is excluded, mid-period and one-day-before-maturity references, and `ytm` round trip
- errors: `MaturityBeforeSettlement` from every method when settlement is not before maturity, `InvalidMarketPrice` for impossible prices
- accrued interest: zero at issue, on coupon dates and for zero-coupon bonds; reference values for annual, semi-annual, day-based and end-of-month schedules; grows monotonically within a period
- clean price: clean equals dirty on coupon dates, is continuous across a coupon date while the dirty price jumps, and `ytm_from_clean` round-trips on several settlement dates
- modified duration and convexity match numerical first and second derivatives
- the round trip `ytm(price(y)) = y`, including zero and negative yields
- zero-coupon Macaulay duration equals the time to maturity in days / 365; higher coupon means lower duration
- the duration-only estimate always undershoots the exact price, and convexity reduces the error
- Newton and bisection agree on yield to 1e-9 across zero, negative and high yields
- results with and without the cash-flow cache are identical, on a cache miss and on a hit, and errors are not cached
- a real ОФЗ-ПД checked against a broker app quote (clean price 842, accrued interest 5.7 ₽, yield 16.65%): the 182-day schedule hits the real coupon date, accrued interest and yield match to the app's rounding
- a real corporate bond with 30-day coupons and a fixed ruble coupon, checked the same way (accrued interest 6.09 ₽, yield 16.04%); the coupon rate has to be taken from the ruble amount, because the percentage shown by the app is rounded

## Roadmap

- [ ] Check a real bond with calendar-month coupons (`Frequent`); the real-bond checks so far cover `DateBased`
- [ ] Check prices and yields on a larger set of bonds with a script over market data
- [ ] Faster yield solver: one pass for price and weighted time, cheaper powers
- [ ] Proportional coupon for a shortened first period
- [ ] Amortizing and floating-rate bonds
- [ ] Portfolio duration and immunization
- [ ] Key rate durations
- [ ] Antithetic variates for variance reduction
- [ ] Greeks, analytic and via finite differences
- [ ] Implied volatility solver
- [ ] Binomial tree for American options
- [x] Bonds: `Result<_, BondError>` API with validation of dates, rates and prices instead of panics
- [x] Bonds: cash flows computed once per call, plus an optional cache for repeated valuations
- [x] Bonds: yield and accrued interest checked against a real ОФЗ and a real corporate bond
- [x] Bonds: valuation on an arbitrary settlement date
- [x] Bonds: accrued interest, clean and dirty price
- [x] Bonds: calendar dates, month-based and day-based coupon schedules, zero-coupon bonds
- [x] Bonds: single-pass analytics (price, duration, convexity in one loop)
- [x] Bonds: Newton's method for YTM, benchmarked against bisection
- [x] Benchmarks with criterion
- [x] Bond analytics: price, YTM, duration, DV01, convexity

## License

MIT