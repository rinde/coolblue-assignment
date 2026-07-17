mod score;
mod state;

use std::mem;

use rand::Rng;
use rand::RngExt;
use rand_distr::Distribution;
use rand_distr::weighted::WeightedAliasIndex;

use crate::domain::{CustomerId, ProblemInstance};
pub(crate) use crate::optimizer::score::MediumSoft;
use crate::optimizer::score::ScoreResult;
use crate::optimizer::state::Diff;
use crate::optimizer::state::OptState;

pub(crate) struct OptimizationParams {
    pub(crate) move_limit: usize,
    pub(crate) partial_score_calculation: bool,
    pub(crate) acceptance_fun: AcceptanceP,
    pub(crate) move_selection: MoveSelection,
}

// hard score: all capacity constraints need to be met
// medium score: more deliveries is better
// soft score: minimize route distance

/// Optimize the problem with simulated annealing.
pub(crate) fn optimize(
    problem: &ProblemInstance,
    params: &OptimizationParams,
    rng: &mut impl Rng,
) -> Solution {
    let (mut opt_state, initial_score) = OptState::init(problem, params.partial_score_calculation);

    let roulette_wheel = params.move_selection.init();

    let mut best_route = opt_state.route.clone();
    let mut best_score = initial_score;
    let mut current_score = initial_score;

    // https://en.wikipedia.org/wiki/Simulated_annealing
    for k in 0..params.move_limit {
        let move_ = roulette_wheel.choose(rng).apply(&mut opt_state, rng);

        if let Some(move_) = move_ {
            let diff = move_.diff();

            let res = opt_state.update_score(diff, problem);
            // enforce no hard score violation
            if let ScoreResult::NoCapacityViolation(med_soft) = res
                // solution is better, or..
                && (med_soft >= current_score
            // .. accept move with probability defined by acceptance_fun
            || {
                let delta = med_soft.delta(current_score);
                let prob = params.acceptance_fun.probability(1.0 - ((k + 1) as f64 / params.move_limit as f64), delta);
                rng.random_bool(prob)
            }) {
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

#[derive(
    Copy, Clone, Debug, clap::ValueEnum, strum::Display, strum::VariantArray, strum::IntoStaticStr,
)]
pub(crate) enum AcceptanceP {
    /// Decreases acceptance probability linearly over time. This is not real
    /// simulated annealing as it ignores delta.
    LinearDecreasing,
    /// Higher probability for solutions that are similar to the current one
    /// (low delta) while gradually decreasing the acceptance probability
    /// following the natural logarithm. This is the 'standard' acceptance
    /// function originally defined by Kirkpatrick et al.
    DeltaLogDecreasing,
}

impl AcceptanceP {
    fn probability(self, proportion_left: f64, delta: f64) -> f64 {
        match self {
            AcceptanceP::LinearDecreasing => proportion_left,
            AcceptanceP::DeltaLogDecreasing => {
                let temperature = proportion_left;
                (-delta / temperature).exp()
            }
        }
    }
}

#[derive(
    Copy, Clone, Debug, clap::ValueEnum, strum::Display, strum::VariantArray, strum::IntoStaticStr,
)]
pub(crate) enum MoveSelection {
    Simple,
    WithTwoOpt,
}

impl MoveSelection {
    fn init(self) -> RouletteWheelSelector {
        match self {
            MoveSelection::Simple => RouletteWheelSelector::new(
                [
                    MoveType::AddDelivery,
                    MoveType::SwapDelivery,
                    MoveType::SwapPickup,
                    MoveType::SwapInRoute,
                ],
                [1, 1, 1, 1],
            ),
            MoveSelection::WithTwoOpt => RouletteWheelSelector::new(
                [
                    MoveType::AddDelivery,
                    MoveType::SwapDelivery,
                    MoveType::SwapPickup,
                    MoveType::SwapInRoute,
                    MoveType::TwoOptSwap,
                ],
                [10, 10, 10, 10, 40],
            ),
        }
    }
}

struct RouletteWheelSelector {
    moves: Vec<MoveType>,
    index: WeightedAliasIndex<u8>,
}

impl RouletteWheelSelector {
    fn new<const N: usize>(moves: [MoveType; N], weights: [u8; N]) -> Self {
        Self {
            moves: moves.to_vec(),
            #[expect(
                clippy::unwrap_used,
                reason = "This is a configuration error and should always be caught in testing"
            )]
            index: WeightedAliasIndex::new(weights.to_vec()).unwrap(),
        }
    }

    fn choose(&self, rng: &mut impl Rng) -> MoveType {
        self.moves[self.index.sample(rng)]
    }
}

#[derive(Debug)]
#[expect(dead_code, reason = "The struct is printed in the CLI")]
pub(crate) struct Solution {
    pub(crate) name: String,
    pub(crate) route: Vec<CustomerId>,
    pub(crate) score: MediumSoft,
}

#[derive(Copy, Clone, Debug)]
enum MoveType {
    AddDelivery,
    SwapDelivery,
    SwapPickup,
    SwapInRoute,
    TwoOptSwap,
}

impl MoveType {
    fn apply(self, opt_state: &mut OptState, rng: &mut impl Rng) -> Option<Move> {
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
            MoveType::TwoOptSwap => {
                if opt_state.route.len() <= 1 {
                    return None;
                }
                let mut index1 = rng.random_range(0..opt_state.route.len());
                let mut index2 = rng.random_range(0..(opt_state.route.len() - 1));
                if index2 >= index1 {
                    index2 += 1;
                } else {
                    mem::swap(&mut index1, &mut index2);
                }
                if opt_state.pickup_index >= index1 && opt_state.pickup_index <= index2 {
                    opt_state.pickup_index = index2 - (opt_state.pickup_index - index1);
                }

                opt_state.route[index1..=index2].reverse();
                Some(Move::TwoOptSwap { index1, index2 })
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
    TwoOptSwap {
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
                    &mut opt_state.unrouted_pickups[unrouted_index],
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
            Move::TwoOptSwap { index1, index2 } => {
                if opt_state.pickup_index >= index1 && opt_state.pickup_index <= index2 {
                    opt_state.pickup_index = index2 - (opt_state.pickup_index - index1);
                }
                opt_state.route[index1..=index2].reverse();
                Diff::new(index1, None, None)
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
            Move::TwoOptSwap { index1, .. } => Diff::new(index1, None, None),
        }
    }
}

#[cfg(test)]
mod test {
    use rand::SeedableRng;
    use typed_index_collections::ti_vec;

    use super::*;
    use crate::domain::{Capacity, Coordinate, Distance, Event, EventKind, Location};

    fn loc(x: u16, y: u16) -> Location {
        Location {
            x: Coordinate(x),
            y: Coordinate(y),
        }
    }

    fn test_problem() -> ProblemInstance {
        ProblemInstance {
            name: String::new(),
            _num_vehicles: 1,
            vehicle_capacity: Capacity(1000),
            events: ti_vec![
                Event {
                    customer_id: CustomerId(0),
                    requested_capacity: Capacity(10),
                    location: loc(0, 0),
                    kind: EventKind::Pickup,
                },
                Event {
                    customer_id: CustomerId(1),
                    requested_capacity: Capacity(10),
                    location: loc(1, 1),
                    kind: EventKind::Pickup,
                },
                Event {
                    customer_id: CustomerId(2),
                    requested_capacity: Capacity(10),
                    location: loc(2, 2),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(3),
                    requested_capacity: Capacity(10),
                    location: loc(3, 3),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(4),
                    requested_capacity: Capacity(10),
                    location: loc(4, 4),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(5),
                    requested_capacity: Capacity(10),
                    location: loc(5, 5),
                    kind: EventKind::Delivery,
                },
            ],
        }
    }

    fn sorted_ids(ids: &[CustomerId]) -> Vec<u16> {
        let mut v: Vec<u16> = ids.iter().map(|c| c.0).collect();
        v.sort_unstable();
        v
    }

    fn rng(seed: u64) -> rand_xoshiro::Xoroshiro128PlusPlus {
        rand_xoshiro::Xoroshiro128PlusPlus::seed_from_u64(seed)
    }

    #[test]
    fn add_delivery_apply_diff_undo() {
        // run with multiple seeds to exercise both the "insertion index
        // before pickup_index" and "insertion index after pickup_index"
        // branches of apply()/undo().
        for seed in 0..30 {
            let problem = test_problem();
            let (mut opt_state, _) = OptState::init(&problem, false);
            let mut rng = rng(seed);

            let before_route = opt_state.route.clone();
            let before_unrouted = sorted_ids(&opt_state.unrouted_deliveries);
            let before_pickup_index = opt_state.pickup_index;

            let move_ = MoveType::AddDelivery
                .apply(&mut opt_state, &mut rng)
                .expect("unrouted deliveries are available");
            let Move::AddDelivery { delivery, index } = move_ else {
                panic!("expected AddDelivery move");
            };

            // apply()
            let mut expected_route = before_route.clone();
            expected_route.insert(index, delivery);
            assert_eq!(opt_state.route, expected_route);
            assert!(!opt_state.unrouted_deliveries.contains(&delivery));
            let expected_pickup_index = if index <= before_pickup_index {
                before_pickup_index + 1
            } else {
                before_pickup_index
            };
            assert_eq!(opt_state.pickup_index, expected_pickup_index);

            // diff()
            assert_eq!(move_.diff(), Diff::new(index, Some(delivery), None));

            // undo()
            let undo_diff = move_.undo(&mut opt_state);
            assert_eq!(opt_state.route, before_route);
            assert_eq!(opt_state.pickup_index, before_pickup_index);
            assert_eq!(sorted_ids(&opt_state.unrouted_deliveries), before_unrouted);
            assert_eq!(undo_diff, Diff::new(index, None, Some(delivery)));
        }
    }

    #[test]
    fn swap_delivery_apply_diff_undo() {
        let problem = test_problem();
        let (mut opt_state, _) = OptState::init(&problem, false);
        // force a deterministic scenario: a single candidate on each side
        // means the random indices are forced regardless of the rng's
        // output (random_range(0..1) is always 0).
        opt_state.route = vec![CustomerId(1), CustomerId(2)];
        opt_state.pickup_index = 0;
        opt_state.unrouted_deliveries = vec![CustomerId(3)];

        let before_route = opt_state.route.clone();
        let before_unrouted = opt_state.unrouted_deliveries.clone();
        let before_pickup_index = opt_state.pickup_index;

        let mut rng = rng(0);
        let move_ = MoveType::SwapDelivery
            .apply(&mut opt_state, &mut rng)
            .expect("a swap is available");
        let Move::SwapDelivery {
            old_delivery,
            new_delivery,
            route_index,
            unrouted_index,
        } = move_
        else {
            panic!("expected SwapDelivery move");
        };

        // apply()
        assert_eq!(route_index, 1);
        assert_eq!(unrouted_index, 0);
        assert_eq!(old_delivery, CustomerId(2));
        assert_eq!(new_delivery, CustomerId(3));
        assert_eq!(opt_state.route, vec![CustomerId(1), CustomerId(3)]);
        assert_eq!(opt_state.unrouted_deliveries, vec![CustomerId(2)]);

        // diff()
        assert_eq!(
            move_.diff(),
            Diff::new(route_index, Some(new_delivery), Some(old_delivery))
        );

        // undo()
        let undo_diff = move_.undo(&mut opt_state);
        assert_eq!(opt_state.route, before_route);
        assert_eq!(opt_state.unrouted_deliveries, before_unrouted);
        assert_eq!(opt_state.pickup_index, before_pickup_index);
        assert_eq!(
            undo_diff,
            Diff::new(route_index, Some(old_delivery), Some(new_delivery))
        );
    }

    #[test]
    fn swap_pickup_apply_diff_undo() {
        let problem = test_problem();
        let (mut opt_state, _) = OptState::init(&problem, false);
        // OptState::init already leaves a single unrouted pickup, so the
        // random index is forced regardless of the rng's output.
        assert_eq!(opt_state.unrouted_pickups, vec![CustomerId(0)]);
        assert_eq!(opt_state.route, vec![CustomerId(1)]);
        assert_eq!(opt_state.pickup_index, 0);

        let before_route = opt_state.route.clone();
        let before_unrouted_pickups = opt_state.unrouted_pickups.clone();
        let before_pickup_index = opt_state.pickup_index;

        let mut rng = rng(0);
        let move_ = MoveType::SwapPickup
            .apply(&mut opt_state, &mut rng)
            .expect("a swap is available");
        let Move::SwapPickup {
            old_pickup,
            new_pickup,
            route_index,
            unrouted_index,
        } = move_
        else {
            panic!("expected SwapPickup move");
        };

        // apply()
        assert_eq!(route_index, 0);
        assert_eq!(unrouted_index, 0);
        assert_eq!(old_pickup, CustomerId(1));
        assert_eq!(new_pickup, CustomerId(0));
        assert_eq!(opt_state.route, vec![CustomerId(0)]);
        assert_eq!(opt_state.unrouted_pickups, vec![CustomerId(1)]);

        // diff()
        assert_eq!(
            move_.diff(),
            Diff::new(route_index, Some(new_pickup), Some(old_pickup))
        );

        // undo()
        let undo_diff = move_.undo(&mut opt_state);
        assert_eq!(opt_state.route, before_route);
        assert_eq!(opt_state.unrouted_pickups, before_unrouted_pickups);
        assert_eq!(opt_state.pickup_index, before_pickup_index);
        assert_eq!(
            undo_diff,
            Diff::new(route_index, Some(old_pickup), Some(new_pickup))
        );
    }

    #[test]
    fn swap_in_route_apply_diff_undo() {
        // run with multiple seeds to exercise the branches where the swap
        // does/doesn't touch pickup_index.
        for seed in 0..30 {
            let problem = test_problem();
            let (mut opt_state, _) = OptState::init(&problem, false);
            opt_state.route = vec![CustomerId(1), CustomerId(2), CustomerId(3), CustomerId(4)];
            opt_state.pickup_index = 0;

            let before_route = opt_state.route.clone();
            let before_pickup_index = opt_state.pickup_index;

            let mut rng = rng(seed);
            let move_ = MoveType::SwapInRoute
                .apply(&mut opt_state, &mut rng)
                .expect("route has more than one entry");
            let Move::SwapInRoute { index1, index2 } = move_ else {
                panic!("expected SwapInRoute move");
            };

            // apply()
            let mut expected_route = before_route.clone();
            expected_route.swap(index1, index2);
            assert_eq!(opt_state.route, expected_route);
            let expected_pickup_index = if before_pickup_index == index1 {
                index2
            } else if before_pickup_index == index2 {
                index1
            } else {
                before_pickup_index
            };
            assert_eq!(opt_state.pickup_index, expected_pickup_index);

            // diff()
            assert_eq!(move_.diff(), Diff::new(index1.min(index2), None, None));

            // undo()
            let undo_diff = move_.undo(&mut opt_state);
            assert_eq!(opt_state.route, before_route);
            assert_eq!(opt_state.pickup_index, before_pickup_index);
            assert_eq!(undo_diff, Diff::new(index1.min(index2), None, None));
        }
    }

    /// Builds a problem where the optimum is known by construction: one
    /// pickup (c0) and five deliveries placed such that the shortest route
    /// forms a rectangle that ends with the pickup because of the tight
    /// capacity constraints.
    ///
    /// c2 -- c3 -- c4
    /// |           |
    /// c1          |
    /// |           |
    /// D --- c0 -- c5
    ///
    /// Optimal route: c1, c2, c3, c4, c5, c0
    /// Optimal distance: 8
    fn known_optimum_problem() -> ProblemInstance {
        ProblemInstance {
            name: "known-optimum".to_owned(),
            _num_vehicles: 1,
            vehicle_capacity: Capacity(50),
            events: ti_vec![
                Event {
                    customer_id: CustomerId(0),
                    requested_capacity: Capacity(10),
                    location: loc(0, 1),
                    kind: EventKind::Pickup,
                },
                Event {
                    customer_id: CustomerId(1),
                    requested_capacity: Capacity(10),
                    location: loc(1, 0),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(2),
                    requested_capacity: Capacity(10),
                    location: loc(2, 0),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(3),
                    requested_capacity: Capacity(10),
                    location: loc(2, 1),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(4),
                    requested_capacity: Capacity(10),
                    location: loc(2, 2),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(5),
                    requested_capacity: Capacity(10),
                    location: loc(0, 2),
                    kind: EventKind::Delivery,
                },
            ],
        }
    }

    #[test]
    fn optimize_finds_known_optimum() {
        let problem = known_optimum_problem();
        let params = OptimizationParams {
            move_limit: 5_000,
            partial_score_calculation: true,
            acceptance_fun: AcceptanceP::DeltaLogDecreasing,
            move_selection: MoveSelection::WithTwoOpt,
        };

        for seed in 0..10 {
            let mut rng = rng(seed);
            let solution = optimize(&problem, &params, &mut rng);

            assert_eq!(
                solution.score,
                MediumSoft::new(problem.events.len(), Distance(8.0)),
            );
            assert_eq!(
                solution.route,
                [1, 2, 3, 4, 5, 0]
                    .into_iter()
                    .map(CustomerId)
                    .collect::<Vec<_>>()
            );
        }
    }
}
