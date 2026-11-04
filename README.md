# quantlab

![CI](https://github.com/JanakUpd/quantlab/actions/workflows/ci.yml/badge.svg?branch=main)

A quantitative finance toolkit in Rust: European option pricing (closed-form Black–Scholes cross-checked by parallel Monte Carlo) and date-based bond analytics (cash-flow schedules, yield, duration, DV01, convexity).

## Features

- **Black–Scholes** closed-form prices for European calls and puts
- **Monte Carlo** pricing with a reported standard error
- **Parallel simulation** with [rayon](https://github.com/rayon-rs/rayon), deterministic for a given seed regardless of thread count
- **Bonds with real dates**: issue and maturity dates, coupon schedules built by calendar months (annual, semi-annual, quarterly, monthly) or by a fixed number of days, and zero-coupon bonds
- **Bond analytics**: price, yield to maturity, Macaulay and modified duration, DV01, convexity
- **Rate-shift sensitivity table** comparing exact repricing with duration and duration + convexity estimates
- **Tests** against independently computed reference values, put–call parity, numerical derivatives and statistical agreement between methods
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

$$d_1 = \frac{\ln(S/K) + (r + \sigma^2/2)\,T}{\sigma\sqrt{T}}, \qquad d_2 = d_1 - \sigma\sqrt{T}$$

where $N$ is the standard normal CDF.

**Monte Carlo**

Terminal prices are sampled under the risk-neutral measure:

$$S_T = S \exp\left(\left(r - \tfrac{\sigma^2}{2}\right)T + \sigma\sqrt{T}\,Z\right), \qquad Z \sim N(0, 1)$$

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
cargo run --release -- bond --coupon 0.05 --issue 2025-06-15 --maturity 2030-06-15 --yield 0.05
```

```
Bond: face 100, issued 2025-06-15, matures 2030-06-15, Frequent { frequency: Annual, rate: 0.05 }
Yield:                 5.0000%
Price:                 99.9879
Macaulay duration:      4.5484
Modified duration:      4.3318
DV01:                  0.0433
Convexity:             23.9599

 Shift(bp)      Exact   Duration   Dur+Conv    Err(D)  Err(D+C)
      -300   114.1347   112.9817   114.0598   -1.1530   -0.0749
      -100   104.4416   104.3192   104.4389   -0.1225   -0.0027
       -10   100.4222   100.4210   100.4222   -0.0012   -0.0000
       +10    99.5559    99.5547    99.5559   -0.0012   +0.0000
      +100    95.7738    95.6566    95.7764   -0.1172   +0.0026
      +300    88.0052    86.9940    88.0721   -1.0112   +0.0669
```

The price is 99.99 rather than exactly 100 even though the coupon equals the yield: payment times are measured in actual days, and the leap day in 2028 makes the last payments slightly later than whole years.

The duration estimate always undershoots the true price, in both directions: price is convex in yield, so the tangent line lies below the curve. Adding the convexity term removes most of the error.

Pass `--price` instead of `--yield` to solve for yield to maturity:

```bash
cargo run --release -- bond --coupon 0.05 --issue 2025-06-15 --maturity 2030-06-15 --price 95.7738   # Yield: 6.0000%
```

Coupons every fixed number of days, and a zero-coupon bond (no `--coupon`):

```bash
cargo run --release -- bond --coupon 0.071 --issue 2025-01-01 --maturity 2026-12-30 --days 182 --yield 0.08
cargo run --release -- bond --issue 2025-01-01 --maturity 2028-01-01 --yield 0.08
```

| Flag | Meaning | Default |
|------|---------|--------:|
| `--face` | Face value | `100` |
| `--coupon` | Annual coupon rate; omit for a zero-coupon bond | — |
| `--issue` | Issue date, `YYYY-MM-DD` | — |
| `--maturity` | Maturity date, `YYYY-MM-DD` | — |
| `--freq` | Coupons per year: 1, 2, 4 or 12 | `1` |
| `--days` | Coupon period in days (instead of `--freq`) | — |
| `--yield` / `--price` | Annual effective yield, or market price to solve it from | — |
| `--shifts` | Yield shifts in basis points for the sensitivity table | `-300,-100,-10,10,100,300` |

### How it works

**Cash flows.** Coupon dates are generated backwards from the maturity date. For calendar-month coupons the $k$-th date is computed as "maturity minus $k \cdot$ step months" directly from the maturity, not from the previous coupon date, so a coupon on the 31st returns to the 31st after a short month instead of drifting to the 28th. For day-based coupons it is "maturity minus $k \cdot$ interval days". Dates are generated while they fall strictly after the issue date; the last payment is the final coupon plus the face value.

The coupon amount is

$$\text{Frequent: } \frac{F\,r}{m}, \qquad \text{DateBased: } \frac{F\,r\,d}{365},$$

where $F$ is the face value, $r$ the annual rate, $m$ the number of coupons per year and $d$ the interval in days.

**Price.** Payment times are measured in actual days divided by 365 from the valuation date (currently the issue date):

$$P(y) = \sum_k \frac{CF_k}{(1 + y)^{t_k}}, \qquad t_k = \frac{\text{days}(\text{valuation}, \text{date}_k)}{365}$$

where $y$ is an annual effective yield.

- **Yield to maturity** is found by Newton's method, $y_{n+1} = y_n + \frac{P(y_n) - P_{market}}{D_{mod}\,P(y_n)}$, starting from the coupon rate. Price is strictly decreasing and convex in yield, so the root is unique and Newton converges in a few steps; bisection is kept as a fallback if it fails to converge.
- **Macaulay duration** is the present-value-weighted average time of the cash flows; **modified duration** $D_{mod} = D_{mac}/(1 + y)$ is the price sensitivity, $dP/dy = -D_{mod} P$.
- **DV01** is the price change for a 1 bp move: $D_{mod} \cdot P \cdot 10^{-4}$.
- **Convexity** is the normalized second derivative of price:
  $$C = \frac{1}{P}\sum_k \frac{t_k (t_k + 1)\, CF_k}{(1 + y)^{t_k + 2}}$$
- Together they give a second-order Taylor expansion:
  $$\frac{\Delta P}{P} \approx -D_{mod}\,\Delta y + \tfrac12 C\,(\Delta y)^2$$

**Current limitations.** Prices are valued on the issue date, so there is no accrued interest yet and the price equals the dirty price. The day count is fixed at actual/365 and there are no holiday calendars. A first coupon period shorter than the regular one still pays a full coupon.

## As a library

```rust
use chrono::NaiveDate;
use quantlab::bonds::{Bond, BondFrequency};
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
let price = bond.price(0.05);
let ytm = bond.ytm(95.7738);
let duration = bond.modified_duration(0.05);
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

Options, ATM call, `cargo bench --bench pricing` on Ryzen 7 9700X:

| Benchmark | Time | Throughput |
|-----------|-----:|-----------:|
| Black–Scholes (closed form) | 34 ns | — |
| Monte Carlo, 1M paths, single thread | 7.8 ms | 128 M paths/s |
| Monte Carlo, 1M paths, parallel | 1.0 ms | 975 M paths/s |
| Monte Carlo, 10M paths, single thread | 78.5 ms | 127 M paths/s |
| Monte Carlo, 10M paths, parallel | 9.9 ms | 1.0 G paths/s |

The closed-form formula is about 225 000× faster than a 1M-path simulation, which is why Monte Carlo is reserved for payoffs without an analytic solution.

Bonds, `cargo bench --bench bonds` on Ryzen 7 9700X:

| Benchmark | Time |
|-----------|-----:|
| Cash-flow schedule, 30y semi-annual (60 payments) | 456 ns |
| Price, 5y annual (5 payments) | 160 ns |
| Price, 30y semi-annual (60 payments) | 1.13 µs |
| Modified duration, 30y | 2.25 µs |
| Convexity, 30y | 2.25 µs |
| Shift table, 6 shifts, 30y | 8.13 µs |

The shift table computes price, duration and convexity once in a single pass (`Bond::analytics`), then reprices only the shifted yields.

Yield solvers, same bond priced at 7%:

| Bond | Payments | Bisection | Newton | Speedup |
|------|---------:|----------:|-------:|--------:|
| 5y annual | 5 | 7.24 µs | 1.92 µs | 3.8× |
| 30y semi-annual | 60 | 50.6 µs | 13.5 µs | 3.7× |
| 5000y semi-annual | 10 000 | 7.41 ms | 1.96 ms | 3.8× |

Bisection takes about 45 iterations; Newton needs far fewer.

These numbers are for a date-aware model: every call rebuilds the payment schedule (about 40% of the price time for the 30-year bond) and discounts each payment with `powf`. An earlier version that assumed equally spaced payments in whole years was roughly an order of magnitude faster, because it needed no dates and no allocation; it was a special case, not a comparable model. Computing the schedule once and reusing it across calls is the next planned optimization.

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
- modified duration and convexity match numerical first and second derivatives
- the round trip `ytm(price(y)) = y`, including zero and negative yields
- zero-coupon Macaulay duration equals the time to maturity in days / 365; higher coupon means lower duration
- the duration-only estimate always undershoots the exact price, and convexity reduces the error
- Newton and bisection agree on yield to 1e-9 across zero, negative and high yields

## Roadmap

- [ ] Valuation on an arbitrary settlement date (instead of the issue date)
- [ ] Accrued interest, clean and dirty price
- [ ] Check prices and yields against quotes of real bonds
- [ ] Compute the cash-flow schedule once and reuse it across calls
- [ ] Return `Result` from pricing instead of panicking on invalid dates
- [ ] Proportional coupon for a shortened first period
- [ ] Amortizing and floating-rate bonds
- [ ] Portfolio duration and immunization
- [ ] Key rate durations
- [ ] Antithetic variates for variance reduction
- [ ] Greeks, analytic and via finite differences
- [ ] Implied volatility solver
- [ ] Binomial tree for American options
- [x] Bonds: calendar dates, month-based and day-based coupon schedules, zero-coupon bonds
- [x] Bonds: single-pass analytics (price, duration, convexity in one loop)
- [x] Bonds: Newton's method for YTM, benchmarked against bisection
- [x] Benchmarks with criterion
- [x] Bond analytics: price, YTM, duration, DV01, convexity

## License

MIT