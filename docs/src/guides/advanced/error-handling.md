# Error Handling

This guide explains how to handle errors gracefully in Astrelis applications. Learn to recover from graphics errors, handle asset loading failures, and provide good user experience during errors.

## Overview

**Error handling** in Astrelis covers:

- Graphics errors (surface lost, device lost)
- Asset loading failures
- Resource allocation errors
- Graceful degradation
- User-facing error messages
- Debug vs release error handling

**Key Principles:**
- Fail gracefully, not catastrophically
- Provide actionable error messages
- Log errors for debugging
- Recover when possible
- Degrade features if necessary

**Comparison to Unity:** Similar to Unity's error handling but requires explicit Result handling in Rust.

## GraphicsError Types

### Surface Lost

**What it is:** Window minimized, resized, or moved between monitors.

**Common scenarios:**
- User minimizes window
- Window dragged to different monitor
- Screen resolution changed
- Graphics driver reset

**Detection:**
```rust
match frame_result {
    Err(GraphicsError::SurfaceLost) => {
        warn!("Surface lost, recreating...");
        // Handle surface lost
    }
    Err(e) => {
        error!("Graphics error: {:?}", e);
    }
    Ok(frame) => {
        // Render normally
    }
}
```

**Recovery:**
```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        match self.renderable.begin_drawing() {
            Ok(mut frame) => {
                // Render frame
                frame.clear_and_render(
                    RenderTarget::Surface,
                    Color::BLACK,
                    |pass| {
                        self.ui.render(pass.wgpu_pass());
                    },
                );
                frame.finish();
            }
            Err(GraphicsError::SurfaceLost) => {
                warn!("Surface lost, attempting recovery...");

                // Recreate surface
                if let Err(e) = self.renderable.recreate_surface() {
                    error!("Failed to recreate surface: {:?}", e);
                    return;
                }

                info!("Surface recreated successfully");

                // Retry rendering next frame
            }
            Err(e) => {
                error!("Unrecoverable graphics error: {:?}", e);
            }
        }
    }
}
```

### Device Lost

**What it is:** GPU driver crashed or device removed.

**Common scenarios:**
- Driver crash (TDR - Timeout Detection and Recovery)
- GPU overheating
- Hardware failure
- Driver update during runtime

**Detection and handling:**
```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        match self.renderable.begin_drawing() {
            Err(GraphicsError::DeviceLost) => {
                error!("GPU device lost!");

                // Show error to user
                self.show_fatal_error("GPU device lost. Please restart the application.");

                // Request graceful shutdown
                ctx.request_exit();
            }
            Err(e) => {
                error!("Graphics error: {:?}", e);
            }
            Ok(frame) => {
                // Render normally
            }
        }
    }
}
```

**Note:** DeviceLost is typically not recoverable without restart.

### Out of Memory

**What it is:** GPU ran out of video memory.

**Common scenarios:**
- Too many large textures loaded
- Excessive buffer allocations
- Memory leak
- GPU memory fragmentation

**Detection:**
```rust
match device.create_buffer(&descriptor) {
    Err(e) if e.to_string().contains("out of memory") => {
        error!("GPU out of memory");
        // Handle OOM
    }
    Err(e) => {
        error!("Buffer creation failed: {:?}", e);
    }
    Ok(buffer) => {
        // Use buffer
    }
}
```

**Prevention and recovery:**
```rust
pub struct MemoryManager {
    allocated_bytes: AtomicU64,
    budget_bytes: u64,
}

impl MemoryManager {
    pub fn can_allocate(&self, size: u64) -> bool {
        let current = self.allocated_bytes.load(Ordering::Relaxed);
        current + size <= self.budget_bytes
    }

    pub fn allocate(&self, size: u64) -> Result<(), &'static str> {
        if !self.can_allocate(size) {
            return Err("GPU memory budget exceeded");
        }

        self.allocated_bytes.fetch_add(size, Ordering::Relaxed);
        Ok(())
    }

    pub fn deallocate(&self, size: u64) {
        self.allocated_bytes.fetch_sub(size, Ordering::Relaxed);
    }
}

// Usage
if memory_manager.can_allocate(texture_size) {
    let texture = load_texture(path)?;
    memory_manager.allocate(texture_size)?;
} else {
    warn!("Texture load skipped: insufficient memory");
    // Use placeholder texture
    let texture = placeholder_texture.clone();
}
```

## Asset Loading Errors

### File Not Found

**Handling missing assets:**
```rust
use astrelis_assets::{AssetServer, Handle};

fn load_texture(&mut self, path: &str) -> Handle<Texture> {
    match self.assets.load::<Texture>(path) {
        Ok(handle) => handle,
        Err(AssetLoadError::NotFound) => {
            warn!("Texture not found: {}, using placeholder", path);

            // Use placeholder texture
            self.assets.load::<Texture>("textures/missing.png")
                .unwrap_or_else(|_| self.create_fallback_texture())
        }
        Err(e) => {
            error!("Failed to load texture {}: {:?}", path, e);
            self.create_fallback_texture()
        }
    }
}

fn create_fallback_texture(&self) -> Handle<Texture> {
    // Create magenta checkerboard (missing texture indicator)
    let pixels = create_missing_texture_pixels();
    self.assets.load_from_bytes("fallback", &pixels).unwrap()
}
```

### Parse Errors

**Handling malformed asset files:**
```rust
use serde::Deserialize;

#[derive(Deserialize)]
pub struct LevelData {
    name: String,
    entities: Vec<EntityData>,
}

fn load_level(&mut self, path: &str) -> Result<LevelData, String> {
    let handle = self.assets.load::<TextAsset>(path)
        .map_err(|e| format!("Failed to load level file: {:?}", e))?;

    let text_asset = self.assets.get(&handle)
        .ok_or("Level file not ready")?;

    serde_json::from_str(&text_asset.content)
        .map_err(|e| {
            error!("Failed to parse level {}: {}", path, e);
            format!("Level file is corrupted: {}", e)
        })
}

// Usage with fallback
fn load_level_safe(&mut self, path: &str) -> LevelData {
    match self.load_level(path) {
        Ok(level) => level,
        Err(e) => {
            warn!("Using default level: {}", e);
            LevelData::default()
        }
    }
}
```

### Async Loading Errors

**Handling async failures:**
```rust
use astrelis_assets::AssetEvent;

impl App for MyGame {
    fn update(&mut self, ctx: &mut AppCtx, time: &FrameTime) {
        // Check for asset events
        for event in self.assets.drain_events() {
            match event {
                AssetEvent::Loaded { path, .. } => {
                    info!("Asset loaded: {}", path);
                }
                AssetEvent::Failed { path, error } => {
                    error!("Asset load failed: {} - {:?}", path, error);

                    // Show error to user
                    self.show_notification(&format!(
                        "Failed to load asset: {}", path
                    ));

                    // Use fallback
                    self.use_fallback_asset(&path);
                }
                AssetEvent::Modified { path } => {
                    info!("Asset modified: {}", path);
                }
            }
        }
    }
}
```

## Resource Allocation Errors

### Buffer Creation Failures

**Handling buffer allocation errors:**
```rust
pub struct BufferManager {
    device: Arc<wgpu::Device>,
    fallback_buffers: HashMap<wgpu::BufferUsages, wgpu::Buffer>,
}

impl BufferManager {
    pub fn create_buffer(&mut self, descriptor: &wgpu::BufferDescriptor) -> Result<wgpu::Buffer, String> {
        self.device.create_buffer(descriptor)
            .map_err(|e| {
                error!("Buffer creation failed: {:?}", e);

                // Check if we have a fallback
                if let Some(fallback) = self.fallback_buffers.get(&descriptor.usage) {
                    warn!("Using fallback buffer");
                    return Ok(fallback.clone());
                }

                format!("Failed to create buffer: {:?}", e)
            })
    }
}
```

### Texture Creation Failures

**Handling texture errors:**
```rust
pub fn create_texture_safe(
    device: &wgpu::Device,
    descriptor: &wgpu::TextureDescriptor,
) -> Result<wgpu::Texture, GraphicsError> {
    // Validate size
    if descriptor.size.width > 16384 || descriptor.size.height > 16384 {
        return Err(GraphicsError::TextureTooLarge);
    }

    // Estimate memory
    let bytes = descriptor.size.width * descriptor.size.height * 4;
    if bytes > 100_000_000 { // 100MB
        warn!("Large texture requested: {}MB", bytes / 1_000_000);
    }

    // Create texture
    device.create_texture(descriptor)
        .map_err(|e| {
            error!("Texture creation failed: {:?}", e);
            GraphicsError::TextureCreationFailed
        })
}
```

## Graceful Degradation

### Feature Fallbacks

**Disabling features on error:**
```rust
pub struct RenderFeatures {
    msaa: bool,
    shadows: bool,
    post_processing: bool,
}

impl App for MyGame {
    fn on_start(&mut self, ctx: &mut AppCtx) {
        // Try to enable MSAA
        self.features.msaa = match self.create_msaa_resources() {
            Ok(_) => {
                info!("MSAA enabled");
                true
            }
            Err(e) => {
                warn!("MSAA disabled: {:?}", e);
                false
            }
        };

        // Try to enable shadows
        self.features.shadows = match self.create_shadow_map() {
            Ok(_) => {
                info!("Shadows enabled");
                true
            }
            Err(e) => {
                warn!("Shadows disabled: {:?}", e);
                false
            }
        };
    }

    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        // Render with available features
        if self.features.msaa {
            self.render_with_msaa();
        } else {
            self.render_without_msaa();
        }

        if self.features.shadows {
            self.render_shadows();
        }
    }
}
```

### Quality Settings

**Automatic quality adjustment:**
```rust
pub enum QualityLevel {
    Ultra,
    High,
    Medium,
    Low,
}

impl App for MyGame {
    fn adjust_quality_on_error(&mut self, error: &GraphicsError) {
        match error {
            GraphicsError::OutOfMemory => {
                // Lower quality
                self.quality = match self.quality {
                    QualityLevel::Ultra => {
                        warn!("Lowering quality to High due to OOM");
                        QualityLevel::High
                    }
                    QualityLevel::High => {
                        warn!("Lowering quality to Medium due to OOM");
                        QualityLevel::Medium
                    }
                    QualityLevel::Medium => {
                        warn!("Lowering quality to Low due to OOM");
                        QualityLevel::Low
                    }
                    QualityLevel::Low => {
                        error!("Already at lowest quality, cannot degrade further");
                        QualityLevel::Low
                    }
                };

                // Apply new quality settings
                self.apply_quality_settings();

                // Reload textures at lower resolution
                self.reload_textures_at_quality(self.quality);
            }
            _ => {}
        }
    }
}
```

## User-Facing Error Messages

### Error Notification System

**Displaying errors to users:**
```rust
pub struct ErrorNotification {
    message: String,
    severity: Severity,
    timestamp: Instant,
}

pub enum Severity {
    Info,
    Warning,
    Error,
}

impl App for MyGame {
    fn show_error(&mut self, message: &str, severity: Severity) {
        // Add to notification queue
        self.notifications.push(ErrorNotification {
            message: message.to_string(),
            severity,
            timestamp: Instant::now(),
        });

        // Log to console
        match severity {
            Severity::Info => info!("{}", message),
            Severity::Warning => warn!("{}", message),
            Severity::Error => error!("{}", message),
        }

        // Update UI
        self.update_error_display();
    }

    fn update_error_display(&mut self) {
        // Remove old notifications
        self.notifications.retain(|n| {
            n.timestamp.elapsed() < Duration::from_secs(5)
        });

        // Rebuild notification UI
        self.ui.build(|root| {
            root.column()
                .gap(Length::px(5))
                .children(self.notifications.iter().map(|notif| {
                    move |c: &mut WidgetBuilder| {
                        let color = match notif.severity {
                            Severity::Info => Color::BLUE,
                            Severity::Warning => Color::YELLOW,
                            Severity::Error => Color::RED,
                        };

                        c.text(&notif.message)
                            .color(color)
                            .build()
                    }
                }))
                .build();
        });
    }
}
```

### Fatal Error Dialog

**Showing critical errors:**
```rust
impl App for MyGame {
    fn show_fatal_error(&mut self, message: &str) {
        error!("FATAL: {}", message);

        // Rebuild UI to show error screen
        self.ui.build(|root| {
            root.column()
                .width(Length::fill())
                .height(Length::fill())
                .justify_content(JustifyContent::Center)
                .align_items(AlignItems::Center)
                .background_color(Color::from_rgb(40, 40, 40))
                .child(|c| {
                    c.text("An Error Occurred")
                        .font_size(32.0)
                        .color(Color::RED)
                        .build()
                })
                .child(|c| {
                    c.text(message)
                        .font_size(16.0)
                        .color(Color::WHITE)
                        .build()
                })
                .child(|c| {
                    c.button("Quit")
                        .on_click(|| {
                            std::process::exit(1);
                        })
                        .build()
                })
                .build();
        });
    }
}
```

## Debug vs Release Error Handling

### Debug Mode

**Verbose error information in debug builds:**
```rust
#[cfg(debug_assertions)]
fn handle_error(error: &GraphicsError) {
    // Detailed error with backtrace
    error!("Graphics error: {:?}", error);
    error!("Backtrace: {:?}", std::backtrace::Backtrace::capture());

    // Panic on error in debug mode for immediate feedback
    panic!("Graphics error in debug mode: {:?}", error);
}

#[cfg(not(debug_assertions))]
fn handle_error(error: &GraphicsError) {
    // Graceful handling in release
    error!("Graphics error: {:?}", error);

    // Don't panic, attempt recovery
    attempt_error_recovery(error);
}
```

### Release Mode

**Graceful recovery in release builds:**
```rust
impl App for MyGame {
    fn render(&mut self, ctx: &mut AppCtx, window_id: WindowId, events: &mut EventBatch) {
        match self.renderable.begin_drawing() {
            Ok(frame) => {
                // Render normally
            }
            Err(e) => {
                #[cfg(debug_assertions)]
                {
                    // Panic immediately in debug
                    panic!("Render error: {:?}", e);
                }

                #[cfg(not(debug_assertions))]
                {
                    // Log and recover in release
                    error!("Render error: {:?}", e);

                    // Attempt recovery
                    if let Err(recovery_error) = self.attempt_recovery(&e) {
                        // Recovery failed, show error to user
                        self.show_fatal_error(&format!(
                            "A graphics error occurred. Please restart the application."
                        ));
                        ctx.request_exit();
                    }
                }
            }
        }
    }
}
```

## Best Practices

### ✅ DO: Log All Errors

```rust
// GOOD: Log with context
error!("Failed to load texture '{}': {:?}", path, error);
```

### ✅ DO: Provide Fallbacks

```rust
// GOOD: Graceful fallback
let texture = match assets.load(path) {
    Ok(handle) => handle,
    Err(e) => {
        warn!("Using placeholder texture: {:?}", e);
        placeholder_texture.clone()
    }
};
```

### ✅ DO: Show User-Friendly Messages

```rust
// GOOD: Clear, actionable message
self.show_error(
    "Failed to load save file. Your progress may be lost.",
    Severity::Warning
);
```

### ❌ DON'T: Silently Ignore Errors

```rust
// BAD: Swallowing errors
let _ = assets.load(path); // What if it fails?

// GOOD: Handle or log
match assets.load(path) {
    Ok(handle) => handle,
    Err(e) => {
        error!("Asset load failed: {:?}", e);
        fallback
    }
}
```

### ❌ DON'T: Panic in Production

```rust
// BAD: Panic in release builds
let handle = assets.load(path).unwrap(); // Don't do this!

// GOOD: Graceful handling
let handle = assets.load(path)
    .unwrap_or_else(|e| {
        error!("Asset load failed: {:?}", e);
        fallback_handle
    });
```

## Comparison to Unity

| Unity | Astrelis | Notes |
|-------|----------|-------|
| Try/catch | Result<T, E> | Rust uses Result type |
| Debug.LogError() | error!() macro | Logging errors |
| Application.Quit() | ctx.request_exit() | Graceful shutdown |
| Resources.Load() | AssetServer.load() | Both can fail |

## Troubleshooting

### Frequent Surface Lost Errors

**Cause:** Window events not handled properly.

**Fix:** Ensure `recreate_surface()` is called on `SurfaceLost`.

### Crashes on GPU Errors

**Cause:** Using `.unwrap()` on graphics operations.

**Fix:** Use proper error handling with `match` or `?` operator.

### Missing Textures Not Visible

**Cause:** No fallback textures.

**Fix:** Create magenta checkerboard placeholder for missing assets.

## Next Steps

- **Practice:** Add error handling to your application
- **Improve:** Create user-friendly error notifications
- **Advanced:** Implement automatic quality adjustment
- **Examples:** `error_handling_demo`, `graceful_degradation`

## See Also

- [Multi-Window Apps](multi-window-apps.md) - Surface lost in multi-window
- [Asset System](../asset-system/loading-assets.md) - Asset loading errors
- API Reference: [`GraphicsError`](../../api/astrelis-render/enum.GraphicsError.html)
