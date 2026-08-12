use crate::AppState;
use askama::Template;
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, patch, post},
    Form, Router,
};
use std::sync::Arc;

const MAX_IMAGE_BYTES: usize = 35 * 1024 * 1024; // 35 MB

async fn encode_as_avif(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        let img = image::load_from_memory(&bytes).map_err(|e| format!("decode failed: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels: Vec<rgb::RGBA8> = rgba
            .pixels()
            .map(|p| rgb::RGBA8 {
                r: p[0],
                g: p[1],
                b: p[2],
                a: p[3],
            })
            .collect();
        let encoded = ravif::Encoder::new()
            .with_quality(80.0)
            .with_speed(6)
            .encode_rgba(ravif::Img::new(&pixels, width as usize, height as usize))
            .map_err(|e| format!("avif encode failed: {e}"))?;
        Ok(encoded.avif_file)
    })
    .await
    .map_err(|e| format!("spawn_blocking panicked: {e}"))?
}

#[derive(Template)]
#[template(path = "admin.html")]
struct AdminTemplate;

async fn admin_page(_session: crate::middleware::AuthSession) -> impl IntoResponse {
    Html(AdminTemplate.render().unwrap())
}

// HTMX partial — list of posts for admin view
async fn htmx_admin_posts(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    // `Viewer::Admin`, and this is the call site that fails quietly if it is
    // not: `Visitor` still compiles, still renders, and simply stops listing the
    // posts an admin most needs to see from the dashboard.
    let posts = crate::db::get_posts_page(
        &state.pool,
        &crate::models::PostFilter::default(),
        0,
        crate::models::Viewer::Admin,
    )
    .await;
    let mut html = String::new();
    for post in &posts {
        html.push_str(&admin_post_card_html(post));
    }
    if html.is_empty() {
        html = "<p>No posts yet.</p>".to_string();
    }
    Html(html)
}

async fn upload_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut caption = None::<String>;
    let mut image_data = None::<(Vec<u8>, String, &'static str)>; // (bytes, content_type, ext)
    let mut post_format = crate::models::PostFormat::Single.as_str().to_string();
    let mut source = "admin".to_string();
    // Absent means public — the normal case, since no upload control sends this
    // yet. Present but unrecognised is a 400, never a silent coercion.
    let mut visibility = crate::models::Visibility::default();
    let mut bad_visibility = false;

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
            }
            Some("format") => {
                if let Ok(v) = field.text().await {
                    post_format = v;
                }
            }
            Some("visibility") => {
                if let Ok(v) = field.text().await {
                    match crate::models::Visibility::from_str(&v) {
                        Some(parsed) => visibility = parsed,
                        None => bad_visibility = true,
                    }
                }
            }
            Some("source") => {
                // Only accept known source values; default to "admin"
                if let Ok(v) = field.text().await {
                    source = if v == "gallery" {
                        v
                    } else {
                        "admin".to_string()
                    };
                }
            }
            Some("image") => {
                let content_type = field
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if !matches!(
                    content_type.as_str(),
                    "image/jpeg" | "image/png" | "image/webp"
                ) {
                    return (
                        StatusCode::BAD_REQUEST,
                        Html("Invalid image type".to_string()),
                    )
                        .into_response();
                }

                let bytes = field.bytes().await.unwrap_or_default();

                if bytes.len() > MAX_IMAGE_BYTES {
                    return (
                        StatusCode::PAYLOAD_TOO_LARGE,
                        Html("Image too large (max 35MB)".to_string()),
                    )
                        .into_response();
                }

                let ext = match validate_magic_bytes(&bytes) {
                    Some(ext) => ext,
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Html("Invalid image file".to_string()),
                        )
                            .into_response()
                    }
                };

                image_data = Some((bytes.to_vec(), format!("image/{ext}"), ext));
            }
            _ => {}
        }
    }

    if bad_visibility {
        return (
            StatusCode::BAD_REQUEST,
            Html("Unknown visibility".to_string()),
        )
            .into_response();
    }

    let caption = caption.unwrap_or_default();
    let (bytes, content_type, ext) = match image_data {
        Some(d) => d,
        None => {
            return (StatusCode::BAD_REQUEST, Html("Missing image".to_string())).into_response()
        }
    };

    // Client sends WebP (converted via canvas); upload original directly and return immediately.
    // AVIF encoding is detached into a background task — it backfills avif_url after the
    // response is sent. Failure is non-fatal: avif_url stays empty, <picture> falls back to WebP.
    let bytes_for_avif = bytes.clone();
    let file_size_bytes = bytes.len() as i64;

    // Intrinsic dimensions for the feed's masonry, read from the image header.
    //
    // Header-only on purpose: `into_dimensions()` parses just enough to get the
    // size and never touches pixel data. A full `load_from_memory` here would
    // add seconds of latency to every upload — up to 35 MB — to obtain two
    // integers. (The only full decode in this file is in `encode_as_avif`,
    // which runs detached after the response is sent, so its dimensions are
    // not available to us here.)
    //
    // `(0, 0)` on an unparseable header is the deliberate degradation: the card
    // then renders without width/height, exactly as it did before migration 012.
    // Dimensions are a layout optimisation, not grounds to reject a drawing.
    let (image_width, image_height) = image::ImageReader::new(std::io::Cursor::new(&bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_dimensions().ok())
        .map(|(w, h)| (w as i64, h as i64))
        .unwrap_or((0, 0));
    let uuid = uuid::Uuid::new_v4().to_string();
    let original_key = format!("{uuid}.{ext}");
    let avif_key = format!("{uuid}-avif.avif");

    let image_url = match state
        .storage
        .upload(&original_key, bytes, &content_type)
        .await
    {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("storage upload error: {e}");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Html("Upload failed".to_string()),
            )
                .into_response();
        }
    };
    let webp_url = image_url.clone(); // already WebP from client

    let post = crate::db::insert_post(
        &state.pool,
        caption.trim(),
        &image_url,
        &webp_url,
        "",
        &post_format,
        file_size_bytes,
        image_width,
        image_height,
        visibility,
    )
    .await;
    tracing::info!("post created: id={}, key={original_key}, size={file_size_bytes} bytes, format={post_format}", post.id);

    let state_clone = Arc::clone(&state);
    let post_id = post.id;
    tokio::spawn(async move {
        match encode_as_avif(bytes_for_avif).await {
            Ok(avif_bytes) => {
                match state_clone
                    .storage
                    .upload(&avif_key, avif_bytes, "image/avif")
                    .await
                {
                    Ok(avif_url) => {
                        if let Err(e) =
                            crate::db::update_post_avif_url(&state_clone.pool, post_id, &avif_url)
                                .await
                        {
                            tracing::error!("avif db update failed for post id={post_id}: {e}");
                        } else {
                            tracing::info!("avif variant ready for post id={post_id}");
                        }
                    }
                    Err(e) => tracing::error!("avif upload failed for post id={post_id}: {e}"),
                }
            }
            Err(e) => tracing::warn!("avif encode failed for post id={post_id}: {e}"),
        }
    });

    // Third caller of the feed's card template, and the easy one to miss: the
    // composer on /artportfolio posts here with source=gallery and swaps the
    // response into #feed. Render the same template the feed uses, or a fresh
    // upload lands as legacy .post-card markup among hm-post cards and looks
    // broken until reload — with the build still green.
    let card_html = if source == "gallery" {
        crate::routes::feed::PostCardTemplate {
            post: &post,
            is_first: false,
            // The composer is behind `{% if is_admin %}`, so only an admin can
            // ever reach this response.
            is_admin: true,
        }
        .render()
        .unwrap()
    } else {
        admin_post_card_html(&post)
    };
    Html(card_html).into_response()
}

async fn delete_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(urls) = crate::db::delete_post_and_get_urls(&state.pool, id).await {
        tracing::info!("deleting post id={id}");
        for url in [&urls.image_url, &urls.webp_url, &urls.avif_url] {
            if url.is_empty() {
                continue;
            } // old posts may have no variants
            if let Err(e) = state.storage.delete_by_url(url).await {
                tracing::error!("storage delete failed for post id={id} url={url}: {e}");
            }
        }
    } else {
        tracing::warn!("delete requested for nonexistent post id={id}");
    }
    StatusCode::OK
}

/// Changes one post's visibility and returns the re-rendered card.
///
/// `AuthSession` is the only gate — the admin router carries no middleware
/// layer, so the extractor on this handler is what stands between a visitor and
/// unhiding anything.
///
/// Returning the card rather than a status is what keeps the badge and the
/// dimming in step with the database without a reload: the button swaps it
/// `outerHTML` into `closest .hm-post`.
#[derive(serde::Deserialize)]
pub struct VisibilityForm {
    pub visibility: String,
}

async fn patch_visibility(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    // `Form`, not `Multipart`: this is what HTMX sends for an `hx-patch` with
    // `hx-vals` and no `hx-encoding`. Multipart would mean an extra attribute of
    // ceremony on every button, for a body that is one short string.
    Form(form): Form<VisibilityForm>,
) -> impl IntoResponse {
    // Fail loudly, not closed. `Visibility::from_row` coerces an unknown value
    // to Hidden when reading a row, because a value nobody recognises must not
    // render to the public — but doing that to a *request* would report success
    // for a typo.
    let visibility = match crate::models::Visibility::from_str(&form.visibility) {
        Some(v) => v,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Html("Unknown visibility".to_string()),
            )
                .into_response()
        }
    };

    if !crate::db::set_post_visibility(&state.pool, id, visibility).await {
        tracing::warn!("visibility change requested for nonexistent post id={id}");
        return (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response();
    }
    tracing::info!("post id={id} visibility set to {}", visibility.as_str());

    // Re-read rather than patching the struct in memory, so the response
    // reflects the row as stored.
    match crate::db::get_post_by_id(&state.pool, id, crate::models::Viewer::Admin).await {
        Some(post) => Html(
            crate::routes::feed::PostCardTemplate {
                post: &post,
                is_first: false,
                is_admin: true,
            }
            .render()
            .unwrap(),
        )
        .into_response(),
        None => (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response(),
    }
}

// ===== Collections, tags, captions (slice 3) =====

#[derive(Template)]
#[template(path = "artportfolio/partials/rail_collections.html")]
struct RailCollectionsTemplate {
    /// The raw list, same order as `rail_collections` below (the latter is
    /// built from it via `collection_rail_links`) — kept alongside it so the
    /// admin delete control has a numeric id. `RailLink` carries none: it is
    /// the same struct `tag_rail_links` builds from a source that has no id
    /// at all.
    collections: Vec<crate::models::CollectionWithCount>,
    rail_collections: Vec<crate::routes::feed::RailLink>,
    is_admin: bool,
}

#[derive(Template)]
#[template(path = "artportfolio/partials/card_edit_popover.html")]
struct CardEditTemplate {
    post_id: i64,
    caption: String,
    tags_joined: String,
}

/// One row of the collection-membership checklist. Not `Collection` or
/// `CollectionWithCount` because neither carries `member` — this is where the
/// two are joined for a single post.
pub struct ChecklistItem {
    pub id: i64,
    pub name: String,
    pub member: bool,
}

#[derive(Template)]
#[template(path = "artportfolio/partials/collection_checklist.html")]
struct CollectionChecklistTemplate {
    post_id: i64,
    items: Vec<ChecklistItem>,
}

/// Assembles the checklist fragment for one post — every collection, each
/// marked `member` against that post's own membership rows. Factored out
/// because three routes (add, remove, GET) all end with exactly this render.
async fn checklist_fragment(pool: &crate::db::DbPool, post_id: i64) -> String {
    let collections =
        crate::db::list_collections_with_counts(pool, crate::models::Viewer::Admin).await;
    let member_ids = crate::db::get_post_collection_ids(pool, post_id).await;
    let items = collections
        .into_iter()
        .map(|c| ChecklistItem {
            id: c.id,
            name: c.name,
            member: member_ids.contains(&c.id),
        })
        .collect();
    CollectionChecklistTemplate { post_id, items }
        .render()
        .unwrap()
}

#[derive(serde::Deserialize)]
pub struct CollectionNameForm {
    pub name: String,
}

/// Creates a collection and returns the rail fragment. A duplicate slug is a
/// 409 carrying the existing collection's name, not a silent no-op — the
/// admin typed a name expecting it to exist as typed, and needs to know it
/// already does under someone else's capitalization.
async fn create_collection_route(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Form(form): Form<CollectionNameForm>,
) -> impl IntoResponse {
    match crate::db::create_collection(&state.pool, form.name.trim()).await {
        Ok(_) => {
            let collections =
                crate::db::list_collections_with_counts(&state.pool, crate::models::Viewer::Admin)
                    .await;
            // Admin, default filter: this fragment renders outside any active
            // search/tag/vis context, so highlighting an active collection
            // here is not meaningful — the accepted rail-staleness trade-off.
            let rail_collections = crate::routes::feed::collection_rail_links(
                &collections,
                &crate::models::PostFilter::default(),
                false,
            );
            let html = RailCollectionsTemplate {
                collections,
                rail_collections,
                is_admin: true,
            }
            .render()
            .unwrap();
            (StatusCode::CREATED, Html(html)).into_response()
        }
        Err(crate::models::CreateCollectionError::InvalidName) => {
            (StatusCode::BAD_REQUEST, Html("Invalid name".to_string())).into_response()
        }
        Err(crate::models::CreateCollectionError::DuplicateSlug(name)) => (
            StatusCode::CONFLICT,
            Html(format!(
                "A collection named \"{}\" already exists.",
                html_escape(&name)
            )),
        )
            .into_response(),
    }
}

/// Deletes a collection and returns the rail fragment. Idempotent by
/// contract — an unknown id still renders the (unchanged) fragment rather
/// than 404ing, since the caller's next view is the same rail either way.
async fn delete_collection_route(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let _ = crate::db::delete_collection(&state.pool, id).await;
    let collections =
        crate::db::list_collections_with_counts(&state.pool, crate::models::Viewer::Admin).await;
    let rail_collections = crate::routes::feed::collection_rail_links(
        &collections,
        &crate::models::PostFilter::default(),
        false,
    );
    Html(
        RailCollectionsTemplate {
            collections,
            rail_collections,
            is_admin: true,
        }
        .render()
        .unwrap(),
    )
}

#[derive(serde::Deserialize)]
pub struct PatchPostForm {
    pub caption: String,
    pub tags: String,
}

/// Replaces a post's caption and tag set, returning the re-rendered card —
/// same swap contract as `patch_visibility`: `hx-target="closest .hm-post"
/// hx-swap="outerHTML"`.
async fn patch_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Form(form): Form<PatchPostForm>,
) -> impl IntoResponse {
    if !crate::db::update_post_caption(&state.pool, id, form.caption.trim()).await {
        return (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response();
    }
    let tags = crate::db::normalize_tags(&form.tags);
    crate::db::set_post_tags(&state.pool, id, &tags).await;

    match crate::db::get_post_by_id(&state.pool, id, crate::models::Viewer::Admin).await {
        Some(post) => Html(
            crate::routes::feed::PostCardTemplate {
                post: &post,
                is_first: false,
                is_admin: true,
            }
            .render()
            .unwrap(),
        )
        .into_response(),
        None => (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response(),
    }
}

/// Adds a post to a collection, 404ing when either side of the pair is
/// missing — `add_post_to_collection` is the function that actually checks
/// existence, unlike its `remove` counterpart below.
async fn add_post_collection(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path((id, cid)): Path<(i64, i64)>,
) -> impl IntoResponse {
    if !crate::db::add_post_to_collection(&state.pool, id, cid).await {
        return (
            StatusCode::NOT_FOUND,
            Html("No such post or collection".to_string()),
        )
            .into_response();
    }
    Html(checklist_fragment(&state.pool, id).await).into_response()
}

/// Removes a post from a collection. `remove_post_from_collection` is
/// idempotent by contract (always `true`) — a stale checklist toggle for a
/// row that is already gone still re-renders 200, never a 404.
async fn remove_post_collection(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path((id, cid)): Path<(i64, i64)>,
) -> impl IntoResponse {
    if !crate::db::remove_post_from_collection(&state.pool, id, cid).await {
        return (
            StatusCode::NOT_FOUND,
            Html("No such post or collection".to_string()),
        )
            .into_response();
    }
    Html(checklist_fragment(&state.pool, id).await).into_response()
}

/// The edit popover, prefilled with the post's current caption and tags.
/// Exists as a GET because `Post` carries no tags — an empty tags input
/// paired with the PATCH's replace-all semantics would silently wipe a
/// post's tags on the first Save nobody meant to touch.
async fn edit_post_fragment(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let post = match crate::db::get_post_by_id(&state.pool, id, crate::models::Viewer::Admin).await
    {
        Some(p) => p,
        None => return (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response(),
    };
    let tags_joined = crate::db::get_post_tags(&state.pool, id).await.join(", ");
    Html(
        CardEditTemplate {
            post_id: id,
            caption: post.caption,
            tags_joined,
        }
        .render()
        .unwrap(),
    )
    .into_response()
}

/// The membership checklist for one post, fetched fresh — used to (re)open
/// the checklist popover, not just to refresh it after a toggle.
async fn collections_checklist_fragment(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if crate::db::get_post_by_id(&state.pool, id, crate::models::Viewer::Admin)
        .await
        .is_none()
    {
        return (StatusCode::NOT_FOUND, Html("No such post".to_string())).into_response();
    }
    Html(checklist_fragment(&state.pool, id).await).into_response()
}

pub fn validate_magic_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("jpeg");
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("png");
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Some("webp");
    }
    None
}

fn admin_post_card_html(post: &crate::models::Post) -> String {
    let caption_html = if post.caption.is_empty() {
        String::new()
    } else {
        format!("    <p>{}</p>\n", html_escape(&post.caption))
    };
    let avif_source = if !post.avif_url.is_empty() {
        format!(
            "    <source srcset=\"{}\" type=\"image/avif\">\n",
            html_escape(&post.avif_url)
        )
    } else {
        String::new()
    };
    let webp_source = if !post.webp_url.is_empty() {
        format!(
            "    <source srcset=\"{}\" type=\"image/webp\">\n",
            html_escape(&post.webp_url)
        )
    } else {
        String::new()
    };
    format!(
        r##"<div class="admin-post" id="admin-post-{}">
  <picture>
{avif_source}{webp_source}    <img src="{}" alt="">
  </picture>
  <div class="info">
{caption_html}    <small>{}</small>
  </div>
  <button class="delete-btn"
          hx-delete="/api/admin/posts/{}"
          hx-target="#admin-post-{}"
          hx-swap="outerHTML"
          hx-confirm="Delete this post?">
    Delete
  </button>
</div>"##,
        post.id,
        html_escape(&post.image_url),
        html_escape(&post.created_at),
        post.id,
        post.id,
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/admin", get(admin_page))
        .route("/htmx/admin/posts", get(htmx_admin_posts))
        .route("/api/admin/posts", post(upload_post))
        .route(
            "/api/admin/posts/{id}",
            delete(delete_post).patch(patch_post),
        )
        .route("/api/admin/posts/{id}/visibility", patch(patch_visibility))
        .route("/api/admin/collections", post(create_collection_route))
        .route(
            "/api/admin/collections/{id}",
            delete(delete_collection_route),
        )
        .route(
            "/api/admin/posts/{id}/collections",
            get(collections_checklist_fragment),
        )
        .route(
            "/api/admin/posts/{id}/collections/{cid}",
            post(add_post_collection).delete(remove_post_collection),
        )
        .route("/api/admin/posts/{id}/edit", get(edit_post_fragment))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid 1x1 red PNG (generated with correct CRCs)
    fn test_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48,
            0x44, 0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00,
            0x00, 0x90, 0x77, 0x53, 0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x78,
            0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x03, 0x01, 0x01, 0x00, 0xC9, 0xFE, 0x92,
            0xEF, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
        ]
    }

    #[test]
    fn test_magic_bytes_jpeg() {
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        assert_eq!(validate_magic_bytes(&jpeg), Some("jpeg"));
    }

    #[test]
    fn test_magic_bytes_png() {
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        assert_eq!(validate_magic_bytes(&png), Some("png"));
    }

    #[test]
    fn test_magic_bytes_invalid() {
        let bad = vec![0x00, 0x01, 0x02];
        assert_eq!(validate_magic_bytes(&bad), None);
    }

    #[test]
    fn test_magic_bytes_webp() {
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // file size (ignored)
        webp.extend_from_slice(b"WEBP");
        assert_eq!(validate_magic_bytes(&webp), Some("webp"));
    }

    #[test]
    fn test_html_escape_special_chars() {
        assert_eq!(html_escape("a & b"), "a &amp; b");
        assert_eq!(html_escape("<script>"), "&lt;script&gt;");
        assert_eq!(html_escape("\"quoted\""), "&quot;quoted&quot;");
    }

    #[tokio::test]
    async fn test_encode_as_avif_returns_nonempty_bytes() {
        let result = encode_as_avif(test_png()).await;
        assert!(
            result.is_ok(),
            "avif encode should succeed: {:?}",
            result.err()
        );
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        // AVIF is an ISOBMFF container; bytes 4..8 are the 'ftyp' box type
        assert_eq!(
            &bytes[4..8],
            b"ftyp",
            "output should be an AVIF/ISOBMFF file"
        );
    }

    // ===== Visibility PATCH route (slice 2) =====

    use axum::body::Body;
    use axum::http::{Request, StatusCode as HttpStatus};
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

    async fn seed_post(pool: &crate::db::DbPool, caption: &str) -> i64 {
        crate::db::insert_post(
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
        .await
        .id
    }

    async fn admin_cookie(pool: &crate::db::DbPool) -> String {
        crate::db::create_session(pool, "test-session", "2099-01-01 00:00:00").await;
        "session=test-session".to_string()
    }

    async fn patch_visibility_req(
        app: &Router,
        id: i64,
        value: &str,
        cookie: Option<&str>,
    ) -> axum::response::Response {
        // Form-encoded, matching what HTMX sends for an hx-patch.
        let mut req = Request::builder()
            .method("PATCH")
            .uri(format!("/api/admin/posts/{id}/visibility"))
            .header("content-type", "application/x-www-form-urlencoded");
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        app.clone()
            .oneshot(req.body(Body::from(format!("visibility={value}"))).unwrap())
            .await
            .unwrap()
    }

    async fn stored_visibility(pool: &crate::db::DbPool, id: i64) -> String {
        crate::db::get_post_by_id(pool, id, crate::models::Viewer::Admin)
            .await
            .unwrap()
            .visibility
    }

    /// The only gate on this route. The admin router carries no middleware
    /// layer, so `AuthSession` on the handler is what stands between a visitor
    /// and unhiding anything.
    #[tokio::test]
    async fn test_patch_visibility_requires_session() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "guarded").await;
        let resp = patch_visibility_req(&app, id, "hidden", None).await;
        assert_ne!(resp.status(), HttpStatus::OK);
        assert_eq!(stored_visibility(&pool, id).await, "public");
    }

    #[tokio::test]
    async fn test_patch_visibility_sets_state() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "moves").await;
        let cookie = admin_cookie(&pool).await;
        let resp = patch_visibility_req(&app, id, "hidden", Some(&cookie)).await;
        assert_eq!(resp.status(), HttpStatus::OK);
        assert_eq!(stored_visibility(&pool, id).await, "hidden");
    }

    /// Fail loudly, not closed. `from_row` coerces an unknown value to Hidden
    /// when reading a row; doing that to a request would report success for a
    /// typo.
    #[tokio::test]
    async fn test_patch_visibility_unknown_string_is_400() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "unchanged").await;
        let cookie = admin_cookie(&pool).await;
        let resp = patch_visibility_req(&app, id, "bogus", Some(&cookie)).await;
        assert_eq!(resp.status(), HttpStatus::BAD_REQUEST);
        assert_eq!(stored_visibility(&pool, id).await, "public");
    }

    #[tokio::test]
    async fn test_patch_visibility_unknown_id_is_404() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let resp = patch_visibility_req(&app, 999999, "hidden", Some(&cookie)).await;
        assert_eq!(resp.status(), HttpStatus::NOT_FOUND);
    }

    /// The response is the re-rendered card, which is what keeps the badge and
    /// the dimming in step with the database without a reload.
    #[tokio::test]
    async fn test_patch_visibility_returns_card_markup() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "swapped").await;
        let cookie = admin_cookie(&pool).await;
        let resp = patch_visibility_req(&app, id, "unlisted", Some(&cookie)).await;
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        let body = String::from_utf8(bytes.to_vec()).unwrap();
        assert!(body.contains("hm-post"), "{body}");
        assert!(body.contains("swapped"), "{body}");
    }

    #[tokio::test]
    async fn test_upload_absent_visibility_defaults_public() {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        crate::db::run_migrations(&pool).await;
        let id = seed_post(&pool, "default").await;
        assert_eq!(stored_visibility(&pool, id).await, "public");
    }

    #[tokio::test]
    async fn test_visibility_default_is_public() {
        assert_eq!(
            crate::models::Visibility::default(),
            crate::models::Visibility::Public
        );
    }

    // ===== Collections, tags, captions (slice 3) =====

    async fn body_text(resp: axum::response::Response) -> String {
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .unwrap();
        String::from_utf8(bytes.to_vec()).unwrap()
    }

    async fn form_req(method: &str, uri: &str, body: &str, cookie: Option<&str>) -> Request<Body> {
        let mut req = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/x-www-form-urlencoded");
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        req.body(Body::from(body.to_string())).unwrap()
    }

    async fn empty_req(method: &str, uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut req = Request::builder().method(method).uri(uri);
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        req.body(Body::empty()).unwrap()
    }

    /// Every new mutation/fragment route is gated by `AuthSession` alone — the
    /// admin router carries no middleware layer, so this loop is what proves
    /// none of the seven slipped through ungated.
    #[tokio::test]
    async fn test_collections_routes_require_session() {
        let (app, pool) = app_with_pool().await;
        let post_id = seed_post(&pool, "guarded").await;

        let reqs = vec![
            form_req("POST", "/api/admin/collections", "name=Nope", None).await,
            empty_req("DELETE", "/api/admin/collections/1", None).await,
            form_req(
                "PATCH",
                &format!("/api/admin/posts/{post_id}"),
                "caption=x&tags=y",
                None,
            )
            .await,
            empty_req(
                "POST",
                &format!("/api/admin/posts/{post_id}/collections/1"),
                None,
            )
            .await,
            empty_req(
                "DELETE",
                &format!("/api/admin/posts/{post_id}/collections/1"),
                None,
            )
            .await,
            empty_req("GET", &format!("/api/admin/posts/{post_id}/edit"), None).await,
            empty_req(
                "GET",
                &format!("/api/admin/posts/{post_id}/collections"),
                None,
            )
            .await,
        ];

        for req in reqs {
            let method = req.method().clone();
            let uri = req.uri().clone();
            let resp = app.clone().oneshot(req).await.unwrap();
            // `AuthSession`'s rejection is always `Redirect::to("/admin/login")`,
            // i.e. 303 See Other — assert that exact status, not merely
            // "not 200". `create_collection_route` succeeds with 201, and
            // `assert_ne!(status, OK)` cannot tell a missing `_session`
            // extractor (which would 201 straight through) from a present one.
            assert_eq!(
                resp.status(),
                HttpStatus::SEE_OTHER,
                "{method} {uri} should redirect to login without a session"
            );
        }

        // Belt and braces on the same blind spot: even if some future
        // rejection shape stopped being a clean 303, no route above may have
        // actually mutated the database.
        assert!(
            crate::db::list_collections_with_counts(&pool, crate::models::Viewer::Admin)
                .await
                .is_empty(),
            "an unauthenticated request must not create a collection"
        );
        assert!(
            crate::db::get_post_collection_ids(&pool, post_id)
                .await
                .is_empty(),
            "an unauthenticated request must not add the post to a collection"
        );
    }

    #[tokio::test]
    async fn test_create_collection_201_with_fragment() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let resp = app
            .clone()
            .oneshot(
                form_req(
                    "POST",
                    "/api/admin/collections",
                    "name=Figure Studies",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::CREATED);
        let body = body_text(resp).await;
        assert!(body.contains(r#"id="rail-collections""#), "{body}");
        assert!(body.contains("Figure Studies"), "{body}");
    }

    #[tokio::test]
    async fn test_create_collection_duplicate_is_409() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let first = app
            .clone()
            .oneshot(
                form_req(
                    "POST",
                    "/api/admin/collections",
                    "name=Figure Studies",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(first.status(), HttpStatus::CREATED);

        // Different case, same slug — the point is that the 409 body carries
        // the *stored* collection's name ("Figure Studies", display case
        // preserved from the first insert), not merely an echo of whatever
        // this request happened to send. Sending the same literal string
        // twice couldn't tell those two apart.
        let second = app
            .clone()
            .oneshot(
                form_req(
                    "POST",
                    "/api/admin/collections",
                    "name=figure studies",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(second.status(), HttpStatus::CONFLICT);
        let body = body_text(second).await;
        assert!(body.contains("Figure Studies"), "{body}");
    }

    #[tokio::test]
    async fn test_create_collection_junk_name_is_400() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let resp = app
            .clone()
            .oneshot(
                form_req(
                    "POST",
                    "/api/admin/collections",
                    "name=%21%21%21", // "!!!"
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_delete_collection_returns_fragment() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let created = crate::db::create_collection(&pool, "Figure Studies")
            .await
            .unwrap();

        let resp = app
            .clone()
            .oneshot(
                empty_req(
                    "DELETE",
                    &format!("/api/admin/collections/{}", created.id),
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        let body = body_text(resp).await;
        assert!(body.contains(r#"id="rail-collections""#), "{body}");

        let remaining =
            crate::db::list_collections_with_counts(&pool, crate::models::Viewer::Admin).await;
        assert!(remaining.iter().all(|c| c.id != created.id));
    }

    #[tokio::test]
    async fn test_patch_post_updates_caption_and_tags() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "old caption").await;
        let cookie = admin_cookie(&pool).await;

        let resp = app
            .clone()
            .oneshot(
                form_req(
                    "PATCH",
                    &format!("/api/admin/posts/{id}"),
                    "caption=New caption&tags=Ink, wash",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        let body = body_text(resp).await;
        assert!(body.contains("hm-post"), "{body}");
        assert!(body.contains("New caption"), "{body}");

        assert_eq!(
            crate::db::get_post_tags(&pool, id).await,
            vec!["ink".to_string(), "wash".to_string()]
        );
    }

    #[tokio::test]
    async fn test_patch_post_replaces_tags() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "caption").await;
        let cookie = admin_cookie(&pool).await;
        crate::db::set_post_tags(&pool, id, &["old".to_string()]).await;

        let resp = app
            .clone()
            .oneshot(
                form_req(
                    "PATCH",
                    &format!("/api/admin/posts/{id}"),
                    "caption=caption&tags=new",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        assert_eq!(
            crate::db::get_post_tags(&pool, id).await,
            vec!["new".to_string()]
        );
    }

    #[tokio::test]
    async fn test_patch_post_unknown_id_is_404() {
        let (app, pool) = app_with_pool().await;
        let cookie = admin_cookie(&pool).await;
        let resp = app
            .clone()
            .oneshot(
                form_req(
                    "PATCH",
                    "/api/admin/posts/999999",
                    "caption=x&tags=y",
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_membership_add_then_remove() {
        let (app, pool) = app_with_pool().await;
        let post_id = seed_post(&pool, "member post").await;
        let cookie = admin_cookie(&pool).await;
        let collection = crate::db::create_collection(&pool, "Ink Studies")
            .await
            .unwrap();

        let add_resp = app
            .clone()
            .oneshot(
                empty_req(
                    "POST",
                    &format!("/api/admin/posts/{post_id}/collections/{}", collection.id),
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(add_resp.status(), HttpStatus::OK);
        let add_body = body_text(add_resp).await;
        assert!(add_body.contains("art-checklist"), "{add_body}");
        assert!(add_body.contains("checked"), "{add_body}");
        assert_eq!(
            crate::db::get_post_collection_ids(&pool, post_id).await,
            vec![collection.id]
        );

        let remove_resp = app
            .clone()
            .oneshot(
                empty_req(
                    "DELETE",
                    &format!("/api/admin/posts/{post_id}/collections/{}", collection.id),
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(remove_resp.status(), HttpStatus::OK);
        let remove_body = body_text(remove_resp).await;
        assert!(!remove_body.contains("checked"), "{remove_body}");
        assert!(crate::db::get_post_collection_ids(&pool, post_id)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn test_membership_unknown_collection_is_404() {
        let (app, pool) = app_with_pool().await;
        let post_id = seed_post(&pool, "post").await;
        let cookie = admin_cookie(&pool).await;

        let resp = app
            .clone()
            .oneshot(
                empty_req(
                    "POST",
                    &format!("/api/admin/posts/{post_id}/collections/999"),
                    Some(&cookie),
                )
                .await,
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_edit_fragment_prefills() {
        let (app, pool) = app_with_pool().await;
        let id = seed_post(&pool, "Old").await;
        let cookie = admin_cookie(&pool).await;
        crate::db::set_post_tags(&pool, id, &["ink".to_string()]).await;

        let resp = app
            .clone()
            .oneshot(empty_req("GET", &format!("/api/admin/posts/{id}/edit"), Some(&cookie)).await)
            .await
            .unwrap();
        assert_eq!(resp.status(), HttpStatus::OK);
        let body = body_text(resp).await;
        assert!(body.contains("Old"), "{body}");
        assert!(body.contains("ink"), "{body}");
    }
}
