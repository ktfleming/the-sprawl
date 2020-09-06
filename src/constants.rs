use crate::map::Degree;

pub const SCREEN_WIDTH: u16 = 200;
pub const SCREEN_HEIGHT: u16 = 150;

pub const JAPAN_LEFT: Degree = Degree(127.59);
pub const JAPAN_RIGHT: Degree = Degree(145.77);
pub const JAPAN_TOP: Degree = Degree(46.5);
pub const JAPAN_BOTTOM: Degree = Degree(25.9);

// Arbitrary coordinate for the (0, 0) tile
pub const JAPAN_CENTER_LONG: Degree = Degree(137.710_62);
pub const JAPAN_CENTER_LAT: Degree = Degree(36.035_645);

/// Velocity limit for how fast you can zoom in/out
pub const SCROLL_DIFF_MAX: f32 = 30.0;

/// The tile size is the side-length, in pixels, of one "tile" on the map.
/// A tile is the smallest unit that can be marked as having a station or not
/// (i.e. it's like a virtual "pixel", which itself is made up of actual pixels
/// on the display). Currently this is set to 1, so one pixel is equal to one tile.
pub const TILE_SIZE: u16 = 1;

pub const NUMBER_OF_TILES_X: u16 = SCREEN_WIDTH / TILE_SIZE;
pub const NUMBER_OF_TILES_Y: u16 = SCREEN_HEIGHT / TILE_SIZE;

/// The width of the current MapFrame cannot be less than this
pub const MIN_ZOOM: Degree = Degree(0.01);
/// The width of the current MapFrame cannot be greater than this
pub const MAX_ZOOM: Degree = Degree(80.0);
