//! Blueprints (`/blueprints`): a factory-game-style canvas for planning code.
//!
//! The owner keeps projects, each project holds plans, and a plan is a graph
//! of machines with typed ports joined by wires. Everyone else gets one
//! read-only demo plan, the site's own request flow. Plan and decisions:
//! `docs/manifests/2026-11-10-blueprints-editor.md`; design:
//! `docs/design/blueprint-planner/README.md`.
//!
//! **Who reaches what.** This is the security-relevant part:
//!
//! - `GET /blueprints` asks [`OptionalOwner`]: the owner gets their projects,
//!   everyone else (signed out, a plain member, a *granted admin*) gets the
//!   demo. Never [`crate::middleware::OptionalAdmin`], which says yes to any
//!   granted admin.
//! - Every other page names a project or plan id, and answers `404` to anyone
//!   but the owner, signed in or not. Never `403`, never a login redirect:
//!   both would confirm the id exists.
//! - Every write is [`RequireOwner`]. A signed-out write is sent to log in,
//!   a signed-in non-owner gets `404`.
//! - Ids come from the path, and every query in `db.rs` is scoped to the
//!   owner's user id as well, so an id that is not theirs behaves exactly like
//!   one that does not exist.
//!
//! The demo is a committed file loaded with `include_str!`. It is never
//! seeded into the database, so it cannot be edited by anyone.

use crate::{
    blueprint_doc::{self, Outline, Summary},
    db,
    middleware::{OptionalAdmin, OptionalOwner, RequireOwner},
    models::{
        BlueprintNameOutcome, BlueprintPlanSummary, BlueprintProject, BlueprintSaveOutcome, UserId,
    },
    AppState,
};
use askama::Template;
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    routing::{get, post, put},
    Form, Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

/// The visitor demo: the site's own request flow. Committed once and never
/// seeded, so nothing can write to it.
const DEMO_PLAN: &str = include_str!("../../docs/design/blueprint-planner/demo-plan.json");

/// Makes JSON safe inside `<script type="application/json">`. The browser
/// never runs it, but it does look for `</script` to end the element, and port
/// types are full of angle brackets (`Vec<Post>`). The escapes are still the
/// same string once `JSON.parse` reads them.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

fn not_found() -> Response {
    StatusCode::NOT_FOUND.into_response()
}

/// A path id. Anything that is not a positive number is a 404 like any other
/// id that does not exist, rather than the 400 a typed `Path<i64>` would give.
fn path_id(s: &str) -> Option<i64> {
    s.parse::<i64>().ok().filter(|n| *n > 0)
}

// ── The rail ────────────────────────────────────────────────────────────────

/// One project in the left rail, with its plans when it is the open one.
pub struct RailProject {
    pub id: i64,
    pub name: String,
    pub count: i64,
    pub open: bool,
    /// The project's own page is the one showing (not one of its plans).
    pub current: bool,
    pub plans: Vec<RailPlan>,
}

pub struct RailPlan {
    pub id: i64,
    pub name: String,
    pub rev: i64,
    pub current: bool,
}

async fn rail(
    state: &AppState,
    owner: UserId,
    open_project: Option<i64>,
    current_plan: Option<i64>,
) -> Vec<RailProject> {
    let projects = db::list_blueprint_projects(&state.pool, owner).await;
    let plans = db::list_blueprint_plans(&state.pool, owner).await;
    projects
        .into_iter()
        .map(|p| {
            let open = Some(p.id) == open_project;
            RailProject {
                plans: plans
                    .iter()
                    .filter(|pl| open && pl.project_id == p.id)
                    .map(|pl| RailPlan {
                        id: pl.id,
                        name: pl.name.clone(),
                        rev: pl.rev,
                        current: Some(pl.id) == current_plan,
                    })
                    .collect(),
                id: p.id,
                name: p.name,
                count: p.plans,
                current: open && current_plan.is_none(),
                open,
            }
        })
        .collect()
}

/// What a create or rename that went wrong says, picked by the `?error=`
/// code the redirect carries. A code, not the sentence, so the URL cannot be
/// made to print arbitrary text on the page.
fn error_text(code: Option<&str>) -> &'static str {
    match code {
        Some("name") => "A name is lower case letters, digits and dashes, up to 64 characters.",
        Some("taken") => "That name is already taken here.",
        _ => "",
    }
}

#[derive(Deserialize, Default)]
pub struct PageQuery {
    /// `?visitor=1`: the owner previews what everyone else sees.
    visitor: Option<String>,
    error: Option<String>,
}

// ── Templates ───────────────────────────────────────────────────────────────

/// The read-only demo every visitor gets.
#[derive(Template)]
#[template(path = "blueprint/demo.html")]
struct DemoTemplate {
    /// base.html reads this for the header's admin button.
    is_admin: bool,
    /// The owner is previewing the demo: offer the way back.
    preview: bool,
    /// Always false: the keys help lists only what the demo can do.
    editable: bool,
    doc_json: String,
    outline: Outline,
}

/// The owner's landing page: the rail and the new-project form.
#[derive(Template)]
#[template(path = "blueprint/home.html")]
struct HomeTemplate {
    is_admin: bool,
    rail: Vec<RailProject>,
    error: &'static str,
}

#[derive(Template)]
#[template(path = "blueprint/project.html")]
struct ProjectTemplate {
    is_admin: bool,
    rail: Vec<RailProject>,
    project: BlueprintProject,
    plans: Vec<BlueprintPlanSummary>,
    error: &'static str,
}

#[derive(Template)]
#[template(path = "blueprint/plan.html")]
struct PlanTemplate {
    is_admin: bool,
    rail: Vec<RailProject>,
    project: BlueprintProject,
    plan_name: String,
    rev: i64,
    /// Whether the canvas edits and saves: always, for the owner's own plan.
    /// The template keeps the flag so the keys help and the toolbar can say
    /// what this page can do.
    editable: bool,
    /// Where the editor PUTs the document.
    save_url: String,
    /// Where the editor fetches the outline from after a save.
    outline_url: String,
    summary: Summary,
    doc_json: String,
    outline: Outline,
}

// ── Pages ───────────────────────────────────────────────────────────────────

fn demo_page(is_admin: bool, preview: bool) -> Response {
    let doc = blueprint_doc::parse(DEMO_PLAN).expect("the committed demo plan is valid");
    Html(
        DemoTemplate {
            is_admin,
            preview,
            editable: false,
            doc_json: script_safe(DEMO_PLAN),
            outline: blueprint_doc::outline(&doc),
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

async fn index(
    OptionalOwner(owner): OptionalOwner,
    OptionalAdmin(is_admin): OptionalAdmin,
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> Response {
    let Some(owner) = owner.filter(|_| q.visitor.is_none()) else {
        return demo_page(is_admin, owner.is_some());
    };
    Html(
        HomeTemplate {
            is_admin: true,
            rail: rail(&state, owner, None, None).await,
            error: error_text(q.error.as_deref()),
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

async fn project_page(
    OptionalOwner(owner): OptionalOwner,
    State(state): State<Arc<AppState>>,
    Path(project): Path<String>,
    Query(q): Query<PageQuery>,
) -> Response {
    let (Some(owner), Some(pid)) = (owner, path_id(&project)) else {
        return not_found();
    };
    let Some(project) = db::get_blueprint_project(&state.pool, pid, owner).await else {
        return not_found();
    };
    let plans = db::list_blueprint_plans(&state.pool, owner)
        .await
        .into_iter()
        .filter(|p| p.project_id == pid)
        .collect();
    Html(
        ProjectTemplate {
            is_admin: true,
            rail: rail(&state, owner, Some(pid), None).await,
            project,
            plans,
            error: error_text(q.error.as_deref()),
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

async fn plan_page(
    OptionalOwner(owner): OptionalOwner,
    State(state): State<Arc<AppState>>,
    Path((project, plan)): Path<(String, String)>,
) -> Response {
    let (Some(owner), Some(pid), Some(plid)) = (owner, path_id(&project), path_id(&plan)) else {
        return not_found();
    };
    let (Some(project), Some(plan)) = (
        db::get_blueprint_project(&state.pool, pid, owner).await,
        db::get_blueprint_plan(&state.pool, pid, plid, owner).await,
    ) else {
        return not_found();
    };
    // A stored plan always passed validation on its way in. If one somehow
    // does not parse, the page still opens on an empty outline rather than
    // failing, and the canvas shows what the model makes of it.
    let doc = blueprint_doc::parse(&plan.payload)
        .or_else(|_| blueprint_doc::parse(blueprint_doc::EMPTY_PLAN))
        .expect("the empty plan is valid");
    let outline = blueprint_doc::outline(&doc);
    Html(
        PlanTemplate {
            is_admin: true,
            rail: rail(&state, owner, Some(pid), Some(plid)).await,
            project,
            plan_name: plan.name,
            rev: plan.rev,
            editable: true,
            save_url: format!("/api/blueprints/{pid}/{plid}"),
            outline_url: format!("/blueprints/{pid}/{plid}/outline"),
            summary: outline.summary,
            doc_json: script_safe(&plan.payload),
            outline,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

// ── Writes: projects and plans ──────────────────────────────────────────────
//
// Plain forms, answered with a 303 to the page that shows the result. A
// boosted form follows that like a link. A name that is refused goes back to
// the form's page with an `?error=` code, never a 4xx: htmx does not swap a
// 4xx, so the owner would see nothing happen.

#[derive(Deserialize)]
pub struct NameForm {
    name: String,
}

fn back(to: &str, outcome: BlueprintNameOutcome, ok: impl FnOnce(i64) -> String) -> Response {
    match outcome {
        BlueprintNameOutcome::Done(id) => Redirect::to(&ok(id)).into_response(),
        BlueprintNameOutcome::Taken => Redirect::to(&format!("{to}?error=taken")).into_response(),
        BlueprintNameOutcome::Missing => not_found(),
    }
}

async fn create_project(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Form(form): Form<NameForm>,
) -> Response {
    let name = form.name.trim();
    if !blueprint_doc::is_slug(name) {
        return Redirect::to("/blueprints?error=name").into_response();
    }
    let outcome = db::create_blueprint_project(&state.pool, owner, name).await;
    back("/blueprints", outcome, |id| format!("/blueprints/{id}"))
}

async fn rename_project(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(project): Path<String>,
    Form(form): Form<NameForm>,
) -> Response {
    let Some(pid) = path_id(&project) else {
        return not_found();
    };
    let here = format!("/blueprints/{pid}");
    let name = form.name.trim();
    if !blueprint_doc::is_slug(name) {
        // Only a project that is the owner's gets a page to go back to.
        return match db::get_blueprint_project(&state.pool, pid, owner).await {
            Some(_) => Redirect::to(&format!("{here}?error=name")).into_response(),
            None => not_found(),
        };
    }
    let outcome = db::rename_blueprint_project(&state.pool, pid, owner, name).await;
    back(&here, outcome, |_| here.clone())
}

async fn delete_project(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(project): Path<String>,
) -> Response {
    let Some(pid) = path_id(&project) else {
        return not_found();
    };
    if db::delete_blueprint_project(&state.pool, pid, owner).await {
        Redirect::to("/blueprints").into_response()
    } else {
        not_found()
    }
}

async fn create_plan(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path(project): Path<String>,
    Form(form): Form<NameForm>,
) -> Response {
    let Some(pid) = path_id(&project) else {
        return not_found();
    };
    let here = format!("/blueprints/{pid}");
    let name = form.name.trim();
    if !blueprint_doc::is_slug(name) {
        return match db::get_blueprint_project(&state.pool, pid, owner).await {
            Some(_) => Redirect::to(&format!("{here}?error=name")).into_response(),
            None => not_found(),
        };
    }
    let outcome =
        db::create_blueprint_plan(&state.pool, pid, owner, name, blueprint_doc::EMPTY_PLAN).await;
    back(&here, outcome, |id| format!("/blueprints/{pid}/{id}"))
}

async fn rename_plan(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path((project, plan)): Path<(String, String)>,
    Form(form): Form<NameForm>,
) -> Response {
    let (Some(pid), Some(plid)) = (path_id(&project), path_id(&plan)) else {
        return not_found();
    };
    let list = format!("/blueprints/{pid}");
    let name = form.name.trim();
    if !blueprint_doc::is_slug(name) {
        return match db::get_blueprint_plan(&state.pool, pid, plid, owner).await {
            Some(_) => Redirect::to(&format!("{list}?error=name")).into_response(),
            None => not_found(),
        };
    }
    let outcome = db::rename_blueprint_plan(&state.pool, pid, plid, owner, name).await;
    back(&list, outcome, |_| list.clone())
}

async fn delete_plan(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path((project, plan)): Path<(String, String)>,
) -> Response {
    let (Some(pid), Some(plid)) = (path_id(&project), path_id(&plan)) else {
        return not_found();
    };
    if db::delete_blueprint_plan(&state.pool, pid, plid, owner).await {
        Redirect::to(&format!("/blueprints/{pid}")).into_response()
    } else {
        not_found()
    }
}

// ── The editor: the outline fragment and the save ───────────────────────────

/// The outline on its own, for the editor to swap in after a save.
#[derive(Template)]
#[template(path = "blueprint/partials/outline.html")]
struct OutlineTemplate {
    outline: Outline,
}

async fn outline_fragment(
    OptionalOwner(owner): OptionalOwner,
    State(state): State<Arc<AppState>>,
    Path((project, plan)): Path<(String, String)>,
) -> Response {
    let (Some(owner), Some(pid), Some(plid)) = (owner, path_id(&project), path_id(&plan)) else {
        return not_found();
    };
    let Some(plan) = db::get_blueprint_plan(&state.pool, pid, plid, owner).await else {
        return not_found();
    };
    let Ok(doc) = blueprint_doc::parse(&plan.payload) else {
        return not_found();
    };
    Html(
        OutlineTemplate {
            outline: blueprint_doc::outline(&doc),
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

/// The rev an `If-Match` header names: `14`, `"14"` or `W/"14"`. Anything
/// else, `*` included, is no rev at all, and the save is refused with 428.
fn if_match(headers: &HeaderMap) -> Option<i64> {
    let raw = headers.get(header::IF_MATCH)?.to_str().ok()?.trim();
    let raw = raw.strip_prefix("W/").unwrap_or(raw);
    let raw = raw
        .strip_prefix('"')
        .and_then(|r| r.strip_suffix('"'))
        .unwrap_or(raw);
    raw.parse::<i64>().ok().filter(|n| *n > 0)
}

fn json_error(status: StatusCode, errors: Vec<String>) -> Response {
    (status, Json(serde_json::json!({ "errors": errors }))).into_response()
}

fn with_etag(status: StatusCode, rev: i64, body: serde_json::Value) -> Response {
    (status, [(header::ETAG, format!("\"{rev}\""))], Json(body)).into_response()
}

/// `PUT /api/blueprints/{project}/{plan}`: the whole document, as text.
///
/// - `If-Match: <rev>` is required (428 without it). The save is one
///   compare-and-set in `db::save_blueprint_plan`: a stale rev changes nothing
///   and gets `412` with the current rev, and the page shows its "changed
///   elsewhere" banner. There is no last-write-wins.
/// - A document that cannot be read is refused with `400` and every problem.
///   Dangling wires and mismatches are counted in the answer, never refused.
/// - The text is stored exactly as sent: key order is a port's row.
async fn save_plan(
    RequireOwner(owner): RequireOwner,
    State(state): State<Arc<AppState>>,
    Path((project, plan)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let (Some(pid), Some(plid)) = (path_id(&project), path_id(&plan)) else {
        return not_found();
    };
    if body.len() > blueprint_doc::MAX_PAYLOAD {
        return json_error(
            StatusCode::PAYLOAD_TOO_LARGE,
            vec!["the plan is larger than 1 MB".into()],
        );
    }
    let Some(if_rev) = if_match(&headers) else {
        return json_error(
            StatusCode::PRECONDITION_REQUIRED,
            vec!["send If-Match with the rev this edit started from".into()],
        );
    };
    let Ok(text) = std::str::from_utf8(&body) else {
        return json_error(
            StatusCode::BAD_REQUEST,
            vec!["the plan is not UTF-8".into()],
        );
    };
    let doc = match blueprint_doc::parse(text) {
        Ok(doc) => doc,
        Err(errors) => return json_error(StatusCode::BAD_REQUEST, errors),
    };
    let summary = blueprint_doc::summarize(&doc);
    match db::save_blueprint_plan(&state.pool, pid, plid, owner, text, if_rev).await {
        BlueprintSaveOutcome::Saved(rev) => with_etag(
            StatusCode::OK,
            rev,
            serde_json::json!({ "rev": rev, "summary": summary }),
        ),
        BlueprintSaveOutcome::Stale(rev) => with_etag(
            StatusCode::PRECONDITION_FAILED,
            rev,
            serde_json::json!({ "rev": rev, "errors": ["this plan changed elsewhere"] }),
        ),
        BlueprintSaveOutcome::Missing => not_found(),
    }
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/blueprints", get(index).post(create_project))
        .route("/blueprints/{project}", get(project_page))
        .route("/blueprints/{project}/rename", post(rename_project))
        .route("/blueprints/{project}/delete", post(delete_project))
        .route("/blueprints/{project}/plans", post(create_plan))
        .route("/blueprints/{project}/{plan}", get(plan_page))
        .route("/blueprints/{project}/{plan}/rename", post(rename_plan))
        .route("/blueprints/{project}/{plan}/delete", post(delete_plan))
        .route(
            "/blueprints/{project}/{plan}/outline",
            get(outline_fragment),
        )
        .route(
            "/api/blueprints/{project}/{plan}",
            // A little over the cap, so an oversized plan reaches the handler
            // and gets its JSON 413 rather than the extractor's plain one.
            put(save_plan).layer(DefaultBodyLimit::max(blueprint_doc::MAX_PAYLOAD + 1024)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

    async fn app_with_pool() -> (Router, crate::db::DbPool) {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        let storage = crate::storage::ObjectStorage::from_env().await;
        let rp_origin = url::Url::parse("http://localhost:3000").unwrap();
        let webauthn = webauthn_rs::prelude::WebauthnBuilder::new("localhost", &rp_origin)
            .unwrap()
            .build()
            .unwrap();
        let state = Arc::new(crate::AppState {
            pool: pool.clone(),
            storage,
            webauthn,
        });
        (router().with_state(state), pool)
    }

    /// The owner, a granted admin and a plain member, each signed in, plus a
    /// project with one plan that belongs to the owner.
    struct World {
        app: Router,
        pool: crate::db::DbPool,
        owner: UserId,
        pid: i64,
        plid: i64,
    }

    const OWNER: &str = "session=owner-sess";
    const ADMIN: &str = "session=admin-sess";
    const MEMBER: &str = "session=member-sess";

    async fn world() -> World {
        let (app, pool) = app_with_pool().await;
        let owner = UserId(db::get_owner_user_id(&pool).await.unwrap());
        db::create_session(&pool, "owner-sess", "2099-01-01 00:00:00", owner.get()).await;
        let alex = db::create_user(&pool, "alex", "hash").await.unwrap();
        db::set_user_admin(&pool, alex, true).await;
        db::create_session(&pool, "admin-sess", "2099-01-01 00:00:00", alex).await;
        let sam = db::create_user(&pool, "sam", "hash").await.unwrap();
        db::create_session(&pool, "member-sess", "2099-01-01 00:00:00", sam).await;
        let BlueprintNameOutcome::Done(pid) =
            db::create_blueprint_project(&pool, owner, "drawingportfolio").await
        else {
            panic!("no project")
        };
        let BlueprintNameOutcome::Done(plid) =
            db::create_blueprint_plan(&pool, pid, owner, "request-flow", DEMO_PLAN).await
        else {
            panic!("no plan")
        };
        World {
            app,
            pool,
            owner,
            pid,
            plid,
        }
    }

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut b = Request::builder().method("GET").uri(uri);
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        b.body(Body::empty()).unwrap()
    }

    fn post(uri: &str, cookie: Option<&str>, form: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded");
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        b.body(Body::from(form.to_string())).unwrap()
    }

    async fn send(app: &Router, req: Request<Body>) -> (StatusCode, String, String) {
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let location = resp
            .headers()
            .get("location")
            .map(|v| v.to_str().unwrap().to_string())
            .unwrap_or_default();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, location, String::from_utf8(bytes.to_vec()).unwrap())
    }

    // ── The demo and its template ───────────────────────────────────────────

    #[test]
    fn test_demo_plan_is_the_documented_shape() {
        let doc: serde_json::Value = serde_json::from_str(DEMO_PLAN).expect("demo plan is JSON");
        assert_eq!(doc["v"], 1);
        assert_eq!(doc["root"], "main");
        let main = &doc["graphs"]["main"];
        assert_eq!(main["nodes"].as_object().unwrap().len(), 8);
        assert_eq!(main["wires"].as_array().unwrap().len(), 12);
        assert_eq!(main["nodes"]["db"]["kind"], "blueprint");
        assert!(doc["graphs"]["db"]["nodes"].is_object());
        let layout = doc["layout"].as_object().unwrap();
        for (g, graph) in doc["graphs"].as_object().unwrap() {
            for name in graph["nodes"].as_object().unwrap().keys() {
                assert!(
                    layout.contains_key(&format!("{g}/{name}")),
                    "{g}/{name} has no position"
                );
            }
        }
        // It must pass the server's own validator, dangling wire and all.
        assert!(blueprint_doc::parse(DEMO_PLAN).is_ok());
    }

    #[test]
    fn test_script_safe_leaves_no_angle_bracket_and_round_trips() {
        let safe = script_safe(DEMO_PLAN);
        assert!(!safe.contains('<') && !safe.contains('>'));
        assert!(DEMO_PLAN.contains("Vec<Post>"), "positive control");
        let a: serde_json::Value = serde_json::from_str(DEMO_PLAN).unwrap();
        let b: serde_json::Value = serde_json::from_str(&safe).unwrap();
        assert_eq!(a, b, "escaping changed what the plan says");
    }

    /// A plan name or note that tries to end the script element cannot.
    #[test]
    fn test_script_safe_holds_against_a_closing_tag_in_the_plan() {
        let hostile = r#"{"note":"</script><script>alert(1)</script>"}"#;
        assert!(!script_safe(hostile).to_lowercase().contains("</script"));
    }

    /// The element ids the scripts look up, against the templates that carry
    /// them. A renamed id does not throw: the canvas just stays blank.
    #[test]
    fn test_templates_and_scripts_agree_on_their_ids() {
        const APP: &str = include_str!("../../static/blueprint-app.js");
        const CANVAS: &str = include_str!("../../templates/blueprint/partials/canvas.html");
        const DEMO: &str = include_str!("../../templates/blueprint/demo.html");
        const PLAN: &str = include_str!("../../templates/blueprint/plan.html");
        for id in ["bp-app", "bp-doc", "bp-counts", "bp-zoom", "bp-live"] {
            assert!(
                APP.contains(&format!("'#{id}'")) || APP.contains(&format!("'{id}'")),
                "app.js no longer reads #{id}"
            );
            let in_page = |t: &str| t.contains(&format!(r#"id="{id}""#));
            assert!(in_page(DEMO) || in_page(CANVAS), "the demo lost #{id}");
            assert!(in_page(PLAN) || in_page(CANVAS), "the plan page lost #{id}");
        }
        // The editor's own ids, read by blueprint-app.js or blueprint-inspector.js.
        const INSPECTOR: &str = include_str!("../../static/blueprint-inspector.js");
        for id in [
            "bp-save",
            "bp-save-text",
            "bp-empty",
            "bp-banner",
            "bp-banner-detail",
            "bp-reload",
            "bp-inspector",
            "bp-outline-wrap",
            "bp-pop",
            "bp-pop-name",
            "bp-pop-label",
            "bp-pop-items",
            "bp-pop-error",
        ] {
            assert!(
                APP.contains(&format!("'#{id}'")) || INSPECTOR.contains(&format!("'#{id}'")),
                "no script reads #{id}"
            );
            assert!(
                PLAN.contains(&format!(r#"id="{id}""#)),
                "the plan page lost #{id}"
            );
        }
        for class in [
            "bp-viewport",
            "bp-stage",
            "bp-wires",
            "bp-nodes",
            "bp-ghost",
            "bp-box",
        ] {
            assert!(CANVAS.contains(class), "the canvas lost .{class}");
        }
    }

    // ── Who gets what at /blueprints ────────────────────────────────────────

    #[tokio::test]
    async fn test_a_visitor_gets_the_read_only_demo() {
        let w = world().await;
        let (status, _, html) = send(&w.app, get("/blueprints", None)).await;
        assert_eq!(status, StatusCode::OK);
        // base.html derives body.site-dark from this marker; it must be the
        // outermost element of the content block.
        assert!(html.contains(
            r#"<div class="site-page bp-page bp-page--demo" id="bp-app" data-mode="demo""#
        ));
        assert!(html.contains(r#"<script type="application/json" id="bp-doc">"#));
        assert!(html.contains("token_gate"), "the demo plan is in the page");
        // The outline is server-rendered: it is there without JavaScript.
        assert!(html.contains(r#"id="bp-outline""#));
        assert!(html.contains("11 machines · 14 wires · 1 dangling · 1 mismatch"));
        assert!(html.contains("mismatch: Request ≠ Header"));
        // Read-only: no form, no edit control, no owner data.
        assert!(!html.contains("<form"), "the demo carries a form");
        assert!(!html.contains("data-bp-action=\"add\""));
        assert!(!html.contains("bp-rail"));
        assert!(
            !html.contains("drawingportfolio</"),
            "an owner project name leaked"
        );
        for js in [
            "blueprint-model.js",
            "blueprint-view.js",
            "blueprint-app.js",
        ] {
            assert!(
                html.contains(&format!("/static/{js}?v=")),
                "{js} is not loaded"
            );
        }
    }

    /// The case `OptionalOwner` exists for: a granted admin is not the owner
    /// and must get the demo, not the owner's projects.
    #[tokio::test]
    async fn test_an_admin_and_a_member_get_the_demo_not_the_projects() {
        let w = world().await;
        for cookie in [ADMIN, MEMBER] {
            let (status, _, html) = send(&w.app, get("/blueprints", Some(cookie))).await;
            assert_eq!(status, StatusCode::OK, "{cookie}");
            assert!(
                html.contains(r#"data-mode="demo""#),
                "{cookie} did not get the demo"
            );
            assert!(!html.contains("bp-rail"), "{cookie} got the projects rail");
            assert!(
                !html.contains("/blueprints/"),
                "{cookie} got a link to a project"
            );
        }
    }

    #[tokio::test]
    async fn test_the_owner_gets_their_projects() {
        let w = world().await;
        let (status, _, html) = send(&w.app, get("/blueprints", Some(OWNER))).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains(r#"class="site-page bp-page bp-page--owner""#));
        assert!(html.contains("drawingportfolio"));
        assert!(html.contains(&format!(r#"href="/blueprints/{}""#, w.pid)));
        assert!(
            html.contains(r#"action="/blueprints""#),
            "the new-project form"
        );
        assert!(!html.contains(r#"data-mode="demo""#));
    }

    #[tokio::test]
    async fn test_the_owner_can_preview_the_visitor_demo() {
        let w = world().await;
        let (_, _, html) = send(&w.app, get("/blueprints?visitor=1", Some(OWNER))).await;
        assert!(html.contains(r#"data-mode="demo""#));
        assert!(html.contains("Back to your projects"));
        // A visitor asking for the preview gets the plain demo, no back link.
        let (_, _, html) = send(&w.app, get("/blueprints?visitor=1", None)).await;
        assert!(!html.contains("Back to your projects"));
    }

    // ── Any id: a 404 for everyone but the owner ────────────────────────────

    #[tokio::test]
    async fn test_project_and_plan_pages_are_404_for_everyone_but_the_owner() {
        let w = world().await;
        let project = format!("/blueprints/{}", w.pid);
        let plan = format!("/blueprints/{}/{}", w.pid, w.plid);
        for uri in [&project, &plan] {
            for cookie in [None, Some(ADMIN), Some(MEMBER), Some("session=nonsense")] {
                let (status, location, body) = send(&w.app, get(uri, cookie)).await;
                assert_eq!(status, StatusCode::NOT_FOUND, "{uri} as {cookie:?}");
                assert!(location.is_empty(), "{uri} as {cookie:?} redirected");
                assert!(
                    !body.contains("request-flow"),
                    "{uri} as {cookie:?} leaked the plan"
                );
            }
            // Positive control: the owner does reach it.
            let (status, _, _) = send(&w.app, get(uri, Some(OWNER))).await;
            assert_eq!(status, StatusCode::OK, "{uri} as the owner");
        }
    }

    #[tokio::test]
    async fn test_ids_that_are_not_the_owners_are_404_even_for_the_owner() {
        let w = world().await;
        // Another user's project, written straight to the database.
        let sam = UserId(2 + w.owner.get()); // not the owner; the row needs no user
        let BlueprintNameOutcome::Done(theirs) =
            db::create_blueprint_project(&w.pool, sam, "theirs").await
        else {
            panic!()
        };
        let BlueprintNameOutcome::Done(their_plan) =
            db::create_blueprint_plan(&w.pool, theirs, sam, "secret", DEMO_PLAN).await
        else {
            panic!()
        };
        let other = db::create_blueprint_project(&w.pool, w.owner, "other").await;
        let BlueprintNameOutcome::Done(other) = other else {
            panic!()
        };
        for uri in [
            format!("/blueprints/{theirs}"),
            format!("/blueprints/{theirs}/{their_plan}"),
            // The owner's plan under the owner's other project.
            format!("/blueprints/{other}/{}", w.plid),
            "/blueprints/999999".to_string(),
            format!("/blueprints/{}/999999", w.pid),
            "/blueprints/abc".to_string(),
            "/blueprints/0".to_string(),
            "/blueprints/-1".to_string(),
            format!("/blueprints/{}/1e3", w.pid),
        ] {
            let (status, _, _) = send(&w.app, get(&uri, Some(OWNER))).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
        }
        // The rail never lists another user's project either.
        let (_, _, html) = send(&w.app, get("/blueprints", Some(OWNER))).await;
        assert!(!html.contains("theirs"));
    }

    #[tokio::test]
    async fn test_the_plan_page_carries_the_plan_and_its_outline() {
        let w = world().await;
        let (status, _, html) = send(
            &w.app,
            get(&format!("/blueprints/{}/{}", w.pid, w.plid), Some(OWNER)),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains(&format!(
            r#"id="bp-app" data-mode="edit" data-rev="1" data-save="/api/blueprints/{0}/{1}" data-outline="/blueprints/{0}/{1}/outline""#,
            w.pid, w.plid
        )));
        // The editor's pieces, and the banner, which starts hidden.
        assert!(html.contains(r#"id="bp-banner" role="alert" hidden"#));
        assert!(html.contains(r#"id="bp-inspector""#));
        assert!(html.contains("/static/blueprint-inspector.js?v="));
        assert!(html.contains(r#"<script type="application/json" id="bp-doc">"#));
        assert!(html.contains("11 machines · 14 wires · 1 dangling · 1 mismatch"));
        assert!(
            html.contains(r#"aria-current="page""#),
            "the rail marks the open plan"
        );
        assert!(html.contains("<title>request-flow · Blueprints</title>"));
    }

    // ── Writes ──────────────────────────────────────────────────────────────

    async fn names(pool: &crate::db::DbPool, owner: UserId) -> Vec<String> {
        db::list_blueprint_projects(pool, owner)
            .await
            .into_iter()
            .map(|p| p.name)
            .collect()
    }

    #[tokio::test]
    async fn test_writes_send_a_visitor_to_log_in_and_404_everyone_else() {
        let w = world().await;
        let writes = [
            "/blueprints".to_string(),
            format!("/blueprints/{}/rename", w.pid),
            format!("/blueprints/{}/delete", w.pid),
            format!("/blueprints/{}/plans", w.pid),
            format!("/blueprints/{}/{}/rename", w.pid, w.plid),
            format!("/blueprints/{}/{}/delete", w.pid, w.plid),
        ];
        for uri in &writes {
            let (status, location, _) = send(&w.app, post(uri, None, "name=hacked")).await;
            assert_eq!(status, StatusCode::SEE_OTHER, "{uri}");
            assert_eq!(location, "/admin/login", "{uri}");
            for cookie in [ADMIN, MEMBER] {
                let (status, _, _) = send(&w.app, post(uri, Some(cookie), "name=hacked")).await;
                assert_eq!(status, StatusCode::NOT_FOUND, "{uri} as {cookie}");
            }
        }
        // Nothing changed.
        assert_eq!(names(&w.pool, w.owner).await, ["drawingportfolio"]);
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(plan.name, "request-flow");
    }

    #[tokio::test]
    async fn test_the_owner_creates_renames_and_deletes_projects() {
        let w = world().await;
        let (status, location, _) = send(
            &w.app,
            post("/blueprints", Some(OWNER), "name=garden-sensor"),
        )
        .await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let id: i64 = location.trim_start_matches("/blueprints/").parse().unwrap();
        assert_eq!(
            names(&w.pool, w.owner).await,
            ["drawingportfolio", "garden-sensor"]
        );

        // Names: refused shapes and duplicates go back to the form with a code.
        for (form, want) in [
            ("name=Garden", "/blueprints?error=name"),
            ("name=a_b", "/blueprints?error=name"),
            ("name=", "/blueprints?error=name"),
            ("name=garden-sensor", "/blueprints?error=taken"),
        ] {
            let (status, location, _) = send(&w.app, post("/blueprints", Some(OWNER), form)).await;
            assert_eq!(
                (status, location.as_str()),
                (StatusCode::SEE_OTHER, want),
                "{form}"
            );
        }
        let (_, _, html) = send(&w.app, get("/blueprints?error=taken", Some(OWNER))).await;
        assert!(html.contains("already taken"));
        let (_, _, html) = send(
            &w.app,
            get("/blueprints?error=%3Cb%3Ex%3C%2Fb%3E", Some(OWNER)),
        )
        .await;
        assert!(!html.contains("<b>x</b>"), "an unknown code prints nothing");

        let rename = format!("/blueprints/{id}/rename");
        let (_, location, _) = send(&w.app, post(&rename, Some(OWNER), "name=greenhouse")).await;
        assert_eq!(location, format!("/blueprints/{id}"));
        assert_eq!(
            names(&w.pool, w.owner).await,
            ["drawingportfolio", "greenhouse"]
        );
        let (_, location, _) =
            send(&w.app, post(&rename, Some(OWNER), "name=drawingportfolio")).await;
        assert_eq!(location, format!("/blueprints/{id}?error=taken"));

        // Deleting a project takes its plans with it.
        let delete = format!("/blueprints/{}/delete", w.pid);
        let (status, location, _) = send(&w.app, post(&delete, Some(OWNER), "")).await;
        assert_eq!(
            (status, location.as_str()),
            (StatusCode::SEE_OTHER, "/blueprints")
        );
        assert_eq!(names(&w.pool, w.owner).await, ["greenhouse"]);
        assert!(db::list_blueprint_plans(&w.pool, w.owner).await.is_empty());
        let (status, _, _) = send(&w.app, post(&delete, Some(OWNER), "")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "deleting it twice");
    }

    #[tokio::test]
    async fn test_the_owner_creates_renames_and_deletes_plans() {
        let w = world().await;
        let create = format!("/blueprints/{}/plans", w.pid);
        let (status, location, _) =
            send(&w.app, post(&create, Some(OWNER), "name=reading-pipeline")).await;
        assert_eq!(status, StatusCode::SEE_OTHER);
        let prefix = format!("/blueprints/{}/", w.pid);
        let id: i64 = location.trim_start_matches(&prefix).parse().unwrap();
        let plan = db::get_blueprint_plan(&w.pool, w.pid, id, w.owner)
            .await
            .unwrap();
        assert_eq!(
            (plan.payload.as_str(), plan.rev),
            (blueprint_doc::EMPTY_PLAN, 1)
        );

        // The empty plan opens.
        let (status, _, html) = send(&w.app, get(&location, Some(OWNER))).await;
        assert_eq!(status, StatusCode::OK);
        assert!(html.contains("0 machines · 0 wires"));

        let (_, location, _) = send(&w.app, post(&create, Some(OWNER), "name=request-flow")).await;
        assert_eq!(location, format!("/blueprints/{}?error=taken", w.pid));
        let (_, location, _) = send(&w.app, post(&create, Some(OWNER), "name=Bad Name")).await;
        assert_eq!(location, format!("/blueprints/{}?error=name", w.pid));

        let rename = format!("/blueprints/{}/{id}/rename", w.pid);
        send(&w.app, post(&rename, Some(OWNER), "name=ingest")).await;
        let plan = db::get_blueprint_plan(&w.pool, w.pid, id, w.owner)
            .await
            .unwrap();
        assert_eq!(
            (plan.name.as_str(), plan.rev),
            ("ingest", 1),
            "a rename leaves the rev alone"
        );

        let delete = format!("/blueprints/{}/{id}/delete", w.pid);
        let (_, location, _) = send(&w.app, post(&delete, Some(OWNER), "")).await;
        assert_eq!(location, format!("/blueprints/{}", w.pid));
        assert!(db::get_blueprint_plan(&w.pool, w.pid, id, w.owner)
            .await
            .is_none());

        // A plan in a project that is not the owner's cannot be made.
        let (status, _, _) = send(
            &w.app,
            post("/blueprints/999999/plans", Some(OWNER), "name=x"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_an_empty_project_shows_its_empty_state() {
        let w = world().await;
        let BlueprintNameOutcome::Done(id) =
            db::create_blueprint_project(&w.pool, w.owner, "garden-sensor").await
        else {
            panic!()
        };
        let (_, _, html) = send(&w.app, get(&format!("/blueprints/{id}"), Some(OWNER))).await;
        assert!(html.contains("No plans in this project yet."));
        assert!(html.contains(&format!(r#"action="/blueprints/{id}/plans""#)));
    }

    // ── The save ────────────────────────────────────────────────────────────

    fn put(uri: &str, cookie: Option<&str>, if_match: Option<&str>, body: &str) -> Request<Body> {
        let mut b = Request::builder()
            .method("PUT")
            .uri(uri)
            .header("content-type", "application/json");
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        if let Some(m) = if_match {
            b = b.header("if-match", m);
        }
        b.body(Body::from(body.to_string())).unwrap()
    }

    fn json(body: &str) -> serde_json::Value {
        serde_json::from_str(body).unwrap_or_else(|_| panic!("not JSON: {body}"))
    }

    #[tokio::test]
    async fn test_a_save_stores_the_text_and_answers_with_the_new_rev() {
        let w = world().await;
        let uri = format!("/api/blueprints/{}/{}", w.pid, w.plid);
        // Ports in an order serde_json's own map would sort: z before a.
        let doc = r#"{"v":1,"root":"main","graphs":{"main":{"nodes":{"m":{"kind":"machine","out":{"z":"T","a":"T"}}},"wires":[["m.z","gone.x"]]}},"layout":{"main/m":[2,3]}}"#;
        let resp = w
            .app
            .clone()
            .oneshot(put(&uri, Some(OWNER), Some(r#""1""#), doc))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()["etag"], r#""2""#);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let j = json(std::str::from_utf8(&body).unwrap());
        assert_eq!(j["rev"], 2);
        // The dangling wire is counted and the save still lands.
        assert_eq!(
            j["summary"],
            serde_json::json!({"nodes":1,"wires":1,"dangling":1,"mismatch":0})
        );
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(
            (plan.payload.as_str(), plan.rev),
            (doc, 2),
            "stored byte for byte"
        );

        // A bare rev works as well as a quoted one, and so does a weak one.
        let (status, _, _) = send(&w.app, put(&uri, Some(OWNER), Some("2"), doc)).await;
        assert_eq!(status, StatusCode::OK);
        let (status, _, _) = send(&w.app, put(&uri, Some(OWNER), Some(r#"W/"3""#), doc)).await;
        assert_eq!(status, StatusCode::OK);

        // The outline fragment shows the saved plan, in its own key order.
        let (status, _, html) = send(
            &w.app,
            get(
                &format!("/blueprints/{}/{}/outline", w.pid, w.plid),
                Some(OWNER),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            html.contains(r#"<div class="bp-outline" id="bp-outline""#),
            "{html}"
        );
        assert!(html.contains("out: z: T → gone.x; a: T"), "{html}");
        assert!(!html.contains("<html"), "a fragment, not a page");
    }

    /// The rule that keeps last-write-wins out: a save from an old rev is
    /// refused with the current rev, and changes nothing.
    #[tokio::test]
    async fn test_a_stale_save_is_refused_with_412_and_changes_nothing() {
        let w = world().await;
        let uri = format!("/api/blueprints/{}/{}", w.pid, w.plid);
        let (status, _, _) = send(
            &w.app,
            put(&uri, Some(OWNER), Some("1"), blueprint_doc::EMPTY_PLAN),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let resp = w
            .app
            .clone()
            .oneshot(put(&uri, Some(OWNER), Some("1"), DEMO_PLAN))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::PRECONDITION_FAILED);
        assert_eq!(resp.headers()["etag"], r#""2""#);
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        assert_eq!(json(std::str::from_utf8(&body).unwrap())["rev"], 2);
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(
            (plan.payload.as_str(), plan.rev),
            (blueprint_doc::EMPTY_PLAN, 2)
        );
    }

    #[tokio::test]
    async fn test_a_save_without_a_usable_if_match_is_refused_with_428() {
        let w = world().await;
        let uri = format!("/api/blueprints/{}/{}", w.pid, w.plid);
        for m in [
            None,
            Some("*"),
            Some(""),
            Some("abc"),
            Some("0"),
            Some("-1"),
            Some(r#""1"#),
        ] {
            let (status, _, body) =
                send(&w.app, put(&uri, Some(OWNER), m, blueprint_doc::EMPTY_PLAN)).await;
            assert_eq!(status, StatusCode::PRECONDITION_REQUIRED, "{m:?}");
            assert!(json(&body)["errors"][0]
                .as_str()
                .unwrap()
                .contains("If-Match"));
        }
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(plan.rev, 1, "nothing was written");
    }

    #[tokio::test]
    async fn test_a_plan_that_cannot_be_read_is_refused_with_every_problem() {
        let w = world().await;
        let uri = format!("/api/blueprints/{}/{}", w.pid, w.plid);
        let bad = r#"{"v":1,"root":"main","graphs":{"main":{"nodes":{"a":{"kind":"source"},"b":{"kind":"machine","status":"blocked"}},"wires":[["a","b.x"]]}}}"#;
        let (status, _, body) = send(&w.app, put(&uri, Some(OWNER), Some("1"), bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(json(&body)["errors"].as_array().unwrap().len(), 3, "{body}");
        let (status, _, _) = send(&w.app, put(&uri, Some(OWNER), Some("1"), "{ not json")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        // Too large: refused before it is parsed.
        let huge = format!(r#"{{"pad":"{}"}}"#, "x".repeat(blueprint_doc::MAX_PAYLOAD));
        let (status, _, body) = send(&w.app, put(&uri, Some(OWNER), Some("1"), &huge)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert!(json(&body)["errors"][0].as_str().unwrap().contains("1 MB"));
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(plan.rev, 1, "nothing was written");
    }

    #[tokio::test]
    async fn test_saves_and_the_outline_are_refused_to_everyone_but_the_owner() {
        let w = world().await;
        let uri = format!("/api/blueprints/{}/{}", w.pid, w.plid);
        let outline = format!("/blueprints/{}/{}/outline", w.pid, w.plid);
        // No session: the login answer, never the handler.
        let (status, location, _) = send(
            &w.app,
            put(&uri, None, Some("1"), blueprint_doc::EMPTY_PLAN),
        )
        .await;
        assert_eq!(
            (status, location.as_str()),
            (StatusCode::SEE_OTHER, "/admin/login")
        );
        let (status, _, _) = send(&w.app, get(&outline, None)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        for cookie in [ADMIN, MEMBER] {
            let (status, _, _) = send(
                &w.app,
                put(&uri, Some(cookie), Some("1"), blueprint_doc::EMPTY_PLAN),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{cookie}");
            let (status, _, body) = send(&w.app, get(&outline, Some(cookie))).await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{cookie}");
            assert!(!body.contains("token_gate"), "{cookie} read the outline");
        }
        // The owner, at a plan that is not there or not under that project.
        let BlueprintNameOutcome::Done(other) =
            db::create_blueprint_project(&w.pool, w.owner, "other").await
        else {
            panic!()
        };
        for wrong in [
            format!("/api/blueprints/{}/999999", w.pid),
            format!("/api/blueprints/{other}/{}", w.plid),
            "/api/blueprints/abc/1".to_string(),
        ] {
            let (status, _, _) = send(
                &w.app,
                put(&wrong, Some(OWNER), Some("1"), blueprint_doc::EMPTY_PLAN),
            )
            .await;
            assert_eq!(status, StatusCode::NOT_FOUND, "{wrong}");
        }
        let plan = db::get_blueprint_plan(&w.pool, w.pid, w.plid, w.owner)
            .await
            .unwrap();
        assert_eq!(
            (plan.rev, plan.payload.as_str()),
            (1, DEMO_PLAN),
            "nothing was written"
        );
    }

    #[test]
    fn test_if_match_reads_bare_quoted_and_weak_revs() {
        let h = |v: &str| {
            let mut m = HeaderMap::new();
            m.insert(header::IF_MATCH, v.parse().unwrap());
            if_match(&m)
        };
        assert_eq!(h("14"), Some(14));
        assert_eq!(h(r#""14""#), Some(14));
        assert_eq!(h(r#"W/"14""#), Some(14));
        assert_eq!(h(" 14 "), Some(14));
        for bad in ["*", "", "x", "0", "-3", r#""14"#, r#""1", "2""#] {
            assert_eq!(h(bad), None, "{bad}");
        }
        assert_eq!(if_match(&HeaderMap::new()), None);
    }
}
