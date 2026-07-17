//! CPU-side sprite and tilemap recording data.

use astrelis_core::{
    color::Color,
    geometry::{Physical, Rect, Size},
    math::{Affine2, UVec2, Vec2},
};

use crate::{Camera2D, TextureHandle};

/// One textured sprite submission.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpriteDraw {
    /// Registered texture.
    pub texture: TextureHandle,
    /// Optional source rectangle in texture pixels.
    pub source: Option<Rect<Physical>>,
    /// Local-to-world transform applied after size and pivot.
    pub transform: Affine2,
    /// Sprite dimensions in world units.
    pub size: Vec2,
    /// Normalized origin within the sprite rectangle.
    pub pivot: Vec2,
    /// Straight-alpha tint multiplied with sampled pixels.
    pub tint: Color,
    /// Signed painter layer; larger layers appear on top.
    pub layer: i32,
}

/// Per-camera 2D scene submissions.
#[derive(Clone, Debug, Default)]
pub struct DrawList2D {
    pub(crate) sprites: Vec<SpriteDraw>,
}

impl DrawList2D {
    /// Creates an empty draw list.
    pub const fn new() -> Self {
        Self {
            sprites: Vec::new(),
        }
    }

    /// Records a sprite.
    pub fn draw_sprite(&mut self, sprite: SpriteDraw) {
        self.sprites.push(sprite);
    }

    /// Removes all submissions while retaining allocation capacity.
    pub fn clear(&mut self) {
        self.sprites.clear();
    }

    /// Returns the number of recorded sprite instances.
    pub fn len(&self) -> usize {
        self.sprites.len()
    }

    /// Returns whether no sprites are recorded.
    pub fn is_empty(&self) -> bool {
        self.sprites.is_empty()
    }
}

/// Atlas metadata used by a tilemap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileAtlas {
    /// Registered atlas texture.
    pub texture: TextureHandle,
    /// Full texture allocation size.
    pub texture_size: Size<Physical, u32>,
    /// Tile dimensions in texels.
    pub tile_size: Size<Physical, u32>,
    /// Empty texels before the first tile.
    pub margin: UVec2,
    /// Empty texels between tiles.
    pub spacing: UVec2,
}

impl TileAtlas {
    /// Returns the source rectangle for a row-major tile index.
    pub fn source(self, tile: u32) -> Option<Rect<Physical>> {
        if self.tile_size.width == 0 || self.tile_size.height == 0 {
            return None;
        }
        let stride_x = self.tile_size.width.checked_add(self.spacing.x)?;
        let stride_y = self.tile_size.height.checked_add(self.spacing.y)?;
        let available_x = self.texture_size.width.checked_sub(self.margin.x)?;
        let columns = available_x
            .checked_add(self.spacing.x)?
            .checked_div(stride_x)?;
        if columns == 0 {
            return None;
        }
        let column = tile % columns;
        let row = tile / columns;
        let x = self.margin.x.checked_add(column.checked_mul(stride_x)?)?;
        let y = self.margin.y.checked_add(row.checked_mul(stride_y)?)?;
        if x.checked_add(self.tile_size.width)? > self.texture_size.width
            || y.checked_add(self.tile_size.height)? > self.texture_size.height
        {
            return None;
        }
        Some(Rect::from_xywh(
            x as f32,
            y as f32,
            self.tile_size.width as f32,
            self.tile_size.height as f32,
        ))
    }
}

#[derive(Clone, Debug)]
struct TileChunk {
    dirty: bool,
    tiles: Vec<(UVec2, u32)>,
}

/// Finite dense tile grid with incrementally rebuilt CPU chunks.
#[derive(Clone, Debug)]
pub struct Tilemap {
    size: UVec2,
    tile_world_size: Vec2,
    chunk_size: UVec2,
    cells: Vec<Option<u32>>,
    chunks: Vec<TileChunk>,
    chunks_per_row: u32,
}

/// Placement and appearance of one recorded tilemap.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TilemapDraw {
    /// Tilemap-local to world transform.
    pub transform: Affine2,
    /// Painter layer shared by the recorded tiles.
    pub layer: i32,
    /// Straight-alpha tile tint.
    pub tint: Color,
}

impl Default for TilemapDraw {
    fn default() -> Self {
        Self {
            transform: Affine2::IDENTITY,
            layer: 0,
            tint: Color::WHITE,
        }
    }
}

impl Tilemap {
    /// Creates an empty map. Chunk dimensions must be nonzero.
    pub fn new(size: UVec2, tile_world_size: Vec2, chunk_size: UVec2) -> Option<Self> {
        if size.min_element() == 0
            || chunk_size.min_element() == 0
            || !tile_world_size.is_finite()
            || tile_world_size.min_element() <= 0.0
        {
            return None;
        }
        let cell_count = usize::try_from(u64::from(size.x) * u64::from(size.y)).ok()?;
        let chunks_per_row = size.x.div_ceil(chunk_size.x);
        let chunk_rows = size.y.div_ceil(chunk_size.y);
        let chunk_count =
            usize::try_from(u64::from(chunks_per_row) * u64::from(chunk_rows)).ok()?;
        Some(Self {
            size,
            tile_world_size,
            chunk_size,
            cells: vec![None; cell_count],
            chunks: vec![
                TileChunk {
                    dirty: true,
                    tiles: Vec::new()
                };
                chunk_count
            ],
            chunks_per_row,
        })
    }

    /// Creates an empty map with 32-by-32 render chunks.
    pub fn with_default_chunks(size: UVec2, tile_world_size: Vec2) -> Option<Self> {
        Self::new(size, tile_world_size, UVec2::splat(32))
    }

    /// Map dimensions in tiles.
    pub const fn size(&self) -> UVec2 {
        self.size
    }

    /// Returns one tile index, or `None` for an empty/out-of-range cell.
    pub fn tile(&self, coordinate: UVec2) -> Option<u32> {
        self.cell_index(coordinate)
            .and_then(|index| self.cells[index])
    }

    /// Replaces a tile and dirties only its containing chunk.
    pub fn set_tile(&mut self, coordinate: UVec2, tile: Option<u32>) -> bool {
        let Some(index) = self.cell_index(coordinate) else {
            return false;
        };
        if self.cells[index] != tile {
            self.cells[index] = tile;
            let chunk = coordinate / self.chunk_size;
            self.chunks[(chunk.y * self.chunks_per_row + chunk.x) as usize].dirty = true;
        }
        true
    }

    /// Appends visible tiles to a camera-specific draw list.
    pub fn record_visible(
        &mut self,
        list: &mut DrawList2D,
        atlas: TileAtlas,
        camera: Camera2D,
        logical_viewport: Vec2,
        draw: TilemapDraw,
    ) -> u32 {
        let Some((view_min, view_max)) = camera.visible_bounds(logical_viewport) else {
            return 0;
        };
        let mut culled = 0;
        for index in 0..self.chunks.len() {
            if self.chunks[index].dirty {
                self.rebuild_chunk(index);
            }
            let chunk = &self.chunks[index];
            if chunk.tiles.is_empty() {
                continue;
            }
            let chunk_x = index as u32 % self.chunks_per_row;
            let chunk_y = index as u32 / self.chunks_per_row;
            let local_min = Vec2::new(
                (chunk_x * self.chunk_size.x) as f32 * self.tile_world_size.x,
                (chunk_y * self.chunk_size.y) as f32 * self.tile_world_size.y,
            );
            let local_max = Vec2::new(
                ((chunk_x + 1) * self.chunk_size.x).min(self.size.x) as f32
                    * self.tile_world_size.x,
                ((chunk_y + 1) * self.chunk_size.y).min(self.size.y) as f32
                    * self.tile_world_size.y,
            );
            let (min, max) = transformed_bounds(draw.transform, local_min, local_max);
            if max.x < view_min.x || max.y < view_min.y || min.x > view_max.x || min.y > view_max.y
            {
                culled += 1;
                continue;
            }
            for &(coordinate, tile) in &chunk.tiles {
                let Some(source) = atlas.source(tile) else {
                    continue;
                };
                let tile_transform = draw.transform
                    * Affine2::from_translation(Vec2::new(
                        coordinate.x as f32 * self.tile_world_size.x,
                        coordinate.y as f32 * self.tile_world_size.y,
                    ));
                list.draw_sprite(SpriteDraw {
                    texture: atlas.texture,
                    source: Some(source),
                    transform: tile_transform,
                    size: self.tile_world_size,
                    pivot: Vec2::ZERO,
                    tint: draw.tint,
                    layer: draw.layer,
                });
            }
        }
        culled
    }

    fn cell_index(&self, coordinate: UVec2) -> Option<usize> {
        if coordinate.x >= self.size.x || coordinate.y >= self.size.y {
            return None;
        }
        Some((coordinate.y * self.size.x + coordinate.x) as usize)
    }

    fn rebuild_chunk(&mut self, index: usize) {
        let chunk_x = index as u32 % self.chunks_per_row;
        let chunk_y = index as u32 / self.chunks_per_row;
        let start = UVec2::new(chunk_x * self.chunk_size.x, chunk_y * self.chunk_size.y);
        let end = (start + self.chunk_size).min(self.size);
        let mut tiles = Vec::new();
        for y in start.y..end.y {
            for x in start.x..end.x {
                let coordinate = UVec2::new(x, y);
                if let Some(tile) = self.tile(coordinate) {
                    tiles.push((coordinate, tile));
                }
            }
        }
        self.chunks[index].tiles = tiles;
        self.chunks[index].dirty = false;
    }
}

pub(crate) fn transformed_bounds(transform: Affine2, min: Vec2, max: Vec2) -> (Vec2, Vec2) {
    let corners = [
        transform.transform_point2(min),
        transform.transform_point2(Vec2::new(max.x, min.y)),
        transform.transform_point2(max),
        transform.transform_point2(Vec2::new(min.x, max.y)),
    ];
    let mut result_min = corners[0];
    let mut result_max = corners[0];
    for point in &corners[1..] {
        result_min = result_min.min(*point);
        result_max = result_max.max(*point);
    }
    (result_min, result_max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atlas_honors_margin_and_spacing() {
        let atlas = TileAtlas {
            texture: TextureHandle::testing(0),
            texture_size: Size::new(70, 36),
            tile_size: Size::new(16, 16),
            margin: UVec2::splat(1),
            spacing: UVec2::splat(1),
        };
        assert_eq!(
            atlas.source(5).unwrap(),
            Rect::from_xywh(18.0, 18.0, 16.0, 16.0)
        );
        assert!(atlas.source(8).is_none());
    }

    #[test]
    fn tile_updates_only_rebuild_when_recorded() {
        let mut map = Tilemap::new(UVec2::new(4, 4), Vec2::splat(16.0), UVec2::splat(2)).unwrap();
        assert!(map.set_tile(UVec2::new(1, 1), Some(0)));
        assert_eq!(map.tile(UVec2::new(1, 1)), Some(0));
        assert!(!map.set_tile(UVec2::new(9, 9), Some(0)));
    }
}
