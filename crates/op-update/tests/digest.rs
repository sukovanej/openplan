use op_update::verify_sha256;

const HELLO_SHA256: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

#[test]
fn a_matching_digest_passes() {
    verify_sha256(b"hello", &format!("{HELLO_SHA256} *openplan.tar.gz\n")).unwrap();
}

#[test]
fn the_digest_compares_without_case() {
    verify_sha256(b"hello", &HELLO_SHA256.to_uppercase()).unwrap();
}

#[test]
fn a_changed_download_fails() {
    let err = verify_sha256(b"hellp", HELLO_SHA256).unwrap_err();
    assert!(err.to_string().contains("digest mismatch"), "{err}");
}

#[test]
fn a_malformed_digest_file_fails() {
    assert!(verify_sha256(b"hello", "").is_err());
    assert!(verify_sha256(b"hello", "not-a-digest").is_err());
    assert!(verify_sha256(b"hello", &HELLO_SHA256[..40]).is_err());
}
