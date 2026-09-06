use std::path::{Path, PathBuf};
use std::process::Command;

use crate::{GitError, Repo};

pub const LANE_BRANCH: &str = "openplan/updates";
pub const LANE_WORKTREE_DIR: &str = "openplan-updates";
const ATTRIBUTES_LINE: &str = "/.plan/**/*.md merge=openplan";

#[derive(Debug, PartialEq, Eq)]
pub enum Rebased {
    Clean,
    Blocked { paths: Vec<String> },
}

impl Repo {
    pub fn lane_worktree(&self) -> PathBuf {
        self.git_common_dir().join(LANE_WORKTREE_DIR)
    }

    // Everything the lane needs, applied in the order that lets a later step assume the earlier
    // one: the driver registration, then the branch, then its worktree, then the attributes commit
    // that makes git call the driver.
    pub fn ensure_lane(&self, default_branch: &str, driver: &str) -> Result<PathBuf, GitError> {
        let root = self.git_common_dir();
        self.register_driver(driver)?;
        if self.branch_commit(LANE_BRANCH).is_err() {
            let tip = self.branch_commit(default_branch)?;
            git(&root, &["branch", LANE_BRANCH, &tip])?;
        }
        let worktree = self.lane_worktree();
        if !worktree.join(".git").exists() {
            let path = path_arg(&worktree);
            git(
                &root,
                &["worktree", "add", "--no-checkout", &path, LANE_BRANCH],
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

    // `.gitattributes` is a tracked file, so the lane commits it rather than the
    // default branch's worktree:
    // it reaches the default branch with the first publish.
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
        commit(worktree, "Merge .plan task files section by section")?;
        Ok(())
    }

    // Whether anything was committed. The lane holds `.plan` alone, so `add --all` cannot pick up
    // a stray code change.
    pub fn lane_commit(&self, message: &str) -> Result<bool, GitError> {
        let worktree = self.lane_worktree();
        git(&worktree, &["add", "--sparse", "--all"])?;
        let staged = git(&worktree, &["diff", "--cached", "--name-only"])?;
        if staged.trim().is_empty() {
            return Ok(false);
        }
        commit(&worktree, message)?;
        Ok(true)
    }

    pub fn lane_rebase(&self, onto: &str) -> Result<Rebased, GitError> {
        let worktree = self.lane_worktree();
        match git(&worktree, &["rebase", onto]) {
            Ok(_) => Ok(Rebased::Clean),
            Err(err) => {
                if !self.lane_rebase_in_progress() {
                    return Err(err);
                }
                Ok(Rebased::Blocked {
                    paths: self.lane_conflicts()?,
                })
            }
        }
    }

    pub fn lane_rebase_in_progress(&self) -> bool {
        let admin = self
            .git_common_dir()
            .join("worktrees")
            .join(LANE_WORKTREE_DIR);
        admin.join("rebase-merge").exists() || admin.join("rebase-apply").exists()
    }

    pub fn lane_conflicts(&self) -> Result<Vec<String>, GitError> {
        let out = git(
            &self.lane_worktree(),
            &["diff", "--name-only", "--diff-filter=U"],
        )?;
        Ok(out.lines().map(str::to_owned).collect())
    }

    pub fn lane_rebase_abort(&self) -> Result<(), GitError> {
        git(&self.lane_worktree(), &["rebase", "--abort"]).map(|_| ())
    }

    // Advance `branch` to `to` without a merge and without a force. A worktree that holds the
    // branch gets its index and its files moved with the ref, so the checkout never goes stale.
    pub fn fast_forward(&self, branch: &str, to: &str) -> Result<(), GitError> {
        let from = self.branch_commit(branch)?;
        if from == to {
            return Ok(());
        }
        if self.ancestry(&[(&from, to)])?.first().copied().flatten()
            != Some(std::cmp::Ordering::Less)
        {
            return Err(GitError::NotFastForward {
                branch: branch.to_owned(),
            });
        }
        match self.worktree_holding(branch)? {
            Some(worktree) => {
                let dirty = git(&worktree, &["status", "--porcelain", "--", ".plan"])?;
                if !dirty.trim().is_empty() {
                    return Err(GitError::WorktreeDirty { path: worktree });
                }
                git(&worktree, &["merge", "--ff-only", to])?;
            }
            None => {
                let reference = format!("refs/heads/{branch}");
                git(
                    &self.git_common_dir(),
                    &["update-ref", &reference, to, &from],
                )?;
            }
        }
        Ok(())
    }

    pub fn worktree_holding(&self, branch: &str) -> Result<Option<PathBuf>, GitError> {
        Ok(self
            .worktrees()?
            .into_iter()
            .find(|worktree| worktree.branch.as_deref() == Some(branch))
            .map(|worktree| worktree.path))
    }

    pub fn push_lane(&self, remote: &str) -> Result<(), GitError> {
        let refspec = format!("refs/heads/{LANE_BRANCH}:refs/heads/{LANE_BRANCH}");
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

fn commit(dir: &Path, message: &str) -> Result<(), GitError> {
    git(
        dir,
        &["commit", "--no-verify", "--no-gpg-sign", "-m", message],
    )
    .map(|_| ())
}

fn path_arg(path: &Path) -> String {
    path.to_string_lossy().into_owned()
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
