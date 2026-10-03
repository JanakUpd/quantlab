# quantlab

![CI](https://github.com/JanakUpd/quantlab/actions/workflows/ci.yml/badge.svg?branch=main)

A European option pricer in Rust. It prices calls and puts with the closed-form Black–Scholes formula and cross-checks the result with a Monte Carlo simulation that runs in parallel across all CPU cores.

## Example

```bash
cargo run --release -- --spot 100 --strike 100 --time 1 --rate 0.05 --vol 0.2 --paths 10000000000
```

```
Black-Scholes:  call = 10.4506   put = 5.5735
MC single:      call = 10.4507 ± 0.0001   (80.01s)
MC multi:       call = 10.4508 ± 0.0001   (9.91s)
Speedup: 8.08x
```

The simulation agrees with the analytic price within its standard error, and the parallel version is about 8× faster than the single-threaded one.

## Features

- **Black–Scholes** closed-form prices for European calls and puts
- **Monte Carlo** pricing with a reported standard error
- **Parallel simulation** with [rayon](https://github.com/rayon-rs/rayon), deterministic for a given seed regardless of thread count
- **Tests** against reference values, put–call parity and statistical agreement between the two methods

## Usage

| Flag | Meaning | Example |
|------|---------|---------|
| `--spot` | Current price of the underlying, S | `100` |
| `--strike` | Strike price, K | `100` |
| `--time` | Time to expiry in years, T | `1` |
| `--rate` | Risk-free rate, r | `0.05` |
| `--vol` | Volatility, σ | `0.2` |
| `--paths` | Monte Carlo paths (default 1 000 000) | `1000000000` |

As a library:

```rust
use quantlab::analytic::black_scholes;
use quantlab::monte_carlo::monte_carlo_call_multithread;
use quantlab::params::OptionParams;

let p = OptionParams { spot: 100.0, strike: 100.0, time: 1.0, rate: 0.05, vol: 0.2 };

let (call, put) = black_scholes(&p);
let (mc_call, std_err) = monte_carlo_call_multithread(&p, 1_000_000, 42);
```

## How it works

### Black–Scholes

$$C = S\,N(d_1) - K e^{-rT} N(d_2), \qquad P = K e^{-rT} N(-d_2) - S\,N(-d_1)$$

$$d_1 = \frac{\ln(S/K) + (r + \sigma^2/2)\,T}{\sigma\sqrt{T}}, \qquad d_2 = d_1 - \sigma\sqrt{T}$$

where $N$ is the standard normal CDF.

### Monte Carlo

Terminal prices are sampled under the risk-neutral measure:

$$S_T = S \exp\left(\left(r - \tfrac{\sigma^2}{2}\right)T + \sigma\sqrt{T}\,Z\right), \qquad Z \sim N(0, 1)$$

The price is the discounted mean payoff, $e^{-rT}\,\overline{\max(S_T - K, 0)}$, and the standard error is $e^{-rT}\,s/\sqrt{n}$, where $s$ is the sample standard deviation of the payoffs. Mean and variance are accumulated as running sums, so memory use does not grow with the number of paths.

### Parallelism

Paths are split into chunks of one million. Each chunk gets its own RNG seeded with `seed + chunk_index`, so chunks are independent and the result does not depend on how rayon schedules them. Partial sums are collected in chunk order and added sequentially, which keeps the floating-point result reproducible.

## Convergence

The standard error shrinks as $1/\sqrt{n}$: every 100× more paths gives one more correct decimal place.

| Paths | Monte Carlo call | Std. error | Black–Scholes |
|------:|-----------------:|-----------:|--------------:|
| 10 000 000 | 10.4538 | 0.0047 | 10.4506 |
| 1 000 000 000 | 10.4505 | 0.0005 | 10.4506 |
| 10 000 000 000 | 10.4508 | 0.0001 | 10.4506 |

## Project structure

```
src/
├── lib.rs           # public API
├── params.rs        # OptionParams
├── analytic.rs      # Black–Scholes formula
├── monte_carlo.rs   # single-threaded and parallel simulation
└── main.rs          # command-line interface
tests/
└── pricing.rs       # integration tests
```

## Performance

Measured with [criterion](https://github.com/bheisler/criterion.rs), ATM call, `cargo bench` on Ryzen 7 9700x:

| Benchmark | Time | Throughput |
|-----------|-----:|-----------:|
| Black–Scholes (closed form) | 34 ns | — |
| Monte Carlo, 1M paths, single thread | 7.8 ms | 128 M paths/s |
| Monte Carlo, 1M paths, parallel | 1.0 ms | 975 M paths/s |
| Monte Carlo, 10M paths, single thread | 78.5 ms | 127 M paths/s |
| Monte Carlo, 10M paths, parallel | 9.9 ms | 1.0 G paths/s |

The closed-form formula is about 225 000× faster than a 1M-path simulation, which is why Monte Carlo is reserved for payoffs without an analytic solution.

## Testing

```bash
cargo test
```

The suite checks:

- prices against known reference values, including a non-unit maturity
- put–call parity, $C - P = S - K e^{-rT}$, on arbitrary parameters
- that Monte Carlo lands within three standard errors of the formula, for both versions
- that the parallel version is reproducible for a fixed seed

## Roadmap

- [ ] Antithetic variates for variance reduction
- [ ] Greeks, analytic and via finite differences
- [ ] Implied volatility solver
- [ ] Binomial tree for American options
- [x] Benchmarks with criterion

## License

MIT