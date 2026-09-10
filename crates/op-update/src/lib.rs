mod digest;
mod github;
mod install;
mod owner;

pub use digest::verify_sha256;
pub use github::{Asset, Github, Release};
pub use install::{replace_bundle, replace_executable};
pub use owner::{Environment, Owner, owner_of};

pub const APP_BUNDLE: &str = "OpenPlan.app";

pub fn target() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => Some("aarch64-apple-darwin"),
        ("macos", "x86_64") => Some("x86_64-apple-darwin"),
        ("linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
        ("linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
        _ => None,
    }
}

pub fn cli_archive_name(target: &str) -> String {
    format!("openplan-{target}.tar.gz")
}

pub fn app_archive_name(target: &str) -> String {
    format!("OpenPlan-{target}.app.tar.gz")
}
