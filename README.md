# quantlab

![CI](https://github.com/JanakUpd/quantlab/actions/workflows/ci.yml/badge.svg?branch=main)

A quantitative finance toolkit in Rust: European option pricing (closed-form Black–Scholes cross-checked by parallel Monte Carlo) and fixed-coupon bond analytics (yield, duration, DV01, convexity).

## Features

- **Black–Scholes** closed-form prices for European calls and puts
- **Monte Carlo** pricing with a reported standard error
- **Parallel simulation** with [rayon](https://github.com/rayon-rs/rayon), deterministic for a given seed regardless of thread count
- **Bond analytics**: price, yield to maturity, Macaulay and modified duration, DV01, convexity
- **Rate-shift sensitivity table** comparing exact repricing with duration and duration + convexity estimates
- **Tests** against reference values, put–call parity, numerical derivatives and statistical agreement between methods
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

```bash
cargo run --release -- bond --coupon 0.05 --maturity 5 --yield 0.05
```

```
Bond: face 100, coupon 5.00%, maturity 5.0y, freq 1
Yield:                5.0000%
Price:               100.0000
Macaulay duration:     4.5460
Modified duration:     4.3295
DV01:                  0.0433
Convexity:            23.9360

 Shift(bp)      Exact   Duration   Dur+Conv    Err(D)  Err(D+C)
      -300   114.1404   112.9884   114.0655   -1.1519   -0.0748
      -100   104.4518   104.3295   104.4492   -0.1223   -0.0027
       -10   100.4341   100.4329   100.4341   -0.0012   -0.0000
       +10    99.5682    99.5671    99.5682   -0.0012   +0.0000
      +100    95.7876    95.6705    95.7902   -0.1171   +0.0026
      +300    88.0219    87.0116    88.0887   -1.0103   +0.0668
```

The duration estimate always undershoots the true price, in both directions: price is convex in yield, so the tangent line lies below the curve. Adding the convexity term removes most of the error.

Pass `--price` instead of `--yield` to solve for yield to maturity:

```bash
cargo run --release -- bond --coupon 0.05 --maturity 5 --price 95.7876   # Yield: 6.0000%
```

| Flag | Meaning | Default |
|------|---------|--------:|
| `--face` | Face value | `100` |
| `--coupon` | Annual coupon rate | — |
| `--maturity` | Years to maturity | — |
| `--freq` | Coupons per year | `1` |
| `--yield` / `--price` | Yield to maturity, or market price to solve it from | — |
| `--shifts` | Yield shifts in basis points for the sensitivity table | `-300,-100,-10,10,100,300` |

### How it works

$$P(y) = \sum_k \frac{CF_k}{(1 + y/m)^{m t_k}}$$

where $m$ is the coupon frequency and $t_k$ the time of the $k$-th cash flow in years.

- **Yield to maturity** is found by Newton's method, $y_{n+1} = y_n + \frac{P(y_n) - P_{market}}{D_{mod}\,P(y_n)}$, starting from the coupon rate. Price is strictly decreasing and convex in yield, so the root is unique and Newton converges in a few steps; bisection is kept as a fallback if it fails to converge.
- **Macaulay duration** is the present-value-weighted average time of the cash flows; **modified duration** $D_{mod} = D_{mac}/(1 + y/m)$ is the price sensitivity, $dP/dy = -D_{mod} P$.
- **DV01** is the price change for a 1 bp move: $D_{mod} \cdot P \cdot 10^{-4}$.
- **Convexity** is the normalized second derivative of price:
  $$C = \frac{1}{P}\sum_k \frac{t_k (t_k + 1/m)\, CF_k}{(1 + y/m)^{m t_k + 2}}$$
- Together they give a second-order Taylor expansion:
  $$\frac{\Delta P}{P} \approx -D_{mod}\,\Delta y + \tfrac12 C\,(\Delta y)^2$$

## As a library

```rust
use quantlab::bonds::Bond;
use quantlab::options::analytic::black_scholes;
use quantlab::options::monte_carlo::monte_carlo_call_multithread;
use quantlab::options::option::EuropeanOption;

let opt = EuropeanOption { spot: 100.0, strike: 100.0, time: 1.0, rate: 0.05, vol: 0.2 };
let (call, put) = black_scholes(&opt);
let (mc_call, std_err) = monte_carlo_call_multithread(&opt, 1_000_000, 42);

let bond = Bond::new(100.0, 0.05, 5.0, 1);
let price = bond.price(0.05);
let ytm = bond.ytm(95.7876);
let duration = bond.modified_duration(0.05);
```

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
    └── bond.rs          # Bond: cash flows, price, YTM, duration, convexity
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

| Benchmark | Before | After | Speedup |
|-----------|-------:|------:|--------:|
| Price, 5y annual (5 cash flows) | 70.4 ns | 4.1 ns | 17× |
| Price, 30y semi-annual (60 cash flows) | 518 ns | 41.4 ns | 12.5× |
| Modified duration, 30y | 1.03 µs | 107 ns | 9.6× |
| Convexity, 30y | 1.04 µs | 114 ns | 9.1× |
| YTM by bisection, 30y | 23.3 µs | 3.42 µs | 6.8× |
| Shift table, 6 shifts, 30y | 27.7 µs | 2.64 µs | 10.5× |

Yield solvers, same bond priced at 7%:

| Bond | Cash flows | Bisection | Newton | Speedup |
|------|-----------:|----------:|-------:|--------:|
| 5y annual | 5 | 565 ns | 74 ns | 7.6× |
| 30y semi-annual | 60 | 3.44 µs | 731 ns | 4.7× |
| 5000y semi-annual | 10 000 | 493 µs | 135 µs | 3.7× |

Bisection always takes about 45 iterations; Newton needs 4–8, more for very long bonds, whose price curve behaves like a perpetuity and is far more convex. Racing the two solvers on separate threads was about 100× slower than Newton alone — spawning a thread costs much more than a sub-microsecond solve.

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

- cash flows, prices and durations against reference values, annual and semi-annual
- modified duration and convexity match numerical first and second derivatives
- the round trip `ytm(price(y)) = y`, including zero and negative yields
- zero-coupon Macaulay duration equals maturity; higher coupon means lower duration
- the duration-only estimate always undershoots the exact price, and convexity reduces the error
- Newton and bisection agree on yield to 1e-9 across zero, negative and high yields

## Roadmap

- [ ] Bonds: single-pass analytics (price, duration, convexity in one loop)
- [ ] Portfolio duration and immunization
- [ ] Key rate durations
- [ ] Antithetic variates for variance reduction
- [ ] Greeks, analytic and via finite differences
- [ ] Implied volatility solver
- [ ] Binomial tree for American options
- [x] Bonds: Newton's method for YTM, benchmarked against bisection
- [x] Bonds: allocation-free cash flows and iterative discount factors instead of `powf`
- [x] Benchmarks with criterion
- [x] Bond analytics: price, YTM, duration, DV01, convexity

## License

MIT
