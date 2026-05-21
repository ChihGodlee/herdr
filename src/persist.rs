//! Session persistence — save/restore workspaces, layouts, and working directories.
//!
//! Stored at `~/.config/herdr/session.json`.

mod io;
mod restore;
mod snapshot;

pub use self::io::{clear, load, save};
pub use self::restore::restore;
pub use self::snapshot::{
    capture, DirectionSnapshot, LayoutSnapshot, PaneSnapshot, SessionSnapshot, TabSnapshot,
    WorkspaceSnapshot,
};
