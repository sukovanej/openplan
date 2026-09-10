use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{GitError, Repo};

pub const ROLLING_UPDATES_BRANCH: &str = "openplan/rolling-updates";
const ROLLING_UPDATES_WORKTREE_DIR: &str = "openplan-rolling-updates";
const ATTRIBUTES_LINE: &str = "/.plan/**/*.md merge=openplan";

#[derive(Debug, PartialEq, Eq)]
pub enum Rebased {
    Clean,
    Blocked { paths: Vec<String> },
}

impl Repo {
    pub fn rolling_updates_worktree(&self) -> PathBuf {
        self.git_common_dir().join(ROLLING_UPDATES_WORKTREE_DIR)
    }

    pub fn branch_has_path(&self, branch: &str, path: &str) -> bool {
        git(
            &self.git_common_dir(),
            &["cat-file", "-e", &format!("{branch}:{path}")],
        )
        .is_ok()
    }

    // Everything the branch needs, applied in the order that lets a later step assume the earlier
    // one: the driver registration, then the branch, then its worktree, then the attributes commit
    // that makes git call the driver.
    pub fn ensure_rolling_updates(
        &self,
        default_branch: &str,
        driver: &str,
    ) -> Result<PathBuf, GitError> {
        let root = self.git_common_dir();
        self.register_driver(driver)?;
        if self.branch_commit(ROLLING_UPDATES_BRANCH).is_err() {
            let tip = self.branch_commit(default_branch)?;
            git(&root, &["branch", ROLLING_UPDATES_BRANCH, &tip])?;
        }
        let worktree = self.rolling_updates_worktree();
        if !worktree.join(".git").exists() {
            git(
                &root,
                &[
                    "worktree",
                    "add",
                    "--no-checkout",
                    &worktree.to_string_lossy(),
                    ROLLING_UPDATES_BRANCH,
                ],
            )?;
            git(&worktree, &["sparse-checkout", "init", "--cone"])?;
            git(&worktree, &["sparse-checkout", "set", ".plan"])?;
            git(&worktree, &["checkout"])?;
        }
        self.ensure_attributes(&worktree)?;
        Ok(worktree)
    }

    fn register_driver(&self, driver: &str) -> Result<(), GitError> {
        let root = self.git_common_dir();
        let command = format!("{driver} %O %A %B %L %P %S %X %Y");
        git(
            &root,
            &["config", "merge.openplan.name", "openplan task merge"],
        )?;
        git(&root, &["config", "merge.openplan.driver", &command])?;
        Ok(())
    }

    // `.gitattributes` is a tracked file, so it is committed here rather than on the default
    // branch. It reaches the default branch with the first publish.
    fn ensure_attributes(&self, worktree: &Path) -> Result<(), GitError> {
        let path = worktree.join(".gitattributes");
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current.lines().any(|line| line.trim() == ATTRIBUTES_LINE) {
            return Ok(());
        }
        let mut next = current;
        if !next.is_empty() && !next.ends_with('\n') {
            next.push('\n');
        }
        next.push_str(ATTRIBUTES_LINE);
        next.push('\n');
        std::fs::write(&path, next).map_err(|e| GitError::Command(e.to_string()))?;
        git(worktree, &["add", "--sparse", ".gitattributes"])?;
        commit(worktree, "Merge .plan task files with the openplan driver")?;
        Ok(())
    }

    // Whether anything was committed. Cone-mode sparse checkout keeps every root-level file, so
    // the paths are named rather than left to `--all`.
    pub fn rolling_updates_commit(&self, message: &str) -> Result<bool, GitError> {
        let worktree = self.rolling_updates_worktree();
        git(
            &worktree,
            &["add", "--sparse", "--all", "--", ".plan", ".gitattributes"],
        )?;
        let staged = git(&worktree, &["diff", "--cached", "--name-only"])?;
        if staged.trim().is_empty() {
            return Ok(false);
        }
        commit(&worktree, message)?;
        Ok(true)
    }

    // One task off the branch. The default branch's version wins where it has one, and where it has
    // none the task was added here, so it goes. Either way the path is staged, so the commit below
    // carries that task alone and leaves any other dirty task to the commit timer.
    pub fn discard_rolling_update(
        &self,
        default_branch: &str,
        path: &str,
        message: &str,
    ) -> Result<(), GitError> {
        let worktree = self.rolling_updates_worktree();
        if self.branch_has_path(default_branch, path) {
            git(&worktree, &["checkout", default_branch, "--", path])?;
        } else {
            git(
                &worktree,
                &[
                    "rm",
                    "--sparse",
                    "--force",
                    "--quiet",
                    "--ignore-unmatch",
                    "--",
                    path,
                ],
            )?;
            // A task written since the last commit is untracked, so the index never knew it and
            // the removal above left the file where it was.
            let file = worktree.join(path);
            if file.exists() {
                std::fs::remove_file(&file).map_err(|e| GitError::Command(e.to_string()))?;
            }
        }
        let staged = git(&worktree, &["diff", "--cached", "--name-only"])?;
        if staged.trim().is_empty() {
            return Ok(());
        }
        commit(&worktree, message)
    }

    // Every task off the branch, and the only way out of a stopped rebase. The clean follows the
    // reset because a reset keeps untracked files, and a task written since the last commit is one.
    pub fn discard_rolling_updates(&self, default_branch: &str) -> Result<(), GitError> {
        let worktree = self.rolling_updates_worktree();
        if self.rolling_updates_rebase_in_progress() {
            git(&worktree, &["rebase", "--abort"])?;
        }
        git(&worktree, &["reset", "--hard", default_branch])?;
        git(&worktree, &["clean", "--force", "--quiet", "--", ".plan"])?;
        self.ensure_attributes(&worktree)
    }

    pub fn rolling_updates_rebase(&self, onto: &str) -> Result<Rebased, GitError> {
        let worktree = self.rolling_updates_worktree();
        match git(&worktree, &["rebase", onto]) {
            Ok(_) => Ok(Rebased::Clean),
            Err(err) => {
                if !self.rolling_updates_rebase_in_progress() {
                    return Err(err);
                }
                Ok(Rebased::Blocked {
                    paths: self.rolling_updates_conflicts()?,
                })
            }
        }
    }

    // Task files only. The branch always carries the `.gitattributes` commit as well, and that is
    // not something a person publishes.
    pub fn rolling_updates_differs(&self, from: &str) -> Result<bool, GitError> {
        let out = git(
            &self.rolling_updates_worktree(),
            &[
                "diff",
                "--name-only",
                from,
                ROLLING_UPDATES_BRANCH,
                "--",
                ".plan",
            ],
        )?;
        Ok(!out.trim().is_empty())
    }

    // `git diff <default-branch>` compares that branch's tree to the working file, so an edit the
    // daemon has not committed yet shows without a read through the index.
    pub fn rolling_updates_diff(&self, from: &str, path: &str) -> Result<String, GitError> {
        let worktree = self.rolling_updates_worktree();
        let diff = git(&worktree, &["diff", from, "--", path])?;
        if diff.trim().is_empty() {
            return untracked_diff(&worktree, path);
        }
        Ok(diff)
    }

    pub fn rolling_updates_rebase_in_progress(&self) -> bool {
        let admin = self
            .git_common_dir()
            .join("worktrees")
            .join(ROLLING_UPDATES_WORKTREE_DIR);
        admin.join("rebase-merge").exists() || admin.join("rebase-apply").exists()
    }

    pub fn rolling_updates_conflicts(&self) -> Result<Vec<String>, GitError> {
        let out = git(
            &self.rolling_updates_worktree(),
            &["diff", "--name-only", "--diff-filter=U"],
        )?;
        Ok(out.lines().map(str::to_owned).collect())
    }

    // The remote gets one branch per person. `-` and not `/` before the name: git cannot hold
    // `openplan/rolling-updates` and `openplan/rolling-updates/milan` at once.
    pub fn rolling_updates_remote_branch(&self) -> String {
        let who = self
            .config_string("user.email")
            .and_then(|email| email.split('@').next().map(str::to_owned))
            .or_else(|| self.config_string("user.name"))
            .unwrap_or_default();
        format!("{ROLLING_UPDATES_BRANCH}-{}", op_task::slug(&who, "local"))
    }

    pub fn rolling_updates_remote(&self, default_branch: &str) -> String {
        self.config_string(&format!("branch.{default_branch}.remote"))
            .unwrap_or_else(|| "origin".to_owned())
    }

    pub fn remote_url(&self, remote: &str) -> Option<String> {
        self.config_string(&format!("remote.{remote}.url"))
    }

    // Force is safe because the destination carries the person: nobody else writes that branch.
    pub fn push_rolling_updates(&self, remote: &str, branch: &str) -> Result<(), GitError> {
        let refspec = format!("refs/heads/{ROLLING_UPDATES_BRANCH}:refs/heads/{branch}");
        git(
            &self.git_common_dir(),
            &["push", "--force", remote, &refspec],
        )
        .map(|_| ())
    }

    pub fn config_string(&self, key: &str) -> Option<String> {
        let value = git(&self.git_common_dir(), &["config", "--get", key]).ok()?;
        let value = value.trim();
        (!value.is_empty()).then(|| value.to_owned())
    }
}

// A task the daemon has not committed yet is untracked, and `git diff <commit>` passes over an
// untracked file in silence. `--no-index` formats the same addition, and it exits 1 when it finds
// one, so the status is read rather than trusted.
fn untracked_diff(worktree: &Path, path: &str) -> Result<String, GitError> {
    if !worktree.join(path).is_file()
        || !git(worktree, &["ls-files", "--", path])?.trim().is_empty()
    {
        return Ok(String::new());
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["diff", "--no-index", "--", "/dev/null", path])
        .output()
        .map_err(|e| GitError::Command(format!("git diff --no-index: {e}")))?;
    match output.status.code() {
        Some(0 | 1) => Ok(String::from_utf8_lossy(&output.stdout).into_owned()),
        _ => Err(GitError::Command(format!(
            "git diff --no-index failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))),
    }
}

fn commit(dir: &Path, message: &str) -> Result<(), GitError> {
    git(
        dir,
        &["commit", "--no-verify", "--no-gpg-sign", "-m", message],
    )
    .map(|_| ())
}

// Git is asked to do the work a person would do by hand. gix cannot rebase and cannot push, and a
// hand-written rebase would have to reimplement the merge driver protocol that git already speaks.
fn git(dir: &Path, args: &[&str]) -> Result<String, GitError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .map_err(|e| GitError::Command(format!("git {}: {e}", args.join(" "))))?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
    }
    Err(GitError::Command(format!(
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr).trim(),
    )))
}
