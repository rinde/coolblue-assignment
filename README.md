# Optimizing TSP with a twist with Simulated Annealing
by Rinde van Lon

## Simulated Annealing

Simulated annealing is a proven local search metaheuristic that allows to converge to the global optimum. Simulated annealing is commonly used for the traveling salesman problem and similar problems.

The way that simulated annealing converges to the optimum is through gradually lowering the temperature (in analogy to annealing in metallurgy). The higher the temperature, the higher the probability is for accepting worse solutions (the exploration phase), the lower the temperature, the lower the probability for accepting worse solutions (the exploitation phase). Simulated annealing converges from a high to a low temperature, usually, the last few steps are equivalent to hill climbing.

To calculate the energy difference, the 'delta' between two solutions the score is normalized to a single number. The score consists of route length (medium score, to be maximized) and distance traveled (soft penalty, to be minimized). Since route length is strictly better than any distance improvement, the delta function heavily emphasizes changes in route length. A change in medium score is in the range `[1.0..∞)`, a change in soft score is in the range `[0.0,1.0]`. See `MediumSoft` in `optimizer/score.rs` for the details.

## Implementation

Features:
* Parses the files from the Homberger Benchmarks, the proportion of events that is a pickup is configurable in the CLI.
* Different scoring strategies, configurable via `--disable-partial-score`, the implementation is in `optimizer/state.rs`: `ScoreState`. The partial score calculator is significantly faster (I've observed differences of ~2x) than the complete score calculator at the cost of increased complexity. This speedup is expected because the amount of calculations is roughly halved. There is more potential for optimization though, such as changing how distances are tracked, how they are calculated (e.g. Euclidean distance using SIMD), and distance caching.
* Different acceptance functions, configurable via `--acceptance-fun`, the implementation is in `optimizer/mod.rs`: `AcceptanceP`.
* Five different moves `AddDelivery, SwapDelivery, SwapPickup, SwapInRoute`, `TwoOptSwap` as defined in `optimizer/mod.rs`.
* Benchmarking, allow running a benchmark comparing different implementations. See `--benchmark` for more information.

I've modeled the problem such that there is always a feasible solution and any infeasible solution is immediately rejected. This is because in my experience the infeasible search space can be very large and it is easy for an optimizer to get stuck in it. The way I guarantee the solution to be feasible is by starting with a route containing a single pickup. The only way that the pickup can change is by `SwapInRoute`/`TwoOptSwap` which changes the index, or by `SwapPickup` which exchanges the routed pickup for an unrouted one.

## Testing strategy

Testing strategy
Due to the limited time the testing strategy is not as complete as I would otherwise prefer it to be. 

The partial score calculation has a limited number of tests, however, to increase confidence in the implementation the code contains a number of `debug_assert*` statements that test certain invariants. Notably, each time the partial score calculator runs, it calls the complete score calculator to ensure consistency. These debug asserts normally only run in debug mode (for performance reasons), however, it is possible to test them on big files by running:
```
RUSTFLAGS="-C debug-assertions" cargo run --release -- --move-limit 10000000 --file path/to/problem
```

The parser doesn't have any tests, this would need to be remedied to make it production ready.

### Shortcomings
 * Very limited input validation, if this library was somehow exposed to customers this would need to be improved.
 * Minimal code documentation.
 * Which moves and in which proportion they work best

## How to run

### Prerequisites

 * Have Rust/Cargo installed
 * Install [Just](https://just.systems/man/en/introduction.html) (optional): `cargo install just`
 * Download some of the [benchmark instances](https://www.sintef.no/projectweb/top/vrptw/homberger-benchmark/) such that you can point to them using the CLI.

 ### Typical run
```
cargo run --release -- --move-limit 1000000 --file homberger_1000_customer_instances/R2_10_10.TXT
```
 ### Overview of options
 ```
 cargo run -- -h
 ```
