# astrelis-render

Shared scene-rendering vocabulary above `astrelis-gpu`. `RenderTarget`
distinguishes a texture allocation from the top-left rendered subextent so
window frames and resize-hysteresis UI views use the same renderer APIs.

Scene passes own and clear their targets. Direct compositor pass sharing is
reserved for Milestone 15.
