use crate::constants::*;
use crate::{
    data::{Station, StationId},
    effect::EffectManager,
    fonts::FontManager,
    map::{zoom_ratio, Degree, MapFrame},
    tile::{Tile, TileStatus},
};
use ahash::RandomState;
use indexmap::IndexMap;
use line_drawing::Supercover;
use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
    time::Duration,
};

const TRACK_COLOR: [u8; 3] = [0x4F, 0x61, 0x6B];
const STATION_COLOR: [u8; 3] = [0xC4, 0x9D, 0xCF];
const BACKGROUND_COLOR: [u8; 3] = [0x32, 0x2F, 0x3D];

const FONT_COLORS: [[[u8; 3]; 10]; 3] = [
    // yellow
    [
        [0x37, 0x33, 0x43],
        [0x4c, 0x4a, 0x49],
        [0x62, 0x60, 0x4f],
        [0x77, 0x77, 0x55],
        [0x8d, 0x8e, 0x5b],
        [0xa2, 0xa4, 0x62],
        [0xb8, 0xbb, 0x68],
        [0xcd, 0xd2, 0x6e],
        [0xe2, 0xe8, 0x74],
        [0xf8, 0xff, 0x7a],
    ],
    // green
    [
        [0x37, 0x33, 0x43],
        [0x3e, 0x49, 0x4c],
        [0x45, 0x60, 0x56],
        [0x4b, 0x76, 0x5f],
        [0x52, 0x8c, 0x69],
        [0x59, 0xa3, 0x72],
        [0x60, 0xb9, 0x7c],
        [0x66, 0xcf, 0x86],
        [0x6d, 0xe6, 0x8f],
        [0x74, 0xfc, 0x98],
    ],
    // blue
    [
        [0x37, 0x33, 0x43],
        [0x36, 0x3f, 0x58],
        [0x35, 0x4b, 0x6c],
        [0x35, 0x56, 0x81],
        [0x34, 0x61, 0x95],
        [0x33, 0x6e, 0xaa],
        [0x32, 0x7a, 0xbe],
        [0x32, 0x85, 0xd3],
        [0x31, 0x92, 0xe7],
        [0x30, 0x9d, 0xfc],
    ],
];

/// Representation of the application state.
pub struct World {
    /// Just a collection of all Stations in Japan. Loaded once and never changes.
    /// key: station ID
    stations: Rc<IndexMap<StationId, Station, RandomState>>,

    /// Static collection of all station connections. Loaded once and never changes.
    /// key: station ID
    /// value: set of station IDs connected to the key station
    connections: Rc<HashMap<StationId, HashSet<StationId, RandomState>, RandomState>>,

    /// The area the user is currently looking at
    map_frame: MapFrame,

    effect_manager: EffectManager,

    font_manager: FontManager,

    /// Which tiles have stations/tracks on them. Recalculated on zoom/pan.
    base_map: HashMap<Tile, TileStatus, RandomState>,

    /// The Duration that elapsed between calls to `update`. Used to determine how many steps
    /// should be processed per `update` call.
    dt: Duration,
}

impl World {
    pub fn new(
        stations: IndexMap<StationId, Station, RandomState>,
        connections: HashMap<StationId, HashSet<StationId, RandomState>, RandomState>,
    ) -> Self {
        let stations = Rc::new(stations);
        let connections = Rc::new(connections);
        Self {
            stations: stations.clone(),
            connections: connections.clone(),
            map_frame: MapFrame::default(),
            effect_manager: EffectManager::new(stations, connections),
            font_manager: FontManager::new(),
            base_map: HashMap::with_hasher(RandomState::new()),
            dt: Duration::default(),
        }
    }

    pub fn init(&mut self) {
        self.update_base_map();
    }

    pub fn zoom(&mut self, mouse_cell: (isize, isize), scroll_diff: f32) {
        let (mouse_x, mouse_y) = mouse_cell;

        // How far right and down, proportionally, the user is zooming in/out at
        let x_factor: f32 = mouse_x as f32 / SCREEN_WIDTH as f32;
        let y_factor: f32 = mouse_y as f32 / SCREEN_HEIGHT as f32;

        let ratio = zoom_ratio(scroll_diff);

        let current_x_size = self.map_frame.width();
        let current_y_size = self.map_frame.height();

        let target_x_size = current_x_size * ratio.into();
        let target_y_size = current_y_size * ratio.into();

        // We want to adjust the left and right boundaries of the MapFrame to
        // make the new size equal to `target_x_size`, but we have to adjust them
        // in the ratio provided by `x_factor`. So if x_factor is close to 0, then the
        // left side will not move very much, but the right side will move a lot.

        // Positive = zoom out, negative = zoom in
        let amount_to_change_x: Degree = target_x_size - current_x_size;

        // For the left side, a positive change is zooming in, so we have to multiply by -1 here
        let left_change: Degree = amount_to_change_x * (x_factor * -1.0).into();
        let right_change: Degree = amount_to_change_x * (1.0 - x_factor).into();

        let new_left: Degree = self.map_frame.upper_left.long + left_change;
        let new_right: Degree = self.map_frame.lower_right.long + right_change;

        let new_length: Degree = new_right - new_left;
        if new_length > MAX_ZOOM || new_length < MIN_ZOOM {
            return;
        }

        let amount_to_change_y = target_y_size - current_y_size;
        let top_change = amount_to_change_y * y_factor.into();

        // For the bottom side, a positive change is zooming in
        let bottom_change = amount_to_change_y * ((1.0 - y_factor) * -1.0).into();

        self.map_frame.upper_left.long += left_change;
        self.map_frame.lower_right.long += right_change;

        self.map_frame.upper_left.lat += top_change;
        self.map_frame.lower_right.lat += bottom_change;

        // Zooming requires updating static positions of stations, tracks, fonts
        self.update_base_map();
    }

    /// Pan the current MapFrame by the specified amount in pixels
    pub fn pan(&mut self, diff_x: isize, diff_y: isize) {
        // diff_x and diff_y are the number of pixels to move, but we have to translate this to the
        // number of degrees to move
        let (degrees_per_pixel_x, degrees_per_pixel_y) = self.map_frame.get_degrees_per_pixel();

        self.map_frame.upper_left.long -= Degree(diff_x as f32) * degrees_per_pixel_x;
        self.map_frame.lower_right.long -= Degree(diff_x as f32) * degrees_per_pixel_x;
        self.map_frame.upper_left.lat += Degree(diff_y as f32) * degrees_per_pixel_y;
        self.map_frame.lower_right.lat += Degree(diff_y as f32) * degrees_per_pixel_y;

        // Panning requires updating static positions of stations, tracks, fonts
        self.update_base_map();
    }

    /// Update all visible tiles in regards to whether they contain stations/tracks.
    fn update_base_map(&mut self) {
        self.base_map.clear();
        let station_width = self.map_frame.station_width();
        let track_width = self.map_frame.track_width();

        let map_frame = &self.map_frame;

        // Only look at visible stations, all others would be wasted computation
        for station in self
            .stations
            .values()
            .filter(|s| map_frame.is_visible(s.coord))
        {
            let station_tile = self.map_frame.get_tile(station.coord);

            for tile in Tile::get_box(station_tile, station_width) {
                let status = if tile == station_tile {
                    TileStatus::Station((*station).clone())
                } else {
                    TileStatus::StationShadow
                };
                self.base_map.insert(tile, status);
            }
            if let Some(connected_stations) = self.connections.get(&station.id) {
                for other_station_id in connected_stations {
                    let other_station = self.stations.get(other_station_id).unwrap();

                    let tile1 = self.map_frame.get_tile(station.coord);
                    let tile2 = self.map_frame.get_tile(other_station.coord);

                    for (inner_x, inner_y) in
                        Supercover::new((tile1.x.0, tile1.y.0), (tile2.x.0, tile2.y.0))
                    {
                        let inner_tile = Tile {
                            x: inner_x.into(),
                            y: inner_y.into(),
                        };

                        for tile in Tile::get_box(inner_tile, track_width) {
                            match self.base_map.get(&tile) {
                                // Stations have priority over tracks, so don't do anything if a
                                // station was already present.
                                Some(TileStatus::Station(_)) | Some(TileStatus::StationShadow) => {}
                                _ => {
                                    self.base_map.insert(tile, TileStatus::Track);
                                }
                            };
                        }
                    }
                }
            }
        }

        // We've just calculated which tiles have a station, so pass this info to the FontManager
        // to get the tiles to draw station names on.
        let mut tiles_with_station: Vec<(&Tile, &String, &StationId)> = self
            .base_map
            .iter()
            .filter_map(|(tile, status)| match status {
                TileStatus::Station(station) => {
                    if self.map_frame.is_visible(station.coord) {
                        Some((tile, &station.name, &station.id))
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect();

        // Sort by tile for a consistent order so that station names don't overlap each other
        // randomly as you zoom in
        tiles_with_station.sort_by(|(t1, _, _), (t2, _, _)| (**t1).cmp(&t2));

        // Eliminate duplicate names on the same tile; these will just create visual noise
        tiles_with_station.dedup_by(|(t1, name1, _), (t2, name2, _)| t1 == t2 && name1 == name2);

        for (tile, font_index) in self
            .font_manager
            .get_font_tiles(&self.map_frame, tiles_with_station)
        {
            // Fonts have a lower priority than stations and tracks
            match self.base_map.get(&tile) {
                Some(TileStatus::Station(_))
                | Some(TileStatus::StationShadow)
                | Some(TileStatus::Track) => {}
                _ => {
                    self.base_map.insert(tile, TileStatus::Font(font_index));
                }
            }
        }
    }

    pub fn inspect(&self, mouse_cell: (isize, isize)) {
        let (mx, my) = mouse_cell;
        let coord = self.map_frame.get_map_coord(mx as i16, my as i16);
        let tile = self.map_frame.get_tile(coord);
        if let Some(TileStatus::Station(station)) = self.base_map.get(&tile) {
            println!("{}", station.name);
        }
    }

    /// Draw the `World` state to the frame buffer.
    pub fn draw(&mut self, buffer: &mut [u8]) {
        let mut effect_tile_map: HashMap<Tile, &[u8; 3]> = HashMap::new();

        // Process lower priority effects first so their colors will be overwritten with higher
        // priority effects if necessary
        self.effect_manager.effects.sort_by_key(|e| e.priority());
        for effect in &self.effect_manager.effects {
            for (tile, color) in effect.get_colors(&self.map_frame) {
                effect_tile_map.insert(tile, color);
            }
        }

        let font_level = self.map_frame.font_level();

        for (i, pixel) in buffer.chunks_exact_mut(4).enumerate() {
            // x and y are the coordinates of the screen pixel in question
            let x = (i % SCREEN_WIDTH as usize) as i16;
            let y = (i / SCREEN_WIDTH as usize) as i16;

            // Translate the pixel position to a map coordinate
            let coord = self.map_frame.get_map_coord(x, y);

            // Look up the tile that that map coordinate is in
            let tile = self.map_frame.get_tile(coord);

            // Determine the color for the tile, starting with the highest priority
            let color: &[u8; 3] = {
                if let Some(effect_color) = effect_tile_map.get(&tile) {
                    *effect_color
                } else {
                    match self.base_map.get(&tile) {
                        Some(TileStatus::Font(font_index)) => &FONT_COLORS[*font_index][font_level],
                        Some(TileStatus::Station(_)) | Some(TileStatus::StationShadow) => {
                            &STATION_COLOR
                        }
                        Some(TileStatus::Track) => &TRACK_COLOR,
                        None => &BACKGROUND_COLOR,
                    }
                }
            };

            let with_alpha: [u8; 4] = [color[0], color[1], color[2], 0xFF];

            pixel.copy_from_slice(&with_alpha);
        }
    }

    /// Run one step of the world's evolution for every frame (1/60 of a second) that has elapsed
    /// since the last call to this function
    pub fn update(&mut self, dt: &Duration) {
        let one_frame = Duration::new(0, 16_666_667);
        self.dt += *dt;

        while self.dt >= one_frame {
            self.dt -= one_frame;
            self.step();
        }
    }

    fn step(&mut self) {
        self.effect_manager.update();
    }
}
