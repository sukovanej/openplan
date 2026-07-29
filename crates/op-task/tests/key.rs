use op_task::Abbreviation;

fn opp() -> Abbreviation {
    "OPP".parse().unwrap()
}

#[test]
fn an_abbreviation_is_exactly_three_uppercase_letters() {
    for good in ["OPP", "WEB", "AAA"] {
        assert_eq!(
            good.parse::<Abbreviation>().unwrap().as_str(),
            good,
            "{good} is a usable abbreviation"
        );
    }
    for bad in [
        "", "O", "OP", "OPPX", "opp", "Opp", "OP1", "OP-", "OPÉ", "O P",
    ] {
        assert!(
            bad.parse::<Abbreviation>().is_err(),
            "{bad:?} must not be an abbreviation"
        );
    }
}

#[test]
fn a_key_is_the_abbreviation_and_the_number() {
    assert_eq!(opp().format_key(42), "OPP-42");
    assert_eq!(opp().format_key(0), "OPP-0");
    assert_eq!(opp().parse_key("OPP-42"), Some(42));
    assert_eq!(opp().parse_key("OPP-0"), Some(0));
}

#[test]
fn no_other_spelling_names_a_task() {
    for refused in [
        "42", "opp-42", "OPP-042", "OPP-+42", "OPP-42x", "OPP42", "OPP-", "OPP--42", "-42",
        "WEB-7", "OPPX-42", "",
    ] {
        assert_eq!(
            opp().parse_key(refused),
            None,
            "{refused:?} must not parse as a key"
        );
    }
}

#[test]
fn a_reference_carries_its_section_across_both_spellings() {
    assert_eq!(opp().format_ref("42"), Some("OPP-42".to_owned()));
    assert_eq!(
        opp().format_ref("42#Design"),
        Some("OPP-42#Design".to_owned())
    );
    assert_eq!(opp().parse_ref("OPP-42"), Some("42".to_owned()));
    assert_eq!(
        opp().parse_ref("OPP-42#Design"),
        Some("42#Design".to_owned())
    );
    assert_eq!(opp().parse_ref("42"), None);
    assert_eq!(opp().parse_ref("WEB-7#Design"), None);
}

#[test]
fn key_shaped_text_is_told_apart_from_prose() {
    for key in ["OPP-42", "WEB-7", "AAA-0"] {
        assert!(op_task::is_key_shaped(key), "{key} is spelled like a key");
    }
    for prose in [
        "Some Page Title",
        "42",
        "opp-42",
        "OPPX-42",
        "OP-42",
        "OPP-042",
        "OPP-",
        "task-crud-6e8b",
    ] {
        assert!(
            !op_task::is_key_shaped(prose),
            "{prose:?} is not spelled like a key"
        );
    }
}

#[test]
fn a_body_reference_resolves_from_a_file_or_a_key_and_nothing_else() {
    assert_eq!(
        op_task::body_ref_id(opp(), "./00042-ship-login-page.md"),
        Some(42)
    );
    assert_eq!(
        op_task::body_ref_id(opp(), "./00042-a-stale-title.md#Design"),
        Some(42)
    );
    assert_eq!(op_task::body_ref_id(opp(), "OPP-42"), Some(42));
    assert_eq!(op_task::body_ref_id(opp(), "OPP-42#Design"), Some(42));
    for refused in ["42", "WEB-7", "opp-42", "Some Page Title", "./notes.md"] {
        assert_eq!(
            op_task::body_ref_id(opp(), refused),
            None,
            "{refused:?} names no task in a body"
        );
    }
}

#[test]
fn body_spans_cover_every_reference_and_skip_bracketed_prose() {
    let body = "see [[OPP-1]] and [[ OPP-2 ]]\nnot [[a[b]] nor [[open";
    let spans = op_task::body_ref_spans(body);
    assert_eq!(
        spans.iter().map(|(_, inner)| *inner).collect::<Vec<_>>(),
        vec!["OPP-1", "OPP-2"]
    );
    let (span, _) = &spans[0];
    assert_eq!(&body[span.clone()], "[[OPP-1]]");
}
