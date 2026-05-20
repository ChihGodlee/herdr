//! OSC 22 mouse pointer shape sequences.
//!
//! Tells the host terminal emulator to change the OS mouse pointer when the
//! user hovers a draggable region in herdr (pane splitters, sidebar dividers,
//! scrollbar thumbs, tabs, workspace cards). Supported on Kitty, WezTerm,
//! Ghostty, xterm. Other terminals silently ignore the sequence per the OSC
//! spec, so emitting unconditionally is safe.
//!
//! Sequence format: `OSC 22 ; <W3C cursor name> ST` where `ST` is `\x1b\\`. An
//! empty payload (`\x1b]22;\x1b\\`) resets the pointer to the terminal
//! default.
//!
//! When running inside `tmux`, sequences must be DCS-wrapped so they reach the
//! outer terminal. The `in_tmux` argument controls that wrapping; callers are
//! expected to detect via `std::env::var_os("TMUX").is_some()`.

use std::io::{self, Write};

use serde::{Deserialize, Serialize};

/// Mouse pointer shapes herdr can request the terminal show. The narrow set
/// covers every draggable region in the UI; extending it later is a
/// non-breaking serde change as long as new variants gain `#[serde(other)]`
/// fallbacks on the deserialize side or are wrapped in `Option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum MousePointerShape {
    /// Reset to terminal default (empty OSC 22 payload).
    #[default]
    Default,
    /// `pointer` — clickable button (used over track scrollbars and toggles).
    Pointer,
    /// `grab` — draggable element at rest (tabs, scrollbar thumbs, workspace cards).
    Grab,
    /// `grabbing` — draggable element actively being dragged.
    Grabbing,
    /// `col-resize` — vertical splitter the user can drag horizontally.
    ColResize,
    /// `row-resize` — horizontal splitter the user can drag vertically.
    RowResize,
}

impl MousePointerShape {
    /// W3C / CSS cursor name placed in the OSC 22 payload, or `None` for
    /// `Default` which uses an empty payload to reset.
    pub fn osc22_name(self) -> Option<&'static str> {
        match self {
            MousePointerShape::Default => None,
            MousePointerShape::Pointer => Some("pointer"),
            MousePointerShape::Grab => Some("grab"),
            MousePointerShape::Grabbing => Some("grabbing"),
            MousePointerShape::ColResize => Some("col-resize"),
            MousePointerShape::RowResize => Some("row-resize"),
        }
    }
}

/// Build the raw bytes for an OSC 22 sequence. When `in_tmux`, wraps the
/// sequence in tmux DCS passthrough so it reaches the outer terminal.
pub fn osc22_bytes_for(shape: MousePointerShape, in_tmux: bool) -> Vec<u8> {
    let name = shape.osc22_name().unwrap_or("");
    let raw = format!("\x1b]22;{name}\x1b\\").into_bytes();
    if in_tmux {
        crate::terminal_notify::wrap_tmux_passthrough(&raw)
    } else {
        raw
    }
}

/// Write an OSC 22 sequence to `writer`. Callers usually ignore the io error
/// because pointer-shape changes are non-critical.
pub fn emit_pointer_shape<W: Write>(
    writer: &mut W,
    shape: MousePointerShape,
    in_tmux: bool,
) -> io::Result<()> {
    let bytes = osc22_bytes_for(shape, in_tmux);
    writer.write_all(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_shape_emits_empty_payload_reset() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::Default, false),
            b"\x1b]22;\x1b\\"
        );
    }

    #[test]
    fn col_resize_emits_w3c_name() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::ColResize, false),
            b"\x1b]22;col-resize\x1b\\"
        );
    }

    #[test]
    fn row_resize_emits_w3c_name() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::RowResize, false),
            b"\x1b]22;row-resize\x1b\\"
        );
    }

    #[test]
    fn grab_emits_w3c_name() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::Grab, false),
            b"\x1b]22;grab\x1b\\"
        );
    }

    #[test]
    fn grabbing_emits_w3c_name() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::Grabbing, false),
            b"\x1b]22;grabbing\x1b\\"
        );
    }

    #[test]
    fn pointer_emits_w3c_name() {
        assert_eq!(
            osc22_bytes_for(MousePointerShape::Pointer, false),
            b"\x1b]22;pointer\x1b\\"
        );
    }

    #[test]
    fn tmux_wraps_in_dcs_passthrough() {
        let raw = osc22_bytes_for(MousePointerShape::Grab, false);
        let wrapped = osc22_bytes_for(MousePointerShape::Grab, true);
        assert!(wrapped.starts_with(b"\x1bPtmux;"));
        assert!(wrapped.ends_with(b"\x1b\\"));
        assert_ne!(raw, wrapped);
        // Inner ESC bytes get doubled by the tmux passthrough wrap.
        assert!(wrapped.windows(2).filter(|w| w == b"\x1b\x1b").count() >= 2);
    }

    #[test]
    fn emit_pointer_shape_writes_bytes_to_writer() {
        let mut buf = Vec::<u8>::new();
        emit_pointer_shape(&mut buf, MousePointerShape::ColResize, false).unwrap();
        assert_eq!(buf, b"\x1b]22;col-resize\x1b\\");
    }

    #[test]
    fn shape_default_is_default_variant() {
        assert_eq!(MousePointerShape::default(), MousePointerShape::Default);
    }
}
