use op_task::rank;

fn key(lower: Option<&str>, upper: Option<&str>) -> String {
    rank::between(lower, upper).expect("bounds straddle a gap")
}

#[test]
fn between_open_ends_is_a_valid_key() {
    assert!(rank::is_valid(&key(None, None)));
}

#[test]
fn between_lands_strictly_between_two_keys() {
    let mid = key(Some("a"), Some("b"));
    assert!("a" < mid.as_str(), "a < {mid}");
    assert!(mid.as_str() < "b", "{mid} < b");
}

#[test]
fn between_after_appends_above() {
    let a = key(None, None);
    let b = key(Some(&a), None);
    assert!(a < b, "{a} < {b}");
}

#[test]
fn between_before_inserts_below() {
    let b = key(None, None);
    let a = key(None, Some(&b));
    assert!(a < b, "{a} < {b}");
}

#[test]
fn repeated_before_inserts_do_not_collapse() {
    // Each insert takes the midpoint of the previous lowest and the open start; the keys must stay
    // distinct and ordered no matter how many times we prepend (no f64 precision collapse).
    let mut lowest = key(None, None);
    let mut seen = vec![lowest.clone()];
    for _ in 0..200 {
        let next = key(None, Some(&lowest));
        assert!(next < lowest, "prepend must sort before: {next} < {lowest}");
        assert!(!seen.contains(&next), "keys must stay distinct: {next}");
        seen.push(next.clone());
        lowest = next;
    }
}

#[test]
fn keys_sort_in_insertion_order_for_a_growing_list() {
    let mut keys = Vec::new();
    let mut prev: Option<String> = None;
    for _ in 0..50 {
        let next = key(prev.as_deref(), None);
        prev = Some(next.clone());
        keys.push(next);
    }
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "append order must equal sorted order");
}

#[test]
fn between_refuses_malformed_keys_instead_of_panicking() {
    // Ranks reach the store from hand-edited frontmatter, so `between` must treat anything outside
    // base-36 as "no key available" rather than indexing a digit table that has no such digit.
    for bad in ["A", "1.5", "", "a-b", "é", " a"] {
        assert_eq!(rank::between(Some(bad), None), None, "lower {bad:?}");
        assert_eq!(rank::between(None, Some(bad)), None, "upper {bad:?}");
    }
}

#[test]
fn between_refuses_bounds_naming_the_same_point() {
    // Trailing zeros do not change a key's value: `a`, `a0` and `a00` are one point with no room
    // between them. Descending for a gap that cannot exist would never terminate.
    for (lower, upper) in [
        ("a", "a0"),
        ("a0", "a"),
        ("a", "a00"),
        ("a", "a"),
        ("0", "00"),
    ] {
        assert_eq!(
            rank::between(Some(lower), Some(upper)),
            None,
            "{lower} .. {upper}"
        );
    }
}

#[test]
fn between_refuses_reversed_bounds() {
    assert_eq!(rank::between(Some("b"), Some("a")), None);
    assert_eq!(rank::between(Some("zz"), Some("a")), None);
}

#[test]
fn between_splits_neighbours_with_no_digit_gap() {
    // `a` and `b` are adjacent digits, so the key has to descend a level rather than pick a midpoint.
    let mid = key(Some("a"), Some("b"));
    assert!(mid.len() > 1, "{mid} must descend below b");
    let deeper = key(Some("a"), Some(&mid));
    assert!("a" < deeper.as_str() && deeper < mid, "{deeper}");
}

#[test]
fn is_ordered_accepts_a_clean_group_and_rejects_the_rest() {
    assert!(rank::is_ordered(&[]));
    assert!(rank::is_ordered(&["5"]));
    assert!(rank::is_ordered(&["5", "a", "az", "b"]));

    assert!(!rank::is_ordered(&["a", "a"]), "colliding");
    assert!(!rank::is_ordered(&["a", "a0"]), "same point");
    assert!(!rank::is_ordered(&["b", "a"]), "out of order");
    assert!(!rank::is_ordered(&["a", "A"]), "malformed");
    assert!(!rank::is_ordered(&["a", ""]), "empty");
}

#[test]
fn spaced_keys_are_valid_ordered_and_leave_room_at_both_ends() {
    for n in [0, 1, 2, 5, 35, 36, 37, 200] {
        let keys = rank::spaced(n);
        assert_eq!(keys.len(), n, "n = {n}");
        assert!(keys.iter().all(|k| rank::is_valid(k)), "n = {n}: {keys:?}");
        let borrowed: Vec<&str> = keys.iter().map(String::as_str).collect();
        assert!(rank::is_ordered(&borrowed), "n = {n}: {keys:?}");
        if let (Some(first), Some(last)) = (keys.first(), keys.last()) {
            assert!(rank::between(None, Some(first)).is_some(), "n = {n}: below");
            assert!(rank::between(Some(last), None).is_some(), "n = {n}: above");
        }
    }
}

#[test]
fn spaced_keys_stay_short_and_evenly_stepped() {
    // The point of rebalancing across the whole range rather than repeatedly halving the top: a
    // large group must not end up with keys whose length grows with the group.
    let keys = rank::spaced(200);
    assert!(
        keys.iter().all(|k| k.len() <= 2),
        "200 keys fit in two base-36 digits: {keys:?}"
    );
    for pair in keys.windows(2) {
        assert!(
            rank::between(Some(&pair[0]), Some(&pair[1])).is_some(),
            "every neighbour pair keeps room for an insert: {pair:?}"
        );
    }
}
