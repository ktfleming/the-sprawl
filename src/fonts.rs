use crate::data::StationId;
use crate::map::MapFrame;
use crate::tile::Tile;
use rusttype::{point, Font, Scale};

pub struct FontManager {
    font: Font<'static>,
}

impl FontManager {
    pub fn new() -> Self {
        let font_data = include_bytes!("../data/Kosugi-Regular.ttf");
        let font = Font::try_from_bytes(font_data as &[u8]).unwrap();
        Self { font }
    }

    /// Get the tiles that should be colored in with fonts in the given MapFrame
    pub fn get_font_tiles(
        &self,
        map_frame: &MapFrame,
        tiles_with_station: Vec<(&Tile, &String, &StationId)>,
    ) -> Vec<(Tile, usize)> {
        let mut result: Vec<(Tile, usize)> = Vec::new();

        // Height should scale based on map frame.
        const MAX_FONT_HEIGHT: f32 = 35.0;

        // When the current MapFrame has this height (in degrees), start showing station names
        const START_FRAME_HEIGHT: f32 = 0.5;
        const END_FRAME_HEIGHT: f32 = 0.01;

        if map_frame.height().0 > START_FRAME_HEIGHT {
            return vec![];
        }

        // How much they're zoomed in past the minimum frame height, from 0.0 to 1.0
        let zoom_factor: f32 =
            (map_frame.height().0 - START_FRAME_HEIGHT) / (END_FRAME_HEIGHT - START_FRAME_HEIGHT);

        let height: f32 = MAX_FONT_HEIGHT * zoom_factor;

        let scale = Scale {
            x: height,
            y: height,
        };

        let v_metrics = self.font.v_metrics(scale);
        let offset = point(0.0, v_metrics.ascent);

        for (tile, name, station_id) in tiles_with_station {
            let glyphs: Vec<_> = self.font.layout(name, scale, offset).collect();
            let width = scale.x;
            for (i, g) in glyphs.iter().enumerate() {
                if g.pixel_bounding_box().is_some() {
                    g.draw(|x, y, v| {
                        let x = x as i32;
                        let y = y as i32;

                        // (x, y) is the position to draw the glyph relative to its own bounding
                        // box. We want to draw the name centered around the station itself. So the
                        // x and y midpoint should be at `tile`.

                        let font_start_x =
                            tile.x.0 - ((glyphs.len() as f32 / 2.0) * width as f32) as i32;
                        let font_start_y = tile.y.0 - (height / 2.0) as i32;
                        let x_adjusted = x + font_start_x + ((width as i32) * i as i32);
                        let y_adjusted = y + font_start_y;

                        if v > 0.1 {
                            let tile = Tile {
                                x: x_adjusted.into(),
                                y: y_adjusted.into(),
                            };
                            let font_index = station_id.0.rem_euclid(3) as usize;
                            result.push((tile, font_index));
                        }
                    })
                }
            }
        }

        result
    }
}
