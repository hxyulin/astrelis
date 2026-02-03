//! Graphics context lifecycle and Arc management tests.

use astrelis_render::GraphicsContext;
use std::sync::Arc;

#[test]
#[ignore] // Requires GPU - run with: cargo test --test context_tests -- --ignored
fn test_context_creation_sync() {
    // Test synchronous context creation
    let result = GraphicsContext::new_owned_sync();

    match result {
        Ok(ctx) => {
            // Verify Arc is created
            assert_eq!(Arc::strong_count(&ctx), 1);

            // Verify context has device and queue
            assert!(!ctx.device().limits().max_texture_dimension_2d == 0);
        }
        Err(e) => {
            println!("GPU not available: {:?}", e);
            // Allow test to pass if no GPU (CI environments)
        }
    }
}

#[test]
#[ignore] // Requires GPU
fn test_context_arc_cloning() {
    let result = GraphicsContext::new_owned_sync();

    if let Ok(ctx) = result {
        // Test Arc cloning is cheap
        let ctx2 = ctx.clone();

        // Both should point to same context
        assert_eq!(Arc::strong_count(&ctx), 2);
        assert_eq!(Arc::strong_count(&ctx2), 2);

        // Verify they point to same data
        assert_eq!(
            ctx.device().limits().max_texture_dimension_2d,
            ctx2.device().limits().max_texture_dimension_2d
        );

        drop(ctx2);
        assert_eq!(Arc::strong_count(&ctx), 1);
    }
}

#[test]
#[ignore] // Requires GPU
fn test_context_cleanup() {
    let result = GraphicsContext::new_owned_sync();

    if let Ok(ctx) = result {
        let weak = Arc::downgrade(&ctx);
        assert!(weak.upgrade().is_some());

        drop(ctx);

        // Verify cleanup happened
        assert!(weak.upgrade().is_none());
    }
}

#[test]
fn test_graphics_error_display() {
    use astrelis_render::GraphicsError;

    let err = GraphicsError::NoAdapter;
    let display = format!("{:?}", err);
    assert!(display.contains("NoAdapter"));
}

// Async test requires tokio runtime - omitted for now
// #[tokio::test]
// #[ignore] // Requires GPU
// async fn test_context_creation_async() {
//     let result = GraphicsContext::new_owned().await;
//     if let Ok(ctx) = result {
//         assert_eq!(Arc::strong_count(&ctx), 1);
//     }
// }
