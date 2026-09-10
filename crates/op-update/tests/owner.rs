use std::path::{Path, PathBuf};

use op_update::{Environment, Owner, owner_of};

fn env(wsl: bool) -> Environment {
    Environment {
        cargo_home: Some(PathBuf::from("/Users/me/.cargo")),
        homebrew_prefix: None,
        wsl,
    }
}

#[test]
fn a_binary_in_local_bin_is_ours() {
    assert_eq!(
        owner_of(Path::new("/Users/me/.local/bin/openplan"), &env(false)),
        None
    );
}

#[test]
fn cargo_install_owns_cargo_home_bin() {
    assert_eq!(
        owner_of(Path::new("/Users/me/.cargo/bin/openplan"), &env(false)),
        Some(Owner::Cargo)
    );
}

#[test]
fn homebrew_owns_its_prefixes() {
    assert_eq!(
        owner_of(
            Path::new("/opt/homebrew/Cellar/openplan/0.1/bin/openplan"),
            &env(false)
        ),
        Some(Owner::Homebrew)
    );
    assert_eq!(
        owner_of(
            Path::new("/usr/local/Cellar/openplan/0.1/bin/openplan"),
            &env(false)
        ),
        Some(Owner::Homebrew)
    );
    let custom = Environment {
        homebrew_prefix: Some(PathBuf::from("/brew")),
        ..env(false)
    };
    assert_eq!(
        owner_of(Path::new("/brew/bin/openplan"), &custom),
        Some(Owner::Homebrew)
    );
}

#[test]
fn nix_and_system_paths_are_refused() {
    assert_eq!(
        owner_of(
            Path::new("/nix/store/abc-openplan/bin/openplan"),
            &env(false)
        ),
        Some(Owner::Nix)
    );
    assert_eq!(
        owner_of(Path::new("/usr/bin/openplan"), &env(false)),
        Some(Owner::SystemPackage)
    );
}

#[test]
fn a_windows_drive_is_refused_only_under_wsl() {
    let exe = Path::new("/mnt/c/Users/me/bin/openplan");
    assert_eq!(owner_of(exe, &env(true)), Some(Owner::DrvFs));
    assert_eq!(owner_of(exe, &env(false)), None);
}

#[test]
fn the_refusal_names_the_owner_and_the_path() {
    let text = Owner::Cargo.refusal(Path::new("/Users/me/.cargo/bin/openplan"));
    assert!(text.contains("cargo install"), "{text}");
    assert!(text.contains("/Users/me/.cargo/bin/openplan"), "{text}");
}
