//! Automated benchmark comparing algorithm variants against each other.
//!
//! Each variant is executed `n` times. The runs are interleaved (variant 0
//! run 0, variant 1 run 0, variant 0 run 1, variant 1 run 1, ...) and handed
//! to a Rayon thread pool so that no single variant systematically runs
//! first (and, e.g., gets an unfair cache/scheduling advantage) or last.

use std::time::{Duration, Instant};

use humantime::format_duration;
use rand::SeedableRng;
use rayon::prelude::*;

use crate::domain::ProblemInstance;
use crate::optimizer::{self, AcceptanceP, MediumSoft, OptimizationParams};

/// What to vary between the runs of a benchmark.
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub(crate) enum BenchmarkMode {
    /// Compare all available acceptance functions against each other.
    Acceptance,
    /// Compare incremental vs. non-incremental score calculation.
    Scoring,
}

#[derive(Clone, Copy)]
struct Variant {
    label: &'static str,
    acceptance_fun: AcceptanceP,
    incremental_score_calculation: bool,
}

struct RunResult {
    score: MediumSoft,
    duration: Duration,
}

/// Runs the benchmark and prints a table with the results to stdout.
pub(crate) fn run(
    problem: &ProblemInstance,
    mode: BenchmarkMode,
    move_limit: usize,
    seed: u64,
    runs: usize,
    default_acceptance_fun: AcceptanceP,
    default_incremental_score_calculation: bool,
) {
    let variants = match mode {
        BenchmarkMode::Acceptance => vec![
            Variant {
                label: "linear-decreasing",
                acceptance_fun: AcceptanceP::LinearDecreasing,
                incremental_score_calculation: default_incremental_score_calculation,
            },
            Variant {
                label: "delta-log-decreasing",
                acceptance_fun: AcceptanceP::DeltaLogDecreasing,
                incremental_score_calculation: default_incremental_score_calculation,
            },
        ],
        BenchmarkMode::Scoring => vec![
            Variant {
                label: "incremental",
                acceptance_fun: default_acceptance_fun,
                incremental_score_calculation: true,
            },
            Variant {
                label: "non-incremental",
                acceptance_fun: default_acceptance_fun,
                incremental_score_calculation: false,
            },
        ],
    };

    // interleave so that a run index is completed for every variant before
    // moving on to the next run index.
    let tasks: Vec<(usize, usize)> = (0..runs)
        .flat_map(|run_idx| (0..variants.len()).map(move |variant_idx| (variant_idx, run_idx)))
        .collect();

    let mut results: Vec<(usize, RunResult)> = tasks
        .par_iter()
        .map(|&(variant_idx, run_idx)| {
            let variant = variants[variant_idx];
            // every variant uses the same seed for a given run index, so
            // differences in outcome are attributable to the variant rather
            // than to the random number stream.
            let mut rng = rand_xoshiro::Xoroshiro128PlusPlus::seed_from_u64(
                seed.wrapping_add(run_idx as u64),
            );
            let params = OptimizationParams {
                move_limit,
                incremental_score_calculation: variant.incremental_score_calculation,
                acceptance_fun: variant.acceptance_fun,
            };

            let start = Instant::now();
            let solution = optimizer::optimize(problem, &params, &mut rng);
            let duration = start.elapsed();

            (
                variant_idx,
                RunResult {
                    score: solution.score,
                    duration,
                },
            )
        })
        .collect();
    results.sort_by_key(|(variant_idx, _)| *variant_idx);

    print_table(&variants, &results);
}

fn print_table(variants: &[Variant], results: &[(usize, RunResult)]) {
    println!(
        "{:<22} {:>5} {:>12} {:>12} {:>12} {:>12}  {:>12}",
        "variant", "runs", "avg medium", "best medium", "avg soft", "best soft", "avg time"
    );
    for (variant_idx, variant) in variants.iter().enumerate() {
        let variant_results: Vec<&RunResult> = results
            .iter()
            .filter(|(idx, _)| *idx == variant_idx)
            .map(|(_, res)| res)
            .collect();
        let n = variant_results.len();

        let avg_medium = variant_results
            .iter()
            .map(|r| r.score.medium_score as f64)
            .sum::<f64>()
            / n as f64;
        let avg_soft = variant_results
            .iter()
            .map(|r| r.score.soft_penalty.0)
            .sum::<f64>()
            / n as f64;
        let best = variant_results.iter().fold(MediumSoft::ZERO, |best, r| {
            if r.score > best { r.score } else { best }
        });
        let avg_time = variant_results.iter().map(|r| r.duration).sum::<Duration>() / n as u32;

        println!(
            "{:<22} {:>5} {:>12.2} {:>12} {:>12.2} {:>12.2}   {:>12}",
            variant.label,
            n,
            avg_medium,
            best.medium_score,
            avg_soft,
            best.soft_penalty.0,
            format_duration(avg_time),
        );
    }
}
