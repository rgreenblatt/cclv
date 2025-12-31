//! Tests for AppState and ScrollState.
//!
//! These tests verify pure state transitions without any TUI dependencies.

use super::*;
use crate::model::{EntryUuid, SessionId};

// ===== Test Helpers =====

fn make_session_id(s: &str) -> SessionId {
    SessionId::new(s).expect("valid session id")
}

fn make_entry_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("valid uuid")
}

fn make_test_session() -> Session {
    Session::new(make_session_id("test-session"))
}

// ===== AppState::new Tests =====

#[test]
fn app_state_new_sets_session() {
    let session = make_test_session();
    let session_id = session.session_id().clone();
    let state = AppState::new(session);

    assert_eq!(state.session().session_id(), &session_id);
}

#[test]
fn app_state_new_defaults_focus_to_main() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert_eq!(state.focus, FocusPane::Main);
}

#[test]
fn app_state_new_initializes_scroll_states_to_default() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert_eq!(state.main_scroll.vertical_offset, 0);
    assert_eq!(state.main_scroll.horizontal_offset, 0);
    assert_eq!(state.subagent_scroll.vertical_offset, 0);
    assert_eq!(state.subagent_scroll.horizontal_offset, 0);
}

#[test]
fn app_state_new_defaults_selected_tab_to_none() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert_eq!(state.selected_tab, None);
}

#[test]
fn app_state_new_defaults_search_to_inactive() {
    let session = make_test_session();
    let state = AppState::new(session);

    matches!(state.search, SearchState::Inactive);
}

#[test]
fn app_state_new_defaults_stats_filter_to_global() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert_eq!(state.stats_filter, StatsFilter::Global);
}

#[test]
fn app_state_new_defaults_stats_visible_to_false() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert!(!state.stats_visible);
}

#[test]
fn app_state_new_defaults_live_mode_to_false() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert!(!state.live_mode);
}

#[test]
fn app_state_new_defaults_auto_scroll_to_true() {
    let session = make_test_session();
    let state = AppState::new(session);

    assert!(state.auto_scroll);
}

// ===== ScrollState::scroll_up Tests =====

#[test]
fn scroll_up_decreases_vertical_offset() {
    let mut scroll = ScrollState {
        vertical_offset: 10,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_up(3);

    assert_eq!(scroll.vertical_offset, 7);
}

#[test]
fn scroll_up_saturates_at_zero() {
    let mut scroll = ScrollState {
        vertical_offset: 2,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_up(5);

    assert_eq!(scroll.vertical_offset, 0);
}

#[test]
fn scroll_up_from_zero_stays_zero() {
    let mut scroll = ScrollState::default();

    scroll.scroll_up(1);

    assert_eq!(scroll.vertical_offset, 0);
}

// ===== ScrollState::scroll_down Tests =====

#[test]
fn scroll_down_increases_vertical_offset() {
    let mut scroll = ScrollState::default();

    scroll.scroll_down(5, 100);

    assert_eq!(scroll.vertical_offset, 5);
}

#[test]
fn scroll_down_respects_max_bound() {
    let mut scroll = ScrollState {
        vertical_offset: 95,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_down(10, 100);

    assert_eq!(scroll.vertical_offset, 100);
}

#[test]
fn scroll_down_clamps_to_max() {
    let mut scroll = ScrollState::default();

    scroll.scroll_down(150, 100);

    assert_eq!(scroll.vertical_offset, 100);
}

// ===== ScrollState::scroll_left Tests =====

#[test]
fn scroll_left_decreases_horizontal_offset() {
    let mut scroll = ScrollState {
        vertical_offset: 0,
        horizontal_offset: 20,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_left(5);

    assert_eq!(scroll.horizontal_offset, 15);
}

#[test]
fn scroll_left_saturates_at_zero() {
    let mut scroll = ScrollState {
        vertical_offset: 0,
        horizontal_offset: 3,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_left(10);

    assert_eq!(scroll.horizontal_offset, 0);
}

// ===== ScrollState::scroll_right Tests =====

#[test]
fn scroll_right_increases_horizontal_offset() {
    let mut scroll = ScrollState::default();

    scroll.scroll_right(7);

    assert_eq!(scroll.horizontal_offset, 7);
}

#[test]
fn scroll_right_accumulates() {
    let mut scroll = ScrollState::default();

    scroll.scroll_right(5);
    scroll.scroll_right(3);

    assert_eq!(scroll.horizontal_offset, 8);
}

// ===== ScrollState::toggle_expand Tests =====

#[test]
fn toggle_expand_adds_uuid_when_not_present() {
    let mut scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");

    scroll.toggle_expand(&uuid);

    assert!(scroll.expanded_messages.contains(&uuid));
}

#[test]
fn toggle_expand_removes_uuid_when_present() {
    let mut scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");
    scroll.expanded_messages.insert(uuid.clone());

    scroll.toggle_expand(&uuid);

    assert!(!scroll.expanded_messages.contains(&uuid));
}

#[test]
fn toggle_expand_twice_returns_to_original_state() {
    let mut scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");

    scroll.toggle_expand(&uuid);
    scroll.toggle_expand(&uuid);

    assert!(!scroll.expanded_messages.contains(&uuid));
}

// ===== ScrollState::is_expanded Tests =====

#[test]
fn is_expanded_returns_false_when_not_in_set() {
    let scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");

    assert!(!scroll.is_expanded(&uuid));
}

#[test]
fn is_expanded_returns_true_when_in_set() {
    let mut scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");
    scroll.expanded_messages.insert(uuid.clone());

    assert!(scroll.is_expanded(&uuid));
}

#[test]
fn is_expanded_after_toggle() {
    let mut scroll = ScrollState::default();
    let uuid = make_entry_uuid("msg-1");

    scroll.toggle_expand(&uuid);

    assert!(scroll.is_expanded(&uuid));
}

// ===== FocusPane Tests =====

#[test]
fn focus_pane_variants_are_distinct() {
    assert_ne!(FocusPane::Main, FocusPane::Subagent);
    assert_ne!(FocusPane::Main, FocusPane::Stats);
    assert_ne!(FocusPane::Main, FocusPane::Search);
    assert_ne!(FocusPane::Subagent, FocusPane::Stats);
    assert_ne!(FocusPane::Subagent, FocusPane::Search);
    assert_ne!(FocusPane::Stats, FocusPane::Search);
}

#[test]
fn focus_pane_equality() {
    assert_eq!(FocusPane::Main, FocusPane::Main);
    assert_eq!(FocusPane::Subagent, FocusPane::Subagent);
    assert_eq!(FocusPane::Stats, FocusPane::Stats);
    assert_eq!(FocusPane::Search, FocusPane::Search);
}

// ===== ScrollState::at_bottom Tests =====

#[test]
fn at_bottom_returns_true_when_at_max() {
    let scroll = ScrollState {
        vertical_offset: 100,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    assert!(scroll.at_bottom(100));
}

#[test]
fn at_bottom_returns_false_when_below_max() {
    let scroll = ScrollState {
        vertical_offset: 50,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    assert!(!scroll.at_bottom(100));
}

#[test]
fn at_bottom_returns_true_when_zero_and_max_is_zero() {
    let scroll = ScrollState::default();

    assert!(scroll.at_bottom(0));
}

#[test]
fn at_bottom_returns_false_when_one_below_max() {
    let scroll = ScrollState {
        vertical_offset: 99,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    assert!(!scroll.at_bottom(100));
}

// ===== ScrollState::scroll_to_bottom Tests =====

#[test]
fn scroll_to_bottom_sets_offset_to_max() {
    let mut scroll = ScrollState::default();

    scroll.scroll_to_bottom(100);

    assert_eq!(scroll.vertical_offset, 100);
}

#[test]
fn scroll_to_bottom_from_middle_position() {
    let mut scroll = ScrollState {
        vertical_offset: 50,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_to_bottom(100);

    assert_eq!(scroll.vertical_offset, 100);
}

#[test]
fn scroll_to_bottom_when_already_at_bottom() {
    let mut scroll = ScrollState {
        vertical_offset: 100,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_to_bottom(100);

    assert_eq!(scroll.vertical_offset, 100);
}

#[test]
fn scroll_to_bottom_with_zero_max() {
    let mut scroll = ScrollState {
        vertical_offset: 0,
        horizontal_offset: 0,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_to_bottom(0);

    assert_eq!(scroll.vertical_offset, 0);
}

#[test]
fn scroll_to_bottom_does_not_affect_horizontal_offset() {
    let mut scroll = ScrollState {
        vertical_offset: 10,
        horizontal_offset: 25,
        expanded_messages: HashSet::new(),
    };

    scroll.scroll_to_bottom(100);

    assert_eq!(scroll.horizontal_offset, 25);
}

#[test]
fn scroll_to_bottom_does_not_affect_expanded_messages() {
    let uuid = make_entry_uuid("msg-1");
    let mut scroll = ScrollState {
        vertical_offset: 10,
        horizontal_offset: 0,
        expanded_messages: {
            let mut set = HashSet::new();
            set.insert(uuid.clone());
            set
        },
    };

    scroll.scroll_to_bottom(100);

    assert!(scroll.expanded_messages.contains(&uuid));
}

// ===== AppState::has_new_messages_indicator Tests =====

#[test]
fn has_new_messages_indicator_returns_true_when_live_mode_and_auto_scroll_paused() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.live_mode = true;
    state.auto_scroll = false;

    assert!(state.has_new_messages_indicator());
}

#[test]
fn has_new_messages_indicator_returns_false_when_not_live_mode() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.live_mode = false;
    state.auto_scroll = false;

    assert!(!state.has_new_messages_indicator());
}

#[test]
fn has_new_messages_indicator_returns_false_when_auto_scroll_active() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.live_mode = true;
    state.auto_scroll = true;

    assert!(!state.has_new_messages_indicator());
}

#[test]
fn has_new_messages_indicator_returns_false_when_neither_live_nor_paused() {
    let session = make_test_session();
    let state = AppState::new(session);
    // Defaults: live_mode = false, auto_scroll = true

    assert!(!state.has_new_messages_indicator());
}

// ===== AppState::cycle_focus Tests =====

#[test]
fn cycle_focus_moves_from_main_to_subagent() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Main;

    state.cycle_focus();

    assert_eq!(state.focus, FocusPane::Subagent);
}

#[test]
fn cycle_focus_moves_from_subagent_to_stats() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Subagent;

    state.cycle_focus();

    assert_eq!(state.focus, FocusPane::Stats);
}

#[test]
fn cycle_focus_moves_from_stats_to_main() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Stats;

    state.cycle_focus();

    assert_eq!(state.focus, FocusPane::Main);
}

#[test]
fn cycle_focus_skips_search_pane() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Search;

    state.cycle_focus();

    // Search should cycle to Main (not stay on Search)
    assert_eq!(state.focus, FocusPane::Main);
}

#[test]
fn cycle_focus_full_cycle_returns_to_start() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Main;

    state.cycle_focus(); // Main -> Subagent
    state.cycle_focus(); // Subagent -> Stats
    state.cycle_focus(); // Stats -> Main

    assert_eq!(state.focus, FocusPane::Main);
}

// ===== AppState::focus_main Tests =====

#[test]
fn focus_main_sets_focus_to_main() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Subagent;

    state.focus_main();

    assert_eq!(state.focus, FocusPane::Main);
}

#[test]
fn focus_main_when_already_on_main() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Main;

    state.focus_main();

    assert_eq!(state.focus, FocusPane::Main);
}

// ===== AppState::focus_subagent Tests =====

#[test]
fn focus_subagent_sets_focus_to_subagent() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Main;

    state.focus_subagent();

    assert_eq!(state.focus, FocusPane::Subagent);
}

#[test]
fn focus_subagent_when_already_on_subagent() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Subagent;

    state.focus_subagent();

    assert_eq!(state.focus, FocusPane::Subagent);
}

// ===== AppState::focus_stats Tests =====

#[test]
fn focus_stats_sets_focus_to_stats() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Main;

    state.focus_stats();

    assert_eq!(state.focus, FocusPane::Stats);
}

#[test]
fn focus_stats_when_already_on_stats() {
    let session = make_test_session();
    let mut state = AppState::new(session);
    state.focus = FocusPane::Stats;

    state.focus_stats();

    assert_eq!(state.focus, FocusPane::Stats);
}
