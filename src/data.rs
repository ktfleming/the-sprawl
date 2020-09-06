use crate::map::{Degree, MapCoord};
use ahash::RandomState;
use csv::Reader;
use indexmap::IndexMap;
use std::{
    collections::{HashMap, HashSet},
    fmt::Display,
};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct StationId(pub u32);

// Corresponds to entries in stations.csv
#[derive(Debug, Clone)]
pub struct Station {
    pub id: StationId,
    pub name: String,
    pub coord: MapCoord,
}

// Corresponds to entries in join.csv
pub struct Connection {
    station_id_1: StationId,
    station_id_2: StationId,
}

impl Display for Station {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {} {}", self.id.0, self.name, self.coord)
    }
}

pub fn load_stations() -> IndexMap<StationId, Station, RandomState> {
    let bytes: &[u8] = include_bytes!("../data/stations.csv");
    let mut reader = Reader::from_reader(bytes);

    let mut result: IndexMap<StationId, Station, RandomState> =
        IndexMap::with_hasher(RandomState::new());

    for record in reader.records() {
        let record = record.unwrap();
        let long: Degree = record.get(2).unwrap().parse().unwrap();
        let lat: Degree = record.get(3).unwrap().parse().unwrap();

        let station = Station {
            id: StationId(record.get(0).unwrap().parse().unwrap()),
            name: record.get(1).unwrap().to_owned(),
            coord: MapCoord { long, lat },
        };

        result.insert(station.id, station);
    }

    result
}

pub fn load_connections() -> HashMap<StationId, HashSet<StationId, RandomState>, RandomState> {
    let bytes: &[u8] = include_bytes!("../data/join.csv");
    let mut reader = Reader::from_reader(bytes);

    let vec: Vec<Connection> = reader
        .records()
        .map(|record| {
            let record = record.unwrap();
            let station_id_1: StationId = StationId(record.get(0).unwrap().parse().unwrap());
            let station_id_2: StationId = StationId(record.get(1).unwrap().parse().unwrap());

            Connection {
                station_id_1,
                station_id_2,
            }
        })
        .collect();

    let mut result: HashMap<StationId, HashSet<StationId, RandomState>, RandomState> =
        HashMap::with_hasher(RandomState::new());

    for record in vec {
        result
            .entry(record.station_id_1)
            .or_default()
            .insert(record.station_id_2);
        result
            .entry(record.station_id_2)
            .or_default()
            .insert(record.station_id_1);
    }

    result
}
