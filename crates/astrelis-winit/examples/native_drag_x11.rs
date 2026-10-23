//! Two native X11 windows using the real drag host, without a renderer or RXUI.
//! For ordinary use, connect to DISPLAY and drag from blue to green; Ctrl-C exits.
//! --smoke injects one gesture ONLY on ASTRELIS_X11_TEST_DISPLAY (isolated Xvfb).
use astrelis_winit::{
    drag::{
        CapturedDrag, DragUpdate, NativeDragTarget,
        x11::{X11DragHost, X11Input},
    },
    winit::{dpi::LogicalPosition, event::MouseButton, window::WindowId},
};
use std::{
    error::Error,
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{self, ConnectionExt, EventMask},
        xtest::ConnectionExt as _,
    },
    rust_connection::RustConnection,
};
fn barrier(c: &RustConnection) -> Result<(), Box<dyn Error>> {
    c.get_input_focus()?.reply()?;
    Ok(())
}
fn create(
    c: &RustConnection,
    root: u32,
    x: i16,
    color: u32,
    title: &str,
) -> Result<u32, Box<dyn Error>> {
    let id = c.generate_id()?;
    c.create_window(
        x11rb::COPY_DEPTH_FROM_PARENT,
        id,
        root,
        x,
        100,
        260,
        200,
        0,
        xproto::WindowClass::INPUT_OUTPUT,
        0,
        &xproto::CreateWindowAux::new()
            .background_pixel(color)
            .event_mask(EventMask::BUTTON_PRESS | EventMask::BUTTON_RELEASE | EventMask::EXPOSURE),
    )?
    .check()?;
    c.change_property8(
        xproto::PropMode::REPLACE,
        id,
        xproto::AtomEnum::WM_NAME,
        xproto::AtomEnum::STRING,
        title.as_bytes(),
    )?
    .check()?;
    c.map_window(id)?.check()?;
    barrier(c)?;
    Ok(id)
}
use x11rb::wrapper::ConnectionExt as _;
fn main() -> Result<(), Box<dyn Error>> {
    let smoke = std::env::args().any(|a| a == "--smoke");
    let display =
        if smoke {
            Some(std::env::var("ASTRELIS_X11_TEST_DISPLAY").map_err(
                |_| "--smoke requires ASTRELIS_X11_TEST_DISPLAY pointing at isolated Xvfb",
            )?)
        } else {
            None
        };
    let (owner, screen) = x11rb::connect(display.as_deref())?;
    let owner = Arc::new(owner);
    let root = owner.setup().roots[screen].root;
    let source = create(
        &owner,
        root,
        80,
        0x234b88,
        "Source: hold primary button and drag to green",
    )?;
    let destination = create(
        &owner,
        root,
        440,
        0x23885b,
        "Destination: native X11 targeting",
    )?;
    let mut host = X11DragHost::connect(display.as_deref())?;
    host.register_xid(WindowId::from(1), source, 1.)?;
    host.register_xid(WindowId::from(2), destination, 1.)?;
    host.arm(WindowId::from(1))?;
    host.arm(WindowId::from(2))?;
    eprintln!(
        "X11 native capabilities: {:?}; windows {source:#x}, {destination:#x}",
        astrelis_winit::drag::NativeDragHost::capabilities(&host)
    );
    let (ready, go) = mpsc::sync_channel(0);
    let injection = if smoke {
        let c = owner.clone();
        Some(std::thread::spawn(move || -> Result<(), String> {
            let run = || -> Result<(), Box<dyn Error>> {
                c.xtest_fake_input(xproto::MOTION_NOTIFY_EVENT, 0, 0, root, 120, 140, 0)?
                    .check()?;
                c.xtest_fake_input(xproto::BUTTON_PRESS_EVENT, 1, 0, root, 0, 0, 0)?
                    .check()?;
                barrier(&c)?;
                go.recv_timeout(Duration::from_secs(3))?;
                c.xtest_fake_input(xproto::MOTION_NOTIFY_EVENT, 0, 0, root, 500, 160, 0)?
                    .check()?;
                c.xtest_fake_input(xproto::BUTTON_RELEASE_EVENT, 1, 0, root, 0, 0, 0)?
                    .check()?;
                barrier(&c)?;
                Ok(())
            };
            run().map_err(|e| e.to_string())
        }))
    } else {
        None
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if smoke && Instant::now() > deadline {
            return Err("timed out waiting for native press".into());
        }
        if let Some(X11Input::Pressed { window, .. }) = host.poll_input()? {
            let mut drag = CapturedDrag::begin(&mut host, window, MouseButton::Left)?;
            if smoke {
                ready.send(())?;
            }
            let mut dropped = None;
            while drag.is_active() {
                if smoke && Instant::now() > deadline {
                    return Err("timed out during native drag".into());
                }
                if let Some(event) = drag.poll_x11()? {
                    eprintln!("{event:?}");
                    if event.update == DragUpdate::Released {
                        dropped = Some(event.target);
                    }
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            drop(drag);
            if smoke {
                assert_eq!(
                    dropped,
                    Some(NativeDragTarget::Window {
                        window: WindowId::from(2),
                        position: LogicalPosition::new(60., 60.)
                    })
                );
                eprintln!(
                    "PASS: captured native motion/release outside source, destination hit, capture released"
                );
                break;
            }
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    if let Some(thread) = injection {
        thread
            .join()
            .map_err(|_| "injection thread panicked")?
            .map_err(|e| -> Box<dyn Error> { e.into() })?;
    }
    owner.destroy_window(source)?.check()?;
    owner.destroy_window(destination)?.check()?;
    Ok(())
}
