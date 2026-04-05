use axum::{
    Router,
    routing::get,
    extract::{Query, State},
    response::{Html, IntoResponse},
    Json,
};
use askama::Template;
use std::sync::Arc;
use serde::Deserialize;
use crate::{AppState, models::Post};

#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate;

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

async fn feed_page() -> impl IntoResponse {
    Html(FeedTemplate.render().unwrap())
}

async fn htmx_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(0);
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }

    let next_page = page + 1;
    let mut html = String::new();

    if posts.is_empty() && page == 0 {
        html.push_str(r#"<div class="empty-state"><p>No posts yet.</p></div>"#);
    } else {
        for post in &posts {
            html.push_str(&post_card_html(post));
        }
        if has_more {
            let load_more = format!(
                "<div class=\"load-more\" id=\"load-more\">\
                  <button hx-get=\"/artportfolio/htmx/posts?page={next_page}\" \
                          hx-target=\"#load-more\" \
                          hx-swap=\"outerHTML\">\
                    Load more\
                  </button>\
                </div>"
            );
            html.push_str(&load_more);
        }
    }

    Html(html)
}

#[derive(serde::Serialize)]
struct PostsResponse {
    posts: Vec<Post>,
    has_more: bool,
}

async fn api_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(0);
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }
    Json(PostsResponse { posts, has_more })
}

pub fn post_card_html(post: &Post) -> String {
    format!(
        r#"<article class="post-card" id="post-{}">
  <img src="{}" alt="{}" loading="lazy">
  <p class="caption">{}</p>
  <small class="date">{}</small>
</article>"#,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.caption),
        html_escape(&post.caption),
        html_escape(&post.created_at),
    )
}

pub fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/artportfolio", get(feed_page))
        .route("/artportfolio/htmx/posts", get(htmx_posts))
        .route("/artportfolio/api/posts", get(api_posts))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn test_app() -> Router {
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
        let state = Arc::new(crate::AppState { pool, storage, webauthn });
        router().with_state(state)
    }

    #[tokio::test]
    async fn test_api_posts_empty() {
        let app = test_app().await;
        let resp = app
            .oneshot(Request::builder().uri("/artportfolio/api/posts").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_api_posts_has_more() {
        let pool = {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .connect("sqlite::memory:")
                .await
                .unwrap();
            crate::db::run_migrations(&pool).await;
            for i in 0..21 {
                crate::db::insert_post(&pool, &format!("caption {i}"), "https://example.com/img.jpg").await;
            }
            pool
        };
        let posts = crate::db::get_posts(&pool, 0).await;
        assert!(posts.len() > 20, "expected 21 rows with has_more=true");
    }

    #[test]
    fn test_post_card_html_escapes_content() {
        let post = crate::models::Post {
            id: 1,
            caption: "<script>alert(1)</script>".to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            created_at: "2024-01-01T00:00:00".to_string(),
        };
        let html = post_card_html(&post);
        assert!(!html.contains("<script>"), "raw script tag should be escaped");
        assert!(html.contains("&lt;script&gt;"));
    }
}
