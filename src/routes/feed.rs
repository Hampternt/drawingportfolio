use crate::{
    middleware::OptionalAuth,
    models::{MonthGroup, Post, PostCounts, Viewer},
    AppState,
};
use askama::Template;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use std::sync::Arc;

#[derive(Template)]
#[template(path = "artportfolio/feed.html")]
struct FeedTemplate {
    /// The **effective** viewer's admin-ness, not the raw session bool. Under
    /// `?visitor=1` this is false, so every admin affordance on the page goes
    /// away at once rather than one control at a time.
    is_admin: bool,
    /// True only for an admin who is currently previewing. Drives the "exit
    /// preview" affordance, which is the one thing that must render while
    /// `is_admin` is false — and must never render for a real visitor.
    is_previewing: bool,
    /// The page head's micro-label, pre-computed — see `head_label()`.
    head_label: String,
    /// First page of posts rendered as HTML, injected directly into the page.
    /// Eliminates the extra HTMX round trip that would otherwise happen on load.
    initial_posts_html: String,
    /// The active search, `""` when there is none. Fills the rail's input so a
    /// shared or reloaded `?q=` URL comes back with its query still on screen.
    q: String,
}

/// Builds the page head's micro-label from the real counts.
///
/// `count_posts` runs only on a full page render — never on HTMX pagination,
/// where the head is not re-rendered and the COUNT would be wasted work on
/// every Load more.
///
/// `counts.total` already encodes the viewer — an admin's total is every post, a
/// visitor's is the public count — so nothing here branches on `viewer` yet. It
/// is threaded now because the admin split (`· 117 public · 4 unlisted · 7
/// hidden`) lands in this function, and the out-of-band label on the page-0 path
/// would otherwise have to learn about the preview after the fact.
fn head_label(counts: &PostCounts, q: Option<&str>, viewer: Viewer) -> String {
    let total = counts.total;
    let noun = if total == 1 { "drawing" } else { "drawings" };
    let _ = viewer;
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
///
/// The preview flag rides along with `q` and `last_month`. Without it, page 0
/// renders as a visitor and the first Load more renders as an admin — hidden
/// posts appear mid-feed, in the middle of the preview that exists to prove they
/// do not.
fn load_more_url(
    next_page: i64,
    q: Option<&str>,
    last_month: Option<&str>,
    preview: bool,
) -> String {
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    s.append_pair("page", &next_page.to_string());
    if let Some(q) = q {
        s.append_pair("q", q);
    }
    if let Some(m) = last_month {
        s.append_pair("last_month", m);
    }
    if preview {
        s.append_pair("visitor", "1");
    }
    format!("/artportfolio/htmx/posts?{}", s.finish())
}

/// The URL the address bar should show for a search — a real page, not the
/// fragment endpoint that produced the swap. See `htmx_posts`.
///
/// Carries the preview flag for the same reason `load_more_url` does — a search
/// while previewing must push a URL that reloads back into the preview, not out
/// of it.
fn page_url(q: Option<&str>, preview: bool) -> String {
    if q.is_none() && !preview {
        return "/artportfolio".to_string();
    }
    let mut s = url::form_urlencoded::Serializer::new(String::new());
    if let Some(q) = q {
        s.append_pair("q", q);
    }
    if preview {
        s.append_pair("visitor", "1");
    }
    format!("/artportfolio?{}", s.finish())
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
    /// Gates the badge and the hover control cluster. Fed from the **effective**
    /// viewer, so a preview drops them along with the hidden posts.
    pub is_admin: bool,
}

/// A page of month sections plus Load more. Rendered by both the inline first
/// page and the HTMX pagination route, so the two cannot drift.
#[derive(Template)]
#[template(path = "artportfolio/partials/post_grid.html")]
struct PostGridTemplate {
    groups: Vec<MonthGroup>,
    /// Threaded to the card, which is `include`d from this template with
    /// `{% let %}` bindings rather than constructed. Put the field on
    /// `PostCardTemplate` alone and every card in the feed loses its badge while
    /// the upload response keeps one — which reads as a caching bug.
    is_admin: bool,
    has_more: bool,
    /// Pre-built, carrying the active query and the month this page ended on.
    load_more_url: String,
    /// Drives two things: whether an empty result renders the empty state (page
    /// 1 of nothing means no drawings; page 3 of nothing just means the end),
    /// and which single image gets `fetchpriority="high"`.
    is_first_page: bool,
    /// The active search, `""` when there is none — the empty state names it.
    q: String,
    /// The page head's label, re-rendered as an out-of-band swap, or `""` to
    /// leave the head alone.
    ///
    /// A search swaps `#feed` only, so without this the head would go on
    /// claiming "32 drawings · newest first" above a single result — a visible
    /// lie, not a stale nicety. Sent on page 0 of an HTMX request (a search or
    /// a cleared filter); never on Load more, which appends and leaves the
    /// total unchanged.
    head_label_oob: String,
}

#[derive(Deserialize)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub q: Option<String>,
    /// The month the previous page ended on, so a month split across pages
    /// renders one divider rather than two.
    pub last_month: Option<String>,
    /// `?visitor=1` — an admin asking to be shown a visitor's view.
    ///
    /// A string rather than a bool: serde parses `Option<bool>` from
    /// `true`/`false`, not from `1`.
    pub visitor: Option<String>,
}

fn is_preview(query: &PageQuery) -> bool {
    query.visitor.as_deref() == Some("1")
}

/// The single viewer every other decision on the page is made from.
///
/// Deriving the db filter and the template flags separately is the failure this
/// function exists to prevent: that combination renders a visitor's post set
/// with admin badges and controls over it, so the preview gets wrong precisely
/// the thing it exists to show.
///
/// The flag only ever downgrades. A visitor sending `?visitor=0` gains nothing,
/// because the session bool is still false.
fn effective_viewer(session_is_admin: bool, preview: bool) -> Viewer {
    if session_is_admin && !preview {
        Viewer::Admin
    } else {
        Viewer::Visitor
    }
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
    head_label_oob: Option<String>,
    viewer: Viewer,
    preview: bool,
) -> String {
    let mut posts = crate::db::get_posts_page(&state.pool, q, page, viewer).await;
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
        is_admin: viewer.is_admin(),
        load_more_url: load_more_url(page + 1, q, next_last_month.as_deref(), preview),
        is_first_page: page == 0,
        q: q.unwrap_or_default().to_string(),
        head_label_oob: head_label_oob.unwrap_or_default(),
        groups,
    }
    .render()
    .unwrap()
}

async fn feed_page(
    OptionalAuth(session_is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> impl IntoResponse {
    let preview = is_preview(&query);
    let viewer = effective_viewer(session_is_admin, preview);
    // `?q=` is read here as well as on the fragment route, so a searched feed
    // survives a reload and is linkable.
    //
    // `?page=` is deliberately ignored: the inlined first page is always page 0,
    // and deep-linking into the middle of an append-only feed would render a
    // page with no way back to the top of it. PageQuery carries the field for
    // the fragment route's sake.
    let q = normalize_q(query.q.as_deref());

    // Fetch first page here so posts arrive in the very first HTTP response.
    // Without this, the browser would load the page and then fire a second
    // request to /artportfolio/htmx/posts?page=0 before anything was visible.
    // No out-of-band label here: the shell renders the head itself.
    let initial_posts_html =
        render_grid(&state, 0, q.as_deref(), None, None, viewer, preview).await;
    let counts = crate::db::count_posts(&state.pool, q.as_deref(), viewer).await;

    Html(
        FeedTemplate {
            // The *effective* viewer, never the raw session bool. Under preview
            // this is false, so the `{% if is_admin %}` upload composer
            // disappears along with the badges — correct, since a visitor has no
            // composer, and it will read as a regression to anyone who has not
            // seen this comment.
            is_admin: viewer.is_admin(),
            // The one thing the raw session bool is for. A real visitor must
            // never see the "exit preview" affordance.
            is_previewing: session_is_admin && preview,
            head_label: head_label(&counts, q.as_deref(), viewer),
            initial_posts_html,
            q: q.unwrap_or_default(),
        }
        .render()
        .unwrap(),
    )
}

async fn htmx_posts(
    OptionalAuth(session_is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> Response {
    let page = query.page.unwrap_or(0);
    let q = normalize_q(query.q.as_deref());
    let preview = is_preview(&query);
    let viewer = effective_viewer(session_is_admin, preview);

    // Page 0 replaces the whole feed, so the head's total has to move with it.
    // Load more only appends, and pays no COUNT.
    let oob = if page == 0 {
        let counts = crate::db::count_posts(&state.pool, q.as_deref(), viewer).await;
        Some(head_label(&counts, q.as_deref(), viewer))
    } else {
        None
    };
    let html = render_grid(
        &state,
        page,
        q.as_deref(),
        query.last_month.as_deref(),
        oob,
        viewer,
        preview,
    )
    .await;

    if page == 0 {
        // Page 0 is a search or a cleared filter — the address bar should read
        // /artportfolio?q=…, a page that actually renders. `hx-push-url="true"`
        // would push this fragment endpoint instead, and reloading THAT gives a
        // bare grid with no shell, styles or nav.
        //
        // Load more (page >= 1) pushes nothing: it appends to what is already
        // on screen and must not rewrite history.
        (
            [("HX-Push-Url", page_url(q.as_deref(), preview))],
            Html(html),
        )
            .into_response()
    } else {
        Html(html).into_response()
    }
}

#[derive(Template)]
#[template(path = "artportfolio/post.html")]
struct PostPageTemplate {
    post: Post,
    is_admin: bool,
}

/// `is_admin` is not decoration here — `base.html` renders it into the
/// `IS_ADMIN` constant the command palette reads, so every page that extends the
/// shell must supply it. The 404 always reports false: it is the response a
/// visitor gets, and an admin who lands on it has asked for an id that does not
/// exist.
#[derive(Template)]
#[template(path = "artportfolio/not_found.html")]
struct NotFoundTemplate {
    is_admin: bool,
}

/// One drawing at its own URL.
///
/// This route is what makes `unlisted` mean anything. Without somewhere to reach
/// an unlisted post, unlisted and hidden would be the same state.
///
/// A hidden post and a missing id produce the same 404 page, byte for byte. A
/// distinguishable response — a 403, or a different message — would confirm the
/// row exists, which is precisely what hiding it is meant to withhold.
async fn post_permalink(
    OptionalAuth(session_is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Query(query): Query<PageQuery>,
) -> Response {
    // A previewing admin gets the visitor's 404, or the preview lies about the
    // one thing it exists to show.
    let viewer = effective_viewer(session_is_admin, is_preview(&query));

    match crate::db::get_post_by_id(&state.pool, id, viewer).await {
        Some(post) => Html(
            PostPageTemplate {
                post,
                is_admin: viewer.is_admin(),
            }
            .render()
            .unwrap(),
        )
        .into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Html(NotFoundTemplate { is_admin: false }.render().unwrap()),
        )
            .into_response(),
    }
}

#[derive(serde::Serialize)]
struct PostsResponse {
    posts: Vec<Post>,
    has_more: bool,
}

async fn api_posts(
    OptionalAuth(session_is_admin): OptionalAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<PageQuery>,
) -> impl IntoResponse {
    let page = query.page.unwrap_or(0);
    // The same filter the HTML feed applies, so the two cannot drift on what
    // "matching" means.
    let q = normalize_q(query.q.as_deref());
    // Reads the session but deliberately not the preview flag: the API has no
    // head, no pagination UI and nothing to preview.
    let viewer = effective_viewer(session_is_admin, false);
    let mut posts = crate::db::get_posts_page(&state.pool, q.as_deref(), page, viewer).await;
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
        // Registered last so the file reads in specificity order. There is no
        // collision to avoid — the two routes above are two segments deep and
        // this one is a single segment.
        .route("/artportfolio/{id}", get(post_permalink))
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
                    crate::models::Visibility::Public,
                )
                .await;
            }
            pool
        };
        let posts = crate::db::get_posts_page(&pool, None, 0, Viewer::Admin).await;
        assert!(posts.len() > 20, "expected 21 rows with has_more=true");
    }

    /// Renders one card the way every caller now does. These four assertions
    /// were written against the old `post_card_html()` format string; they are
    /// ported rather than dropped because they pin behaviour the template still
    /// owes — escaping, the picture fallback, and the empty-caption case.
    fn card(post: &crate::models::Post, is_first: bool) -> String {
        PostCardTemplate {
            post,
            is_first,
            is_admin: false,
        }
        .render()
        .unwrap()
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
            visibility: crate::models::Visibility::Public.as_str().to_string(),
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
            visibility: crate::models::Visibility::Public.as_str().to_string(),
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
            visibility: crate::models::Visibility::Public.as_str().to_string(),
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
            visibility: crate::models::Visibility::Public.as_str().to_string(),
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
            visibility: crate::models::Visibility::Public.as_str().to_string(),
        };
        let html = card(&post, false);
        assert!(
            html.contains("<picture>"),
            "picture element should always be present"
        );
        assert!(!html.contains("image/avif"), "no avif source for empty url");
        assert!(!html.contains("image/webp"), "no webp source for empty url");
    }

    /// A visitor-shaped `PostCounts` — `total` is all these assertions read.
    fn visitor_counts(total: i64) -> PostCounts {
        PostCounts {
            total,
            public: total,
            unlisted: 0,
            hidden: 0,
        }
    }

    #[test]
    fn test_head_label_states_the_real_total() {
        // Replaces the pre-count_posts version of this test, which pinned a
        // vaguer fallback ("newest first" with no number) that existed only
        // because there was no COUNT to state a total with.
        assert_eq!(
            head_label(&visitor_counts(117), None, Viewer::Visitor),
            "117 drawings · newest first"
        );
        assert_eq!(
            head_label(&visitor_counts(1), None, Viewer::Visitor),
            "1 drawing · newest first"
        );
        assert_eq!(
            head_label(&visitor_counts(0), None, Viewer::Visitor),
            "0 drawings · newest first"
        );
    }

    #[test]
    fn test_head_label_names_the_active_search() {
        assert_eq!(
            head_label(&visitor_counts(12), Some("loomis"), Viewer::Visitor),
            "12 drawings · matching \"loomis\""
        );
        assert_eq!(
            head_label(&visitor_counts(1), Some("loomis"), Viewer::Visitor),
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
            load_more_url(1, Some("100%"), Some("2026-07"), false),
            "/artportfolio/htmx/posts?page=1&q=100%25&last_month=2026-07"
        );
        assert_eq!(
            load_more_url(1, None, None, false),
            "/artportfolio/htmx/posts?page=1"
        );
        assert_eq!(
            load_more_url(3, Some("a&b"), None, false),
            "/artportfolio/htmx/posts?page=3&q=a%26b"
        );
    }

    #[test]
    fn test_page_url_is_the_page_not_the_fragment_endpoint() {
        assert_eq!(page_url(None, false), "/artportfolio");
        assert_eq!(page_url(Some("loomis"), false), "/artportfolio?q=loomis");
        assert_eq!(page_url(Some("100%"), false), "/artportfolio?q=100%25");
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

    /// Renders a fragment route and returns its body as a string.
    async fn fragment(app: Router, uri: &str) -> String {
        let resp = app
            .oneshot(Request::builder().uri(uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(body.to_vec()).unwrap()
    }

    #[tokio::test]
    async fn test_search_re_renders_the_head_label_out_of_band() {
        // A search swaps #feed alone, so the head has to travel with it or it
        // goes on stating the unfiltered total above a filtered feed.
        let html = fragment(test_app().await, "/artportfolio/htmx/posts?q=loomis").await;
        assert!(
            html.contains(r#"id="art-head-label" hx-swap-oob="true""#),
            "the fragment must carry the head label as an OOB swap: {html}"
        );
        assert!(
            html.contains("matching"),
            "and the label must name the active search: {html}"
        );
    }

    #[tokio::test]
    async fn test_load_more_leaves_the_head_label_alone() {
        let html = fragment(test_app().await, "/artportfolio/htmx/posts?page=1").await;
        assert!(
            !html.contains("hx-swap-oob"),
            "appending changes no total, so it must not touch the head: {html}"
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
                crate::models::Visibility::Public,
            )
            .await;
        }
        let hits = crate::db::get_posts_page(&pool, Some("loomis"), 0, Viewer::Admin).await;
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

    // ===== Visibility enforcement (slice 2) =====

    /// A router plus the pool behind it, so a test can seed rows and sessions
    /// the handlers will actually see. `test_app()` keeps its pool private.
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

    async fn seed(pool: &crate::db::DbPool, caption: &str, visibility: crate::models::Visibility) {
        let post = crate::db::insert_post(
            pool,
            caption,
            "https://example.com/img.jpg",
            "",
            "",
            crate::models::PostFormat::Single.as_str(),
            0,
            0,
            0,
            crate::models::Visibility::Public,
        )
        .await;
        crate::db::set_post_visibility(pool, post.id, visibility).await;
    }

    /// A session row plus the cookie header that presents it.
    async fn admin_cookie(pool: &crate::db::DbPool) -> String {
        crate::db::create_session(pool, "test-session", "2099-01-01 00:00:00").await;
        "session=test-session".to_string()
    }

    async fn body_of(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    async fn get(app: &Router, uri: &str, cookie: Option<&str>) -> axum::response::Response {
        let mut req = Request::builder().uri(uri);
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        app.clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_feed_page_visitor_omits_hidden() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "on show", crate::models::Visibility::Public).await;
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        seed(&pool, "by link only", crate::models::Visibility::Unlisted).await;
        let body = body_of(get(&app, "/artportfolio", None).await).await;
        assert!(body.contains("on show"));
        assert!(!body.contains("kept back"));
        assert!(!body.contains("by link only"));
    }

    #[tokio::test]
    async fn test_feed_page_admin_sees_all() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "on show", crate::models::Visibility::Public).await;
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let cookie = admin_cookie(&pool).await;
        let body = body_of(get(&app, "/artportfolio", Some(&cookie)).await).await;
        assert!(body.contains("on show"));
        assert!(body.contains("kept back"));
    }

    #[tokio::test]
    async fn test_htmx_posts_visitor_omits_hidden_page_0() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "on show", crate::models::Visibility::Public).await;
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let body = body_of(get(&app, "/artportfolio/htmx/posts?page=0", None).await).await;
        assert!(body.contains("on show"));
        assert!(!body.contains("kept back"));
    }

    /// The leak this task exists to close. `htmx_posts` never extracted
    /// `OptionalAuth`, so page 0 could render filtered while the first Load more
    /// handed back everything — a page-0-only test would pass throughout.
    #[tokio::test]
    async fn test_htmx_posts_visitor_omits_hidden_page_1() {
        let (app, pool) = app_with_pool().await;
        for i in 0..25 {
            seed(
                &pool,
                &format!("public {i}"),
                crate::models::Visibility::Public,
            )
            .await;
        }
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let body = body_of(get(&app, "/artportfolio/htmx/posts?page=1", None).await).await;
        assert!(!body.contains("kept back"));
    }

    #[tokio::test]
    async fn test_api_posts_visitor_omits_hidden() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "on show", crate::models::Visibility::Public).await;
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let body = body_of(get(&app, "/artportfolio/api/posts", None).await).await;
        assert!(body.contains("on show"));
        assert!(!body.contains("kept back"));
    }

    /// Unlisted is out of the API as well as the feed. Serving it here would
    /// make the JSON endpoint a way to enumerate exactly what the feed hides.
    #[tokio::test]
    async fn test_api_posts_visitor_omits_unlisted() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "by link only", crate::models::Visibility::Unlisted).await;
        let body = body_of(get(&app, "/artportfolio/api/posts", None).await).await;
        assert!(!body.contains("by link only"));
    }

    #[tokio::test]
    async fn test_load_more_url_carries_visitor_flag() {
        assert!(load_more_url(1, None, None, true).contains("visitor=1"));
        assert!(!load_more_url(1, None, None, false).contains("visitor"));
    }

    #[tokio::test]
    async fn test_page_url_carries_visitor_flag() {
        let url = page_url(Some("cat"), true);
        assert!(url.contains("q=cat"), "{url}");
        assert!(url.contains("visitor=1"), "{url}");
        assert_eq!(page_url(None, false), "/artportfolio");
        assert_eq!(page_url(None, true), "/artportfolio?visitor=1");
    }

    #[tokio::test]
    async fn test_effective_viewer_preview_downgrades_admin() {
        assert_eq!(effective_viewer(true, false), Viewer::Admin);
        assert_eq!(effective_viewer(true, true), Viewer::Visitor);
    }

    /// The flag only ever downgrades — a visitor cannot promote themselves by
    /// omitting it, or by sending any value at all.
    #[tokio::test]
    async fn test_effective_viewer_preview_flag_cannot_promote() {
        assert_eq!(effective_viewer(false, false), Viewer::Visitor);
        assert_eq!(effective_viewer(false, true), Viewer::Visitor);
    }

    #[tokio::test]
    async fn test_preview_hides_hidden_posts_from_admin() {
        let (app, pool) = app_with_pool().await;
        seed(&pool, "on show", crate::models::Visibility::Public).await;
        seed(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let cookie = admin_cookie(&pool).await;
        let body = body_of(get(&app, "/artportfolio?visitor=1", Some(&cookie)).await).await;
        assert!(body.contains("on show"));
        assert!(!body.contains("kept back"));
    }

    // ===== Permalink (slice 2) =====

    /// Returns the post id as well, since the permalink is addressed by it.
    async fn seed_id(
        pool: &crate::db::DbPool,
        caption: &str,
        visibility: crate::models::Visibility,
    ) -> i64 {
        let post = crate::db::insert_post(
            pool,
            caption,
            "https://example.com/img.jpg",
            "",
            "",
            crate::models::PostFormat::Single.as_str(),
            0,
            0,
            0,
            crate::models::Visibility::Public,
        )
        .await;
        crate::db::set_post_visibility(pool, post.id, visibility).await;
        post.id
    }

    #[tokio::test]
    async fn test_permalink_public_is_200_for_visitor() {
        let (app, pool) = app_with_pool().await;
        let id = seed_id(&pool, "on show", crate::models::Visibility::Public).await;
        let resp = get(&app, &format!("/artportfolio/{id}"), None).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_of(resp).await.contains("on show"));
    }

    /// The state's entire reason to exist: out of the feed, still served here.
    #[tokio::test]
    async fn test_permalink_unlisted_is_200_for_visitor() {
        let (app, pool) = app_with_pool().await;
        let id = seed_id(&pool, "by link only", crate::models::Visibility::Unlisted).await;
        let resp = get(&app, &format!("/artportfolio/{id}"), None).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_of(resp).await.contains("by link only"));
    }

    #[tokio::test]
    async fn test_permalink_hidden_is_404_for_visitor() {
        let (app, pool) = app_with_pool().await;
        let id = seed_id(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let resp = get(&app, &format!("/artportfolio/{id}"), None).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert!(!body_of(resp).await.contains("kept back"));
    }

    #[tokio::test]
    async fn test_permalink_hidden_is_200_for_admin() {
        let (app, pool) = app_with_pool().await;
        let id = seed_id(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let cookie = admin_cookie(&pool).await;
        let resp = get(&app, &format!("/artportfolio/{id}"), Some(&cookie)).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert!(body_of(resp).await.contains("kept back"));
    }

    /// A hidden post and a missing id must be indistinguishable from outside —
    /// same status *and* same bytes. Anything that differed would confirm the
    /// row exists.
    #[tokio::test]
    async fn test_permalink_unknown_id_is_404() {
        let (app, pool) = app_with_pool().await;
        let hidden = seed_id(&pool, "kept back", crate::models::Visibility::Hidden).await;

        let missing_resp = get(&app, "/artportfolio/999999", None).await;
        assert_eq!(missing_resp.status(), StatusCode::NOT_FOUND);
        let missing_body = body_of(missing_resp).await;

        let hidden_resp = get(&app, &format!("/artportfolio/{hidden}"), None).await;
        let hidden_body = body_of(hidden_resp).await;

        assert_eq!(missing_body, hidden_body);
    }

    #[tokio::test]
    async fn test_permalink_hidden_is_404_for_previewing_admin() {
        let (app, pool) = app_with_pool().await;
        let id = seed_id(&pool, "kept back", crate::models::Visibility::Hidden).await;
        let cookie = admin_cookie(&pool).await;
        let resp = get(
            &app,
            &format!("/artportfolio/{id}?visitor=1"),
            Some(&cookie),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    /// The card has to link somewhere, or the permalink is unreachable by
    /// anything but a typed URL.
    #[tokio::test]
    async fn test_card_links_to_its_permalink() {
        let post = sample_post(42, "linked");
        assert!(card(&post, false).contains("href=\"/artportfolio/42\""));
    }
}
