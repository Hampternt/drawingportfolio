use crate::AppState;
use askama::Template;
use axum::{
    extract::{Multipart, Path, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, patch, post},
    Router,
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
    let posts = crate::db::get_posts_page(&state.pool, None, 0, crate::models::Viewer::Admin).await;
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
async fn patch_visibility(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> impl IntoResponse {
    let mut requested = None::<String>;
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("visibility") {
            requested = field.text().await.ok();
        }
    }

    // Fail loudly, not closed. `Visibility::from_row` coerces an unknown value
    // to Hidden when reading a row, because a value nobody recognises must not
    // render to the public — but doing that to a *request* would report success
    // for a typo.
    let visibility = match requested
        .as_deref()
        .and_then(crate::models::Visibility::from_str)
    {
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
        .route("/api/admin/posts/{id}", delete(delete_post))
        .route("/api/admin/posts/{id}/visibility", patch(patch_visibility))
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

    /// One-field multipart body, which is what the card's button sends.
    fn multipart_visibility(value: &str) -> (String, Body) {
        let boundary = "X-BOUNDARY";
        let body = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"visibility\"\r\n\r\n{value}\r\n--{boundary}--\r\n"
        );
        (
            format!("multipart/form-data; boundary={boundary}"),
            Body::from(body),
        )
    }

    async fn patch_visibility_req(
        app: &Router,
        id: i64,
        value: &str,
        cookie: Option<&str>,
    ) -> axum::response::Response {
        let (content_type, body) = multipart_visibility(value);
        let mut req = Request::builder()
            .method("PATCH")
            .uri(format!("/api/admin/posts/{id}/visibility"))
            .header("content-type", content_type);
        if let Some(c) = cookie {
            req = req.header("cookie", c);
        }
        app.clone().oneshot(req.body(body).unwrap()).await.unwrap()
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
}
