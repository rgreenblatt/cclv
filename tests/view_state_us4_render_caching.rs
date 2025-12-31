//! Acceptance tests for User Story 4: Render Caching
//!
//! Tests the 3 acceptance scenarios from spec.md lines 82-85:
//! 1. Cache hit on revisit: Previously viewed entries render instantly from cache
//! 2. Cache invalidation on resize: Viewport resize invalidates cache
//! 3. LRU eviction under memory pressure: Old entries evicted when capacity reached

use cclv::model::EntryUuid;
use cclv::state::WrapMode;
use cclv::view_state::cache::{CachedRender, RenderCache, RenderCacheKey};
use ratatui::text::Line;

// ===== Test Helpers =====

/// Create a test EntryUuid.
fn test_uuid(s: &str) -> EntryUuid {
    EntryUuid::new(s).expect("Valid test UUID")
}

/// Create a test CachedRender with given line count.
fn test_render(line_count: usize) -> CachedRender {
    let lines = (0..line_count)
        .map(|i| Line::from(format!("Line {}", i)))
        .collect();
    CachedRender { lines }
}

// ===== US4 Scenario 1: Cache Hit on Revisit =====

#[test]
fn us4_scenario1_cache_hit_on_revisit() {
    // GIVEN: User has viewed entries with code blocks
    // WHEN: User scrolls away and back
    // THEN: Cached content renders instantly (no re-highlighting)

    // DOING: Put entry in cache, verify cache hit on revisit
    // EXPECT: cache.get() returns Some after initial cache.put()

    let mut cache = RenderCache::new(100);
    let uuid = test_uuid("entry-with-code-block");
    let key = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::Wrap);

    // GIVEN: Entry was previously rendered (syntax highlighted)
    let rendered_lines = test_render(25); // 25 lines of syntax-highlighted code
    cache.put(key.clone(), rendered_lines.clone());

    // VERIFY: Cache now contains 1 entry
    assert_eq!(cache.len(), 1, "Cache should contain the rendered entry");

    // WHEN: User scrolls away (simulated by cache not being accessed)
    // ... time passes ...

    // WHEN: User scrolls back to the same entry with same viewport
    let retrieved = cache.get(&key);

    // THEN: Cache hit - content retrieved instantly
    assert!(
        retrieved.is_some(),
        "Cache should return the previously rendered entry (cache hit)"
    );

    // THEN: Retrieved content matches original (no re-highlighting needed)
    let cached_lines = &retrieved.unwrap().lines;
    assert_eq!(
        cached_lines.len(),
        25,
        "Cached render should have same line count as original"
    );

    // RESULT: Cache hit on revisit
    // MATCHES: Yes - cache.get() returned Some with matching content
    // THEREFORE: US4 Scenario 1 verified
}

#[test]
fn us4_scenario1_cache_miss_on_first_view() {
    // GIVEN: User views an entry for the first time
    // THEN: Cache miss (content must be rendered)

    // DOING: Query cache for never-seen entry
    // EXPECT: cache.get() returns None

    let mut cache = RenderCache::new(100);
    let uuid = test_uuid("never-seen-entry");
    let key = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::Wrap);

    // WHEN: User views entry for first time
    let result = cache.get(&key);

    // THEN: Cache miss
    assert!(
        result.is_none(),
        "Cache should return None for never-seen entry (cache miss)"
    );

    // RESULT: Cache miss on first view
    // MATCHES: Yes - cache.get() returned None as expected
    // THEREFORE: First-view cache miss verified
}

// ===== US4 Scenario 2: Cache Invalidation on Resize =====

#[test]
fn us4_scenario2_cache_invalidation_on_resize() {
    // GIVEN: Viewport resizes
    // WHEN: Cached content is shown
    // THEN: Cache invalidates and content re-renders for new width

    // DOING: Cache entry at width 80, verify miss at width 100
    // EXPECT: Different widths produce different cache keys (miss on resize)

    let mut cache = RenderCache::new(100);
    let uuid = test_uuid("entry-to-resize");

    // GIVEN: Entry rendered at width 80
    let key_width_80 = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::Wrap);
    cache.put(key_width_80.clone(), test_render(20));

    // VERIFY: Cache hit at original width
    assert!(
        cache.get(&key_width_80).is_some(),
        "Cache should hit at original width 80"
    );

    // WHEN: Viewport resizes to width 100
    let key_width_100 = RenderCacheKey::new(uuid.clone(), 100, true, WrapMode::Wrap);

    // THEN: Cache miss (different width = different key)
    let result_after_resize = cache.get(&key_width_100);
    assert!(
        result_after_resize.is_none(),
        "Cache should miss after viewport resize (width 80 -> 100)"
    );

    // WHEN: Content re-renders at new width and gets cached
    cache.put(key_width_100.clone(), test_render(18)); // Different line count due to width

    // THEN: Both widths now have cached entries
    assert!(
        cache.get(&key_width_80).is_some(),
        "Original width 80 should still be cached"
    );
    assert!(
        cache.get(&key_width_100).is_some(),
        "New width 100 should now be cached"
    );
    assert_eq!(cache.len(), 2, "Cache should have 2 entries (one per width)");

    // RESULT: Cache invalidation on resize
    // MATCHES: Yes - different widths produce different keys, forcing re-render
    // THEREFORE: US4 Scenario 2 verified
}

#[test]
fn us4_scenario2_cache_invalidation_on_expand_state_change() {
    // Expanded/collapsed state affects rendering (different heights)
    // GIVEN: Entry cached in collapsed state
    // WHEN: User expands the entry
    // THEN: Cache miss (must re-render at expanded height)

    // DOING: Cache collapsed entry, verify miss when expanded
    // EXPECT: Different expanded states produce different keys

    let mut cache = RenderCache::new(100);
    let uuid = test_uuid("entry-to-expand");

    // GIVEN: Entry cached in collapsed state (expanded=false)
    let key_collapsed = RenderCacheKey::new(uuid.clone(), 80, false, WrapMode::Wrap);
    cache.put(key_collapsed.clone(), test_render(3)); // Collapsed: 3 lines

    // VERIFY: Cache hit for collapsed
    assert!(
        cache.get(&key_collapsed).is_some(),
        "Cache should hit for collapsed entry"
    );

    // WHEN: Entry is expanded
    let key_expanded = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::Wrap);

    // THEN: Cache miss (different expanded state)
    assert!(
        cache.get(&key_expanded).is_none(),
        "Cache should miss when entry is expanded (expanded state changed)"
    );

    // WHEN: Content re-renders in expanded state
    cache.put(key_expanded.clone(), test_render(50)); // Expanded: 50 lines

    // THEN: Both states cached
    assert_eq!(
        cache.len(),
        2,
        "Cache should have entries for both collapsed and expanded states"
    );

    // RESULT: Cache invalidation on expand/collapse
    // MATCHES: Yes - expanded state affects cache key
    // THEREFORE: Expand state invalidation verified
}

#[test]
fn us4_scenario2_cache_invalidation_on_wrap_mode_change() {
    // Wrap mode affects line breaking and rendering
    // GIVEN: Entry cached with WrapMode::Wrap
    // WHEN: Wrap mode changes to WrapMode::NoWrap
    // THEN: Cache miss (must re-render with new wrap behavior)

    // DOING: Cache entry with Wrap mode, verify miss with NoWrap
    // EXPECT: Different wrap modes produce different keys

    let mut cache = RenderCache::new(100);
    let uuid = test_uuid("entry-with-long-lines");

    // GIVEN: Entry cached with wrap mode
    let key_wrap = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::Wrap);
    cache.put(key_wrap.clone(), test_render(30)); // Wrapped: 30 lines

    // VERIFY: Cache hit with wrap
    assert!(
        cache.get(&key_wrap).is_some(),
        "Cache should hit for wrapped entry"
    );

    // WHEN: Wrap mode changes to NoWrap
    let key_nowrap = RenderCacheKey::new(uuid.clone(), 80, true, WrapMode::NoWrap);

    // THEN: Cache miss (different wrap mode)
    assert!(
        cache.get(&key_nowrap).is_none(),
        "Cache should miss when wrap mode changes (Wrap -> NoWrap)"
    );

    // WHEN: Content re-renders with NoWrap
    cache.put(key_nowrap.clone(), test_render(10)); // NoWrap: fewer lines

    // THEN: Both wrap modes cached
    assert_eq!(
        cache.len(),
        2,
        "Cache should have entries for both wrap modes"
    );

    // RESULT: Cache invalidation on wrap mode change
    // MATCHES: Yes - wrap mode affects cache key
    // THEREFORE: Wrap mode invalidation verified
}

// ===== US4 Scenario 3: LRU Eviction Under Memory Pressure =====

#[test]
fn us4_scenario3_lru_eviction_under_memory_pressure() {
    // GIVEN: System has limited memory (capacity limit)
    // WHEN: Many entries are viewed
    // THEN: Old cache entries are evicted (LRU) without affecting functionality

    // DOING: Fill cache to capacity, add one more, verify LRU eviction
    // EXPECT: Oldest entry evicted when capacity exceeded

    // Create cache with capacity of 3 entries
    let mut cache = RenderCache::new(3);

    let uuid1 = test_uuid("entry-1");
    let uuid2 = test_uuid("entry-2");
    let uuid3 = test_uuid("entry-3");
    let uuid4 = test_uuid("entry-4");

    let key1 = RenderCacheKey::new(uuid1.clone(), 80, true, WrapMode::Wrap);
    let key2 = RenderCacheKey::new(uuid2.clone(), 80, true, WrapMode::Wrap);
    let key3 = RenderCacheKey::new(uuid3.clone(), 80, true, WrapMode::Wrap);
    let key4 = RenderCacheKey::new(uuid4.clone(), 80, true, WrapMode::Wrap);

    // GIVEN: User views 3 entries (fills cache to capacity)
    cache.put(key1.clone(), test_render(10));
    cache.put(key2.clone(), test_render(15));
    cache.put(key3.clone(), test_render(20));

    // VERIFY: Cache is at capacity
    assert_eq!(cache.len(), 3, "Cache should be at capacity (3 entries)");

    // WHEN: User views a 4th entry (memory pressure - exceeds capacity)
    cache.put(key4.clone(), test_render(25));

    // THEN: Cache still at capacity (LRU eviction occurred)
    assert_eq!(
        cache.len(),
        3,
        "Cache should remain at capacity after eviction"
    );

    // THEN: Oldest entry (key1) was evicted, others remain
    assert!(cache.get(&key1).is_none(), "Key1 (oldest) should be evicted");
    assert!(cache.get(&key2).is_some(), "Key2 should remain");
    assert!(cache.get(&key3).is_some(), "Key3 should remain");
    assert!(cache.get(&key4).is_some(), "Key4 (newest) should be added");

    // RESULT: LRU eviction under memory pressure
    // MATCHES: Yes - cache stayed at capacity, old entry evicted
    // THEREFORE: US4 Scenario 3 verified
}

#[test]
fn us4_scenario3_lru_eviction_preserves_functionality() {
    // Verify that evicted entries can be re-rendered without data loss
    // GIVEN: Entry is evicted from cache
    // WHEN: User scrolls back to that entry
    // THEN: Entry can be re-rendered (no data loss)

    // DOING: Fill cache, evict entry, verify it can be re-added
    // EXPECT: Evicted entry can be put() again without error

    let mut cache = RenderCache::new(2); // Small capacity

    let uuid1 = test_uuid("evicted-entry");
    let uuid2 = test_uuid("other-entry-1");
    let uuid3 = test_uuid("other-entry-2");

    let key1 = RenderCacheKey::new(uuid1.clone(), 80, true, WrapMode::Wrap);
    let key2 = RenderCacheKey::new(uuid2.clone(), 80, true, WrapMode::Wrap);
    let key3 = RenderCacheKey::new(uuid3.clone(), 80, true, WrapMode::Wrap);

    // Add entries to fill cache
    cache.put(key1.clone(), test_render(10));
    cache.put(key2.clone(), test_render(15));

    // WHEN: Add third entry, causing eviction of key1 (oldest)
    cache.put(key3.clone(), test_render(20));

    // Entry 1 might be evicted (depends on LRU order)
    // Let's force the scenario by adding more entries
    cache.put(
        RenderCacheKey::new(test_uuid("temp-1"), 80, true, WrapMode::Wrap),
        test_render(5),
    );

    // VERIFY: Entry 1 might be evicted (cache behavior is LRU-based)
    // Note: Can't guarantee key1 specifically was evicted, but capacity is maintained

    // WHEN: User scrolls back to evicted entry (re-render occurs)
    cache.put(key1.clone(), test_render(10));

    // THEN: Entry can be re-cached without error
    assert!(
        cache.get(&key1).is_some(),
        "Evicted entry can be re-rendered and re-cached"
    );

    // THEN: Cache capacity still maintained
    assert_eq!(
        cache.len(),
        2,
        "Cache should maintain capacity limit after re-caching"
    );

    // RESULT: Eviction does not affect functionality
    // MATCHES: Yes - evicted entries can be re-rendered
    // THEREFORE: Eviction preserves functionality verified
}

#[test]
fn us4_scenario3_get_updates_lru_ordering() {
    // Verify that cache.get() updates LRU ordering (keeps accessed entries alive)
    // GIVEN: Cache with 3 entries at capacity
    // WHEN: Oldest entry is accessed via get()
    // THEN: That entry becomes most recent and is not evicted next

    // DOING: Fill cache, access oldest, add new entry, verify oldest not evicted
    // EXPECT: Accessed entry survives next eviction

    let mut cache = RenderCache::new(3);

    let uuid1 = test_uuid("oldest-entry");
    let uuid2 = test_uuid("middle-entry");
    let uuid3 = test_uuid("newest-entry");
    let uuid4 = test_uuid("trigger-eviction");

    let key1 = RenderCacheKey::new(uuid1.clone(), 80, true, WrapMode::Wrap);
    let key2 = RenderCacheKey::new(uuid2.clone(), 80, true, WrapMode::Wrap);
    let key3 = RenderCacheKey::new(uuid3.clone(), 80, true, WrapMode::Wrap);
    let key4 = RenderCacheKey::new(uuid4.clone(), 80, true, WrapMode::Wrap);

    // GIVEN: Add 3 entries in order (1 is oldest)
    cache.put(key1.clone(), test_render(10));
    cache.put(key2.clone(), test_render(15));
    cache.put(key3.clone(), test_render(20));

    // WHEN: Access the oldest entry (key1) - moves it to most recent
    let _ = cache.get(&key1);

    // WHEN: Add 4th entry (should evict key2, the new least-recently-used)
    cache.put(key4.clone(), test_render(25));

    // THEN: key1 should still be cached (it was accessed)
    assert!(
        cache.get(&key1).is_some(),
        "Accessed entry should survive eviction (LRU updated by get)"
    );

    // THEN: key4 should be cached (newly added)
    assert!(cache.get(&key4).is_some(), "New entry should be cached");

    // THEN: key2 should be evicted (was least recently used after key1 access)
    assert!(
        cache.get(&key2).is_none(),
        "Least recently used entry should be evicted"
    );

    // RESULT: get() updates LRU ordering
    // MATCHES: Yes - accessed entry survived eviction
    // THEREFORE: LRU ordering update verified
}
