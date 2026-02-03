//! Sprite sheet support for animations and sprite-based rendering.
//!
//! A sprite sheet (also known as a texture atlas with uniform cells) contains
//! multiple sprites arranged in a grid, all with the same dimensions.
//! This is commonly used for:
//! - Character animations
//! - Loading spinners
//! - Progress bar segments
//! - Tile sets

use crate::context::GraphicsContext;

/// A sprite sheet containing uniformly-sized sprites in a grid layout.
///
/// All sprites in a sprite sheet have the same dimensions and are arranged
/// in rows and columns. This makes it efficient for animations where each
/// frame is the same size.
///
/// # Example
///
/// ```ignore
/// // Create a sprite sheet from a texture
/// let sprite_sheet = SpriteSheet::new(
///     context,
///     texture,
///     SpriteSheetDescriptor {
///         sprite_width: 32,
///         sprite_height: 32,
///         columns: 8,
///         rows: 4,
///         ..Default::default()
///     },
/// );
///
/// // Get UV coordinates for sprite at index 5
/// let uv = sprite_sheet.sprite_uv(5);
///
/// // Or by row/column
/// let uv = sprite_sheet.sprite_uv_at(1, 2);
/// ```
#[derive(Debug)]
pub struct SpriteSheet {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    /// Width of each sprite in pixels
    sprite_width: u32,
    /// Height of each sprite in pixels
    sprite_height: u32,
    /// Number of columns in the grid
    columns: u32,
    /// Number of rows in the grid
    rows: u32,
    /// Total texture width
    texture_width: u32,
    /// Total texture height
    texture_height: u32,
    /// Padding between sprites (in pixels)
    padding: u32,
    /// Margin around the entire sheet (in pixels)
    margin: u32,
}

/// Descriptor for creating a sprite sheet.
#[derive(Debug, Clone)]
pub struct SpriteSheetDescriptor {
    /// Width of each sprite in pixels
    pub sprite_width: u32,
    /// Height of each sprite in pixels
    pub sprite_height: u32,
    /// Number of sprite columns
    pub columns: u32,
    /// Number of sprite rows
    pub rows: u32,
    /// Padding between sprites (default: 0)
    pub padding: u32,
    /// Margin around the entire sheet (default: 0)
    pub margin: u32,
}

impl Default for SpriteSheetDescriptor {
    fn default() -> Self {
        Self {
            sprite_width: 32,
            sprite_height: 32,
            columns: 1,
            rows: 1,
            padding: 0,
            margin: 0,
        }
    }
}

/// UV coordinates for a sprite (normalized 0-1 range).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpriteUV {
    /// U coordinate of the left edge
    pub u_min: f32,
    /// V coordinate of the top edge
    pub v_min: f32,
    /// U coordinate of the right edge
    pub u_max: f32,
    /// V coordinate of the bottom edge
    pub v_max: f32,
}

impl SpriteUV {
    /// Create new sprite UV coordinates.
    pub fn new(u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        Self {
            u_min,
            v_min,
            u_max,
            v_max,
        }
    }

    /// Get UV coordinates as arrays for shader upload.
    pub fn as_arrays(&self) -> ([f32; 2], [f32; 2]) {
        ([self.u_min, self.v_min], [self.u_max, self.v_max])
    }

    /// Flip the sprite horizontally.
    pub fn flip_horizontal(&self) -> Self {
        Self {
            u_min: self.u_max,
            v_min: self.v_min,
            u_max: self.u_min,
            v_max: self.v_max,
        }
    }

    /// Flip the sprite vertically.
    pub fn flip_vertical(&self) -> Self {
        Self {
            u_min: self.u_min,
            v_min: self.v_max,
            u_max: self.u_max,
            v_max: self.v_min,
        }
    }
}

impl SpriteSheet {
    /// Create a new sprite sheet from an existing texture.
    pub fn new(
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        texture_width: u32,
        texture_height: u32,
        descriptor: SpriteSheetDescriptor,
    ) -> Self {
        Self {
            texture,
            view,
            sprite_width: descriptor.sprite_width,
            sprite_height: descriptor.sprite_height,
            columns: descriptor.columns,
            rows: descriptor.rows,
            texture_width,
            texture_height,
            padding: descriptor.padding,
            margin: descriptor.margin,
        }
    }

    /// Create a sprite sheet from raw pixel data.
    ///
    /// # Arguments
    ///
    /// * `context` - Graphics context
    /// * `data` - Raw RGBA pixel data
    /// * `texture_width` - Width of the texture in pixels
    /// * `texture_height` - Height of the texture in pixels
    /// * `descriptor` - Sprite sheet configuration
    pub fn from_data(
        context: &GraphicsContext,
        data: &[u8],
        texture_width: u32,
        texture_height: u32,
        descriptor: SpriteSheetDescriptor,
    ) -> Self {
        let size = wgpu::Extent3d {
            width: texture_width,
            height: texture_height,
            depth_or_array_layers: 1,
        };

        let texture = context.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("Sprite Sheet Texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        context.queue().write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(texture_width * 4),
                rows_per_image: Some(texture_height),
            },
            size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self::new(texture, view, texture_width, texture_height, descriptor)
    }

    /// Get UV coordinates for a sprite by linear index.
    ///
    /// Sprites are indexed left-to-right, top-to-bottom, starting from 0.
    pub fn sprite_uv(&self, index: u32) -> SpriteUV {
        let col = index % self.columns;
        let row = index / self.columns;
        self.sprite_uv_at(row, col)
    }

    /// Get UV coordinates for a sprite by row and column.
    pub fn sprite_uv_at(&self, row: u32, col: u32) -> SpriteUV {
        // Calculate pixel coordinates
        let x = self.margin + col * (self.sprite_width + self.padding);
        let y = self.margin + row * (self.sprite_height + self.padding);

        // Convert to normalized UV coordinates
        let u_min = x as f32 / self.texture_width as f32;
        let v_min = y as f32 / self.texture_height as f32;
        let u_max = (x + self.sprite_width) as f32 / self.texture_width as f32;
        let v_max = (y + self.sprite_height) as f32 / self.texture_height as f32;

        SpriteUV {
            u_min,
            v_min,
            u_max,
            v_max,
        }
    }

    /// Get the total number of sprites in the sheet.
    pub fn sprite_count(&self) -> u32 {
        self.columns * self.rows
    }

    /// Get the sprite dimensions in pixels.
    pub fn sprite_size(&self) -> (u32, u32) {
        (self.sprite_width, self.sprite_height)
    }

    /// Get the grid dimensions (columns, rows).
    pub fn grid_size(&self) -> (u32, u32) {
        (self.columns, self.rows)
    }

    /// Get the texture view for binding.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get the underlying texture.
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }

    /// Get texture dimensions.
    pub fn texture_size(&self) -> (u32, u32) {
        (self.texture_width, self.texture_height)
    }
}

/// Animation state for cycling through sprite sheet frames.
#[derive(Debug, Clone)]
pub struct SpriteAnimation {
    /// First frame index (inclusive)
    start_frame: u32,
    /// Last frame index (inclusive)
    end_frame: u32,
    /// Current frame index
    current_frame: u32,
    /// Time per frame in seconds
    frame_duration: f32,
    /// Time accumulated since last frame change
    elapsed: f32,
    /// Whether the animation loops
    looping: bool,
    /// Whether the animation is playing
    playing: bool,
    /// Direction (1 = forward, -1 = backward)
    direction: i32,
}

impl SpriteAnimation {
    /// Create a new animation for all frames.
    pub fn new(total_frames: u32, fps: f32) -> Self {
        Self {
            start_frame: 0,
            end_frame: total_frames.saturating_sub(1),
            current_frame: 0,
            frame_duration: 1.0 / fps,
            elapsed: 0.0,
            looping: true,
            playing: true,
            direction: 1,
        }
    }

    /// Create an animation for a range of frames.
    pub fn with_range(start: u32, end: u32, fps: f32) -> Self {
        Self {
            start_frame: start,
            end_frame: end,
            current_frame: start,
            frame_duration: 1.0 / fps,
            elapsed: 0.0,
            looping: true,
            playing: true,
            direction: 1,
        }
    }

    /// Set whether the animation loops.
    pub fn looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Update the animation with elapsed time.
    ///
    /// Returns true if the frame changed.
    pub fn update(&mut self, dt: f32) -> bool {
        if !self.playing {
            return false;
        }

        self.elapsed += dt;

        if self.elapsed >= self.frame_duration {
            self.elapsed -= self.frame_duration;

            let frame_count = self.end_frame - self.start_frame + 1;
            let relative_frame = self.current_frame - self.start_frame;
            let new_relative = (relative_frame as i32 + self.direction) as u32;

            if new_relative >= frame_count {
                if self.looping {
                    self.current_frame = self.start_frame;
                } else {
                    self.playing = false;
                    self.current_frame = self.end_frame;
                }
            } else {
                self.current_frame = self.start_frame + new_relative;
            }

            return true;
        }

        false
    }

    /// Get the current frame index.
    pub fn current_frame(&self) -> u32 {
        self.current_frame
    }

    /// Jump to a specific frame.
    pub fn set_frame(&mut self, frame: u32) {
        self.current_frame = frame.clamp(self.start_frame, self.end_frame);
        self.elapsed = 0.0;
    }

    /// Play the animation.
    pub fn play(&mut self) {
        self.playing = true;
    }

    /// Pause the animation.
    pub fn pause(&mut self) {
        self.playing = false;
    }

    /// Stop and reset the animation.
    pub fn stop(&mut self) {
        self.playing = false;
        self.current_frame = self.start_frame;
        self.elapsed = 0.0;
    }

    /// Check if the animation is playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Check if the animation has finished (only relevant for non-looping).
    pub fn is_finished(&self) -> bool {
        !self.looping && !self.playing && self.current_frame == self.end_frame
    }

    /// Reverse the animation direction.
    pub fn reverse(&mut self) {
        self.direction = -self.direction;
    }

    /// Set the playback speed (fps).
    pub fn set_fps(&mut self, fps: f32) {
        self.frame_duration = 1.0 / fps;
    }

    /// Get normalized progress (0.0 to 1.0).
    pub fn progress(&self) -> f32 {
        let frame_count = self.end_frame - self.start_frame + 1;
        if frame_count <= 1 {
            return 1.0;
        }
        (self.current_frame - self.start_frame) as f32 / (frame_count - 1) as f32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sprite_uv_calculation() {
        // 4x4 sprites, 32x32 each, in a 128x128 texture
        let descriptor = SpriteSheetDescriptor {
            sprite_width: 32,
            sprite_height: 32,
            columns: 4,
            rows: 4,
            padding: 0,
            margin: 0,
        };

        // We can't actually create a sprite sheet without a GPU context,
        // but we can test the UV calculation logic
        let uv = calculate_sprite_uv(&descriptor, 128, 128, 0, 0);
        assert_eq!(uv.u_min, 0.0);
        assert_eq!(uv.v_min, 0.0);
        assert_eq!(uv.u_max, 0.25);
        assert_eq!(uv.v_max, 0.25);

        let uv = calculate_sprite_uv(&descriptor, 128, 128, 1, 1);
        assert_eq!(uv.u_min, 0.25);
        assert_eq!(uv.v_min, 0.25);
        assert_eq!(uv.u_max, 0.5);
        assert_eq!(uv.v_max, 0.5);
    }

    fn calculate_sprite_uv(
        desc: &SpriteSheetDescriptor,
        tex_w: u32,
        tex_h: u32,
        row: u32,
        col: u32,
    ) -> SpriteUV {
        let x = desc.margin + col * (desc.sprite_width + desc.padding);
        let y = desc.margin + row * (desc.sprite_height + desc.padding);

        SpriteUV {
            u_min: x as f32 / tex_w as f32,
            v_min: y as f32 / tex_h as f32,
            u_max: (x + desc.sprite_width) as f32 / tex_w as f32,
            v_max: (y + desc.sprite_height) as f32 / tex_h as f32,
        }
    }

    #[test]
    fn test_animation_basic() {
        let mut anim = SpriteAnimation::new(4, 10.0);
        assert_eq!(anim.current_frame(), 0);

        // Advance one frame
        assert!(anim.update(0.1));
        assert_eq!(anim.current_frame(), 1);

        // Advance to end
        anim.update(0.1);
        anim.update(0.1);
        anim.update(0.1);
        assert_eq!(anim.current_frame(), 0); // Looped back
    }

    #[test]
    fn test_animation_no_loop() {
        let mut anim = SpriteAnimation::new(3, 10.0).looping(false);

        anim.update(0.1);
        anim.update(0.1);
        anim.update(0.1);

        assert!(anim.is_finished());
        assert_eq!(anim.current_frame(), 2);
    }

    #[test]
    fn test_uv_flip() {
        let uv = SpriteUV::new(0.0, 0.0, 0.5, 0.5);

        let flipped_h = uv.flip_horizontal();
        assert_eq!(flipped_h.u_min, 0.5);
        assert_eq!(flipped_h.u_max, 0.0);

        let flipped_v = uv.flip_vertical();
        assert_eq!(flipped_v.v_min, 0.5);
        assert_eq!(flipped_v.v_max, 0.0);
    }
}
