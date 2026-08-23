use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};

use op_task::tag::{Tag, normalize_name};
use op_task::{PartialMetadata, parse_partial};

use crate::{Store, StoreError, atomic_replace, with_file_lock, write_temp};

impl Store {
    pub fn tags_dir(&self) -> PathBuf {
        self.plan_dir().join("tags")
    }

    pub fn tag_exists(&self, name: &str) -> bool {
        matches!(normalize_name(name), Ok(name) if self.tag_names().is_ok_and(|names| names.contains(&name)))
    }

    pub fn list_tags(&self) -> Result<Vec<Tag>, StoreError> {
        self.tag_names()?
            .into_iter()
            .map(|name| self.read_tag_file(&self.tag_file(&name), name))
            .collect()
    }

    pub fn read_tag(&self, name: &str) -> Result<Tag, StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        self.read_tag_file(&path, name)
    }

    pub fn create_tag(&self, tag: &Tag) -> Result<(), StoreError> {
        let name = normalized(&tag.name)?;
        let contents = self.tag_file_string(tag)?;
        std::fs::create_dir_all(self.tags_dir())?;
        let tmp = write_temp(&self.tags_dir(), contents.as_bytes())?;
        // Publish with a non-clobbering hard link, like `link_id`: a name another writer took in the
        // meantime is reported rather than overwritten.
        let result = match std::fs::hard_link(&tmp, self.tag_file(&name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                Err(StoreError::TagExists { name })
            }
            Err(e) => Err(e.into()),
        };
        let _ = std::fs::remove_file(&tmp);
        result
    }

    pub fn write_tag(&self, tag: &Tag) -> Result<(), StoreError> {
        let name = normalized(&tag.name)?;
        let path = self.tag_path(&name)?;
        let contents = self.tag_file_string(tag)?;
        with_file_lock(
            &path,
            || StoreError::TagNotFound { name: name.clone() },
            || atomic_replace(&path, contents.as_bytes()),
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
        let from = self.tag_path(&name)?;
        let mut renamed = self.read_tag_file(&from, name.clone())?;
        renamed.rename(new_display_name).map_err(invalid)?;
        let new_name = renamed.name.clone();
        let referencing = self.tasks_tagged(&name)?;

        let tmp = write_temp(&self.tags_dir(), self.tag_file_string(&renamed)?.as_bytes())?;
        let published = match std::fs::hard_link(&tmp, self.tag_file(&new_name)) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => Err(StoreError::TagExists {
                name: new_name.clone(),
            }),
            Err(e) => Err(e.into()),
        };
        let _ = std::fs::remove_file(&tmp);
        published?;

        std::fs::remove_file(&from)?;
        for id in &referencing {
            self.retag(*id, &name, &new_name)?;
        }
        Ok(referencing)
    }

    pub fn delete_tag(&self, name: &str, force: bool) -> Result<(), StoreError> {
        let name = normalized(name)?;
        let path = self.tag_path(&name)?;
        if !force {
            let count = self.tasks_tagged(&name)?.len();
            if count > 0 {
                return Err(StoreError::TagReferenced { name, count });
            }
        }
        match std::fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Err(StoreError::TagNotFound { name }),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn tag_names(&self) -> Result<BTreeSet<String>, StoreError> {
        let dir = self.tags_dir();
        if !dir.exists() {
            return Ok(BTreeSet::new());
        }
        let mut names = BTreeSet::new();
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
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
