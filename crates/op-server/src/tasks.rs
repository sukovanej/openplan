use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use op_api::{
    ApiErrorBody, Board, Comment, CreateComment, CreateTag, CreateTask, DocumentChange,
    DocumentChangeKind, FieldChange, Flow, FlowQuery, HistoryEntry, KeyError, Metadata,
    ResolveConflict, RevisionView, SearchHit, Status, SyncResult, SyncView, TagChange, TagPatch,
    TagView, TaskAtRevision, TaskChange, TaskDetail, TaskListItem, TaskPatch, TaskSnapshot,
    TaskSummary, TaskTree, TaskTreeView, WriteBody, WriteTaskFile,
};
use op_backend::{Change, ChangeKind, Committed, LogEntry, RevisionId};
use op_task::{Abbreviation, Task, layout};
use op_tracker::{HistoryQuery, TrackerError};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::project::sync_view;
use crate::{ApiError, AppState, Project, actor_of, blocking, project_of};

const HISTORY_PAGE: usize = 100;

// `fresh` is for a caller with no change stream: it reads the writes of other processes first.
#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct ReadQuery {
    #[serde(default)]
    fresh: bool,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct TreeQuery {
    #[serde(default)]
    fresh: bool,
    depth: Option<usize>,
}

// An empty `q` matches nothing, so a caller may send every keystroke.
#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct SearchQuery {
    #[serde(default)]
    q: String,
    #[serde(default)]
    fresh: bool,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct PageQuery {
    // Only revisions older than this one; omit for the newest.
    before: Option<String>,
    limit: Option<usize>,
}

#[derive(Deserialize, utoipa::IntoParams)]
pub(crate) struct DeleteTagQuery {
    #[serde(default)]
    force: bool,
}

#[derive(Serialize, ToSchema)]
pub(crate) struct CreatedTask {
    id: String,
}

impl Project {
    // The watcher or the watchdog can catch the backend up first, so this compares the head with
    // what the index holds rather than trust its own refresh to report the move.
    pub(crate) fn fresh(&self) {
        if let Err(err) = self.tracker().backend().refresh() {
            tracing::warn!(project = %self.name(), error = %err, "outside changes not read");
        }
        self.catch_up();
    }

    // A write answers from the index, so the index must hold the write before the answer goes out.
    pub(crate) fn written(&self, committed: Option<&Committed>) {
        if committed.is_some() {
            self.catch_up();
        }
    }

    fn abbreviation(&self) -> Result<Abbreviation, ApiError> {
        self.index()
            .abbreviation()
            .ok_or_else(|| ApiError::from(TrackerError::NotInitialized))
    }

    pub(crate) fn number(&self, key: &str) -> Result<u64, ApiError> {
        let abbreviation = self.abbreviation()?;
        abbreviation
            .parse_key(key)
            .ok_or_else(|| KeyError::new(abbreviation, key).into())
    }

    fn detail(&self, key: &str, number: u64) -> Result<TaskDetail, ApiError> {
        self.index()
            .detail(&self.name(), number)
            .ok_or_else(|| no_such_task(key))
    }
}

pub(crate) fn no_such_task(key: &str) -> ApiError {
    ApiError::not_found(format!("no such task: {key}"))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks",
    params(("project" = String, Path, description = "Project name"), ReadQuery),
    responses(
        (status = 200, description = "Every task of the project", body = Vec<TaskListItem>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_tasks(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<Vec<TaskListItem>>, ApiError> {
    let project = project_of(&state, &project)?;
    let items = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        Ok(project.index().list(&project.name()))
    })
    .await?;
    Ok(Json(items))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/tasks",
    params(("project" = String, Path, description = "Project name")),
    request_body = CreateTask,
    responses(
        (status = 201, description = "Created", body = CreatedTask),
        (status = 400, description = "The task is invalid (unknown parent, dependency, or tag)", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The project has no tasks yet, or another writer kept moving them", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn create_task(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateTask>,
) -> Result<Response, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let created = op_task::now();
    let id = blocking(move || {
        let abbreviation = project.abbreviation()?;
        let task = body.into_task(created, abbreviation)?;
        let created = project.tracker().create_task(&actor, &task)?;
        project.written(created.committed.as_ref());
        Ok(abbreviation.format_key(created.number))
    })
    .await?;
    Ok((StatusCode::CREATED, Json(CreatedTask { id })).into_response())
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ReadQuery
    ),
    responses(
        (status = 200, description = "The task", body = TaskDetail),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<TaskDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let detail = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        let number = project.number(&id)?;
        project.detail(&id, number)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(
    patch,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    request_body = TaskPatch,
    responses(
        (status = 200, description = "The updated task", body = TaskDetail),
        (status = 400, description = "The patch is invalid (unknown parent, or a parent cycle)", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "Another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn patch_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(patch): Json<TaskPatch>,
) -> Result<Json<TaskDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let detail = blocking(move || {
        let abbreviation = project.abbreviation()?;
        let number = project.number(&id)?;
        let updated = project.tracker().update_task(&actor, number, |task| {
            patch
                .clone()
                .apply(task, abbreviation)
                .map_err(|err| TrackerError::Invalid(err.to_string()))
        })?;
        project.written(updated.committed.as_ref());
        project.detail(&id, number)
    })
    .await?;
    Ok(Json(detail))
}

// The whole file, as `openplan get` prints it. An agent or a person edits a task this way now that
// no task file sits in the checkout.
#[utoipa::path(
    put,
    path = "/api/projects/{project}/tasks/{id}/file",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    request_body = WriteTaskFile,
    responses(
        (status = 200, description = "The written task", body = TaskDetail),
        (status = 400, description = "The text is not a task file, drops a comment, or names an unknown task or tag", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "Another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn write_task_file(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<WriteTaskFile>,
) -> Result<Json<TaskDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let written = Task::from_file_string(&body.text)
        .map_err(|err| ApiError::bad_request(format!("not a task file: {err}")))?;
    let detail = blocking(move || {
        let number = project.number(&id)?;
        let updated = project.tracker().update_task(&actor, number, |task| {
            let kept = op_task::comment::parse(&task.body);
            let now = op_task::comment::parse(&written.body);
            if !now.starts_with(&kept) {
                return Err(TrackerError::Invalid(
                    "the comment log is append-only; keep every entry the task has and add new \
                     ones with `openplan comment`"
                        .to_owned(),
                ));
            }
            *task = written.clone();
            Ok(())
        })?;
        project.written(updated.committed.as_ref());
        project.detail(&id, number)
    })
    .await?;
    Ok(Json(detail))
}

// The web editor's save. The daemon merges the edit with what other writers changed since the
// editor read the body, so a save never drops their lines.
#[utoipa::path(
    put,
    path = "/api/projects/{project}/tasks/{id}/body",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    request_body = WriteBody,
    responses(
        (status = 200, description = "The task with the body written", body = TaskDetail),
        (status = 400, description = "The text adds a conflict or edits inside one, names a key that is not this store's, or has a second `# ` title", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "Another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn write_body(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<WriteBody>,
) -> Result<Json<TaskDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let detail = blocking(move || {
        let number = project.number(&id)?;
        let (base, text) = body.into_bodies(project.abbreviation()?)?;
        let updated = project.tracker().edit_body(&actor, number, &base, &text)?;
        project.written(updated.committed.as_ref());
        project.detail(&id, number)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/tasks/{id}/resolve",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    request_body = ResolveConflict,
    responses(
        (status = 200, description = "The task with the block resolved", body = TaskDetail),
        (status = 400, description = "The text adds a conflict", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 409, description = "The block is no longer in the task, or another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn resolve_conflict(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<ResolveConflict>,
) -> Result<Json<TaskDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let detail = blocking(move || {
        let number = project.number(&id)?;
        let updated = project
            .tracker()
            .resolve_block(&actor, number, &body.block, &body.text)?;
        project.written(updated.committed.as_ref());
        project.detail(&id, number)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/tasks/{id}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn delete_task(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    blocking(move || {
        let number = project.number(&id)?;
        let committed = project.tracker().delete_task(&actor, number)?;
        project.written(committed.as_ref());
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// The subtree rooted at one task. A detail page embeds only the direct children; this walks the
// whole task set.
#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/tree",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        TreeQuery
    ),
    responses(
        (status = 200, description = "The subtree rooted at the task", body = TaskTreeView),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_task_tree(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<TreeQuery>,
) -> Result<Json<TaskTreeView>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        project.number(&id)?;
        let summaries: Vec<TaskSummary> = project
            .index()
            .list(&project.name())
            .into_iter()
            .map(|row| TaskSummary {
                id: row.id,
                title: row.title,
                metadata: row.metadata,
            })
            .collect();
        let mut cycles = Vec::new();
        let tree = TaskTree::build(&summaries, &id, query.depth, &mut cycles)
            .ok_or_else(|| no_such_task(&id))?;
        let mut seen = std::collections::HashSet::new();
        cycles.retain(|id| seen.insert(id.clone()));
        Ok(TaskTreeView { tree, cycles })
    })
    .await?;
    Ok(Json(view))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/comments",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ReadQuery
    ),
    responses(
        (status = 200, description = "The log, oldest first", body = Vec<Comment>),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_comments(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<Vec<Comment>>, ApiError> {
    let project = project_of(&state, &project)?;
    let comments = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        let number = project.number(&id)?;
        project
            .index()
            .comments(number)
            .ok_or_else(|| no_such_task(&id))
    })
    .await?;
    Ok(Json(comments))
}

// The log is append-only, so this is its whole write surface. The daemon stamps the time, so one
// clock orders every entry; the caller carries the identity, because only the process that ran the
// command can see who ran it.
#[utoipa::path(
    post,
    path = "/api/projects/{project}/tasks/{id}/comments",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$")
    ),
    request_body = CreateComment,
    responses(
        (status = 201, description = "The appended entry", body = Comment),
        (status = 400, description = "The text or the author is empty", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such task", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn add_comment(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<CreateComment>,
) -> Result<Response, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let entry = new_comment(body)?;
    let written = Comment::from(&entry);
    blocking(move || {
        let number = project.number(&id)?;
        let committed = project.tracker().add_comment(&actor, number, &entry)?;
        project.written(committed.as_ref());
        Ok(())
    })
    .await?;
    Ok((StatusCode::CREATED, Json(written)).into_response())
}

fn new_comment(body: CreateComment) -> Result<op_task::comment::NewComment, ApiError> {
    if body.text.trim().is_empty() {
        return Err(ApiError::bad_request("a comment needs text"));
    }
    if body.author.trim().is_empty() {
        return Err(ApiError::bad_request(
            "a comment needs an author; an unsigned entry in an append-only log is worse than none",
        ));
    }
    // An entry heading is one line, and a line break there would let one write append entries no one
    // signed.
    let one_line = |field: &str, value: &str| match value.contains(['\n', '\r']) {
        true => Err(ApiError::bad_request(format!(
            "a comment's {field} is one line; this one holds a line break"
        ))),
        false => Ok(()),
    };
    one_line("author", &body.author)?;
    if let Some(agent) = &body.agent {
        one_line("agent", agent)?;
    }
    Ok(op_task::comment::NewComment {
        at: op_task::now(),
        author: body.author.trim().to_owned(),
        agent: body.agent.filter(|agent| !agent.trim().is_empty()),
        text: body.text.to_owned(),
    })
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/history",
    params(("project" = String, Path, description = "Project name"), PageQuery),
    responses(
        (status = 200, description = "The revisions of the project, newest first", body = Vec<HistoryEntry>),
        (status = 404, description = "No such project, or no such revision to page from", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn project_history(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(page): Query<PageQuery>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    let project = project_of(&state, &project)?;
    let entries = blocking(move || {
        let log = project.tracker().history(&history_query(page))?;
        history(&project, log)
    })
    .await?;
    Ok(Json(entries))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/history",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        PageQuery
    ),
    responses(
        (status = 200, description = "The revisions that changed the task, newest first", body = Vec<HistoryEntry>),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such revision to page from", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn task_history(
    State(state): State<AppState>,
    Path((project, id)): Path<(String, String)>,
    Query(page): Query<PageQuery>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    let project = project_of(&state, &project)?;
    let entries = blocking(move || {
        let number = project.number(&id)?;
        let log = project
            .tracker()
            .task_history(number, &history_query(page))?;
        history(&project, log)
    })
    .await?;
    Ok(Json(entries))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tasks/{id}/revisions/{revision}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("id" = String, Path, description = "Task key", pattern = "^[A-Z]{3}-(0|[1-9][0-9]*)$"),
        ("revision" = String, Path, description = "Revision id, as the history names it")
    ),
    responses(
        (status = 200, description = "The task as it stood at the revision", body = TaskAtRevision),
        (status = 400, description = "The key is invalid", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such revision", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn task_revision(
    State(state): State<AppState>,
    Path((project, id, revision)): Path<(String, String, String)>,
) -> Result<Json<TaskAtRevision>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = blocking(move || {
        let abbreviation = project.abbreviation()?;
        let number = project.number(&id)?;
        let raw = project
            .tracker()
            .task_at(number, &RevisionId::new(revision.clone()))?;
        Ok(TaskAtRevision {
            id,
            revision,
            task: raw.map(|raw| snapshot(&raw, abbreviation)),
        })
    })
    .await?;
    Ok(Json(view))
}

fn snapshot(raw: &str, abbreviation: Abbreviation) -> TaskSnapshot {
    let partial = op_task::parse_partial(raw);
    TaskSnapshot {
        title: partial.title.clone().unwrap_or_default(),
        metadata: Metadata::from_partial(partial.metadata, &partial.conflicts, abbreviation),
        comments: op_index::comments_of(&partial.body),
        body: op_task::comment::strip(&partial.body),
        raw: raw.to_owned(),
    }
}

fn history_query(page: PageQuery) -> HistoryQuery {
    HistoryQuery {
        before: page.before.map(RevisionId::new),
        limit: Some(page.limit.unwrap_or(HISTORY_PAGE)),
    }
}

// The index lock is not held while the revisions are read.
fn history(project: &Project, log: Vec<LogEntry>) -> Result<Vec<HistoryEntry>, ApiError> {
    let abbreviation = project.index().abbreviation();
    let key = |number: u64| match abbreviation {
        Some(abbreviation) => abbreviation.format_key(number),
        None => number.to_string(),
    };
    log.into_iter()
        .map(|entry| {
            let described = project.tracker().describe(&entry)?;
            Ok(HistoryEntry {
                revision: revision_view(&entry.revision),
                summary: described.lines(abbreviation),
                tasks: described
                    .tasks
                    .into_iter()
                    .map(|task| task_change(task, &key))
                    .collect(),
                tags: described.tags.into_iter().map(tag_change).collect(),
                changes: entry
                    .changes
                    .into_iter()
                    .map(|change| document_change(change, &key))
                    .collect(),
            })
        })
        .collect()
}

pub(crate) fn revision_view(revision: &op_backend::Revision) -> RevisionView {
    RevisionView {
        id: revision.id.to_string(),
        parents: revision.parents.iter().map(ToString::to_string).collect(),
        author: revision.author.name.clone(),
        email: revision.author.email.clone(),
        agent: revision.author.via.clone(),
        at: revision.at.into(),
        message: revision.message.clone(),
    }
}

fn document_change(change: Change, key: &dyn Fn(u64) -> String) -> DocumentChange {
    DocumentChange {
        task: layout::task_number(&change.path).map(key),
        tag: layout::tag_name(&change.path).map(str::to_owned),
        kind: change_kind(change.kind),
        path: change.path,
    }
}

fn change_kind(kind: ChangeKind) -> DocumentChangeKind {
    match kind {
        ChangeKind::Added => DocumentChangeKind::Added,
        ChangeKind::Modified => DocumentChangeKind::Modified,
        ChangeKind::Removed => DocumentChangeKind::Removed,
    }
}

fn task_change(task: op_tracker::TaskChange, key: &dyn Fn(u64) -> String) -> TaskChange {
    TaskChange {
        task: key(task.number),
        kind: change_kind(task.kind),
        title: task.title,
        fields: task
            .fields
            .into_iter()
            .map(|field| field_change(field, key))
            .collect(),
    }
}

fn field_change(field: op_tracker::FieldChange, key: &dyn Fn(u64) -> String) -> FieldChange {
    use op_tracker::FieldChange as Field;
    let keys = |numbers: Vec<u64>| numbers.into_iter().map(key).collect();
    match field {
        Field::Number { from, to } => FieldChange::Number {
            from: key(from),
            to: key(to),
        },
        Field::Status { from, to } => FieldChange::Status { from, to },
        Field::Parent { from, to } => FieldChange::Parent {
            from: from.map(key),
            to: to.map(key),
        },
        Field::Order => FieldChange::Order,
        Field::Dependencies { from, to } => FieldChange::Dependencies {
            from: keys(from),
            to: keys(to),
        },
        Field::Tags { from, to } => FieldChange::Tags { from, to },
        Field::Title { from, to } => FieldChange::Title { from, to },
        Field::Description => FieldChange::Description,
        Field::Comments { added, removed } => FieldChange::Comments { added, removed },
        Field::Conflicts { from, to } => FieldChange::Conflicts { from, to },
        Field::Other(name) => FieldChange::Other { name },
        Field::Frontmatter => FieldChange::Frontmatter,
    }
}

fn tag_change(tag: op_tracker::TagChange) -> TagChange {
    TagChange {
        tag: tag.name,
        kind: change_kind(tag.kind),
        renamed_from: tag.renamed_from,
    }
}

fn no_remote(project: &Project) -> ApiError {
    ApiError::not_found(format!(
        "project {} has no remote to sync with: its tasks are {}",
        project.name(),
        match project.kind() {
            op_api::BackendKind::Git => "on a git branch of a repository with no remote",
            op_api::BackendKind::Local => "in a local directory",
        }
    ))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/sync",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "How the last sync with the remote went", body = SyncView),
        (status = 404, description = "No such project, or the project has no remote", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_sync(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<SyncView>, ApiError> {
    let project = project_of(&state, &project)?;
    let status = project.sync_status().ok_or_else(|| no_remote(&project))?;
    Ok(Json(sync_view(&status)))
}

// Syncs now rather than at the next tick, and answers when the sync is done.
#[utoipa::path(
    post,
    path = "/api/projects/{project}/sync",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "The sync ran", body = SyncResult),
        (status = 404, description = "No such project, or the project has no remote", body = ApiErrorBody),
        (status = 502, description = "The remote could not be reached, or refused the push", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn run_sync(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<SyncResult>, ApiError> {
    let project = project_of(&state, &project)?;
    let result = blocking(move || {
        let backend = project.tracker().backend();
        let remote = backend.remote().ok_or_else(|| no_remote(&project))?;
        let report = remote.sync()?;
        project.catch_up();
        Ok(SyncResult {
            received: report.received,
            sent: report.sent,
            merged: report.merged,
            status: sync_view(&remote.status()),
        })
    })
    .await?;
    Ok(Json(result))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/board",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "Every task grouped by status and flattened into render-ordered rows", body = Board),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_board(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Board>, ApiError> {
    let project = project_of(&state, &project)?;
    let board = blocking(move || Ok(Board::build(&project.index().list(&project.name())))).await?;
    Ok(Json(board))
}

// The board of every project at once, which is what the UI opens on. A project that cannot answer
// contributes nothing, and `/api/projects` says why.
#[utoipa::path(
    get,
    path = "/api/board",
    responses(
        (status = 200, description = "Every task of every servable project, grouped by status and flattened into render-ordered rows", body = Board)
    )
)]
pub(crate) async fn get_merged_board(
    State(state): State<AppState>,
) -> Result<Json<Board>, ApiError> {
    let board = blocking(move || Ok(Board::build(&every_task(&state)))).await?;
    Ok(Json(board))
}

fn every_task(state: &AppState) -> Vec<TaskListItem> {
    let mut tasks = Vec::new();
    for project in state.projects() {
        if project.blocked().is_some() {
            continue;
        }
        tasks.append(&mut project.index().list(&project.name()));
    }
    tasks
}

// The implementation order of the tasks a query selects, across every project it names. Each
// parameter repeats; the repeats of one name are alternatives, and two names narrow each other.
pub(crate) async fn flow(
    state: &AppState,
    parameters: &[(String, String)],
) -> Result<Flow, ApiError> {
    let query = flow_query(parameters)?;
    let projects: Vec<Arc<Project>> = match query.projects.is_empty() {
        false => query
            .projects
            .iter()
            .map(|name| project_of(state, name))
            .collect::<Result<_, _>>()?,
        true => state
            .projects()
            .into_iter()
            .filter(|project| project.blocked().is_none())
            .collect(),
    };
    blocking(move || {
        let mut tasks = Vec::new();
        for project in projects {
            tasks.append(&mut project.index().list(&project.name()));
        }
        Ok(Flow::build(&tasks, &query)?)
    })
    .await
}

fn flow_query(parameters: &[(String, String)]) -> Result<FlowQuery, ApiError> {
    let mut query = FlowQuery::default();
    let mut keys = Vec::new();
    let once = |values: &mut Vec<String>, value: &String| {
        if !values.contains(value) {
            values.push(value.clone());
        }
    };
    for (name, value) in parameters {
        match name.as_str() {
            "project" => once(&mut query.projects, value),
            "status" => {
                let status = value
                    .parse::<Status>()
                    .map_err(|err| ApiError::bad_request(err.to_string()))?;
                if !query.statuses.contains(&status) {
                    query.statuses.push(status);
                }
            }
            "task" => once(&mut keys, value),
            "tag" => once(&mut query.tags, value),
            _ => {
                return Err(ApiError::bad_request(format!(
                    "unknown query parameter: {name}"
                )));
            }
        }
    }
    if !keys.is_empty() && query.projects.is_empty() {
        return Err(ApiError::bad_request(
            "a task parameter needs a project parameter: two projects can use the same \
             abbreviation, so a key alone names no task",
        ));
    }
    query.tasks = query
        .projects
        .iter()
        .flat_map(|project| {
            keys.iter()
                .map(|key| (project.clone(), key.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    Ok(query)
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/search",
    params(("project" = String, Path, description = "Project name"), SearchQuery),
    responses(
        (status = 200, description = "Every task of the project whose text contains the query", body = Vec<SearchHit>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn search_project(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let project = project_of(&state, &project)?;
    let hits = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        Ok(project.index().search(&project.name(), &query.q))
    })
    .await?;
    Ok(Json(hits))
}

// Search across every servable project, which is what the web palette asks: it opens over the
// merged board, so it searches the tasks that board shows.
#[utoipa::path(
    get,
    path = "/api/search",
    params(SearchQuery),
    responses(
        (status = 200, description = "Every task of every servable project whose text contains the query", body = Vec<SearchHit>)
    )
)]
pub(crate) async fn search_all(
    State(state): State<AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<Json<Vec<SearchHit>>, ApiError> {
    let hits = blocking(move || {
        let mut hits = Vec::new();
        for project in state.projects() {
            if project.blocked().is_some() {
                continue;
            }
            if query.fresh {
                project.fresh();
            }
            hits.append(&mut project.index().search(&project.name(), &query.q));
        }
        // Each index ranks its own hits, so the whole set is ranked again. Stable, so a rank keeps
        // the project order and the id order inside each project.
        hits.sort_by_key(|hit| hit.matched);
        Ok(hits)
    })
    .await?;
    Ok(Json(hits))
}

const TAG_NAME_PARAM: &str = "Tag name, or any spelling that normalizes to one";

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tags",
    params(("project" = String, Path, description = "Project name")),
    responses(
        (status = 200, description = "Every tag the project registers", body = Vec<TagView>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 422, description = "A tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_tags(
    State(state): State<AppState>,
    Path(project): Path<String>,
) -> Result<Json<Vec<TagView>>, ApiError> {
    let project = project_of(&state, &project)?;
    let tags = blocking(move || {
        Ok(project
            .tracker()
            .plan()?
            .tags()?
            .iter()
            .map(TagView::from)
            .collect())
    })
    .await?;
    Ok(Json(tags))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/tags",
    params(("project" = String, Path, description = "Project name")),
    request_body = CreateTag,
    responses(
        (status = 201, description = "Registered", body = TagView),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The name is already registered", body = ApiErrorBody),
        (status = 422, description = "The color is not a palette name", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn create_tag(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateTag>,
) -> Result<Response, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let view = blocking(move || {
        let tag = body
            .into_tag()
            .map_err(|err| ApiError::bad_request(err.to_string()))?;
        let tag = project.tracker().create_tag(&actor, &tag)?;
        Ok(TagView::from(&tag))
    })
    .await?;
    Ok((StatusCode::CREATED, Json(view)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM)
    ),
    responses(
        (status = 200, description = "The tag", body = TagView),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 422, description = "The tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
) -> Result<Json<TagView>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = blocking(move || Ok(TagView::from(&project.tracker().plan()?.tag(&name)?))).await?;
    Ok(Json(view))
}

// A rename moves the tag and every task that carries it in one revision.
#[utoipa::path(
    patch,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM)
    ),
    request_body = TagPatch,
    responses(
        (status = 200, description = "The updated tag", body = TagView),
        (status = 400, description = "The new name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 409, description = "The new name is already registered", body = ApiErrorBody),
        (status = 422, description = "The color is not a palette name, or the tag file is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn patch_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(patch): Json<TagPatch>,
) -> Result<Json<TagView>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let view = blocking(move || {
        let tracker = project.tracker();
        let name = match &patch.name {
            Some(display) => {
                let renamed = tracker.rename_tag(&actor, &name, display)?;
                project.written(renamed.committed.as_ref());
                renamed.value.0.name
            }
            None => name,
        };
        let tag = match patch.changes_content() {
            true => tracker.update_tag(&actor, &name, |tag| {
                patch.clone().apply(tag);
                Ok(())
            })?,
            false => tracker.plan()?.tag(&name)?,
        };
        Ok(TagView::from(&tag))
    })
    .await?;
    Ok(Json(view))
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/tags/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = TAG_NAME_PARAM),
        DeleteTagQuery
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 400, description = "The name cannot be normalized to a tag name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such tag", body = ApiErrorBody),
        (status = 409, description = "Tasks reference the tag (`reason: tag_referenced`, the one `force` answers)", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn delete_tag(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    headers: HeaderMap,
    Query(query): Query<DeleteTagQuery>,
) -> Result<StatusCode, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    blocking(move || {
        project.tracker().delete_tag(&actor, &name, query.force)?;
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
