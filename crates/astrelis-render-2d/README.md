# astrelis-render-2d

A Y-down, logical-pixel-oriented scene renderer for sprites, atlas regions,
and finite chunked tilemaps. Submission order is stable within signed layers,
and compatible adjacent sprites are instanced without changing alpha order.

Run the direct-window demo with:

```text
cargo run -p astrelis-render-2d --example sprite_tilemap
```
