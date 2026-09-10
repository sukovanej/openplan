use std::path::{Path, PathBuf};

pub struct Environment {
    pub cargo_home: Option<PathBuf>,
    pub homebrew_prefix: Option<PathBuf>,
    pub wsl: bool,
}

impl Environment {
    // The executable path arrives canonical, so the prefixes must be canonical too: on macOS
    // `/var` is a link to `/private/var`.
    pub fn detect() -> Self {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let cargo_home = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| home.map(|home| home.join(".cargo")));
        Self {
            cargo_home: cargo_home.map(canonical),
            homebrew_prefix: std::env::var_os("HOMEBREW_PREFIX")
                .map(PathBuf::from)
                .map(canonical),
            wsl: std::fs::read_to_string("/proc/sys/kernel/osrelease")
                .is_ok_and(|release| release.to_ascii_lowercase().contains("microsoft")),
        }
    }
}

fn canonical(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Owner {
    Cargo,
    Homebrew,
    Nix,
    SystemPackage,
    DrvFs,
}

impl Owner {
    pub fn refusal(self, exe: &Path) -> String {
        let exe = exe.display();
        match self {
            Owner::Cargo => {
                format!("cargo install owns {exe}; run `cargo install openplan` to update it")
            }
            Owner::Homebrew => {
                format!("Homebrew owns {exe}; run `brew upgrade openplan` to update it")
            }
            Owner::Nix => format!("Nix owns {exe}; update it through your Nix configuration"),
            Owner::SystemPackage => {
                format!("a system package owns {exe}; update it through your package manager")
            }
            Owner::DrvFs => format!(
                "{exe} is on a Windows drive (DrvFs), which cannot replace a binary atomically; install openplan under the Linux file system"
            ),
        }
    }
}

pub fn owner_of(exe: &Path, env: &Environment) -> Option<Owner> {
    let under = |prefix: &Path| exe.starts_with(prefix);
    if env
        .cargo_home
        .as_deref()
        .is_some_and(|home| under(&home.join("bin")))
    {
        return Some(Owner::Cargo);
    }
    if env.homebrew_prefix.as_deref().is_some_and(under)
        || under(Path::new("/opt/homebrew"))
        || under(Path::new("/home/linuxbrew/.linuxbrew"))
        || exe.components().any(|part| part.as_os_str() == "Cellar")
    {
        return Some(Owner::Homebrew);
    }
    if under(Path::new("/nix/store")) {
        return Some(Owner::Nix);
    }
    if under(Path::new("/usr")) {
        return Some(Owner::SystemPackage);
    }
    if env.wsl && under(Path::new("/mnt")) {
        return Some(Owner::DrvFs);
    }
    None
}
