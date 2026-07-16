//! Optimizer for TSP with multiple deliveries, single pickup, and capacity
//! constraints. The optimizer maximizes the number of deliveries, and minimizes
//! the distance traveled.
//!
//! To see an overview of the commandline options, see cargo run -- -h
use std::time::Instant;

use clap::Parser;
use humantime::format_duration;
use rand::SeedableRng;

use crate::benchmark::BenchmarkMode;
use crate::optimizer::{AcceptanceP, OptimizationParams};

mod benchmark;
mod domain;
mod optimizer;
mod parser;

/// Optimize a vehicle routing problem instance.
#[derive(Debug, Parser)]
#[command(version, about)]
struct Cli {
    /// Path to the problem instance file to read.
    #[arg(long, default_value = "solomon-100/c101.txt")]
    file: String,

    /// Maximum number of optimizer moves to perform.
    #[arg(long, default_value_t = 1_000_000)]
    move_limit: usize,

    /// Seed for the random number generator.
    #[arg(long, default_value_t = 7)]
    seed: u64,

    #[arg(long, default_value_t = false)]
    disable_incremental_score: bool,

    #[arg(long, value_enum, default_value_t = AcceptanceP::DeltaLogDecreasing)]
    acceptance_fun: AcceptanceP,

    /// The number of events n in the input that should be considered as pickup
    /// events. The selected events are in 0..n range. Values outside of the
    /// [0.0, 0.9] range will be ignored and it is guaranteed that there will
    /// always be at least one pickup.
    #[arg(long, default_value_t = 0.02)]
    pickup_proportion: f64,

    /// Run an automated benchmark instead of a single optimization run. The
    /// selected mode determines which algorithm variants are compared;
    /// every variant is run `benchmark-runs` times.
    #[arg(long, value_enum)]
    benchmark: Option<BenchmarkMode>,

    /// Number of times each algorithm variant is run when `--benchmark` is
    /// used. The runs of all variants are interleaved and executed in
    /// parallel using Rayon.
    #[arg(long, default_value_t = 10)]
    benchmark_runs: usize,
}

fn main() {
    let cli = Cli::parse();

    let start = Instant::now();
    let mut problem =
        parser::parse(&cli.file).unwrap_or_else(|err| panic!("Parsing failed {err:?}"));
    let parse_duration = start.elapsed();

    // turn events into pickups
    let num_pickups =
        1.max((problem.events.len() as f64 * cli.pickup_proportion.min(0.9)) as usize);
    for event in problem.events.iter_mut().take(num_pickups) {
        event.kind = domain::EventKind::Pickup;
    }

    if let Some(mode) = cli.benchmark {
        println!("Parsed file in {}", format_duration(parse_duration));
        benchmark::run(
            &problem,
            mode,
            cli.move_limit,
            cli.seed,
            cli.benchmark_runs,
            cli.acceptance_fun,
            !cli.disable_incremental_score,
        );
        return;
    }

    let mut rng = rand_xoshiro::Xoroshiro128PlusPlus::seed_from_u64(cli.seed);

    let start = Instant::now();
    let solution = optimizer::optimize(
        &problem,
        &OptimizationParams {
            move_limit: cli.move_limit,
            incremental_score_calculation: !cli.disable_incremental_score,
            acceptance_fun: cli.acceptance_fun,
        },
        &mut rng,
    );
    let solve_duration = start.elapsed();
    println!(
        "Parsed in {}, solved in {}",
        format_duration(parse_duration),
        format_duration(solve_duration)
    );
    println!("{solution:?}");
}
