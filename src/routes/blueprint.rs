//! Blueprint planner, slice S0: the canvas spike.
//!
//! A throwaway page that proves the hand-rolled canvas (pan, zoom, drag and
//! wiring) before anything is stored. There is no migration, no API and no
//! persistence: the page loads the demo plan, and every edit lives in the tab
//! until it is closed. Plan and decisions:
//! `docs/manifests/2026-11-05-blueprint-canvas-spike.md`.
//!
//! The route sits behind [`RequireOwner`] and at a deliberately temporary path,
//! because the planner's real name and URL are still an open question (issue 1
//! in `docs/design/blueprint-planner/README.md`). It has no hub tile, nav link
//! or palette entry until slice S2 settles that.

use crate::{middleware::RequireOwner, AppState};
use askama::Template;
use axum::{
    response::{Html, IntoResponse},
    routing::get,
    Router,
};
use std::sync::Arc;

/// The design canvas's demo plan: the site's own request flow. Slice S2 reuses
/// this file as the visitor demo, so it is committed once and never seeded.
const DEMO_PLAN: &str = include_str!("../../docs/design/blueprint-planner/demo-plan.json");

#[derive(Template)]
#[template(path = "blueprint/spike.html")]
struct SpikeTemplate {
    /// base.html reads this for the header's settings button. Only the owner
    /// reaches this page, and the owner is always an effective admin.
    is_admin: bool,
    demo_json: String,
}

/// Makes JSON safe inside `<script type="application/json">`. The browser
/// never runs it, but it does look for `</script` to end the element, and the
/// demo's port types are full of angle brackets (`Vec<Post>`). `<` is
/// still the same string once `JSON.parse` reads it.
fn script_safe(json: &str) -> String {
    json.replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
}

async fn spike_page(_: RequireOwner) -> impl IntoResponse {
    Html(
        SpikeTemplate {
            is_admin: true,
            demo_json: script_safe(DEMO_PLAN),
        }
        .render()
        .unwrap(),
    )
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route("/blueprint-spike", get(spike_page))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
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

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut b = Request::builder().method("GET").uri(uri);
        if let Some(c) = cookie {
            b = b.header("cookie", c);
        }
        b.body(Body::empty()).unwrap()
    }

    async fn body_text(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    fn render() -> String {
        SpikeTemplate {
            is_admin: true,
            demo_json: script_safe(DEMO_PLAN),
        }
        .render()
        .unwrap()
    }

    #[test]
    fn test_demo_plan_is_the_documented_shape() {
        let doc: serde_json::Value = serde_json::from_str(DEMO_PLAN).expect("demo plan is JSON");
        assert_eq!(doc["v"], 1);
        assert_eq!(doc["root"], "main");
        let main = &doc["graphs"]["main"];
        assert_eq!(main["nodes"].as_object().unwrap().len(), 8);
        assert_eq!(main["wires"].as_array().unwrap().len(), 12);
        // The blueprint node opens into a graph that exists.
        assert_eq!(main["nodes"]["db"]["kind"], "blueprint");
        assert!(doc["graphs"]["db"]["nodes"].is_object());
        // Every node in every graph has a position in the layout.
        let layout = doc["layout"].as_object().unwrap();
        for (g, graph) in doc["graphs"].as_object().unwrap() {
            for name in graph["nodes"].as_object().unwrap().keys() {
                assert!(layout.contains_key(&format!("{g}/{name}")), "{g}/{name} has no position");
            }
        }
    }

    #[test]
    fn test_script_safe_leaves_no_angle_bracket_and_round_trips() {
        let safe = script_safe(DEMO_PLAN);
        assert!(!safe.contains('<') && !safe.contains('>'));
        // Positive control: the raw plan really does carry them.
        assert!(DEMO_PLAN.contains("Vec<Post>"));
        let a: serde_json::Value = serde_json::from_str(DEMO_PLAN).unwrap();
        let b: serde_json::Value = serde_json::from_str(&safe).unwrap();
        assert_eq!(a, b, "escaping changed what the plan says");
    }

    #[test]
    fn test_page_carries_the_site_dark_marker_and_the_plan() {
        let html = render();
        // base.html derives body.site-dark from this marker on every boosted
        // navigation; it has to be the outermost element in the content block.
        assert!(html.contains(r#"<section class="site-page bp-spike""#));
        assert!(html.contains(r#"<script type="application/json" id="bp-demo">"#));
        assert!(html.contains(r#""token_gate""#));
        for js in ["blueprint-model.js", "blueprint-view.js", "blueprint-app.js"] {
            assert!(html.contains(&format!("/static/{js}?v=")), "{js} is not loaded");
        }
    }

    #[tokio::test]
    async fn test_owner_gets_the_spike() {
        let (app, pool) = app_with_pool().await;
        let id = crate::db::get_owner_user_id(&pool).await.unwrap();
        crate::db::create_session(&pool, "owner-sess", "2099-01-01 00:00:00", id).await;
        let resp = app
            .oneshot(get("/blueprint-spike", Some("session=owner-sess")))
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_text(resp).await.contains("bp-spike"));
    }

    /// A signed-in member, admin or not, is told the page does not exist, never
    /// that it is forbidden; a visitor with no session is sent to log in.
    #[tokio::test]
    async fn test_everyone_but_the_owner_is_refused() {
        let (app, pool) = app_with_pool().await;
        for (name, admin) in [("alex", true), ("sam", false)] {
            let id = crate::db::create_user(&pool, name, "hash").await.unwrap();
            if admin {
                crate::db::set_user_admin(&pool, id, true).await;
            }
            let sess = format!("{name}-sess");
            crate::db::create_session(&pool, &sess, "2099-01-01 00:00:00", id).await;
            let resp = app
                .clone()
                .oneshot(get("/blueprint-spike", Some(&format!("session={sess}"))))
                .await
                .unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{name} reached the spike");
        }
        let resp = app.oneshot(get("/blueprint-spike", None)).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SEE_OTHER);
    }
}
