mod score;
mod state;

use std::mem;

use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;
use rand::seq::IndexedRandom;

use crate::domain::{CustomerId, ProblemInstance};
use crate::optimizer::score::MediumSoft;
use crate::optimizer::score::ScoreResult;
use crate::optimizer::state::Diff;
use crate::optimizer::state::OptState;

const MOVES: [MoveType; 4] = [
    MoveType::AddDelivery,
    MoveType::SwapDelivery,
    MoveType::SwapPickup,
    MoveType::SwapInRoute,
];

pub(crate) struct OptimizationParams {
    pub(crate) move_limit: usize,
    pub(crate) seed: u64,
    pub(crate) incremental_score_calculation: bool,
}

// hard score: all capacity constraints need to be met
// medium score: more deliveries is better
// soft score: minimize route distance

/// Optimize the problem with simulated annealing.
pub(crate) fn optimize(problem: &ProblemInstance, params: &OptimizationParams) -> Solution {
    let mut rng = rand_xoshiro::Xoroshiro128PlusPlus::seed_from_u64(params.seed);

    let mut opt_state = OptState::init(problem, params.incremental_score_calculation);

    let mut best_route = opt_state.route.clone();
    let mut best_score = MediumSoft::ZERO;
    let mut current_score = MediumSoft::ZERO;

    // https://en.wikipedia.org/wiki/Simulated_annealing
    for k in 0..params.move_limit {
        #[expect(clippy::unwrap_used, reason = "MOVES is not empty so this cannot fail")]
        let move_ = MOVES
            .choose(&mut rng)
            .unwrap()
            .apply(&mut opt_state, &mut rng);

        if let Some(move_) = move_ {
            let diff = move_.diff();

            let res = opt_state.update_score(diff, problem);
            // enforce no hard score violation
            if let ScoreResult::NoCapacityViolation(med_soft) = res
                // solution is better, or..
                && (med_soft >= current_score
            // .. accept move with probability decreasing over time
            || rng.random_bool(1.0 - ((k + 1) as f64 / params.move_limit as f64)))
            {
                // accept move
                current_score = med_soft;
                if med_soft > best_score {
                    // save best result
                    best_score = med_soft;
                    best_route.clone_from(&opt_state.route);
                }
            } else {
                // reject move
                let diff = move_.undo(&mut opt_state);
                let undo_res = opt_state.update_score(diff, problem);
                assert_ne!(undo_res, ScoreResult::CapacityViolation);
            }
        }
    }

    Solution {
        name: problem.name.clone(),
        route: best_route,
        score: best_score,
    }
}

#[derive(Debug)]
#[expect(dead_code, reason = "The struct is printed in the CLI")]
pub(crate) struct Solution {
    pub(crate) name: String,
    pub(crate) route: Vec<CustomerId>,
    pub(crate) score: MediumSoft,
}

enum MoveType {
    AddDelivery,
    SwapDelivery,
    SwapPickup,
    SwapInRoute,
}

impl MoveType {
    fn apply(&self, opt_state: &mut OptState, rng: &mut impl Rng) -> Option<Move> {
        match self {
            MoveType::AddDelivery => {
                if opt_state.unrouted_deliveries.is_empty() {
                    return None;
                }
                let delivery = opt_state
                    .unrouted_deliveries
                    .swap_remove(rng.random_range(0..opt_state.unrouted_deliveries.len()));

                let insertion_index = rng.random_range(0..=opt_state.route.len());

                if insertion_index <= opt_state.pickup_index {
                    opt_state.pickup_index += 1;
                }
                opt_state.route.insert(insertion_index, delivery);
                Some(Move::AddDelivery {
                    delivery,
                    index: insertion_index,
                })
            }
            MoveType::SwapDelivery => {
                if opt_state.unrouted_deliveries.is_empty() || opt_state.route.len() <= 1 {
                    return None;
                }

                let mut route_index = rng.random_range(0..opt_state.route.len() - 1);
                if route_index >= opt_state.pickup_index {
                    route_index += 1;
                }
                let unrouted_index = rng.random_range(0..opt_state.unrouted_deliveries.len());
                mem::swap(
                    &mut opt_state.unrouted_deliveries[unrouted_index],
                    &mut opt_state.route[route_index],
                );

                Some(Move::SwapDelivery {
                    old_delivery: opt_state.unrouted_deliveries[unrouted_index],
                    new_delivery: opt_state.route[route_index],
                    route_index,
                    unrouted_index,
                })
            }
            MoveType::SwapPickup => {
                if opt_state.unrouted_pickups.is_empty() {
                    return None;
                }
                let unrouted_index = rng.random_range(0..opt_state.unrouted_pickups.len());

                mem::swap(
                    &mut opt_state.unrouted_pickups[unrouted_index],
                    &mut opt_state.route[opt_state.pickup_index],
                );
                Some(Move::SwapPickup {
                    old_pickup: opt_state.unrouted_pickups[unrouted_index],
                    new_pickup: opt_state.route[opt_state.pickup_index],
                    route_index: opt_state.pickup_index,
                    unrouted_index,
                })
            }
            MoveType::SwapInRoute => {
                if opt_state.route.len() <= 1 {
                    return None;
                }
                let index1 = rng.random_range(0..opt_state.route.len());
                let mut index2 = rng.random_range(0..(opt_state.route.len() - 1));
                if index2 >= index1 {
                    index2 += 1;
                }
                if index1 == opt_state.pickup_index {
                    opt_state.pickup_index = index2;
                } else if index2 == opt_state.pickup_index {
                    opt_state.pickup_index = index1;
                }
                opt_state.route.swap(index1, index2);
                Some(Move::SwapInRoute { index1, index2 })
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Move {
    AddDelivery {
        delivery: CustomerId,
        index: usize,
    },
    SwapDelivery {
        old_delivery: CustomerId,
        new_delivery: CustomerId,
        route_index: usize,
        unrouted_index: usize,
    },
    SwapPickup {
        old_pickup: CustomerId,
        new_pickup: CustomerId,
        route_index: usize,
        unrouted_index: usize,
    },
    SwapInRoute {
        index1: usize,
        index2: usize,
    },
}

impl Move {
    fn undo(&self, opt_state: &mut OptState) -> Diff {
        match *self {
            Move::AddDelivery { delivery, index } => {
                if index < opt_state.pickup_index {
                    opt_state.pickup_index -= 1;
                }
                let removed_id = opt_state.route.remove(index);
                debug_assert_eq!(removed_id, delivery);
                opt_state.unrouted_deliveries.push(delivery);
                Diff::new(index, None, Some(delivery))
            }
            Move::SwapDelivery {
                old_delivery,
                new_delivery,
                route_index,
                unrouted_index,
            } => {
                mem::swap(
                    &mut opt_state.route[route_index],
                    &mut opt_state.unrouted_deliveries[unrouted_index],
                );
                Diff::new(route_index, Some(old_delivery), Some(new_delivery))
            }
            Move::SwapPickup {
                old_pickup,
                new_pickup,
                route_index,
                unrouted_index,
            } => {
                mem::swap(
                    &mut opt_state.route[route_index],
                    &mut opt_state.unrouted_deliveries[unrouted_index],
                );
                Diff::new(route_index, Some(old_pickup), Some(new_pickup))
            }
            Move::SwapInRoute { index1, index2 } => {
                if index1 == opt_state.pickup_index {
                    opt_state.pickup_index = index2;
                } else if index2 == opt_state.pickup_index {
                    opt_state.pickup_index = index1;
                }
                opt_state.route.swap(index1, index2);

                Diff::new(index1.min(index2), None, None)
            }
        }
    }

    fn diff(self) -> Diff {
        match self {
            Move::AddDelivery { index, delivery } => Diff::new(index, Some(delivery), None),
            Move::SwapDelivery {
                old_delivery,
                new_delivery,
                route_index,
                ..
            } => Diff::new(route_index, Some(new_delivery), Some(old_delivery)),
            Move::SwapPickup {
                old_pickup,
                new_pickup,
                route_index,
                ..
            } => Diff::new(route_index, Some(new_pickup), Some(old_pickup)),
            Move::SwapInRoute { index1, index2 } => Diff::new(index1.min(index2), None, None),
        }
    }
}
