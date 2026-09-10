use anyhow::{Result, bail};
use sha2::{Digest as _, Sha256};

pub fn verify_sha256(bytes: &[u8], published: &str) -> Result<()> {
    let Some(expected) = published.split_whitespace().next() else {
        bail!("the published digest file is empty");
    };
    if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
        bail!("the published digest {expected:?} is not a SHA-256 hex string");
    }
    let actual = format!("{:x}", Sha256::digest(bytes));
    if !actual.eq_ignore_ascii_case(expected) {
        bail!("digest mismatch: the release publishes {expected}, the download hashes to {actual}");
    }
    Ok(())
}
