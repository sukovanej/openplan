use std::fs::OpenOptions;
use std::io;
use std::os::unix::process::CommandExt as _;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use fs2::FileExt as _;

use op_api::DaemonInfo;

use crate::home::Home;
use crate::serve::SERVE_ARG;
use crate::serving;

const READY_DEADLINE: Duration = Duration::from_secs(5);
const STOP_DEADLINE: Duration = Duration::from_secs(5);

pub enum Started {
    Already(DaemonInfo),
    Fresh(DaemonInfo),
}

impl Started {
    pub fn into_info(self) -> DaemonInfo {
        match self {
            Started::Already(info) | Started::Fresh(info) => info,
        }
    }
}

pub enum StopOutcome {
    NotRunning,
    RemovedStale { pid: u32 },
    Stopped { pid: u32, port: u16 },
}

pub struct Control {
    home: Home,
    client: op_client::Client,
}

impl Control {
    pub fn resolve() -> Result<Self> {
        Ok(Self::at(Home::resolve()?))
    }

    pub fn at(home: Home) -> Self {
        Self {
            home,
            client: op_client::Client::default(),
        }
    }

    pub fn home(&self) -> &Home {
        &self.home
    }

    pub fn running(&self) -> Option<DaemonInfo> {
        serving(&self.home, &self.client)
    }

    pub fn recorded(&self) -> Option<DaemonInfo> {
        self.home.read_info()
    }

    pub fn ensure(&self, port: u16) -> Result<Started> {
        if let Some(info) = self.running() {
            return Ok(Started::Already(info));
        }

        self.home.ensure_dir()?;
        let lock_deadline = Instant::now() + READY_DEADLINE;

        // The start-lock is a distinct file from the daemon's lifetime lock, so holding it
        // never contends with the spawned daemon. It serializes concurrent starters;
        // re-checking health inside the wait lets a daemon another caller just brought up
        // short-circuit us.
        let start_lock = loop {
            if let Some(info) = self.running() {
                return Ok(Started::Already(info));
            }
            let lock = self.home.open_start_lock()?;
            match lock.try_lock_exclusive() {
                Ok(()) => break lock,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    if Instant::now() >= lock_deadline {
                        bail!(
                            "another openplan instance is starting the daemon but it is not ready yet; see {}",
                            self.home.log_path().display()
                        );
                    }
                    std::thread::sleep(Duration::from_millis(30));
                }
                Err(e) => return Err(e.into()),
            }
        };

        if let Some(info) = self.running() {
            fs2::FileExt::unlock(&start_lock).ok();
            return Ok(Started::Already(info));
        }

        self.spawn_detached(port)?;

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

    pub fn stop(&self) -> Result<StopOutcome> {
        let Some(info) = self.home.read_info() else {
            return Ok(StopOutcome::NotRunning);
        };

        if self.home.lock_is_free()? {
            self.home.clear_info();
            return Ok(StopOutcome::RemovedStale { pid: info.pid });
        }

        let clean =
            crate::serves(&self.client, &info) && self.client.shutdown(&crate::base_url(info.port));
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

    fn poll_ready(&self, deadline: Instant) -> Result<DaemonInfo> {
        let mut backoff = Duration::from_millis(20);
        loop {
            if let Some(info) = self.running() {
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

    // The daemon serves the registry, not the directory that started it, so it works out of
    // `OPENPLAN_HOME` — which outlives every worktree this workflow creates and removes.
    fn spawn_detached(&self, port: u16) -> Result<()> {
        self.home.ensure_dir()?;
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.home.log_path())
            .with_context(|| format!("opening {}", self.home.log_path().display()))?;
        let log_err = log.try_clone()?;
        let exe = std::env::current_exe().context("locating current executable")?;

        Command::new(exe)
            .current_dir(self.home.dir())
            .args([SERVE_ARG, &port.to_string()])
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
