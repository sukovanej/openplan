use std::fs::{File, OpenOptions};
use std::io;
use std::os::unix::process::CommandExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;

pub use op_api::DaemonInfo;

pub const DEFAULT_PORT: u16 = 7373;

const READY_DEADLINE: Duration = Duration::from_secs(5);
const STOP_DEADLINE: Duration = Duration::from_secs(5);

pub struct Home {
    dir: PathBuf,
}

pub enum Started {
    Already(DaemonInfo),
    Fresh(DaemonInfo),
}

enum StopOutcome {
    NotRunning,
    RemovedStale { pid: u32 },
    Stopped { pid: u32, port: u16 },
}

impl Home {
    pub fn resolve() -> Result<Self> {
        let dir = match std::env::var_os("OPLAN_HOME").filter(|v| !v.is_empty()) {
            Some(v) => PathBuf::from(v),
            None => home_dir()
                .context("could not determine home directory; set OPLAN_HOME")?
                .join(".plan"),
        };
        Ok(Self { dir })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn info_path(&self) -> PathBuf {
        self.dir.join("daemon.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.dir.join("daemon.lock")
    }

    pub fn start_lock_path(&self) -> PathBuf {
        self.dir.join("daemon.start.lock")
    }

    pub fn log_path(&self) -> PathBuf {
        self.dir.join("daemon.log")
    }

    pub fn ensure_dir(&self) -> io::Result<()> {
        std::fs::create_dir_all(&self.dir)
    }

    pub fn read_info(&self) -> Option<DaemonInfo> {
        let text = std::fs::read_to_string(self.info_path()).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write_info(&self, info: &DaemonInfo) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(info)?;
        let tmp = self.dir.join(format!("daemon.json.{}.tmp", info.pid));
        std::fs::write(&tmp, &bytes)?;
        std::fs::rename(&tmp, self.info_path())?;
        Ok(())
    }

    pub fn clear_info(&self) {
        let _ = std::fs::remove_file(self.info_path());
    }

    pub fn open_lock(&self) -> io::Result<File> {
        Self::open_lock_file(&self.lock_path())
    }

    pub fn open_start_lock(&self) -> io::Result<File> {
        Self::open_lock_file(&self.start_lock_path())
    }

    fn open_lock_file(path: &Path) -> io::Result<File> {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
    }

    // A free lock proves no live daemon holds it: fs2's flock is released when the
    // holding process's file handle closes, i.e. on exit — a truer liveness signal
    // than the recorded pid, which the OS may have recycled.
    pub fn lock_is_free(&self) -> Result<bool> {
        let lock = self.open_lock()?;
        match lock.try_lock_exclusive() {
            Ok(()) => {
                fs2::FileExt::unlock(&lock).ok();
                Ok(true)
            }
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

pub struct Control {
    home: Home,
    client: op_client::Client,
}

impl Control {
    pub fn resolve() -> Result<Self> {
        Ok(Self {
            home: Home::resolve()?,
            client: op_client::Client::default(),
        })
    }

    pub fn start(&self, port: u16, root: &Path) -> Result<()> {
        match self.ensure_daemon(port, root)? {
            // port 0 means "any", so a differing bound port there is expected, not ignored.
            Started::Already(info) if port != 0 && info.port != port => {
                println!(
                    "already running (pid {}, port {}); ignoring requested port {}",
                    info.pid, info.port, port
                );
            }
            Started::Already(info) => {
                println!("already running (pid {}, port {})", info.pid, info.port);
            }
            Started::Fresh(info) => {
                println!(
                    "started (pid {}, port {}); singleton for OPLAN_HOME={}",
                    info.pid,
                    info.port,
                    self.home.dir().display()
                );
            }
        }
        Ok(())
    }

    pub fn restart(&self, port: u16, root: &Path) -> Result<()> {
        match self.stop_local()? {
            StopOutcome::NotRunning => {}
            StopOutcome::RemovedStale { pid } => {
                println!("removed stale daemon.json for pid {pid}");
            }
            StopOutcome::Stopped { pid, port } => println!("stopped (pid {pid}, port {port})"),
        }
        self.start(port, root)
    }

    pub fn ping(&self, override_url: Option<&str>) -> Result<bool> {
        if let Some(url) = override_url {
            let up = self.client.health(url.trim_end_matches('/')).is_some();
            if up {
                println!("running (daemon at {url})");
            } else {
                println!("not running (no oplan daemon at {url})");
            }
            return Ok(up);
        }

        match self.home.read_info() {
            Some(info) if self.serves_identity(&info) => {
                let uptime = fmt_uptime(now_unix().saturating_sub(info.started_at));
                println!(
                    "running (pid {}, port {}, up {}, v{})",
                    info.pid, info.port, uptime, info.version
                );
                Ok(true)
            }
            Some(info) => {
                println!("not running (stale daemon.json for pid {})", info.pid);
                Ok(false)
            }
            None => {
                println!("not running");
                Ok(false)
            }
        }
    }

    pub fn stop(&self, override_url: Option<&str>) -> Result<()> {
        if let Some(url) = override_url {
            let base = url.trim_end_matches('/');
            if self.client.shutdown(base) {
                println!("stopping (daemon at {url})");
            } else {
                println!("not running (no oplan daemon at {url})");
            }
            return Ok(());
        }

        match self.stop_local()? {
            StopOutcome::NotRunning => println!("not running"),
            StopOutcome::RemovedStale { pid } => {
                println!("not running (removed stale daemon.json for pid {pid})");
            }
            StopOutcome::Stopped { pid, port } => println!("stopped (pid {pid}, port {port})"),
        }
        Ok(())
    }

    fn stop_local(&self) -> Result<StopOutcome> {
        let Some(info) = self.home.read_info() else {
            return Ok(StopOutcome::NotRunning);
        };

        if self.home.lock_is_free()? {
            self.home.clear_info();
            return Ok(StopOutcome::RemovedStale { pid: info.pid });
        }

        let clean = self.serves_identity(&info) && self.client.shutdown(&base(info.port));
        if !clean {
            signal_term(info.pid)?;
        }
        self.wait_until_exited(info.pid)?;
        self.home.clear_info();
        Ok(StopOutcome::Stopped {
            pid: info.pid,
            port: info.port,
        })
    }

    pub fn ensure_daemon(&self, port: u16, root: &Path) -> Result<Started> {
        if let Some(info) = self.healthy_info() {
            return Ok(Started::Already(info));
        }

        self.home.ensure_dir()?;
        let lock_deadline = Instant::now() + READY_DEADLINE;

        // The start-lock is a distinct file from the daemon's lifetime lock, so holding it
        // never contends with the spawned daemon. It serializes concurrent starters;
        // re-checking health inside the wait lets a daemon another caller just brought up
        // short-circuit us.
        let start_lock = loop {
            if let Some(info) = self.healthy_info() {
                return Ok(Started::Already(info));
            }
            let lock = self.home.open_start_lock()?;
            match lock.try_lock_exclusive() {
                Ok(()) => break lock,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= lock_deadline {
                        bail!(
                            "another oplan instance is starting the daemon but it is not ready yet; see {}",
                            self.home.log_path().display()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(30));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if let Some(info) = self.healthy_info() {
            fs2::FileExt::unlock(&start_lock).ok();
            return Ok(Started::Already(info));
        }

        self.spawn_detached(port, root)?;

        // Hold the start-lock across the readiness wait so a concurrent starter blocks until
        // this daemon is confirmed serving instead of racing to spawn a second one. The
        // daemon takes its own lifetime lock, so nothing here contends with it.
        // Fresh budget: time spent contending for the start-lock must not shorten the window
        // the freshly spawned daemon gets to bind and answer /health.
        let ready_deadline = Instant::now() + READY_DEADLINE;
        let ready = self.poll_ready(ready_deadline);
        fs2::FileExt::unlock(&start_lock).ok();
        drop(start_lock);
        Ok(Started::Fresh(ready?))
    }

    fn healthy_info(&self) -> Option<DaemonInfo> {
        let info = self.home.read_info()?;
        self.serves_identity(&info).then_some(info)
    }

    // A live daemon answers /health with its own identity; requiring the pid to match
    // daemon.json rejects a stale record whose port was recycled by an unrelated service.
    fn serves_identity(&self, info: &DaemonInfo) -> bool {
        self.client
            .health(&base(info.port))
            .is_some_and(|live| live.pid == info.pid)
    }

    fn poll_ready(&self, deadline: Instant) -> Result<DaemonInfo> {
        let mut backoff = Duration::from_millis(20);
        loop {
            if let Some(info) = self.healthy_info() {
                return Ok(info);
            }
            if Instant::now() >= deadline {
                bail!(
                    "daemon did not become ready in time; see {}",
                    self.home.log_path().display()
                );
            }
            std::thread::sleep(backoff);
            backoff = (backoff * 2).min(Duration::from_millis(200));
        }
    }

    fn wait_until_exited(&self, pid: u32) -> Result<()> {
        let start = Instant::now();
        loop {
            if !pid_alive(pid) {
                return Ok(());
            }
            if start.elapsed() > STOP_DEADLINE {
                bail!(
                    "daemon (pid {pid}) did not exit within {STOP_DEADLINE:?}; see {}",
                    self.home.log_path().display()
                );
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn spawn_detached(&self, port: u16, root: &Path) -> Result<()> {
        self.home.ensure_dir()?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.home.log_path())
            .with_context(|| format!("opening {}", self.home.log_path().display()))?;
        let log_err = log.try_clone()?;
        let exe = std::env::current_exe().context("locating current executable")?;
        let cwd = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let port_arg = port.to_string();

        Command::new(exe)
            .current_dir(&cwd)
            .arg("--root")
            .arg(&cwd)
            .args(["server", "start", "--foreground", "--port", &port_arg])
            .stdin(Stdio::null())
            .stdout(Stdio::from(log))
            .stderr(Stdio::from(log_err))
            .process_group(0)
            .spawn()
            .context("spawning detached daemon")?;
        Ok(())
    }
}

fn pid_alive(pid: u32) -> bool {
    use nix::errno::Errno;
    use nix::sys::signal::kill;
    use nix::unistd::Pid;

    let Ok(pid) = i32::try_from(pid) else {
        return false;
    };
    // Signal 0 probes existence; EPERM means the process exists but isn't ours to signal.
    matches!(kill(Pid::from_raw(pid), None), Ok(()) | Err(Errno::EPERM))
}

fn signal_term(pid: u32) -> Result<()> {
    use nix::errno::Errno;
    use nix::sys::signal::{Signal, kill};
    use nix::unistd::Pid;

    let pid = i32::try_from(pid).context("daemon pid out of range")?;
    // ESRCH means the process already exited between our probe and this signal — the
    // outcome we wanted, not a failure.
    match kill(Pid::from_raw(pid), Signal::SIGTERM) {
        Ok(()) | Err(Errno::ESRCH) => Ok(()),
        Err(e) => Err(e).with_context(|| format!("sending SIGTERM to pid {pid}")),
    }
}

fn base(port: u16) -> String {
    format!("http://127.0.0.1:{port}")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn fmt_uptime(secs: u64) -> String {
    let (h, m, s) = (secs / 3600, (secs % 3600) / 60, secs % 60);
    if h > 0 {
        format!("{h}h{m}m{s}s")
    } else if m > 0 {
        format!("{m}m{s}s")
    } else {
        format!("{s}s")
    }
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
}
