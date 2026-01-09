//! UI state machine (pure).
//!
//! All state transitions are pure functions testable without TUI.

pub mod app_state;
pub mod expand_handler;
pub mod match_navigation_handler;
pub mod mouse_handler;
pub mod scroll_handler;
pub mod search;
pub mod search_input_handler;
pub mod session_modal;
pub mod session_modal_handler;
pub mod tab_handler;
pub mod viewed_session;
pub mod wrap_handler;

// Re-export for convenience
pub use app_state::{AppState, ConversationSelection, FocusPane, InputMode, WrapContext, WrapMode};
pub use expand_handler::handle_expand_action;
pub use match_navigation_handler::{next_match, prev_match};
pub use mouse_handler::{
    EntryClickResult, TabClickResult, detect_entry_click, detect_tab_click, handle_entry_click,
    handle_mouse_click, handle_mouse_scroll,
};
pub use scroll_handler::handle_scroll_action;
pub use search::{SearchMatch, SearchQuery, SearchState, agent_ids_with_matches, execute_search};
pub use search_input_handler::{
    activate_search_input, cancel_search, handle_backspace, handle_char_input, handle_cursor_left,
    handle_cursor_right, submit_search,
};
pub use session_modal::SessionModalState;
pub use session_modal_handler::handle_session_modal_key;
pub use tab_handler::handle_tab_action;
pub use viewed_session::ViewedSession;
pub use wrap_handler::handle_toggle_wrap;
