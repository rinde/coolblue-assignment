use rand::Rng;
use rand::RngExt;
use rand::SeedableRng;
use rand::seq::IndexedRandom;

use crate::domain::Distance;
use crate::domain::{Capacity, CustomerId, EventType, Location, ProblemInstance};

pub(crate) struct OptimizationParams {
    pub(crate) move_limit: usize,
    pub(crate) seed: u64,
}

// hard score: all capacity constraints need to be met
// medium score: more deliveries is better
// soft score: route distance

pub(crate) fn optimize(problem: &ProblemInstance, params: &OptimizationParams) -> Solution {
    let mut rng = rand_xoshiro::Xoroshiro128PlusPlus::seed_from_u64(params.seed);

    let mut unrouted_pickups = problem
        .events
        .iter()
        .filter(|e| e.event_type == EventType::Pickup)
        // .sorted_by_key(|e| e.requested_capacity)
        .map(|e| e.customer_id)
        .collect::<Vec<_>>();

    let unrouted_deliveries = problem
        .events
        .iter()
        .filter(|e| e.event_type == EventType::Delivery)
        // .sorted_by_key(|e| e.requested_capacity)
        .map(|e| e.customer_id)
        .collect();

    let mut opt_state = OptState {
        route: vec![
            unrouted_pickups
                .pop()
                .unwrap_or_else(|| panic!("at least one pickup is required")),
        ],
        unrouted_pickups,
        unrouted_deliveries,
        pickup_index: 0,
        free_capacity_at_start: problem.vehicle_capacity,
        location_at: vec![],
        distance_at: vec![],
        distance_at_end: Distance(0.0),
        earliest_pickup_index: 0,
    };

    const MOVES: [MoveType; 4] = [
        MoveType::AddDelivery,
        MoveType::SwapDelivery,
        MoveType::SwapPickup,
        MoveType::SwapOrder,
    ];

    let mut best_route = opt_state.route.clone();
    let mut best_score = MediumSoft::ZERO;
    let mut current_score = MediumSoft::ZERO;

    // https://en.wikipedia.org/wiki/Simulated_annealing
    for k in 0..params.move_limit {
        let move_ = MOVES
            .choose(&mut rng)
            .unwrap()
            .apply(&mut opt_state, &mut rng);

        if let Some(move_) = move_ {
            println!("{:?}", move_);
            println!("{:?}", opt_state.route);
            println!("pickup index {}", opt_state.pickup_index);
            let diff = move_.first_changed_route_index();

            let res = update_score(diff, problem, &mut opt_state);
            // enforce no hard score violation
            if let ScoreResult::NoCapacityViolation(med_soft) = res
                // solution is better, or..
                && (med_soft >= current_score
            // .. accept move with probability decreasing over time
            || rng.random_bool(1.0 - ((k + 1) as f64 / params.move_limit as f64)))
            {
                println!(" > accept");
                // accept move
                current_score = med_soft;
                if med_soft > best_score {
                    // save best result
                    best_score = med_soft;
                    best_route.clone_from(&opt_state.route);
                }
            } else {
                println!(" > reject");
                // reject move
                let diff = move_.undo(&mut opt_state);
                println!("{:?}", opt_state.route);
                println!("pickup index {}", opt_state.pickup_index);
                let undo_res = update_score(diff, problem, &mut opt_state);
                assert_ne!(undo_res, ScoreResult::CapacityViolation);
            }
        }
    }

    Solution {
        route: best_route,
        score: best_score,
    }
}

#[derive(Debug)]
pub(crate) struct Solution {
    pub(crate) route: Vec<CustomerId>,
    pub(crate) score: MediumSoft,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum ScoreResult {
    NoCapacityViolation(MediumSoft),
    CapacityViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub(crate) struct MediumSoft {
    pub(crate) medium_score: usize,
    pub(crate) soft_score: Distance,
}
impl MediumSoft {
    const ZERO: Self = Self {
        medium_score: 0,
        soft_score: Distance(0.0),
    };
}

#[derive(Debug, Clone, Copy, Default)]
struct Diff {
    add: Option<CustomerId>,
    remove: Option<CustomerId>,
    index: usize,
}

fn update_score(diff: Diff, problem: &ProblemInstance, opt_state: &mut OptState) -> ScoreResult {
    if let Some(event) = diff.remove.map(|c| &problem.events[c])
        && event.event_type == EventType::Delivery
    {
        opt_state.free_capacity_at_start += event.requested_capacity;
    }
    if let Some(event) = diff.add.map(|c| &problem.events[c])
        && event.event_type == EventType::Delivery
    {
        opt_state.free_capacity_at_start -= event.requested_capacity;
    }

    if opt_state.free_capacity_at_start < Capacity::ZERO {
        return ScoreResult::CapacityViolation;
    }

    let mut current_capacity = opt_state.free_capacity_at_start;
    let pickup = &problem.events[opt_state.route[opt_state.pickup_index]];
    let mut earliest_pickup_index = opt_state.route.len();
    for (i, c) in opt_state.route.iter().copied().enumerate() {
        if current_capacity >= pickup.requested_capacity {
            earliest_pickup_index = i;
            break;
        }
        current_capacity -= problem.events[c].requested_capacity;
    }

    // pickup happens before enough capacity is freed
    if opt_state.pickup_index < earliest_pickup_index {
        return ScoreResult::CapacityViolation;
    }

    let route_len = opt_state.route.len();
    opt_state.distance_at.resize(route_len, Distance(0.0));
    opt_state.location_at.resize(route_len, Location::DEPOT);

    for i in diff.index..route_len {
        opt_state.location_at[i] = problem.events[opt_state.route[i]].location;
    }

    let (mut prev_loc, mut prev_dist) = if diff.index == 0 {
        (Location::DEPOT, Distance(0.0))
    } else {
        (
            opt_state.location_at[diff.index - 1],
            opt_state.distance_at[diff.index - 1],
        )
    };

    for i in diff.index..route_len {
        opt_state.distance_at[i] = prev_dist + opt_state.location_at[i].distance(prev_loc);
        prev_loc = opt_state.location_at[i];
        prev_dist = opt_state.distance_at[i];
    }
    opt_state.distance_at_end = *opt_state.distance_at.last().unwrap()
        + opt_state
            .location_at
            .last()
            .unwrap()
            .distance(Location::DEPOT);

    ScoreResult::NoCapacityViolation(MediumSoft {
        medium_score: opt_state.route.iter().len(),
        soft_score: opt_state.distance_at_end,
    })
}

struct OptState {
    route: Vec<CustomerId>,
    unrouted_pickups: Vec<CustomerId>,
    unrouted_deliveries: Vec<CustomerId>,
    pickup_index: usize,

    // derived stats
    free_capacity_at_start: Capacity,
    location_at: Vec<Location>,
    distance_at: Vec<Distance>,
    distance_at_end: Distance,
    earliest_pickup_index: usize,
}

enum MoveType {
    AddDelivery,
    SwapDelivery,
    SwapPickup,
    SwapOrder,
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
                if opt_state.unrouted_deliveries.is_empty() {
                    return None;
                }
                None
            }
            MoveType::SwapPickup => None,
            MoveType::SwapOrder => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum Move {
    AddDelivery { delivery: CustomerId, index: usize },
    SwapDelivery {},
    SwapPickup {},
    SwapOrder {},
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
                Diff {
                    index,
                    add: None,
                    remove: Some(delivery),
                }
            }
            Move::SwapDelivery {} => Diff::default(),
            Move::SwapPickup {} => Diff::default(),
            Move::SwapOrder {} => Diff::default(),
        }
    }

    fn first_changed_route_index(self) -> Diff {
        match self {
            Move::AddDelivery { index, delivery } => Diff {
                index,
                add: Some(delivery),
                remove: None,
            },
            Move::SwapDelivery {} => Diff::default(),
            Move::SwapPickup {} => Diff::default(),
            Move::SwapOrder {} => Diff::default(),
        }
    }
}

// moves
// add delivery
// swap delivery (add 1, remove 1)
// swap pickup (add 1, remove 1)
// swap order (p/d)
