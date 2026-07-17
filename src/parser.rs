use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use typed_index_collections::TiVec;

use crate::domain::{
    Capacity, Coordinate, CustomerId, Event, EventKind, Location, ProblemInstance,
};

pub(crate) fn parse(path: &str) -> Result<ProblemInstance, Error> {
    let file = File::open(path).map_err(Error::Io)?;
    let reader = BufReader::new(file);

    let mut lines = reader.lines();

    let name = lines
        .next()
        .ok_or(Error::UnexpectedFileFormat("1"))?
        .map_err(Error::Io)?;
    for _ in 0..3 {
        let _ = lines.next().ok_or(Error::UnexpectedFileFormat("2"))?;
    }
    let vehicle_line = lines
        .next()
        .ok_or(Error::UnexpectedFileFormat("3"))?
        .map_err(Error::Io)?;
    let mut vehicle_line_iter = vehicle_line.split_whitespace();

    let num_vehicles = vehicle_line_iter
        .next()
        .ok_or(Error::UnexpectedFileFormat("4"))?
        .parse::<usize>()
        .map_err(Error::ParseIntError)?;
    let vehicle_capacity = vehicle_line_iter
        .next()
        .ok_or(Error::UnexpectedFileFormat("5"))?
        .parse::<i16>()
        .map(Capacity)
        .map_err(Error::ParseIntError)?;

    for _ in 0..5 {
        let _ = lines.next().ok_or(Error::UnexpectedFileFormat("6"))?;
    }

    let events = lines
        .enumerate()
        .map(|(i, line)| {
            let line = line.map_err(Error::Io)?;
            let mut line_iter = line
                .split_whitespace()
                .map(|x| x.parse::<u16>().map_err(Error::ParseIntError));

            let customer_id = line_iter
                .next()
                .ok_or(Error::UnexpectedFileFormat("7"))?
                .map(|c| CustomerId(c - 1))?;
            // sanity check to ensure ids are unique and sequential
            if customer_id.0 != i as u16 {
                return Err(Error::UnexpectedFileFormat("8"));
            }
            let x = line_iter
                .next()
                .ok_or(Error::UnexpectedFileFormat("9"))?
                .map(Coordinate)?;
            let y = line_iter
                .next()
                .ok_or(Error::UnexpectedFileFormat("10"))?
                .map(Coordinate)?;
            let requested_capacity = line_iter
                .next()
                .ok_or(Error::UnexpectedFileFormat("11"))?
                .map(|c| Capacity(c as i16))?;
            Ok(Event {
                customer_id,
                requested_capacity,
                location: Location { x, y },
                kind: EventKind::Delivery,
            })
        })
        .collect::<Result<TiVec<CustomerId, Event>, Error>>()?;

    Ok(ProblemInstance {
        name,
        _num_vehicles: num_vehicles,
        vehicle_capacity,
        events,
    })
}

#[derive(Debug)]
#[expect(dead_code, reason = "The error message is for printing to the user")]
pub(crate) enum Error {
    Io(std::io::Error),
    UnexpectedFileFormat(&'static str),
    ParseIntError(std::num::ParseIntError),
}
