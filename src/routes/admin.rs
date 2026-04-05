use axum::{
    Router,
    routing::{get, post, delete},
    extract::{State, Path, Multipart},
    response::{Html, IntoResponse},
    http::StatusCode,
};
use askama::Template;
use std::sync::Arc;
use crate::AppState;

const MAX_IMAGE_BYTES: usize = 35 * 1024 * 1024; // 35 MB

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
    let posts = crate::db::get_posts(&state.pool, 0).await;
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
    let mut image_data = None::<(Vec<u8>, String)>; // (bytes, content_type)

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
            }
            Some("image") => {
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                // Validate MIME type
                if !matches!(content_type.as_str(), "image/jpeg" | "image/png" | "image/webp") {
                    return (StatusCode::BAD_REQUEST, Html("Invalid image type".to_string()))
                        .into_response();
                }

                let bytes = field.bytes().await.unwrap_or_default();

                if bytes.len() > MAX_IMAGE_BYTES {
                    return (StatusCode::PAYLOAD_TOO_LARGE, Html("Image too large (max 35MB)".to_string()))
                        .into_response();
                }

                // Validate magic bytes
                let ext = match validate_magic_bytes(&bytes) {
                    Some(ext) => ext,
                    None => return (StatusCode::BAD_REQUEST, Html("Invalid image file".to_string())).into_response(),
                };

                image_data = Some((bytes.to_vec(), format!("image/{}", ext)));
            }
            _ => {}
        }
    }

    let (caption, (bytes, content_type)) = match (caption, image_data) {
        (Some(c), Some(d)) if !c.trim().is_empty() => (c, d),
        _ => return (StatusCode::BAD_REQUEST, Html("Missing caption or image".to_string())).into_response(),
    };

    // Generate unique key
    let key = format!("{}.{}", uuid::Uuid::new_v4(), content_type.split('/').last().unwrap_or("jpg"));

    let bytes_len = bytes.len();
    let image_url = match state.storage.upload(&key, bytes, &content_type).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("R2 upload error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Upload failed".to_string())).into_response();
        }
    };

    let post = crate::db::insert_post(&state.pool, caption.trim(), &image_url).await;
    tracing::info!("post created: id={}, key={key}, size={} bytes", post.id, bytes_len);
    Html(admin_post_card_html(&post)).into_response()
}

async fn delete_post(
    _session: crate::middleware::AuthSession,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    if let Some(image_url) = crate::db::delete_post_and_get_url(&state.pool, id).await {
        tracing::info!("deleting post id={id}");
        if let Err(e) = state.storage.delete_by_url(&image_url).await {
            tracing::error!("storage delete failed for post id={id}: {e}");
        }
    } else {
        tracing::warn!("delete requested for nonexistent post id={id}");
    }
    StatusCode::OK
}

fn validate_magic_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some("jpeg"); }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Some("png"); }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { return Some("webp"); }
    None
}

fn admin_post_card_html(post: &crate::models::Post) -> String {
    format!(
        r##"<div class="admin-post" id="admin-post-{}">
  <img src="{}" alt="">
  <div class="info">
    <p>{}</p>
    <small>{}</small>
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
        html_escape(&post.caption),
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
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
