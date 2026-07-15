use op_task::rank;

#[test]
fn between_open_ends_is_a_valid_key() {
    let key = rank::between(None, None);
    assert!(rank::is_valid(&key));
}

#[test]
fn between_lands_strictly_between_two_keys() {
    let a = "a".to_owned();
    let b = "b".to_owned();
    let mid = rank::between(Some(&a), Some(&b));
    assert!(a.as_str() < mid.as_str(), "{a} < {mid}");
    assert!(mid.as_str() < b.as_str(), "{mid} < {b}");
}

#[test]
fn between_after_appends_above() {
    let a = rank::between(None, None);
    let b = rank::between(Some(&a), None);
    assert!(a < b, "{a} < {b}");
}

#[test]
fn between_before_inserts_below() {
    let b = rank::between(None, None);
    let a = rank::between(None, Some(&b));
    assert!(a < b, "{a} < {b}");
}

#[test]
fn repeated_before_inserts_do_not_collapse() {
    // Each insert takes the midpoint of the previous lowest and the open start; the keys must stay
    // distinct and ordered no matter how many times we prepend (no f64 precision collapse).
    let mut lowest = rank::between(None, None);
    let mut seen = vec![lowest.clone()];
    for _ in 0..200 {
        let next = rank::between(None, Some(&lowest));
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
        let key = rank::between(prev.as_deref(), None);
        prev = Some(key.clone());
        keys.push(key);
    }
    let mut sorted = keys.clone();
    sorted.sort();
    assert_eq!(keys, sorted, "append order must equal sorted order");
}
