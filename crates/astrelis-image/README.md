# astrelis-image

Optional decoding of encoded raster images into `astrelis-paint::Image`.

The crate keeps codec dependencies out of the core paint and UI crates. Its
default features decode PNG, JPEG, and WebP; applications may disable defaults
and enable only the formats they ship.
