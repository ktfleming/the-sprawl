use crate::{
    constants::*,
    tile::{Tile, TileIterator, TilePos},
};
use derive_more::{Add, AddAssign, Div, From, FromStr, Mul, Sub, SubAssign};
use std::fmt::Display;

/// For longitude and latitude
#[derive(
    Clone,
    Copy,
    Debug,
    Add,
    Sub,
    Mul,
    Div,
    From,
    FromStr,
    AddAssign,
    SubAssign,
    PartialOrd,
    PartialEq,
)]
#[mul(forward)]
#[div(forward)]
#[from(forward)]
pub struct Degree(pub f32);

#[derive(Clone, Copy, Debug)]
pub struct MapCoord {
    pub long: Degree,
    pub lat: Degree,
}

impl MapCoord {
    pub fn distance_to(&self, other: &MapCoord) -> Degree {
        let long_dist: Degree = self.long - other.long;
        let lat_dist: Degree = self.lat - other.lat;

        let sum_of_squares: Degree = (long_dist * long_dist) + (lat_dist * lat_dist);

        Degree(sum_of_squares.0.sqrt())
    }
}

impl Display for MapCoord {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.long.0, self.lat.0)
    }
}

/// A rectangle view onto the map. Values are lat/long
#[derive(Debug)]
pub struct MapFrame {
    pub upper_left: MapCoord,
    pub lower_right: MapCoord,
}

impl MapFrame {
    pub fn width(&self) -> Degree {
        self.lower_right.long - self.upper_left.long
    }

    pub fn height(&self) -> Degree {
        self.upper_left.lat - self.lower_right.lat
    }

    /// Get the tile that contains the given map coordinate
    pub fn get_tile(&self, coord: MapCoord) -> Tile {
        let degrees_from_center_x = coord.long - JAPAN_CENTER_LONG;
        let degrees_from_center_y = JAPAN_CENTER_LAT - coord.lat;

        // The number of degrees per tile depends on how far we're zoomed-in,
        // i.e. the dimensions of the current MapFrame
        let degrees_per_tile_x = self.width() / NUMBER_OF_TILES_X.into();
        let degrees_per_tile_y = self.height() / NUMBER_OF_TILES_Y.into();

        // There's no bounds-checking on panning, meaning that if you pan really far away from the
        // tile center (middle of Japan), it's possible that these offets could saturate at the
        // max/min values for i32...but everything will be offscreen anyway, so it shouldn't
        // matter.
        let tile_offset_left: i32 = (degrees_from_center_x / degrees_per_tile_x).0 as i32;
        let tile_offset_top: i32 = (degrees_from_center_y / degrees_per_tile_y).0 as i32;

        Tile {
            x: TilePos(tile_offset_left),
            y: TilePos(tile_offset_top),
        }
    }

    /// Get all visible tiles for this MapFrame
    pub fn visible_tiles(&self) -> TileIterator {
        let upper_left = self.get_tile(self.upper_left);
        let lower_right = self.get_tile(self.lower_right);

        TileIterator::new(upper_left, lower_right)
    }

    /// Get how many map degrees (long and lat) a single pixel in this frame currently represents
    pub fn get_degrees_per_pixel(&self) -> (Degree, Degree) {
        let degrees_per_pixel_x = self.width() / SCREEN_WIDTH.into();
        let degrees_per_pixel_y = self.height() / SCREEN_HEIGHT.into();

        (degrees_per_pixel_x, degrees_per_pixel_y)
    }

    /// Translate a (visible) screen pixel position to a map coordinate
    pub fn get_map_coord(&self, pixel_x: i16, pixel_y: i16) -> MapCoord {
        let (degrees_per_pixel_x, degrees_per_pixel_y) = self.get_degrees_per_pixel();

        // Get offsets from the top-left corner
        let map_x: Degree = self.upper_left.long + degrees_per_pixel_x * Degree(pixel_x as f32);
        let map_y: Degree = self.upper_left.lat - degrees_per_pixel_y * Degree(pixel_y as f32);

        MapCoord {
            long: map_x,
            lat: map_y,
        }
    }

    /// Check whether the given MapCoord is visible in this MapFrame
    pub fn is_visible(&self, coord: MapCoord) -> bool {
        // At high zoom levels, add a "margin" to the bounds we're checking, so that we can draw
        // tracks and station names that originate from a station that's actually off-screen, to
        // avoid pop-in.
        let margin: Degree = if self.height().0 < 0.05 {
            // Rough formula that seems to work well; start at margin of 10% and increase as we
            // zoom in more
            (0.10 + (0.05 - self.height().0)).into()
        } else {
            0.0.into()
        };

        coord.long >= (self.upper_left.long - self.width() * margin)
            && coord.long <= (self.lower_right.long + self.width() * margin)
            && coord.lat <= (self.upper_left.lat + self.height() * margin)
            && coord.lat >= (self.lower_right.lat - self.height() * margin)
    }

    /// How many tiles (on one side) to use to draw a station
    pub fn station_width(&self) -> i32 {
        let height = self.height().0;

        if height < 0.02 {
            5
        } else if height < 0.05 {
            4
        } else if height < 0.1 {
            3
        } else if height < 0.3 {
            2
        } else {
            1
        }
    }

    /// How many tiles (on one side) to use to draw a track piece
    pub fn track_width(&self) -> i32 {
        let height = self.height().0;

        if height < 0.06 {
            2
        } else {
            1
        }
    }

    /// How bright/emphasized (0-9) fonts should be at the current zoom level
    pub fn font_level(&self) -> usize {
        // Note: fonts don't even appear until height < 0.5
        let height = self.height().0;

        if height < 0.015 {
            9
        } else if height < 0.0175 {
            8
        } else if height < 0.02 {
            7
        } else if height < 0.025 {
            6
        } else if height < 0.03 {
            5
        } else if height < 0.035 {
            4
        } else if height < 0.04 {
            3
        } else if height < 0.045 {
            2
        } else if height < 0.05 {
            1
        } else {
            0
        }
    }
}

impl Default for MapFrame {
    /// MapFrame that fits most of Japan
    fn default() -> Self {
        Self {
            upper_left: MapCoord {
                long: JAPAN_LEFT,
                lat: JAPAN_TOP,
            },
            lower_right: MapCoord {
                long: JAPAN_RIGHT,
                lat: JAPAN_BOTTOM,
            },
        }
    }
}

/// The ratio that the side lengths of the current MapFrame should change,
/// given an amount that was scrolled. For example, if this returns 1.1, it means
/// that if the MapFrame is currently showing 10 degrees of longitude and 7 degrees
/// of latitude, then we want to adjust it to show 10.1 degrees of longitude and
/// 7.7 degrees of latitude
pub fn zoom_ratio(scroll_diff: f32) -> f32 {
    let mut clamped = scroll_diff;

    // scroll_diff seems to vary between 0.1 and around 30 for very forceful scrolling
    if clamped > SCROLL_DIFF_MAX {
        clamped = SCROLL_DIFF_MAX;
    } else if clamped < -1.0 * SCROLL_DIFF_MAX {
        clamped = -1.0 * SCROLL_DIFF_MAX;
    }

    let sign = if clamped.is_sign_positive() {
        1.0
    } else {
        -1.0
    };
    let ratio = clamped.abs() / SCROLL_DIFF_MAX;

    // Three levels of zooming depending on how fast the mouse wheel is scrolled
    let offset = {
        if ratio < 0.33 {
            0.10
        } else if ratio < 0.66 {
            0.20
        } else {
            0.30
        }
    };

    1.0 - (offset * sign)
}
