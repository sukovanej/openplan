use std::sync::Arc;

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use op_api::{
    ApiErrorBody, CreateDoc, DocAtRevision, DocDetail, DocListItem, DocMetadata, DocPatch,
    DocSnapshot, FieldUpdate, HistoryEntry, WriteDocText,
};
use op_backend::RevisionId;
use op_task::Abbreviation;
use op_tracker::TrackerError;

use crate::tasks::{PageQuery, ReadQuery, history, history_query};
use crate::{ApiError, AppState, Project, actor_of, blocking, project_of};

const DOC_NAME_PARAM: &str = "Doc name (its file stem)";

impl Project {
    fn doc(&self, name: &str) -> Result<DocDetail, ApiError> {
        self.index()
            .doc_detail(&self.name(), name)
            .ok_or_else(|| no_such_doc(name))
    }
}

fn no_such_doc(name: &str) -> ApiError {
    ApiError::not_found(format!("no such doc: {name}"))
}

// The docs of every servable project in one read, which the docs page opens on. Each row carries
// its project, so two projects that name a doc the same stay two rows. A project that cannot answer
// drops out, as it drops off the merged board, unless the caller named it.
#[utoipa::path(
    get,
    path = "/api/docs",
    params(
        ("project" = Option<Vec<String>>, Query, description = "Project name; omit to take every project the daemon serves")
    ),
    responses(
        (status = 200, description = "Every doc of every named project", body = Vec<DocListItem>),
        (status = 400, description = "The query names an unknown parameter", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "A named project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_all_docs(
    State(state): State<AppState>,
    Query(parameters): Query<Vec<(String, String)>>,
) -> Result<Json<Vec<DocListItem>>, ApiError> {
    let named = doc_projects(&parameters)?;
    let projects: Vec<Arc<Project>> = match named.is_empty() {
        false => named
            .iter()
            .map(|name| project_of(&state, name))
            .collect::<Result<_, _>>()?,
        true => state
            .projects()
            .into_iter()
            .filter(|project| project.blocked().is_none())
            .collect(),
    };
    let docs = blocking(move || {
        let mut docs = Vec::new();
        for project in projects {
            docs.append(&mut project.index().list_docs(&project.name()));
        }
        // Two projects can name a doc the same, so the project breaks the tie.
        docs.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.project.cmp(&b.project)));
        Ok(docs)
    })
    .await?;
    Ok(Json(docs))
}

fn doc_projects(parameters: &[(String, String)]) -> Result<Vec<String>, ApiError> {
    let mut projects = Vec::new();
    for (name, value) in parameters {
        match name.as_str() {
            "project" if !projects.contains(value) => projects.push(value.clone()),
            "project" => {}
            _ => {
                return Err(ApiError::bad_request(format!(
                    "unknown query parameter: {name}"
                )));
            }
        }
    }
    Ok(projects)
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/docs",
    params(("project" = String, Path, description = "Project name"), ReadQuery),
    responses(
        (status = 200, description = "Every doc of the project", body = Vec<DocListItem>),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn list_docs(
    State(state): State<AppState>,
    Path(project): Path<String>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<Vec<DocListItem>>, ApiError> {
    let project = project_of(&state, &project)?;
    let docs = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        Ok(project.index().list_docs(&project.name()))
    })
    .await?;
    Ok(Json(docs))
}

#[utoipa::path(
    post,
    path = "/api/projects/{project}/docs",
    params(("project" = String, Path, description = "Project name")),
    request_body = CreateDoc,
    responses(
        (status = 201, description = "Created", body = DocDetail),
        (status = 400, description = "The name is not a doc name, or the parent does not exist", body = ApiErrorBody),
        (status = 404, description = "No such project", body = ApiErrorBody),
        (status = 409, description = "The name is taken, the project has no tasks yet, or another writer kept moving them", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn create_doc(
    State(state): State<AppState>,
    Path(project): Path<String>,
    headers: HeaderMap,
    Json(body): Json<CreateDoc>,
) -> Result<Response, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let created = op_task::now();
    let detail = blocking(move || {
        let doc = body
            .into_doc(created, project.abbreviation()?)
            .map_err(|err| ApiError::bad_request(err.to_string()))?;
        let created = project.tracker().create_doc(&actor, &doc)?;
        project.written(created.committed.as_ref());
        project.doc(&created.value.name)
    })
    .await?;
    Ok((StatusCode::CREATED, Json(detail)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/docs/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM),
        ReadQuery
    ),
    responses(
        (status = 200, description = "The doc", body = DocDetail),
        (status = 404, description = "No such project, or no such doc", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn get_doc(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    Query(query): Query<ReadQuery>,
) -> Result<Json<DocDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let detail = blocking(move || {
        if query.fresh {
            project.fresh();
        }
        project.doc(&name)
    })
    .await?;
    Ok(Json(detail))
}

// Every change in the patch is one revision: the body, the parent, and a rename, which moves the
// doc and every doc nested under it.
#[utoipa::path(
    patch,
    path = "/api/projects/{project}/docs/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM)
    ),
    request_body = DocPatch,
    responses(
        (status = 200, description = "The updated doc", body = DocDetail),
        (status = 400, description = "The new name is not a doc name, or the parent is unknown, is the doc itself, or closes a cycle", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such doc", body = ApiErrorBody),
        (status = 409, description = "The new name is taken, or another writer kept moving the tasks", body = ApiErrorBody),
        (status = 422, description = "The doc is stored in a form the daemon cannot read", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn patch_doc(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(patch): Json<DocPatch>,
) -> Result<Json<DocDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let detail = blocking(move || {
        let name = op_task::doc::normalize_name(&name)
            .map_err(|err| ApiError::bad_request(err.to_string()))?;
        if patch == DocPatch::default() {
            return project.doc(&name);
        }
        let invalid = |err: op_task::doc::ParseNameError| TrackerError::Invalid(err.to_string());
        let body = patch
            .body
            .as_deref()
            .map(|body| {
                op_api::body_from_keys(project.abbreviation()?, body).map_err(ApiError::from)
            })
            .transpose()?;
        let updated = project.tracker().update_doc(&actor, &name, |doc| {
            if let Some(body) = &body {
                doc.set_content(body);
            }
            match &patch.parent {
                FieldUpdate::Keep => {}
                FieldUpdate::Clear => doc.set_parent(None).map_err(invalid)?,
                FieldUpdate::Set(parent) => doc.set_parent(Some(parent)).map_err(invalid)?,
            }
            if let Some(display) = &patch.name {
                doc.rename(display).map_err(invalid)?;
            }
            Ok(())
        })?;
        project.written(updated.committed.as_ref());
        project.doc(&updated.value.name)
    })
    .await?;
    Ok(Json(detail))
}

// The web editor's save, as for a task: the daemon merges the edit with what other writers changed
// since the editor read the text. A new title renames the doc.
#[utoipa::path(
    put,
    path = "/api/projects/{project}/docs/{name}/text",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM)
    ),
    request_body = WriteDocText,
    responses(
        (status = 200, description = "The doc with the text written, under its new name when the title changed", body = DocDetail),
        (status = 400, description = "The title is empty or names no doc; the body adds a conflict or edits inside one, or names a key that is not this store's", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such doc", body = ApiErrorBody),
        (status = 409, description = "The new name is taken, or another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn write_doc_text(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    headers: HeaderMap,
    Json(body): Json<WriteDocText>,
) -> Result<Json<DocDetail>, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    let detail = blocking(move || {
        let (base, text) = body.into_texts(project.abbreviation()?)?;
        let updated = project
            .tracker()
            .edit_doc_text(&actor, &name, &base, &text)?;
        project.written(updated.committed.as_ref());
        project.doc(&updated.value.name)
    })
    .await?;
    Ok(Json(detail))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/docs/{name}/history",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM),
        PageQuery
    ),
    responses(
        (status = 200, description = "The revisions that changed the doc under this name, newest first", body = Vec<HistoryEntry>),
        (status = 400, description = "The name is not a doc name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such revision to page from", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn doc_history(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    Query(page): Query<PageQuery>,
) -> Result<Json<Vec<HistoryEntry>>, ApiError> {
    let project = project_of(&state, &project)?;
    let entries = blocking(move || {
        let (log, names): (Vec<_>, Vec<_>) = project
            .tracker()
            .doc_revisions(&name, &history_query(page))?
            .into_iter()
            .unzip();
        let mut entries = history(&project, log)?;
        // Each revision names the doc by the name it had then, so its line is the doc's own change.
        for (entry, name) in entries.iter_mut().zip(names) {
            entry.docs.retain(|change| change.doc == name);
        }
        Ok(entries)
    })
    .await?;
    Ok(Json(entries))
}

#[utoipa::path(
    get,
    path = "/api/projects/{project}/docs/{name}/revisions/{revision}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM),
        ("revision" = String, Path, description = "Revision id, as the history names it")
    ),
    responses(
        (status = 200, description = "The doc as it stood at the revision", body = DocAtRevision),
        (status = 400, description = "The name is not a doc name", body = ApiErrorBody),
        (status = 404, description = "No such project, or no such revision", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn doc_revision(
    State(state): State<AppState>,
    Path((project, name, revision)): Path<(String, String, String)>,
) -> Result<Json<DocAtRevision>, ApiError> {
    let project = project_of(&state, &project)?;
    let view = blocking(move || {
        let abbreviation = project.abbreviation()?;
        let raw = project
            .tracker()
            .doc_at(&name, &RevisionId::new(revision.clone()))?;
        Ok(DocAtRevision {
            name,
            revision,
            doc: raw.map(|raw| doc_snapshot(&raw, abbreviation)),
        })
    })
    .await?;
    Ok(Json(view))
}

fn doc_snapshot(raw: &str, abbreviation: Abbreviation) -> DocSnapshot {
    let partial = op_task::doc::parse_partial(raw);
    DocSnapshot {
        title: partial.title.clone().unwrap_or_default(),
        metadata: DocMetadata::from_partial(&partial.metadata, &partial.conflicts),
        body: op_api::body_to_keys(
            abbreviation,
            op_task::layout::DOCS,
            &op_task::doc::content(&partial.body),
        ),
        raw: raw.to_owned(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/projects/{project}/docs/{name}",
    params(
        ("project" = String, Path, description = "Project name"),
        ("name" = String, Path, description = DOC_NAME_PARAM)
    ),
    responses(
        (status = 204, description = "Deleted"),
        (status = 404, description = "No such project, or no such doc", body = ApiErrorBody),
        (status = 409, description = "Another writer kept moving the tasks", body = ApiErrorBody),
        (status = 503, description = "The project is registered but not being served", body = ApiErrorBody)
    )
)]
pub(crate) async fn delete_doc(
    State(state): State<AppState>,
    Path((project, name)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    let project = project_of(&state, &project)?;
    let actor = actor_of(&headers, &project);
    blocking(move || {
        let committed = project.tracker().delete_doc(&actor, &name)?;
        project.written(committed.as_ref());
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}
