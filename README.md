# Optimizing TSP with a twist with Simulated Annealing


## Simulated Annealing

Simulated annealing is a proven local search metaheuristic that allows to converge to the global optimum. Simulated annealing is commonly used for the traveling salesman problem and similar problems.

Different acceptance functions

## Implementation

Features:
* Parses the files from the Homberger Benchmarks, the proportion of events that is a pickup is configurable in the CLI.
* 

Different scoring strategies

Debug assertions

Testing strategy

### Shortcomings
 * Very limited input validation, if this library was somehow exposed to customers this would need to be improved.
 * Minimal code documentation.

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

 ### Benchmarking

 Use `--benchmark` to compare algorithm variants automatically instead of running the optimizer once. Choose what to vary with `--benchmark acceptance` (compares the acceptance functions) or `--benchmark scoring` (compares incremental vs. non-incremental score calculation), and control how many times each variant is run with `--benchmark-runs`. All runs are interleaved across variants and executed in parallel with [Rayon](https://github.com/rayon-rs/rayon). The result is a table with, per variant, the average and best final score (medium and soft) and the average run time.
```
cargo run --release -- --file solomon-100/c101.txt --benchmark acceptance --benchmark-runs 10
```
