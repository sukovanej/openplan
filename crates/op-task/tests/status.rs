use std::str::FromStr;

use op_task::Status;

#[test]
fn every_status_roundtrips_through_str() {
    for status in Status::ALL {
        assert_eq!(Status::from_str(status.as_str()).ok(), Some(status));
    }
}

#[test]
fn in_review_maps_to_its_wire_string() {
    assert_eq!(Status::InReview.as_str(), "in_review");
    assert_eq!(Status::from_str("in_review").ok(), Some(Status::InReview));
}

#[test]
fn unknown_status_is_rejected() {
    assert!(Status::from_str("in-review").is_err());
    assert!(Status::from_str("review").is_err());
}
