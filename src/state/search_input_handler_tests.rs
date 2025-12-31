//! Tests for search input handler.
//!
//! All tests written BEFORE implementation (TDD).
//! Tests verify runtime behavior of search input state transitions.

use super::*;

// ===== activate_search_input tests =====

#[test]
fn activate_from_inactive_creates_typing_state() {
    let state = SearchState::Inactive;
    let result = activate_search_input(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "", "Query should start empty");
            assert_eq!(cursor, 0, "Cursor should start at 0");
        }
        _ => panic!("Expected Typing state, got {:?}", result),
    }
}

#[test]
fn activate_from_typing_is_noop() {
    let state = SearchState::Typing {
        query: "existing".to_string(),
        cursor: 5,
    };
    let result = activate_search_input(state.clone());

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "existing", "Query should be unchanged");
            assert_eq!(cursor, 5, "Cursor should be unchanged");
        }
        _ => panic!("Expected Typing state, got {:?}", result),
    }
}

#[test]
fn activate_from_active_is_noop() {
    let query = crate::state::SearchQuery::new("test").unwrap();
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };
    let result = activate_search_input(state);

    match result {
        SearchState::Active { .. } => {
            // Should remain in Active state
        }
        _ => panic!("Expected Active state to remain unchanged"),
    }
}

// ===== cancel_search tests =====

#[test]
fn cancel_from_typing_returns_inactive() {
    let state = SearchState::Typing {
        query: "partial".to_string(),
        cursor: 3,
    };
    let result = cancel_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should transition to Inactive"
    );
}

#[test]
fn cancel_from_active_returns_inactive() {
    let query = crate::state::SearchQuery::new("test").unwrap();
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };
    let result = cancel_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should transition to Inactive"
    );
}

#[test]
fn cancel_from_inactive_is_noop() {
    let state = SearchState::Inactive;
    let result = cancel_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should remain Inactive"
    );
}

// ===== handle_char_input tests =====

#[test]
fn char_input_inserts_at_cursor_position() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2, // Between 'e' and 's'
    };
    let state = handle_char_input(state, 'X');

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "teXst", "Should insert 'X' at position 2");
            assert_eq!(cursor, 3, "Cursor should advance to 3");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn char_input_appends_when_cursor_at_end() {
    let state = SearchState::Typing {
        query: "hello".to_string(),
        cursor: 5,
    };
    let state = handle_char_input(state, '!');

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "hello!", "Should append '!'");
            assert_eq!(cursor, 6, "Cursor should advance to 6");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn char_input_inserts_at_beginning_when_cursor_zero() {
    let state = SearchState::Typing {
        query: "world".to_string(),
        cursor: 0,
    };
    let state = handle_char_input(state, 'H');

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "Hworld", "Should prepend 'H'");
            assert_eq!(cursor, 1, "Cursor should advance to 1");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn char_input_works_on_empty_query() {
    let state = SearchState::Typing {
        query: String::new(),
        cursor: 0,
    };
    let state = handle_char_input(state, 'a');

    match state {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "a", "Should create query with 'a'");
            assert_eq!(cursor, 1, "Cursor should advance to 1");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn char_input_noop_when_inactive() {
    let state = SearchState::Inactive;
    let state = handle_char_input(state, 'x');

    assert!(
        matches!(state, SearchState::Inactive),
        "Should remain Inactive when not in Typing state"
    );
}

#[test]
fn char_input_noop_when_active() {
    let query = crate::state::SearchQuery::new("test").unwrap();
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };
    let state = handle_char_input(state, 'x');

    assert!(
        matches!(state, SearchState::Active { .. }),
        "Should remain Active when not in Typing state"
    );
}

// ===== handle_backspace tests =====

#[test]
fn backspace_deletes_char_before_cursor() {
    let state = SearchState::Typing {
        query: "hello".to_string(),
        cursor: 3, // After 'l'
    };
    let result = handle_backspace(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "helo", "Should delete 'l' at position 2");
            assert_eq!(cursor, 2, "Cursor should move back to 2");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn backspace_at_end_deletes_last_char() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 4,
    };
    let result = handle_backspace(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "tes", "Should delete last 't'");
            assert_eq!(cursor, 3, "Cursor should move to 3");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn backspace_at_start_is_noop() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 0,
    };
    let result = handle_backspace(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 0, "Cursor should remain at 0");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn backspace_on_empty_query_is_noop() {
    let state = SearchState::Typing {
        query: String::new(),
        cursor: 0,
    };
    let result = handle_backspace(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "", "Query should remain empty");
            assert_eq!(cursor, 0, "Cursor should remain at 0");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn backspace_noop_when_inactive() {
    let state = SearchState::Inactive;
    let result = handle_backspace(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should remain Inactive"
    );
}

#[test]
fn backspace_noop_when_active() {
    let query = crate::state::SearchQuery::new("test").unwrap();
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };
    let result = handle_backspace(state);

    assert!(
        matches!(result, SearchState::Active { .. }),
        "Should remain Active"
    );
}

// ===== handle_cursor_left tests =====

#[test]
fn cursor_left_moves_back_one_position() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 3,
    };
    let result = handle_cursor_left(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 2, "Cursor should move to 2");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn cursor_left_saturates_at_zero() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 0,
    };
    let result = handle_cursor_left(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 0, "Cursor should remain at 0");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn cursor_left_noop_when_inactive() {
    let state = SearchState::Inactive;
    let result = handle_cursor_left(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should remain Inactive"
    );
}

// ===== handle_cursor_right tests =====

#[test]
fn cursor_right_moves_forward_one_position() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 2,
    };
    let result = handle_cursor_right(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 3, "Cursor should move to 3");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn cursor_right_saturates_at_query_length() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 4,
    };
    let result = handle_cursor_right(state);

    match result {
        SearchState::Typing { query, cursor } => {
            assert_eq!(query, "test", "Query should be unchanged");
            assert_eq!(cursor, 4, "Cursor should remain at 4");
        }
        _ => panic!("Expected Typing state"),
    }
}

#[test]
fn cursor_right_noop_when_inactive() {
    let state = SearchState::Inactive;
    let result = handle_cursor_right(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should remain Inactive"
    );
}

// ===== submit_search tests =====

#[test]
fn submit_with_empty_query_returns_inactive() {
    let state = SearchState::Typing {
        query: String::new(),
        cursor: 0,
    };
    let result = submit_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Empty query should transition to Inactive"
    );
}

#[test]
fn submit_with_whitespace_only_query_returns_inactive() {
    let state = SearchState::Typing {
        query: "   ".to_string(),
        cursor: 3,
    };
    let result = submit_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Whitespace-only query should transition to Inactive"
    );
}

#[test]
fn submit_with_valid_query_returns_active() {
    let state = SearchState::Typing {
        query: "test".to_string(),
        cursor: 4,
    };
    let result = submit_search(state);

    match result {
        SearchState::Active { query, .. } => {
            assert_eq!(query.as_str(), "test", "Query should be preserved");
        }
        _ => panic!("Expected Active state with query 'test'"),
    }
}

#[test]
fn submit_preserves_query_with_leading_trailing_spaces() {
    let state = SearchState::Typing {
        query: " query ".to_string(),
        cursor: 7,
    };
    let result = submit_search(state);

    match result {
        SearchState::Active { query, .. } => {
            assert_eq!(
                query.as_str(),
                " query ",
                "Query with spaces should be preserved"
            );
        }
        _ => panic!("Expected Active state"),
    }
}

#[test]
fn submit_noop_when_inactive() {
    let state = SearchState::Inactive;
    let result = submit_search(state);

    assert!(
        matches!(result, SearchState::Inactive),
        "Should remain Inactive"
    );
}

#[test]
fn submit_noop_when_already_active() {
    let query = crate::state::SearchQuery::new("existing").unwrap();
    let state = SearchState::Active {
        query,
        matches: vec![],
        current_match: 0,
    };
    let result = submit_search(state);

    match result {
        SearchState::Active { query, .. } => {
            assert_eq!(query.as_str(), "existing", "Should preserve existing query");
        }
        _ => panic!("Expected Active state"),
    }
}
