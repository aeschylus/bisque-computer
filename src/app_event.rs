//! Custom winit user events for bisque-computer.
//!
//! `EventLoop::<AppEvent>` replaces the default unit-typed loop so background
//! threads can wake the main event loop without going through a window event.

/// Application-level events sent via `EventLoopProxy::send_event()`.
#[derive(Debug, Clone)]
pub enum AppEvent {
    /// New bytes have arrived from a PTY reader thread.
    ///
    /// The main event loop handles this by calling `drain_all_output()` on
    /// the pane tree and requesting an immediate redraw, eliminating the
    /// ~16ms frame-rate polling latency floor. The coalescing dirty flag in
    /// each `TerminalPane` ensures at most one `PtyData` event is in-flight
    /// per pane at any time; rapid bursts do not flood the event queue.
    PtyData,
}
