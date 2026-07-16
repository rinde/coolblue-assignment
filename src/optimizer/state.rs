use crate::{
    domain::{Capacity, CustomerId, Distance, EventKind, Location, ProblemInstance},
    optimizer::score::{MediumSoft, ScoreResult},
};

/// Efficiently tracks the optimization state.
pub(super) struct OptState {
    pub(super) route: Vec<CustomerId>,
    pub(super) unrouted_pickups: Vec<CustomerId>,
    pub(super) unrouted_deliveries: Vec<CustomerId>,
    pub(super) pickup_index: usize,

    score: ScoreState,
}

enum ScoreState {
    AtOnce,
    Incremental(IncrementalScoreState),
}

#[derive(Default)]
struct IncrementalScoreState {
    free_capacity_at_start: Capacity,
    location_at: Vec<Location>,
    distance_at: Vec<Distance>,
    distance_at_end: Distance,
    // the earliest index where the vehicle has enough capacity to carry the pickup
    earliest_pickup_index: usize,
}

impl IncrementalScoreState {
    pub(super) fn update_score(
        &mut self,
        route: &[CustomerId],
        pickup_index: usize,
        diff: Diff,
        problem: &ProblemInstance,
    ) -> ScoreResult {
        #[cfg(debug_assertions)]
        let score = calculate_score(route, problem);

        if let Some(event) = diff.remove.map(|c| &problem.events[c])
            && event.kind == EventKind::Delivery
        {
            self.free_capacity_at_start += event.requested_capacity;
        }
        if let Some(event) = diff.add.map(|c| &problem.events[c])
            && event.kind == EventKind::Delivery
        {
            self.free_capacity_at_start -= event.requested_capacity;
        }

        if self.free_capacity_at_start < Capacity::ZERO {
            #[cfg(debug_assertions)]
            debug_assert_eq!(score, ScoreResult::CapacityViolation);
            return ScoreResult::CapacityViolation;
        }

        let mut current_capacity = self.free_capacity_at_start;
        let pickup = &problem.events[route[pickup_index]];
        debug_assert_eq!(pickup.kind, EventKind::Pickup);
        let mut earliest_pickup_index = route.len();
        for (i, c) in route.iter().copied().enumerate() {
            if current_capacity >= pickup.requested_capacity {
                earliest_pickup_index = i;
                break;
            }
            if i != pickup_index {
                current_capacity += problem.events[c].requested_capacity;
            }
        }

        // pickup happens before enough capacity is freed
        if pickup_index < earliest_pickup_index {
            #[cfg(debug_assertions)]
            debug_assert_eq!(score, ScoreResult::CapacityViolation);
            return ScoreResult::CapacityViolation;
        }
        self.earliest_pickup_index = earliest_pickup_index;

        // calculate distances
        let route_len = route.len();
        self.distance_at.resize(route_len, Distance(0.0));
        self.location_at.resize(route_len, Location::DEPOT);

        for (i, &c) in route.iter().enumerate().skip(diff.index) {
            self.location_at[i] = problem.events[c].location;
        }

        let (mut prev_loc, mut prev_dist) = if diff.index == 0 {
            (Location::DEPOT, Distance(0.0))
        } else {
            (
                self.location_at[diff.index - 1],
                self.distance_at[diff.index - 1],
            )
        };

        for i in diff.index..route_len {
            self.distance_at[i] = prev_dist + self.location_at[i].distance(prev_loc);
            prev_loc = self.location_at[i];
            prev_dist = self.distance_at[i];
        }
        self.distance_at_end = self.distance_at[route_len - 1]
            + self.location_at[route_len - 1].distance(Location::DEPOT);

        let incremental_score =
            ScoreResult::NoCapacityViolation(MediumSoft::new(route.len(), self.distance_at_end));
        #[cfg(debug_assertions)]
        debug_assert_eq!(incremental_score, score);
        incremental_score
    }
}

impl OptState {
    pub(super) fn init(problem: &ProblemInstance, incremental: bool) -> Self {
        let mut unrouted_pickups = problem
            .events
            .iter()
            .filter(|e| e.kind == EventKind::Pickup)
            // .sorted_by_key(|e| e.requested_capacity)
            .map(|e| e.customer_id)
            .collect::<Vec<_>>();

        let unrouted_deliveries = problem
            .events
            .iter()
            .filter(|e| e.kind == EventKind::Delivery)
            // .sorted_by_key(|e| e.requested_capacity)
            .map(|e| e.customer_id)
            .collect();

        let mut state = Self {
            route: vec![
                unrouted_pickups
                    .pop()
                    .unwrap_or_else(|| panic!("at least one pickup is required")),
            ],
            unrouted_pickups,
            unrouted_deliveries,
            pickup_index: 0,
            score: if incremental {
                ScoreState::Incremental(IncrementalScoreState {
                    free_capacity_at_start: problem.vehicle_capacity,
                    ..Default::default()
                })
            } else {
                ScoreState::AtOnce
            },
        };
        let result = state.update_score(Diff::new(0, Some(state.route[0]), None), problem);
        debug_assert_ne!(result, ScoreResult::CapacityViolation);
        state
    }

    pub(super) fn update_score(&mut self, diff: Diff, problem: &ProblemInstance) -> ScoreResult {
        match &mut self.score {
            ScoreState::AtOnce => calculate_score(&self.route, problem),
            ScoreState::Incremental(score) => {
                score.update_score(&self.route, self.pickup_index, diff, problem)
            }
        }
    }
}

fn calculate_score(route: &[CustomerId], problem: &ProblemInstance) -> ScoreResult {
    let mut free_capacity = problem.vehicle_capacity;
    // calculate initial free capacity
    for &customer in route {
        let event = problem.events[customer];
        match event.kind {
            EventKind::Pickup => {}
            EventKind::Delivery => {
                free_capacity -= event.requested_capacity;
                if free_capacity < Capacity::ZERO {
                    return ScoreResult::CapacityViolation;
                }
            }
        }
    }

    let mut distance_traveled = Distance(0.0);
    let mut prev_loc = Location::DEPOT;
    for &customer in route {
        let event = problem.events[customer];
        match event.kind {
            EventKind::Pickup => {
                free_capacity -= event.requested_capacity;
                if free_capacity < Capacity::ZERO {
                    return ScoreResult::CapacityViolation;
                }
            }
            EventKind::Delivery => free_capacity += event.requested_capacity,
        }
        distance_traveled += prev_loc.distance(event.location);
        prev_loc = event.location;
    }
    distance_traveled += prev_loc.distance(Location::DEPOT);

    ScoreResult::NoCapacityViolation(MediumSoft::new(route.len(), distance_traveled))
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Diff {
    index: usize,
    add: Option<CustomerId>,
    remove: Option<CustomerId>,
}
impl Diff {
    pub(crate) const fn new(
        index: usize,
        add: Option<CustomerId>,
        remove: Option<CustomerId>,
    ) -> Self {
        Self { index, add, remove }
    }
}

#[cfg(test)]
mod test {
    use typed_index_collections::ti_vec;

    use super::*;
    use crate::domain::{Coordinate, Event, ProblemInstance};

    #[test]
    fn test_incremental_opt_state() {
        let problem = ProblemInstance {
            name: String::new(),
            _num_vehicles: 0,
            vehicle_capacity: Capacity(200),
            events: ti_vec![
                Event {
                    customer_id: CustomerId(0),
                    requested_capacity: Capacity(40),
                    location: loc(0, 10),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(1),
                    requested_capacity: Capacity(50),
                    location: loc(0, 20),
                    kind: EventKind::Pickup,
                },
                Event {
                    customer_id: CustomerId(2),
                    requested_capacity: Capacity(160),
                    location: loc(0, 30),
                    kind: EventKind::Delivery,
                },
            ],
        };

        let mut state = OptState::init(&problem, true);

        // add a delivery to the route
        state.route.push(state.unrouted_deliveries.remove(0));

        assert_eq!(state.route, vec![CustomerId(1), CustomerId(0)]);
        let score = state.update_score(Diff::new(1, Some(CustomerId(0)), None), &problem);
        assert_ne!(score, ScoreResult::CapacityViolation);
        if let ScoreResult::NoCapacityViolation(x) = score {
            assert_eq!(x, MediumSoft::new(2, Distance(40.0)));
        }
        assert_eq!(state.route.len(), 2);
        assert_eq!(state.unrouted_pickups.len(), 0);
        assert_eq!(state.unrouted_deliveries.len(), 1);

        let ScoreState::Incremental(incr) = &state.score else {
            panic!();
        };
        assert_eq!(incr.free_capacity_at_start, Capacity(160));
        assert_eq!(incr.earliest_pickup_index, 0);
        assert_eq!(incr.distance_at, vec![Distance(20.0), Distance(30.0)]);
        assert_eq!(incr.distance_at_end, Distance(40.0));

        // add another delivery to the front of the route
        state
            .route
            .insert(0, state.unrouted_deliveries.pop().unwrap());
        state.pickup_index = 1;
        assert_eq!(
            state.route,
            vec![CustomerId(2), CustomerId(1), CustomerId(0)]
        );
        let score = state.update_score(Diff::new(0, Some(CustomerId(2)), None), &problem);
        let ScoreState::Incremental(incr) = &state.score else {
            panic!();
        };
        assert_ne!(score, ScoreResult::CapacityViolation);
        if let ScoreResult::NoCapacityViolation(x) = score {
            assert_eq!(x, MediumSoft::new(3, Distance(60.0)));
        }
        assert_eq!(state.route.len(), 3);
        assert_eq!(state.unrouted_pickups.len(), 0);
        assert_eq!(state.unrouted_deliveries.len(), 0);
        assert_eq!(incr.free_capacity_at_start, Capacity(0));
        assert_eq!(incr.earliest_pickup_index, 1);
        assert_eq!(
            incr.distance_at,
            vec![Distance(30.0), Distance(40.0), Distance(50.0)]
        );
        assert_eq!(incr.distance_at_end, Distance(60.0));
    }

    #[test]
    fn test_score_calculation() {
        let problem = ProblemInstance {
            name: String::new(),
            _num_vehicles: 0,
            vehicle_capacity: Capacity(200),
            events: ti_vec![
                Event {
                    customer_id: CustomerId(0),
                    requested_capacity: Capacity(40),
                    location: loc(0, 10),
                    kind: EventKind::Delivery,
                },
                Event {
                    customer_id: CustomerId(1),
                    requested_capacity: Capacity(50),
                    location: loc(10, 10),
                    kind: EventKind::Pickup,
                },
                Event {
                    customer_id: CustomerId(2),
                    requested_capacity: Capacity(160),
                    location: loc(10, 0),
                    kind: EventKind::Delivery,
                },
            ],
        };

        let score = calculate_score(&[CustomerId(0), CustomerId(1), CustomerId(2)], &problem);
        assert_eq!(score, ScoreResult::CapacityViolation);
        let score = calculate_score(&[CustomerId(2), CustomerId(1), CustomerId(0)], &problem);
        assert_eq!(
            score,
            ScoreResult::NoCapacityViolation(MediumSoft {
                medium_score: 3,
                soft_penalty: Distance(40.0)
            })
        );
    }

    fn loc(x: u16, y: u16) -> Location {
        Location {
            x: Coordinate(x),
            y: Coordinate(y),
        }
    }
}
