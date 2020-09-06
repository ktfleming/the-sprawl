use crate::data::Station;
use derive_more::{Add, AddAssign, Div, From, Mul, Sub};

#[derive(
    Clone, Copy, Debug, Add, AddAssign, Sub, Mul, Div, From, PartialOrd, PartialEq, Eq, Hash, Ord,
)]
#[mul(forward)]
#[div(forward)]
#[from(forward)]
pub struct TilePos(pub i32);

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug, PartialOrd, Ord)]
pub struct Tile {
    pub x: TilePos,
    pub y: TilePos,
}

impl Tile {
    /// Get a TileIterator for the box with the given center tile and side length.
    pub fn get_box(center: Tile, side_length: i32) -> TileIterator {
        // Needed to make the calculations work
        let side_length = side_length - 1;
        let x_start: i32 = center.x.0 - (side_length as f32 / 2.0).floor() as i32;
        let mut x_end: i32 = center.x.0 + (side_length as f32 / 2.0).floor() as i32;
        let y_start: i32 = center.y.0 - (side_length as f32 / 2.0).floor() as i32;
        let mut y_end: i32 = center.y.0 + (side_length as f32 / 2.0).floor() as i32;

        // Account for "uneven" boxes
        if side_length % 2 == 1 {
            x_end += 1;
            y_end += 1;
        }

        let upper_left = Tile {
            x: x_start.into(),
            y: y_start.into(),
        };
        let lower_right = Tile {
            x: x_end.into(),
            y: y_end.into(),
        };

        TileIterator::new(upper_left, lower_right)
    }
}

/// The items that can be present in the world's "base map". An empty tile is represented by not
/// being present in the HashMap.
pub enum TileStatus {
    /// This tile should be used for drawing the font (station name) layer. Contains the index for
    /// which font color to use.
    Font(usize),

    /// For simplicity, only one station can be "present" in a tile at once, even if there are
    /// actually multiple ones overlapping. It shouldn't affect the drawing in anyway, since the
    /// tile is the smallest unit we can draw.
    Station(Station),

    /// When you zoom in, each station can take up more than one tile. We still want to only keep
    /// the center/main tile as the one that "actually" as the station, since that's used to
    /// determine where to draw the station name. The surrounding tiles will just contain the
    /// "shadow" of the station.
    StationShadow,

    Track,
}

pub struct TileIterator {
    upper_left: Tile,
    lower_right: Tile,
    x: TilePos,
    y: TilePos,
}

impl TileIterator {
    pub fn new(upper_left: Tile, lower_right: Tile) -> Self {
        TileIterator {
            upper_left,
            lower_right,
            x: upper_left.x,
            y: upper_left.y,
        }
    }
}

/// Iterator to step through tiles in order from the top row to the bottom row, going left-to-right
/// within a row.
impl Iterator for TileIterator {
    type Item = Tile;

    fn next(&mut self) -> Option<Self::Item> {
        if self.y > self.lower_right.y {
            return None;
        }

        if self.x < self.lower_right.x {
            // Increase x first if possible
            let result = Some(Tile {
                x: self.x,
                y: self.y,
            });
            self.x += 1.into();
            result
        } else {
            // Reached the end of the row -- go to next row
            let result = Some(Tile {
                x: self.x,
                y: self.y,
            });
            self.x = self.upper_left.x;
            self.y += 1.into();
            result
        }
    }
}
