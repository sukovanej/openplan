use sha2::{Digest, Sha256};

// The release version is not part of the API.
fn spec_hash() -> String {
    let mut spec = op_server::openapi();
    spec.info.version = String::new();
    let json = spec.to_json().expect("the spec serializes");
    Sha256::digest(json.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn the_api_version_moves_with_the_api() {
    let hash = spec_hash();
    assert_eq!(
        hash,
        op_api::API_SPEC_SHA256,
        "the HTTP API changed: raise `API_VERSION` in crates/op-api/src/daemon.rs, and set \
         `API_SPEC_SHA256` there to {hash}"
    );
}
