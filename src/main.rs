//! Optimizer for VRP with multiple deliveries, single pickup, and capacity
//! constraints. The optimizer maximizes the number of deliveries, and minimizes
//! the distance traveled.
//!
//! To see an overview of the commandline options, see cargo run -- -h
use std::time::Instant;

use clap::Parser;
use humantime::format_duration;

use crate::{domain::CustomerId, optimizer::OptimizationParams};

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
}

fn main() {
    let cli = Cli::parse();

    let start = Instant::now();
    let mut problem =
        parser::parse(&cli.file).unwrap_or_else(|err| panic!("Parsing failed {err:?}"));
    let parse_duration = start.elapsed();

    problem.events[CustomerId(0)].kind = domain::EventKind::Pickup;

    let start = Instant::now();
    let solution = optimizer::optimize(
        &problem,
        &OptimizationParams {
            move_limit: cli.move_limit,
            seed: cli.seed,
            incremental_score_calculation: !cli.disable_incremental_score,
        },
    );
    let solve_duration = start.elapsed();
    println!(
        "Parsed in {}, solved in {}",
        format_duration(parse_duration),
        format_duration(solve_duration)
    );
    println!("{solution:?}");
}
