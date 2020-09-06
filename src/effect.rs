use crate::{
    data::{Station, StationId},
    map::{Degree, MapCoord, MapFrame},
    tile::{Tile, TilePos},
};
use ahash::RandomState;
use crossbeam_channel::{unbounded, Sender};
use indexmap::IndexMap;
use line_drawing::Supercover;
use pathfinding::directed::astar::astar;
use rand::{thread_rng, Rng};
use rand_distr::{Distribution, Gamma};
use std::{
    collections::{HashMap, HashSet},
    iter::FromIterator,
    rc::Rc,
    sync::{Arc, RwLock},
    thread,
};

const MAX_STATION_POPULARITY: u32 = 20;
const MAX_EFFECTS: usize = 500;
const STATION_BLINK_COLOR: [u8; 3] = [0xff, 0xFF, 0x00];
const TRAIN_COLOR: [u8; 3] = [0x2A, 0xAF, 0xDB];

pub struct EffectManager {
    pub effects: Vec<Box<dyn Effect>>,
    stations: Rc<IndexMap<StationId, Station, RandomState>>,
    connections: Rc<HashMap<StationId, HashSet<StationId, RandomState>, RandomState>>,

    /// Used to keep track of how often trains visit each station in order to adjust A* heuristics
    station_popularity: Arc<RwLock<HashMap<StationId, u32, RandomState>>>,

    // Channel to update the station_popularity map
    write_sender: Sender<StationId>,
}

impl EffectManager {
    pub fn new(
        stations: Rc<IndexMap<StationId, Station, RandomState>>,
        connections: Rc<HashMap<StationId, HashSet<StationId, RandomState>, RandomState>>,
    ) -> Self {
        let (write_sender, write_receiver) = unbounded();

        let build_hasher = RandomState::new();
        let station_popularity: Arc<RwLock<HashMap<StationId, u32, RandomState>>> =
            Arc::new(RwLock::new(HashMap::with_hasher(build_hasher)));

        let pop_clone = station_popularity.clone();

        // Thread that will update the station_popularity map when messages come in from the Trains
        thread::spawn(move || {
            for msg in write_receiver {
                if let Ok(mut guard) = pop_clone.write() {
                    let current = guard.entry(msg).or_default();
                    *current = current.saturating_add(1);
                    if let Some(max) = guard.values().max() {
                        if *max > MAX_STATION_POPULARITY {
                            for val in guard.values_mut() {
                                *val /= (MAX_STATION_POPULARITY / 10) as u32
                            }
                        }
                    }
                }
            }
        });

        Self {
            effects: Vec::new(),
            stations,
            connections,
            station_popularity,
            write_sender,
        }
    }

    pub fn update(&mut self) {
        // Update the internal state of all effects
        for effect in self.effects.iter_mut() {
            effect.update();
        }

        // Remove any expired effects
        self.effects.retain(|effect| effect.is_valid());

        // Add some new effects, maybe
        if self.effects.len() < MAX_EFFECTS {
            let mut rng = thread_rng();

            let roll: f32 = rng.gen();

            if roll < 0.001 {
                let blink = StationBlink::new(self.stations.clone());
                self.effects.push(Box::new(blink));
            }

            if roll < 0.15 {
                if let Some(train) = Train::new(
                    self.stations.clone(),
                    self.connections.clone(),
                    self.write_sender.clone(),
                    self.station_popularity.clone(),
                ) {
                    self.effects.push(Box::new(train));
                }
            }
        }
    }
}

pub trait Effect {
    fn update(&mut self);

    /// When this turns to false, this Effect will be removed on the next update cycle. Should start
    /// at true and only flip to false once.
    fn is_valid(&self) -> bool;

    /// Given the current visible MapFrame, return which tiles should be colored in
    fn get_colors(&self, map_frame: &MapFrame) -> Vec<(Tile, &[u8; 3])>;

    fn priority(&self) -> u8; // higher = more priority
}

/// An effect that represents a station that's blinking for a few frames
pub struct StationBlink {
    coord: MapCoord,
    remaining_frames: u16,
}

impl StationBlink {
    pub fn new(stations: Rc<IndexMap<StationId, Station, RandomState>>) -> Self {
        let mut rng = thread_rng();

        let random_station_index = rng.gen_range(0, stations.len());
        let random_station = stations.get_index(random_station_index).unwrap().1;

        StationBlink {
            coord: random_station.coord,
            remaining_frames: rng.gen_range(500, 1000),
        }
    }
}

impl Effect for StationBlink {
    fn update(&mut self) {
        self.remaining_frames = self.remaining_frames.saturating_sub(1);
    }

    fn is_valid(&self) -> bool {
        self.remaining_frames > 0
    }

    fn priority(&self) -> u8 {
        2
    }

    fn get_colors(&self, map_frame: &MapFrame) -> Vec<(Tile, &[u8; 3])> {
        // Blink every x frames
        const BLINK_RATE: u16 = 100;
        if self.remaining_frames % BLINK_RATE * 2 < BLINK_RATE {
            let tile = map_frame.get_tile(self.coord);

            Tile::get_box(tile, map_frame.station_width())
                .map(|t| (t, &STATION_BLINK_COLOR))
                .collect()
        } else {
            vec![]
        }
    }
}

#[derive(Debug)]
pub struct TrackSection {
    start_station_id: StationId,
    end_station_id: StationId,
    length: Degree,
}

/// An effect that represents a train traveling, lighting up the track on the way
pub struct Train {
    // Shared with the World struct; needed to calculate the path to take
    stations: Rc<IndexMap<StationId, Station, RandomState>>,

    /// Stations pairs to traverse in order
    track_sections: Vec<TrackSection>,

    /// Current index in the `track_sections` Vec. Basically what section of the line the train is on
    current_section_index: usize,

    /// How much of the current section the train has traveled. Units are degrees
    current_line_progress: Degree,

    /// How far to travel each move
    degrees_per_move: Degree,

    write_sender: Sender<StationId>,
}

impl Train {
    pub fn new(
        stations: Rc<IndexMap<StationId, Station, RandomState>>,
        connections: Rc<HashMap<StationId, HashSet<StationId, RandomState>, RandomState>>,
        write_sender: Sender<StationId>,
        station_popularity: Arc<RwLock<HashMap<StationId, u32, RandomState>>>,
    ) -> Option<Self> {
        // Chose a random start and end station. The graph of stations only has 2 connected
        // components (Okinawa and everything else), so there's a good chance that there will be a
        // path between them.
        let mut rng = thread_rng();
        let start_index = rng.gen_range(0, stations.len());
        let end_index = rng.gen_range(0, stations.len());

        let start_id = stations.get_index(start_index).unwrap().0;
        let end_id = stations.get_index(end_index).unwrap().0;

        // For the A* heuristic, use the current "popularity" of a station. This should balance
        // things out so that the absolute shortest path isn't taken all the time, and promote
        // usage of less-traveled stations.
        // For obtaining the heuristic throught he RwLock, we're using `try_read` instead of `read`
        // so we don't block the thread. It's fine if occasionally we can't get the proper
        // heuristic due to the RwLock being locked for writing.

        let get_neighbors = |id: &StationId| -> Vec<(StationId, u32)> {
            let neighbor_ids: Vec<&StationId> =
                connections.get(id).map(Vec::from_iter).unwrap_or_default();

            neighbor_ids
                .iter()
                .map(|i| {
                    let score: u32 = if let Ok(guard) = station_popularity.try_read() {
                        guard.get(i).copied().unwrap_or(1)
                    } else {
                        1
                    };
                    (**i, score)
                })
                .collect()
        };

        let heuristic = |id: &StationId| -> u32 {
            if let Ok(guard) = station_popularity.try_read() {
                guard.get(id).copied().unwrap_or(1)
            } else {
                1
            }
        };

        if let Some((station_ids, _)) = astar(start_id, get_neighbors, heuristic, |id| id == end_id)
        {
            let mut track_sections: Vec<TrackSection> = Vec::new();
            for window in station_ids.windows(2) {
                let start_station_id = window[0];
                let end_station_id = window[1];
                let start_coord = stations.get(&start_station_id).unwrap().coord;
                let end_coord = stations.get(&end_station_id).unwrap().coord;

                track_sections.push(TrackSection {
                    start_station_id,
                    end_station_id,
                    length: start_coord.distance_to(&end_coord),
                });
            }

            // Just based on trying out various values, this distribution seems to give a good
            // range of speeds
            let gamma = Gamma::new(1.0, 0.002).unwrap();
            let degrees_per_move = gamma.sample(&mut rng) + 0.0005;

            Some(Self {
                stations,
                track_sections,
                current_section_index: 0,
                current_line_progress: 0.0.into(),
                degrees_per_move: degrees_per_move.into(),
                write_sender,
            })
        } else {
            // If there was no path to be found, just give up
            None
        }
    }

    /// Get the tile-wise path (between two stations) that the train is currently traveling on
    fn get_current_path(
        &self,
        current_track_section: &TrackSection,
        map_frame: &MapFrame,
    ) -> Vec<Tile> {
        let start_station = self
            .stations
            .get(&current_track_section.start_station_id)
            .unwrap();
        let end_station = self
            .stations
            .get(&current_track_section.end_station_id)
            .unwrap();

        let start_tile = map_frame.get_tile(start_station.coord);
        let end_tile = map_frame.get_tile(end_station.coord);

        let tiles_in_path: Vec<(TilePos, TilePos)> = Supercover::new(
            (start_tile.x.0, start_tile.y.0),
            (end_tile.x.0, end_tile.y.0),
        )
        .map(|(x, y)| (TilePos(x), TilePos(y)))
        .collect();

        tiles_in_path
            .into_iter()
            .map(|(x, y)| Tile { x, y })
            .collect()
    }
}

impl Effect for Train {
    fn update(&mut self) {
        // Travel a fixed amount of degrees per x ticks
        self.current_line_progress += self.degrees_per_move;

        // Seems like this should always be Some, but once it was None for some reason. Ignore
        // that case, not sure why it's happening at the moment. It happened when I set all the
        // trains to be really fast (0.007 degrees / tick).
        if let Some(current_track_section) = self.track_sections.get(self.current_section_index) {
            if self.current_line_progress >= current_track_section.length {
                self.current_line_progress = 0.0.into();
                self.current_section_index += 1;

                // Reached a new station at the end of the current TrackSection; update the popularity map
                if let Some(current_station) =
                    self.stations.get(&current_track_section.end_station_id)
                {
                    // The channel is unbounded so this shouldn't error; regardless we can ignore
                    // errors here. It's not vital that every message gets through, we're only
                    // using this for rough heuristics
                    let _ = self.write_sender.try_send(current_station.id);
                }
            }
        }
    }

    fn is_valid(&self) -> bool {
        self.current_section_index < self.track_sections.len()
    }

    fn priority(&self) -> u8 {
        1
    }

    fn get_colors(&self, map_frame: &MapFrame) -> Vec<(Tile, &[u8; 3])> {
        if let Some(current_track_section) = self.track_sections.get(self.current_section_index) {
            let path = self.get_current_path(&current_track_section, map_frame);

            // Find the tile in the current track that the train is on
            let tile_index = ((self.current_line_progress / current_track_section.length).0
                * path.len() as f32) as usize;
            let current_tile = path.get(tile_index).unwrap();

            Tile::get_box(*current_tile, map_frame.track_width())
                .map(|t| (t, &TRAIN_COLOR))
                .collect()
        } else {
            // Only came across this case once, not sure exactly what causes it. It's so rare that
            // let's just ignore it.
            vec![]
        }
    }
}
