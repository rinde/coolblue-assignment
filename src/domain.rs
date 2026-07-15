use derive_more::{Add, AddAssign, Sub, SubAssign};
use typed_index_collections::TiVec;

#[derive(
    Debug, Clone, Copy, PartialEq, PartialOrd, Ord, Eq, Default, Add, AddAssign, Sub, SubAssign,
)]
pub(crate) struct Capacity(pub i16);

impl Capacity {
    pub(crate) const ZERO: Capacity = Capacity(0);
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(crate) struct CustomerId(pub u16);
impl std::fmt::Debug for CustomerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.0)
    }
}

impl From<usize> for CustomerId {
    fn from(value: usize) -> Self {
        CustomerId(value as u16)
    }
}

impl From<CustomerId> for usize {
    fn from(value: CustomerId) -> Self {
        value.0 as usize
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Coordinate(pub u16);

#[derive(Debug, Clone, Copy, Default, Add, PartialOrd, PartialEq)]
pub(crate) struct Distance(pub f64);

#[derive(Debug, Clone)]
pub(crate) struct ProblemInstance {
    pub(crate) name: String,
    pub(crate) _num_vehicles: usize,
    pub(crate) vehicle_capacity: Capacity,
    pub(crate) events: TiVec<CustomerId, Event>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Event {
    pub(crate) customer_id: CustomerId,
    pub(crate) requested_capacity: Capacity,
    pub(crate) location: Location,
    pub(crate) event_type: EventType,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum EventType {
    Pickup,
    Delivery,
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Location {
    pub x: Coordinate,
    pub y: Coordinate,
}

impl Location {
    pub(crate) const DEPOT: Location = Location {
        x: Coordinate(0),
        y: Coordinate(0),
    };

    pub(crate) fn distance(self, other: Location) -> Distance {
        // TODO optimize?
        Distance(
            (((self.x.0.abs_diff(other.x.0) as u64).pow(2)
                + (self.y.0.abs_diff(other.y.0) as u64).pow(2)) as f64)
                .sqrt(),
        )
    }
}
