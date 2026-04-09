//! Application lifecycle states.

/// Application lifecycle states.
///
/// Models a mobile-friendly lifecycle where an application can be suspended
/// and resumed. On desktop, `Resumed` fires once at startup and `Suspended`
/// only at exit.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AppLifecycle {
    /// The application has been resumed and can create/interact with windows.
    /// On desktop this fires once after initialization.
    /// On mobile this fires each time the app enters the foreground.
    Resumed,
    /// The application has been suspended.
    /// All windows and GPU surfaces should be released.
    /// On mobile this fires when the app enters the background.
    Suspended,
    /// The application is about to exit.
    Exiting,
}
