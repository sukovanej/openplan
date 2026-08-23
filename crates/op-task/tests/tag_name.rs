use op_task::tag::normalize_name;

#[test]
fn case_and_separators_normalize() {
    for (input, expected) in [
        ("backend", "backend"),
        ("Backend", "backend"),
        ("Front End", "front-end"),
        ("front_end", "front-end"),
        ("  Front   End  ", "front-end"),
        ("front--end", "front-end"),
        ("v2", "v2"),
        ("2026-q1", "2026-q1"),
    ] {
        assert_eq!(
            normalize_name(input).ok().as_deref(),
            Some(expected),
            "{input:?}"
        );
    }
}

#[test]
fn a_name_that_normalization_cannot_reach_is_rejected() {
    for input in ["C++", "", "   ", "-wip", "back/end", "café", "a.b", "#tag"] {
        assert!(normalize_name(input).is_err(), "{input:?} must be rejected");
    }
}

#[test]
fn the_rejection_states_the_rule() {
    let message = normalize_name("C++").unwrap_err().to_string();
    assert!(message.contains("\"C++\""), "{message}");
    assert!(message.contains(op_task::tag::NAME_RULE), "{message}");
}

#[test]
fn normalization_is_idempotent() {
    for input in ["Front End", "BACK_END", "v2"] {
        let once = normalize_name(input).unwrap();
        assert_eq!(normalize_name(&once).ok().as_deref(), Some(once.as_str()));
    }
}
