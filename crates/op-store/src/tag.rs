use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};

use op_task::tag::{Tag, normalize_name};
use op_task::{PartialMetadata, parse_partial};

use crate::{Store, StoreError, atomic_replace, with_file_lock, write_temp};

impl Store {
    pub fn tags_dir(&self) -> PathBuf {
        self.plan_dir().join("tags")
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, StoreError> {
        self.tag_names()?
            .into_iter()
            .map(|name| self.read_tag_file(&self.tag_file(&name), name))
            .collect()
    }

    // One scan and one open per tag, hashed rather than parsed by the watcher: a tag file the
    // parser would reject must still register as a change.
    pub fn read_all_raw_tags(&self) -> Result<BTreeMap<String, String>, StoreError> {
        self.tag_names()?
            .into_iter()
            .filter_map(|name| match std::fs::read_to_string(self.tag_file(&name)) {
                Ok(text) => Some(Ok((name, text))),
                // Deleted between the scan and the read: it simply is not in the registry.
                Err(err) if err.kind() == io::ErrorKind::NotFound => None,
                Err(err) => Some(Err(err.into())),
            })
            .collect()
    }

    pub fn read_tag(&self, name: &str) -> Result<Tag, StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        self.read_tag_file(&path, name)
    }

    pub fn create_tag(&self, tag: &Tag) -> Result<(), StoreError> {
        let mut tag = tag.clone();
        tag.name = normalized(&tag.name)?;
        self.publish_tag(&tag)
    }

    // A verbatim overwrite of a tag's file, the tag-side twin of `replace_raw`: `lint --fix`
    // materializes a derived color and must not have the rest of the file reflowed through
    // `Tag::to_file_string`. It addresses the file rather than the registry, so a stem the
    // normalizer would have named differently is still repairable.
    pub fn replace_raw_tag(&self, name: &str, bytes: &[u8]) -> Result<(), StoreError> {
        let dir = self.tags_dir();
        let path = dir.join(format!("{name}.md"));
        // The name is a file stem, not a path: one carrying a separator would write outside the
        // registry entirely.
        if path.parent() != Some(dir.as_path()) || !path.is_file() {
            return Err(StoreError::TagNotFound {
                name: name.to_owned(),
            });
        }
        with_file_lock(
            &path,
            || StoreError::TagNotFound {
                name: name.to_owned(),
            },
            || atomic_replace(&path, bytes),
        )
    }

    pub fn update_tag(
        &self,
        name: &str,
        mutate: impl FnOnce(&mut Tag) -> Result<(), StoreError>,
    ) -> Result<Tag, StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        with_file_lock(
            &path,
            || StoreError::TagNotFound { name: name.clone() },
            || {
                let mut tag = self.read_tag_file(&path, name.clone())?;
                mutate(&mut tag)?;
                // The path came from the name the caller asked for, so a mutator that renamed the
                // tag would write the new tag over the old file and report a move that never
                // happened.
                if tag.name != name {
                    return Err(StoreError::Invalid(format!(
                        "an update cannot rename {name} to {}; `rename_tag` also moves the file \
                         and the tasks that reference it",
                        tag.name
                    )));
                }
                atomic_replace(&path, self.tag_file_string(&tag)?.as_bytes())?;
                Ok(tag)
            },
        )
    }

    // `new_display_name` carries the new identity and the new H1 at once, the way `Tag::new` reads
    // the name it is created with. Returns the tasks whose `tags:` the rename rewrote; references
    // on other branches keep the old name and are left dangling by design, because a write only
    // reaches this worktree's files.
    pub fn rename_tag(&self, name: &str, new_display_name: &str) -> Result<Vec<u64>, StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        with_file_lock(
            &path,
            || StoreError::TagNotFound { name: name.clone() },
            || {
                let mut renamed = self.read_tag_file(&path, name.clone())?;
                renamed.rename(new_display_name).map_err(invalid)?;
                // Only the heading moved, so there is no file to publish and no task to rewrite.
                // Publishing would hard-link the tag onto itself and read as `TagExists`.
                if renamed.name == name {
                    atomic_replace(&path, self.tag_file_string(&renamed)?.as_bytes())?;
                    return Ok(Vec::new());
                }
                let referencing = self.tasks_tagged(&name)?;
                // The reference scan is lenient, so it counts a task the model cannot write back.
                // Find that task before anything is published, or the rename stops with the new
                // file already in place and the tasks still on the old name.
                for id in &referencing {
                    self.read(*id)?;
                }
                self.publish_tag(&renamed)?;
                // The old name stays registered until the last task moves off it, so no task ever
                // holds a name this branch does not know.
                for id in &referencing {
                    self.retag(*id, &name, &renamed.name)?;
                }
                std::fs::remove_file(&path)?;
                Ok(referencing)
            },
        )
    }

    pub fn delete_tag(&self, name: &str, force: bool) -> Result<(), StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        with_file_lock(
            &path,
            || StoreError::TagNotFound { name: name.clone() },
            || {
                if !force {
                    let count = self.tasks_tagged(&name)?.len();
                    if count > 0 {
                        return Err(StoreError::TagReferenced {
                            name: name.clone(),
                            count,
                        });
                    }
                }
                std::fs::remove_file(&path)?;
                Ok(())
            },
        )
    }

    pub(crate) fn tag_names(&self) -> Result<BTreeSet<String>, StoreError> {
        let dir = self.tags_dir();
        if !dir.exists() {
            return Ok(BTreeSet::new());
        }
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") || !path.is_file() {
                continue;
            }
            // The stem is the identity, so a file the normalizer would have named differently —
            // `Backend.md`, `two words.md` — names no tag rather than a second spelling of one.
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str())
                && normalize_name(stem).is_ok_and(|normalized| normalized == stem)
            {
                names.insert(stem.to_owned());
            }
        }
        Ok(names)
    }

    fn tag_file(&self, name: &str) -> PathBuf {
        self.tags_dir().join(format!("{name}.md"))
    }

    fn tag_path(&self, name: &str) -> Result<PathBuf, StoreError> {
        self.tag_names()?
            .contains(name)
            .then(|| self.tag_file(name))
            .ok_or_else(|| StoreError::TagNotFound {
                name: name.to_owned(),
            })
    }

    fn read_tag_file(&self, path: &Path, name: String) -> Result<Tag, StoreError> {
        match std::fs::read_to_string(path) {
            Ok(text) => Tag::from_file_string(name, &text).map_err(|source| StoreError::TagFile {
                path: path.to_owned(),
                source,
            }),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(StoreError::TagNotFound { name }),
            Err(e) => Err(e.into()),
        }
    }

    // A non-clobbering hard link, like `link_id`: a name another writer took in the meantime is
    // reported rather than overwritten, and the watcher only ever sees a complete file.
    fn publish_tag(&self, tag: &Tag) -> Result<(), StoreError> {
        let contents = self.tag_file_string(tag)?;
        std::fs::create_dir_all(self.tags_dir())?;
        let tmp = write_temp(&self.tags_dir(), contents.as_bytes())?;
        let result = match std::fs::hard_link(&tmp, self.tag_file(&tag.name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(StoreError::TagExists {
                name: tag.name.clone(),
            }),
            Err(e) => Err(e.into()),
        };
        let _ = std::fs::remove_file(&tmp);
        result
    }

    fn tag_file_string(&self, tag: &Tag) -> Result<String, StoreError> {
        tag.to_file_string().map_err(|source| StoreError::TagFile {
            path: self.tag_file(&tag.name),
            source,
        })
    }

    // Read leniently: a task file too broken for the model to write is still a reference a delete
    // must count and a rename must report.
    fn tasks_tagged(&self, name: &str) -> Result<Vec<u64>, StoreError> {
        let mut ids = Vec::new();
        for (id, raw) in self.read_all_raw()? {
            if let PartialMetadata::Fields(fields) = parse_partial(&raw.text).metadata
                && fields
                    .tags
                    .is_ok_and(|tags| tags.iter().any(|tag| tag == name))
            {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    // Neither `validate` nor `in_file_form`: a rename is a mechanical substitution, so it must not
    // fail on another dangling tag the task carries, and it must leave the body's references alone.
    fn retag(&self, id: u64, name: &str, new_name: &str) -> Result<(), StoreError> {
        self.with_lock(id, || {
            let mut task = self.read(id)?;
            let tags = task
                .frontmatter
                .tags
                .iter()
                .map(|tag| if tag == name { new_name } else { tag }.to_owned())
                .collect();
            task.set_tags(tags);
            atomic_replace(&self.task_path(id)?, task.to_file_string()?.as_bytes())
        })
    }
}

fn normalized(name: &str) -> Result<String, StoreError> {
    normalize_name(name).map_err(invalid)
}

fn invalid(err: op_task::tag::ParseNameError) -> StoreError {
    StoreError::Invalid(err.to_string())
}
