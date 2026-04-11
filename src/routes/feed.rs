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
use crate::{AppState, models::Post, middleware::OptionalAuth};

#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate {
    is_admin: bool,
    /// First page of posts rendered as HTML, injected directly into the page.
    /// Eliminates the extra HTMX round trip that would otherwise happen on load.
    initial_posts_html: String,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

async fn feed_page(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Fetch first page here so posts arrive in the very first HTTP response.
    // Without this, the browser would load the page and then fire a second
    // request to /artportfolio/htmx/posts?page=0 before anything was visible.
    let mut posts = crate::db::get_posts(&state.pool, 0).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }
    let initial_posts_html = render_posts_html(&posts, has_more, 1);

    Html(FeedTemplate { is_admin, initial_posts_html }.render().unwrap())
}

async fn htmx_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let page = q.page.unwrap_or(0);
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more { posts.truncate(20); }
    Html(render_posts_html(&posts, has_more, page + 1))
}

/// Renders a page of posts into an HTML string.
/// Used both for the inline first page (feed_page) and subsequent HTMX loads (htmx_posts),
/// so the two code paths always produce identical markup.
fn render_posts_html(posts: &[Post], has_more: bool, next_page: i64) -> String {
    if posts.is_empty() && next_page == 1 {
        return r#"<div class="empty-state"><p>No posts yet.</p></div>"#.to_string();
    }
    let mut html = String::new();
    for (i, post) in posts.iter().enumerate() {
        html.push_str(&post_card_html(post, i == 0));
    }
    if has_more {
        html.push_str(&format!(
            "<div class=\"load-more\" id=\"load-more\">\
              <button hx-get=\"/artportfolio/htmx/posts?page={next_page}\" \
                      hx-target=\"#load-more\" \
                      hx-swap=\"outerHTML\">\
                Load more\
              </button>\
            </div>"
        ));
    }
    html
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

pub fn post_card_html(post: &Post, is_first: bool) -> String {
    let caption_html = if post.caption.is_empty() {
        String::new()
    } else {
        format!("  <p class=\"caption\">{}</p>\n", html_escape(&post.caption))
    };
    let loading = if is_first { r#"loading="eager" fetchpriority="high""# } else { r#"loading="lazy""# };
    let avif_source = if !post.avif_url.is_empty() {
        format!("    <source srcset=\"{}\" type=\"image/avif\">\n", html_escape(&post.avif_url))
    } else {
        String::new()
    };
    let webp_source = if !post.webp_url.is_empty() {
        format!("    <source srcset=\"{}\" type=\"image/webp\">\n", html_escape(&post.webp_url))
    } else {
        String::new()
    };
    format!(
        r#"<article class="post-card" id="post-{}">
  <picture>
{avif_source}{webp_source}    <img src="{}" alt="{}" {loading}>
  </picture>
{caption_html}  <small class="date">{}</small>
</article>"#,
        post.id,
        html_escape(&post.image_url),
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
                crate::db::insert_post(&pool, &format!("caption {i}"), "https://example.com/img.jpg", "", "", crate::models::PostFormat::Single.as_str(), 0).await;
            }
            pool
        };
        let posts = crate::db::get_posts(&pool, 0).await;
        assert!(posts.len() > 20, "expected 21 rows with has_more=true");
    }

    #[test]
    fn test_post_card_empty_caption_omits_p_tag() {
        let post = crate::models::Post {
            id: 2,
            caption: "".to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            webp_url: "".to_string(),
            avif_url: "".to_string(),
            format: "single".to_string(),
            file_size_bytes: 0,
            created_at: "2024-01-01T00:00:00".to_string(),
        };
        let html = post_card_html(&post, false);
        assert!(!html.contains("class=\"caption\""), "empty caption must not render p.caption");
    }

    #[test]
    fn test_post_card_html_escapes_content() {
        let post = crate::models::Post {
            id: 1,
            caption: "<script>alert(1)</script>".to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            webp_url: "".to_string(),
            avif_url: "".to_string(),
            format: crate::models::PostFormat::Single.as_str().to_string(),
            file_size_bytes: 0,
            created_at: "2024-01-01T00:00:00".to_string(),
        };
        let html = post_card_html(&post, false);
        assert!(!html.contains("<script>"), "raw script tag should be escaped");
        assert!(html.contains("&lt;script&gt;"));
    }

    #[test]
    fn test_post_card_picture_element_with_variants() {
        let post = crate::models::Post {
            id: 3,
            caption: "".to_string(),
            image_url: "https://example.com/img.jpeg".to_string(),
            webp_url: "https://example.com/img-webp.webp".to_string(),
            avif_url: "https://example.com/img-avif.avif".to_string(),
            format: "single".to_string(),
            file_size_bytes: 0,
            created_at: "2024-01-01T00:00:00".to_string(),
        };
        let html = post_card_html(&post, false);
        assert!(html.contains("<picture>"), "should contain picture element");
        assert!(html.contains("type=\"image/avif\""), "should contain avif source");
        assert!(html.contains("type=\"image/webp\""), "should contain webp source");
        assert!(html.contains("img-avif.avif"), "should reference avif url");
        assert!(html.contains("img-webp.webp"), "should reference webp url");
    }

    #[test]
    fn test_post_card_picture_omits_sources_for_empty_variant_urls() {
        let post = crate::models::Post {
            id: 4,
            caption: "".to_string(),
            image_url: "https://example.com/img.jpeg".to_string(),
            webp_url: "".to_string(),
            avif_url: "".to_string(),
            format: "single".to_string(),
            file_size_bytes: 0,
            created_at: "2024-01-01T00:00:00".to_string(),
        };
        let html = post_card_html(&post, false);
        assert!(html.contains("<picture>"), "picture element should always be present");
        assert!(!html.contains("image/avif"), "no avif source for empty url");
        assert!(!html.contains("image/webp"), "no webp source for empty url");
    }
}
