use crate::{domain::CustomerId, optimizer::OptimizationParams};

mod domain;
mod optimizer;
mod parser;

fn main() {
    let mut problem = parser::parse("solomon-100/c101.txt").unwrap_or_else(|err| panic!("{err:?}"));

    problem.events[CustomerId(0)].event_type = domain::EventType::Pickup;

    let solution = optimizer::optimize(
        &problem,
        &OptimizationParams {
            move_limit: 100_000,
            seed: 7,
        },
    );
    println!("{solution:?}");
}
