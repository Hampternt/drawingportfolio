use crate::{middleware::OptionalAuth, models::Post, AppState};
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate {
    is_admin: bool,
    /// The page head's micro-label, pre-computed because it is only sometimes
    /// possible to state a total honestly — see `head_label()`.
    head_label: String,
    /// First page of posts rendered as HTML, injected directly into the page.
    /// Eliminates the extra HTMX round trip that would otherwise happen on load.
    initial_posts_html: String,
}

/// Builds the page head's micro-label.
///
/// Page 0 holds at most 20 posts, so `posts.len()` is the real total only when
/// there is no second page. With more, saying "20 drawings" would be plainly
/// false rather than merely approximate, so the count is omitted until a real
/// `COUNT` lands with caption search in the next slice.
fn head_label(page_len: usize, has_more: bool) -> String {
    if has_more {
        "newest first".to_string()
    } else {
        format!("{page_len} drawings · newest first")
    }
}

/// One drawing card — the single source of card markup in the app.
///
/// `pub` because `routes::admin` renders it for the upload response: a new post
/// must come back as an `hm-post` like every other card, not as the legacy
/// markup the admin dashboard still uses.
#[derive(Template)]
#[template(path = "partials/post_card.html")]
pub struct PostCardTemplate<'a> {
    pub post: &'a Post,
    pub is_first: bool,
}

/// A page of cards plus Load more. Rendered by both the inline first page and
/// the HTMX pagination route, so the two cannot drift.
#[derive(Template)]
#[template(path = "artportfolio/partials/post_grid.html")]
struct PostGridTemplate<'a> {
    posts: &'a [Post],
    has_more: bool,
    next_page: i64,
    /// Drives two things: whether an empty list renders the empty state (page 1
    /// of nothing means no drawings; page 3 of nothing just means the end), and
    /// which single image gets `fetchpriority="high"`.
    is_first_page: bool,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
}

/// Fetches one page and renders the grid, also returning how many posts it holds
/// and whether another page follows.
///
/// `get_posts` asks for 21 rows to answer "is there another page?" without a
/// COUNT; the 21st is dropped before rendering. Returning both lets `feed_page`
/// build the page head without a second query.
async fn render_page(state: &Arc<AppState>, page: i64) -> (String, usize, bool) {
    let mut posts = crate::db::get_posts(&state.pool, page).await;
    let has_more = posts.len() > 20;
    if has_more {
        posts.truncate(20);
    }
    let html = PostGridTemplate {
        posts: &posts,
        has_more,
        next_page: page + 1,
        is_first_page: page == 0,
    }
    .render()
    .unwrap();
    (html, posts.len(), has_more)
}

async fn feed_page(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // Fetch first page here so posts arrive in the very first HTTP response.
    // Without this, the browser would load the page and then fire a second
    // request to /artportfolio/htmx/posts?page=0 before anything was visible.
    let (initial_posts_html, page_len, has_more) = render_page(&state, 0).await;

    Html(
        FeedTemplate {
            is_admin,
            head_label: head_label(page_len, has_more),
            initial_posts_html,
        }
        .render()
        .unwrap(),
    )
}

async fn htmx_posts(
    State(state): State<Arc<AppState>>,
    Query(q): Query<PageQuery>,
) -> impl IntoResponse {
    let (html, _, _) = render_page(&state, q.page.unwrap_or(0)).await;
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
    if has_more {
        posts.truncate(20);
    }
    Json(PostsResponse { posts, has_more })
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
    use sqlx::sqlite::SqlitePoolOptions;
    use tower::ServiceExt;

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
        let state = Arc::new(crate::AppState {
            pool,
            storage,
            webauthn,
        });
        router().with_state(state)
    }

    #[tokio::test]
    async fn test_api_posts_empty() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/artportfolio/api/posts")
                    .body(Body::empty())
                    .unwrap(),
            )
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
                crate::db::insert_post(
                    &pool,
                    &format!("caption {i}"),
                    "https://example.com/img.jpg",
                    "",
                    "",
                    crate::models::PostFormat::Single.as_str(),
                    0,
                    0,
                    0,
                )
                .await;
            }
            pool
        };
        let posts = crate::db::get_posts(&pool, 0).await;
        assert!(posts.len() > 20, "expected 21 rows with has_more=true");
    }

    /// Renders one card the way every caller now does. These four assertions
    /// were written against the old `post_card_html()` format string; they are
    /// ported rather than dropped because they pin behaviour the template still
    /// owes — escaping, the picture fallback, and the empty-caption case.
    fn card(post: &crate::models::Post, is_first: bool) -> String {
        PostCardTemplate { post, is_first }.render().unwrap()
    }

    /// A Post with the fields these tests do not care about filled in.
    fn sample_post(id: i64, caption: &str) -> crate::models::Post {
        crate::models::Post {
            id,
            caption: caption.to_string(),
            image_url: "https://example.com/img.jpg".to_string(),
            webp_url: "".to_string(),
            avif_url: "".to_string(),
            format: "single".to_string(),
            file_size_bytes: 0,
            created_at: "2024-01-01T00:00:00".to_string(),
            image_width: 0,
            image_height: 0,
        }
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
            image_width: 0,
            image_height: 0,
        };
        let html = card(&post, false);
        assert!(
            !html.contains("hm-post__caption"),
            "empty caption must not render the caption paragraph"
        );
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
            image_width: 0,
            image_height: 0,
        };
        let html = card(&post, false);
        // The security property, and the only part that must never change: no
        // raw tag survives into the output, in the caption or the alt text.
        assert!(
            !html.contains("<script>") && !html.contains("</script>"),
            "raw script tag should be escaped: {html}"
        );

        // Askama escapes to NUMERIC character references (&#60;), where the
        // hand-rolled html_escape this replaced used named ones (&lt;). Both are
        // valid HTML and render identically, so accept either rather than
        // pinning one engine's spelling — the assertion above is the real test.
        let escaped_lt = html.contains("&#60;") || html.contains("&lt;");
        let escaped_gt = html.contains("&#62;") || html.contains("&gt;");
        assert!(
            escaped_lt && escaped_gt,
            "angle brackets must appear as entities: {html}"
        );
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
            image_width: 0,
            image_height: 0,
        };
        let html = card(&post, false);
        assert!(html.contains("<picture>"), "should contain picture element");
        assert!(
            html.contains("type=\"image/avif\""),
            "should contain avif source"
        );
        assert!(
            html.contains("type=\"image/webp\""),
            "should contain webp source"
        );
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
            image_width: 0,
            image_height: 0,
        };
        let html = card(&post, false);
        assert!(
            html.contains("<picture>"),
            "picture element should always be present"
        );
        assert!(!html.contains("image/avif"), "no avif source for empty url");
        assert!(!html.contains("image/webp"), "no webp source for empty url");
    }

    #[test]
    fn test_head_label_states_a_total_only_when_it_knows_one() {
        // No second page: page 0 IS the whole feed, so the count is the truth.
        assert_eq!(head_label(8, false), "8 drawings · newest first");
        assert_eq!(head_label(0, false), "0 drawings · newest first");

        // A second page exists, so page 0 was truncated to 20. Saying "20
        // drawings" here would be false, not merely approximate.
        assert_eq!(head_label(20, true), "newest first");
        assert!(
            !head_label(20, true).contains("20"),
            "must not report the page size as the total"
        );
    }

    #[test]
    fn test_post_card_emits_dimensions_when_known() {
        let mut post = sample_post(5, "a drawing");
        post.image_width = 1600;
        post.image_height = 900;
        let html = card(&post, false);
        assert!(
            html.contains("width=\"1600\""),
            "known width is emitted so the masonry can reserve the box: {html}"
        );
        assert!(html.contains("height=\"900\""));
    }

    #[test]
    fn test_post_card_omits_dimensions_when_zero() {
        // Pre-012 rows carry 0. width="0" would collapse the image to nothing,
        // so both attributes must be absent rather than zero.
        let html = card(&sample_post(6, "legacy row"), false);
        assert!(!html.contains("width=\"0\""), "width=0 would collapse it");
        assert!(!html.contains("height=\"0\""));
    }

    #[test]
    fn test_post_card_first_is_eager_rest_are_lazy() {
        let post = sample_post(7, "above the fold");
        let first = card(&post, true);
        assert!(first.contains("loading=\"eager\""));
        assert!(
            first.contains("fetchpriority=\"high\""),
            "the first card is the LCP candidate"
        );

        let rest = card(&post, false);
        assert!(rest.contains("loading=\"lazy\""));
        assert!(!rest.contains("fetchpriority"));
    }

    #[test]
    fn test_upload_response_renders_the_same_card_markup_as_the_feed() {
        // routes::admin renders PostCardTemplate for the composer's upload
        // response (source=gallery) and swaps it into #feed. If that path ever
        // diverges from the feed's own markup, a fresh upload looks broken
        // among its neighbours while the build stays green — so pin the class
        // the feed's CSS actually targets.
        let html = card(&sample_post(8, "just uploaded"), false);
        assert!(
            html.contains("class=\"hm-post\""),
            "upload response must be design-system markup, not legacy .post-card: {html}"
        );
        assert!(
            !html.contains("class=\"post-card\""),
            "legacy card class must not appear in the feed's card"
        );
    }
}
