//! Restoring and remembering the main window (section 9: "Fensterzustand"),
//! and the zoom of the interface (section 11: "Oberflächenzoom").

use tauri::{PhysicalPosition, PhysicalSize, Runtime, WebviewWindow};

use crate::services::settings::ui::WindowState;

/// Part of the window that must lie on a monitor, so it can be grabbed: the
/// left part of the title bar.
const GRAB_WIDTH: i64 = 120;
const GRAB_HEIGHT: i64 = 32;

/// Applies the saved state (if it still fits on a monitor) and shows the
/// window. The window starts hidden (`tauri.conf.json`), so it never jumps.
pub fn restore<R: Runtime>(window: &WebviewWindow<R>, state: Option<WindowState>) {
    if let Some(state) = state {
        let monitors: Vec<(i64, i64, i64, i64)> = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|monitor| {
                let position = monitor.position();
                let size = monitor.size();
                (
                    i64::from(position.x),
                    i64::from(position.y),
                    i64::from(size.width),
                    i64::from(size.height),
                )
            })
            .collect();
        if fits(&state, &monitors) {
            let _ = window.set_size(PhysicalSize::new(state.width, state.height));
            let _ = window.set_position(PhysicalPosition::new(state.x, state.y));
        } else {
            log::info!("saved window position is off screen; default used");
        }
        if state.maximized {
            let _ = window.maximize();
        }
    }
    if let Err(error) = window.show() {
        log::error!("cannot show the window: {error}");
    }
}

/// The current state. While the window is maximized, the normal size and
/// position from `previous` are kept, so un-maximizing after the next start
/// returns to them.
pub fn capture<R: Runtime>(
    window: &WebviewWindow<R>,
    previous: Option<WindowState>,
) -> Option<WindowState> {
    let maximized = window.is_maximized().ok()?;
    if window.is_minimized().unwrap_or(false) || window.is_fullscreen().unwrap_or(false) {
        // Minimized or full-screen geometry is no useful start state.
        return previous.map(|state| WindowState { maximized, ..state });
    }
    if maximized && let Some(previous) = previous {
        return Some(WindowState {
            maximized: true,
            ..previous
        });
    }
    let position = window.outer_position().ok()?;
    let size = window.inner_size().ok()?;
    let state = WindowState {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
    };
    state.validate().ok().map(|()| state)
}

/// Sets the zoom factor of the interface.
pub fn apply_zoom<R: Runtime>(window: &WebviewWindow<R>, zoom: f64) {
    if let Err(error) = window.set_zoom(zoom) {
        log::warn!("zoom not applied: {error}");
    }
}

/// Whether the grab area of the window lies on one of the monitors
/// (`x, y, width, height` in physical pixels).
fn fits(state: &WindowState, monitors: &[(i64, i64, i64, i64)]) -> bool {
    let left = i64::from(state.x);
    let top = i64::from(state.y);
    monitors.iter().any(|&(x, y, width, height)| {
        left + GRAB_WIDTH > x
            && left < x + width - GRAB_WIDTH / 2
            && top >= y - GRAB_HEIGHT / 2
            && top < y + height - GRAB_HEIGHT
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: i32, y: i32) -> WindowState {
        WindowState {
            x,
            y,
            width: 1000,
            height: 700,
            maximized: false,
        }
    }

    #[test]
    fn a_window_must_be_grabbable_on_some_monitor() {
        let monitors = [(0, 0, 1920, 1080), (1920, 0, 2560, 1440)];
        assert!(fits(&at(100, 100), &monitors));
        assert!(fits(&at(2000, 50), &monitors), "second monitor");
        assert!(fits(&at(-60, 0), &monitors), "partly off to the left");
        assert!(!fits(&at(-500, 100), &monitors), "title bar off screen");
        assert!(!fits(&at(100, 1070), &monitors), "below every monitor");
        assert!(!fits(&at(100, -200), &monitors), "above every monitor");
        assert!(!fits(&at(5000, 100), &monitors), "monitor unplugged");
        assert!(!fits(&at(100, 100), &[]), "no monitor known");
    }
}
