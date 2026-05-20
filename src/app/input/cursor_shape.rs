//! Compute the OS mouse pointer shape from the current hover position and
//! drag state.
//!
//! Called from [`App::handle_mouse`](super::App::handle_mouse) after every
//! mouse event. Produces a [`MousePointerShape`] which the renderer publishes
//! via OSC 22 (see `src/cursor_shape.rs`). When the experimental flag is off
//! or the layout is mobile, always returns `Default` so the renderer emits a
//! reset.

use crossterm::event::MouseEvent;
use ratatui::layout::{Direction, Rect};

use super::ScrollbarClickTarget;
use crate::app::state::{AppState, DragTarget, Mode, ViewLayout};
use crate::cursor_shape::MousePointerShape;

impl AppState {
    /// Recompute the desired pointer shape based on the most recent mouse
    /// event. Sets `pending_mouse_pointer_shape` for the next render.
    pub(super) fn recompute_mouse_pointer_shape(&mut self, mouse: MouseEvent) {
        self.pending_mouse_pointer_shape =
            self.compute_mouse_pointer_shape(mouse.column, mouse.row);
    }

    /// Recompute the desired pointer shape after a view-geometry change
    /// (resize, mode switch, workspace open) using the last known mouse
    /// position. No-op when no mouse has been seen yet.
    pub(crate) fn recompute_mouse_pointer_shape_from_last_pos(&mut self) {
        if let Some((col, row)) = self.last_mouse_pos {
            self.pending_mouse_pointer_shape = self.compute_mouse_pointer_shape(col, row);
        }
    }

    fn compute_mouse_pointer_shape(&self, col: u16, row: u16) -> MousePointerShape {
        // Disabled by config → always default.
        if !self.mouse_pointer_shapes_enabled {
            return MousePointerShape::Default;
        }

        // Mobile layout is touch-first; mouse-hover semantics don't apply.
        if self.view.layout == ViewLayout::Mobile {
            return MousePointerShape::Default;
        }

        // Active drag overrides hover — keep shape sticky as cursor leaves
        // the source region. crossterm's Drag events reach handle_mouse so
        // this fires every drag tick.
        if let Some(drag) = &self.drag {
            return shape_for_drag_target(&drag.target);
        }

        // Modal overlays: only modal-relevant scrollbars / clickable items
        // produce non-default shapes.
        match self.mode {
            Mode::Onboarding
            | Mode::Settings
            | Mode::ConfirmClose
            | Mode::RenameWorkspace
            | Mode::RenameTab
            | Mode::RenamePane
            | Mode::Resize
            | Mode::ContextMenu => {
                return MousePointerShape::Default;
            }
            Mode::ReleaseNotes => {
                return self
                    .release_notes_scrollbar_target_at(col, row)
                    .map(|t| shape_for_scrollbar_target(&t))
                    .unwrap_or(MousePointerShape::Default);
            }
            Mode::ProductAnnouncement => {
                return self
                    .product_announcement_scrollbar_target_at(col, row)
                    .map(|t| shape_for_scrollbar_target(&t))
                    .unwrap_or(MousePointerShape::Default);
            }
            Mode::KeybindHelp => {
                return self
                    .keybind_help_scrollbar_target_at(col, row)
                    .map(|t| shape_for_scrollbar_target(&t))
                    .unwrap_or(MousePointerShape::Default);
            }
            Mode::GlobalMenu => {
                return if self.global_menu_item_at(col, row).is_some() {
                    MousePointerShape::Pointer
                } else {
                    MousePointerShape::Default
                };
            }
            Mode::Terminal | Mode::Navigate => {} // fall through to non-modal path
        }

        // Non-modal hit-tests, in priority order matching handle_mouse's
        // Down(Left) branches in mouse.rs.
        if self.on_sidebar_divider(col, row) {
            return MousePointerShape::ColResize;
        }
        if self.on_sidebar_section_divider(col, row) {
            return MousePointerShape::RowResize;
        }
        if let Some(border) = self.find_border_at(col, row) {
            return match border.direction {
                // A horizontal split places children side-by-side; the
                // divider is a vertical line that the user drags horizontally.
                Direction::Horizontal => MousePointerShape::ColResize,
                // A vertical split stacks children top/bottom; the divider
                // is a horizontal line dragged vertically.
                Direction::Vertical => MousePointerShape::RowResize,
            };
        }
        if let Some((_, target)) = self.scrollbar_target_at(col, row) {
            return shape_for_scrollbar_target(&target);
        }
        if let Some(target) = self.workspace_list_scrollbar_target_at(col, row) {
            return shape_for_scrollbar_target(&target);
        }
        if let Some(target) = self.agent_panel_scrollbar_target_at(col, row) {
            return shape_for_scrollbar_target(&target);
        }
        if self.tab_at(col, row).is_some() {
            return MousePointerShape::Grab;
        }
        if self.on_tab_scroll_left_button(col, row)
            || self.on_tab_scroll_right_button(col, row)
            || self.on_new_tab_button(col, row)
        {
            return MousePointerShape::Pointer;
        }
        if self.workspace_at_row(row).is_some() {
            return MousePointerShape::Grab;
        }
        if self.on_collapsed_sidebar_toggle(col, row)
            || self.on_agent_panel_scope_toggle(col, row)
        {
            return MousePointerShape::Pointer;
        }
        if self.clickable_toast_at(col, row) {
            return MousePointerShape::Pointer;
        }
        // Global launcher (bottom-right corner button). Mirrors the gating
        // in mouse.rs `handle_mouse` lines 73-88.
        let launcher_enabled = !self.sidebar_collapsed
            && matches!(
                self.mode,
                Mode::Terminal | Mode::Navigate | Mode::Resize | Mode::GlobalMenu | Mode::KeybindHelp
            );
        if launcher_enabled && in_rect(self.global_launcher_rect(), col, row) {
            return MousePointerShape::Pointer;
        }

        MousePointerShape::Default
    }
}

fn shape_for_drag_target(target: &DragTarget) -> MousePointerShape {
    match target {
        DragTarget::PaneSplit { direction, .. } => match direction {
            Direction::Horizontal => MousePointerShape::ColResize,
            Direction::Vertical => MousePointerShape::RowResize,
        },
        DragTarget::SidebarDivider => MousePointerShape::ColResize,
        DragTarget::SidebarSectionDivider => MousePointerShape::RowResize,
        DragTarget::TabReorder { .. } | DragTarget::WorkspaceReorder { .. } => {
            MousePointerShape::Grabbing
        }
        DragTarget::WorkspaceListScrollbar { .. }
        | DragTarget::AgentPanelScrollbar { .. }
        | DragTarget::PaneScrollbar { .. }
        | DragTarget::ReleaseNotesScrollbar { .. }
        | DragTarget::ProductAnnouncementScrollbar { .. }
        | DragTarget::KeybindHelpScrollbar { .. } => MousePointerShape::Grabbing,
    }
}

fn shape_for_scrollbar_target(target: &ScrollbarClickTarget) -> MousePointerShape {
    match target {
        ScrollbarClickTarget::Thumb { .. } => MousePointerShape::Grab,
        ScrollbarClickTarget::Track { .. } => MousePointerShape::Pointer,
    }
}

fn in_rect(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{MouseButton, MouseEventKind};

    use super::super::{app_for_mouse_test, mouse};

    #[test]
    fn disabled_config_always_returns_default() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = false;
        // Mock a sidebar divider hit by setting last_mouse_pos to the divider edge.
        let divider_col = app.state.view.sidebar_rect.x + app.state.view.sidebar_rect.width - 1;
        app.state.recompute_mouse_pointer_shape(mouse(
            MouseEventKind::Moved,
            divider_col,
            5,
        ));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::Default
        );
    }

    #[test]
    fn enabled_returns_col_resize_over_sidebar_divider() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = true;
        let divider_col = app.state.view.sidebar_rect.x + app.state.view.sidebar_rect.width - 1;
        app.state.recompute_mouse_pointer_shape(mouse(
            MouseEventKind::Moved,
            divider_col,
            5,
        ));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::ColResize
        );
    }

    #[test]
    fn enabled_returns_default_in_pane_content() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = true;
        // Middle of terminal_area is plain pane content.
        let term = app.state.view.terminal_area;
        app.state.recompute_mouse_pointer_shape(mouse(
            MouseEventKind::Moved,
            term.x + term.width / 2,
            term.y + term.height / 2,
        ));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::Default
        );
    }

    #[test]
    fn drag_pane_split_horizontal_returns_col_resize() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = true;
        app.state.drag = Some(crate::app::state::DragState {
            target: DragTarget::PaneSplit {
                path: Vec::new(),
                direction: Direction::Horizontal,
                area: Rect::new(0, 0, 80, 20),
            },
        });
        app.state
            .recompute_mouse_pointer_shape(mouse(MouseEventKind::Drag(MouseButton::Left), 0, 0));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::ColResize
        );
    }

    #[test]
    fn drag_tab_reorder_returns_grabbing() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = true;
        app.state.drag = Some(crate::app::state::DragState {
            target: DragTarget::TabReorder {
                ws_idx: 0,
                source_tab_idx: 0,
                insert_idx: None,
            },
        });
        app.state
            .recompute_mouse_pointer_shape(mouse(MouseEventKind::Drag(MouseButton::Left), 5, 5));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::Grabbing
        );
    }

    #[test]
    fn mobile_layout_returns_default_even_when_enabled() {
        let mut app = app_for_mouse_test();
        app.state.mouse_pointer_shapes_enabled = true;
        app.state.view.layout = ViewLayout::Mobile;
        app.state
            .recompute_mouse_pointer_shape(mouse(MouseEventKind::Moved, 5, 5));
        assert_eq!(
            app.state.pending_mouse_pointer_shape,
            MousePointerShape::Default
        );
    }
}
