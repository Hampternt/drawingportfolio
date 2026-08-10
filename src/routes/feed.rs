use crate::{
    middleware::OptionalAuth,
    models::{MonthGroup, Post},
    AppState,
};
use askama::Template;
use axum::{
    extract::{Query, State},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate {
    is_admin: bool,
    /// The page head's micro-label, pre-computed — see `head_label()`.
    head_label: String,
    /// First page of posts rendered as HTML, injected directly into the page.
    /// Eliminates the extra HTMX round trip that would otherwise happen on load.
    initial_posts_html: String,
}

/// Builds the page head's micro-label from the real total.
///
/// `count_posts` runs only on a full page render — never on HTMX pagination,
/// where the head is not re-rendered and the COUNT would be wasted work on
/// every Load more.
fn head_label(total: i64, q: Option<&str>) -> String {
    let noun = if total == 1 { "drawing" } else { "drawings" };
    match q {
        // Deliberately unescaped: the template renders this through `{{ }}` and
        // Askama escapes it on the way out. Escaping here too would paint
        // entities on the page.
        Some(q) => format!("{total} {noun} · matching \"{q}\""),
        None => format!("{total} {noun} · newest first"),
    }
}

/// Trims the raw query and treats blank as absent.
///
/// One normalisation at the handler edge kills three separate defects: a head
/// label reading `matching ""`, a `%%` pattern that matches every row, and a
/// pushed URL carrying a pointless empty `?q=`.
fn normalize_q(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Splits a page of posts into month sections.
///
/// `created_at` is ISO8601 `TEXT`, so its first seven characters are `YYYY-MM`.
/// Rows arrive already sorted `created_at DESC`, so this is one pass over
/// consecutive runs — a hash map would risk reordering the feed.
///
/// `last_month` is the month the previous page ended on. When the first group
/// matches it, that group's divider is suppressed: page 1 opening with more
/// June posts must not draw a second `2026-06` rule under the one page 0 drew.
fn group_by_month(posts: Vec<Post>, last_month: Option<&str>) -> Vec<MonthGroup> {
    let mut groups: Vec<MonthGroup> = Vec::new();
    for post in posts {
        // `get(..7)` rather than `[..7]`: a malformed short timestamp would
        // panic on a slice, and one bad row is not worth a 500.
        let label = post.created_at.get(..7).unwrap_or("").to_string();
        match groups.last_mut() {
            Some(group) if group.label == label => {
                group.count += 1;
                group.posts.push(post);
            }
            _ => groups.push(MonthGroup {
                // Only the leading group can be a continuation of the previous
                // page; every later one starts a month this page opened.
                show_divider: !groups.is_empty() || Some(label.as_str()) != last_month,
                label,
                count: 1,
                posts: vec![post],
            }),
        }
    }
    groups
}

/// The Load more URL, built in Rust because Askama escapes HTML, not URLs — a
/// query containing `&` or `%` must be percent-encoded before it reaches an
/// attribute.
fn load_more_url(next_page: i64, q: Option<&str>, last_month: Option<&str>) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    s.append_pair("page", &next_page.to_string());
    if let Some(q) = q {
        s.append_pair("q", q);
    }
    if let Some(m) = last_month {
        s.append_pair("last_month", m);
    }
    format!("/artportfolio/htmx/posts?{}", s.finish())
}

/// The URL the address bar should show for a search — a real page, not the
/// fragment endpoint that produced the swap. See `htmx_posts`.
fn page_url(q: Option<&str>) -> String {
    match q {
        Some(q) => {
            let mut s = url::form_urlencoded::Serializer::new(String::new());
            s.append_pair("q", q);
            format!("/artportfolio?{}", s.finish())
        }
        None => "/artportfolio".to_string(),
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

/// A page of month sections plus Load more. Rendered by both the inline first
/// page and the HTMX pagination route, so the two cannot drift.
#[derive(Template)]
#[template(path = "artportfolio/partials/post_grid.html")]
struct PostGridTemplate {
    groups: Vec<MonthGroup>,
    has_more: bool,
    /// Pre-built, carrying the active query and the month this page ended on.
    load_more_url: String,
    /// Drives two things: whether an empty result renders the empty state (page
    /// 1 of nothing means no drawings; page 3 of nothing just means the end),
    /// and which single image gets `fetchpriority="high"`.
    is_first_page: bool,
    /// The active search, `""` when there is none — the empty state names it.
    q: String,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub q: Option<String>,
    /// The month the previous page ended on, so a month split across pages
    /// renders one divider rather than two.
    pub last_month: Option<String>,
}

/// Fetches one page and renders its month sections.
///
/// `get_posts_page` asks for 21 rows to answer "is there another page?" without
/// a COUNT; the 21st is dropped before grouping.
async fn render_grid(
    state: &Arc<AppState>,
    page: i64,
    q: Option<&str>,
    last_month: Option<&str>,
) -> String {
    let mut posts = crate::db::get_posts_page(&state.pool, q, page).await;
    let has_more = posts.len() > 20;
    if has_more {
        posts.truncate(20);
    }
    let groups = group_by_month(posts, last_month);
    // The next page has to know which month this one ended on, or it draws a
    // duplicate divider for a month already on screen.
    let next_last_month = groups.last().map(|g| g.label.clone());
    PostGridTemplate {
        has_more,
        load_more_url: load_more_url(page + 1, q, next_last_month.as_deref()),
        is_first_page: page == 0,
        q: q.unwrap_or_default().to_string(),
        groups,
    }
    .render()
    .unwrap()
}

async fn feed_page(
    OptionalAuth(is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> impl IntoResponse {
    // `?q=` is read here as well as on the fragment route, so a searched feed
    // survives a reload and is linkable.
    let q = normalize_q(query.q.as_deref());

    // Fetch first page here so posts arrive in the very first HTTP response.
    // Without this, the browser would load the page and then fire a second
    // request to /artportfolio/htmx/posts?page=0 before anything was visible.
    let initial_posts_html = render_grid(&state, 0, q.as_deref(), None).await;
    let total = crate::db::count_posts(&state.pool, q.as_deref()).await;

    Html(
        FeedTemplate {
            is_admin,
            head_label: head_label(total, q.as_deref()),
            initial_posts_html,
        }
        .render()
        .unwrap(),
    )
}

async fn htmx_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> Response {
    let page = query.page.unwrap_or(0);
    let q = normalize_q(query.q.as_deref());
    let html = render_grid(&state, page, q.as_deref(), query.last_month.as_deref()).await;

    if page == 0 {
        // Page 0 is a search or a cleared filter — the address bar should read
        // /artportfolio?q=…, a page that actually renders. `hx-push-url="true"`
        // would push this fragment endpoint instead, and reloading THAT gives a
        // bare grid with no shell, styles or nav.
        //
        // Load more (page >= 1) pushes nothing: it appends to what is already
        // on screen and must not rewrite history.
        ([("HX-Push-Url", page_url(q.as_deref()))], Html(html)).into_response()
    } else {
        Html(html).into_response()
    }
}

#[derive(serde::Serialize)]
struct PostsResponse {
    posts: Vec<Post>,
    has_more: bool,
}

async fn api_posts(
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(0);
    // The same filter the HTML feed applies, so the two cannot drift on what
    // "matching" means.
    let q = normalize_q(query.q.as_deref());
    let mut posts = crate::db::get_posts_page(&state.pool, q.as_deref(), page).await;
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
        let posts = crate::db::get_posts_page(&pool, None, 0).await;
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
    fn test_head_label_states_the_real_total() {
        // Replaces the pre-count_posts version of this test, which pinned a
        // vaguer fallback ("newest first" with no number) that existed only
        // because there was no COUNT to state a total with.
        assert_eq!(head_label(117, None), "117 drawings · newest first");
        assert_eq!(head_label(1, None), "1 drawing · newest first");
        assert_eq!(head_label(0, None), "0 drawings · newest first");
    }

    #[test]
    fn test_head_label_names_the_active_search() {
        assert_eq!(
            head_label(12, Some("loomis")),
            "12 drawings · matching \"loomis\""
        );
        assert_eq!(
            head_label(1, Some("loomis")),
            "1 drawing · matching \"loomis\""
        );
    }

    #[test]
    fn test_normalize_q_treats_blank_as_absent() {
        assert_eq!(normalize_q(None), None);
        assert_eq!(normalize_q(Some("")), None);
        assert_eq!(normalize_q(Some("   ")), None);
        assert_eq!(normalize_q(Some("  loomis ")).as_deref(), Some("loomis"));
    }

    /// A post carrying only the field month grouping reads.
    fn post_dated(id: i64, created_at: &str) -> crate::models::Post {
        let mut post = sample_post(id, "");
        post.created_at = created_at.to_string();
        post
    }

    #[test]
    fn test_group_by_month_splits_on_the_iso_prefix() {
        let posts = vec![
            post_dated(1, "2026-08-03T10:00:00"),
            post_dated(2, "2026-08-01T10:00:00"),
            post_dated(3, "2026-07-20T10:00:00"),
            post_dated(4, "2026-06-30T10:00:00"),
            post_dated(5, "2026-06-02T10:00:00"),
        ];
        let groups = group_by_month(posts, None);

        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].label, "2026-08");
        assert_eq!(groups[0].count, 2);
        assert_eq!(groups[1].label, "2026-07");
        assert_eq!(groups[1].count, 1);
        assert_eq!(groups[2].label, "2026-06");
        assert_eq!(groups[2].count, 2);
        assert!(
            groups.iter().all(|g| g.show_divider),
            "with no previous page every month opens with its own divider"
        );
        assert_eq!(
            groups.iter().map(|g| g.posts.len()).sum::<usize>(),
            5,
            "grouping loses no posts"
        );
    }

    #[test]
    fn test_last_month_suppresses_only_a_matching_leading_divider() {
        // Page 1 opens with more July posts, then rolls into June.
        let posts = vec![
            post_dated(1, "2026-07-09T10:00:00"),
            post_dated(2, "2026-06-28T10:00:00"),
        ];
        let groups = group_by_month(posts, Some("2026-07"));

        assert!(
            !groups[0].show_divider,
            "2026-07 is already on screen from the previous page"
        );
        assert_eq!(
            groups[0].label, "2026-07",
            "the label survives suppression — the next page's last_month is built from it"
        );
        assert!(groups[1].show_divider, "June is new on this page");
    }

    #[test]
    fn test_a_last_month_that_does_not_match_suppresses_nothing() {
        let posts = vec![post_dated(1, "2026-07-09T10:00:00")];
        let groups = group_by_month(posts, Some("2026-05"));
        assert!(groups[0].show_divider);
    }

    #[test]
    fn test_group_by_month_of_nothing_is_empty() {
        assert!(group_by_month(vec![], None).is_empty());
        assert!(group_by_month(vec![], Some("2026-07")).is_empty());
    }

    #[test]
    fn test_load_more_url_percent_encodes_the_query() {
        assert_eq!(
            load_more_url(1, Some("100%"), Some("2026-07")),
            "/artportfolio/htmx/posts?page=1&q=100%25&last_month=2026-07"
        );
        assert_eq!(
            load_more_url(1, None, None),
            "/artportfolio/htmx/posts?page=1"
        );
        assert_eq!(
            load_more_url(3, Some("a&b"), None),
            "/artportfolio/htmx/posts?page=3&q=a%26b"
        );
    }

    #[test]
    fn test_page_url_is_the_page_not_the_fragment_endpoint() {
        assert_eq!(page_url(None), "/artportfolio");
        assert_eq!(page_url(Some("loomis")), "/artportfolio?q=loomis");
        assert_eq!(page_url(Some("100%")), "/artportfolio?q=100%25");
    }

    #[tokio::test]
    async fn test_htmx_page_0_pushes_the_page_url_not_the_fragment_url() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/artportfolio/htmx/posts?q=loomis")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get("HX-Push-Url").unwrap(),
            "/artportfolio?q=loomis",
            "pushing the fragment URL would give a bare grid on reload"
        );
    }

    #[tokio::test]
    async fn test_load_more_pushes_no_url() {
        let app = test_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/artportfolio/htmx/posts?page=1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            resp.headers().get("HX-Push-Url").is_none(),
            "appending to the feed must not rewrite the address bar"
        );
    }

    #[tokio::test]
    async fn test_api_posts_applies_the_same_caption_filter() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        for caption in ["Loomis head", "figure drawing"] {
            crate::db::insert_post(
                &pool,
                caption,
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
        let hits = crate::db::get_posts_page(&pool, Some("loomis"), 0).await;
        assert_eq!(
            hits.len(),
            1,
            "the JSON API filters through the same db call"
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
