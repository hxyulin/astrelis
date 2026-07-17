# astrelis-render

Shared scene-rendering vocabulary above `astrelis-gpu`. `RenderTarget`
distinguishes a texture allocation from the top-left rendered subextent so
window frames and resize-hysteresis UI views use the same renderer APIs.

Scene passes can own and clear standalone targets or load a compositor-owned
color attachment through `CompositedRenderTarget`. The latter makes the full
attachment and rectangular viewport/scissor region explicit.
