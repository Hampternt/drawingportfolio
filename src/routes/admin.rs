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
const MAX_COMPRESS_BYTES: usize = 4 * 1024 * 1024; // 4 MB threshold for auto-compression

fn compress_to_webp(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let img = image::load_from_memory(bytes)
        .map_err(|e| format!("decode failed: {e}"))?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::WebP)
        .map_err(|e| format!("encode failed: {e}"))?;
    Ok(buf.into_inner())
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
    let mut image_data = None::<(Vec<u8>, String)>;
    let mut keep_original = false;
    let mut post_format = crate::models::PostFormat::Single.as_str().to_string();
    let mut source = "admin".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
            }
            Some("keep_original") => {
                // Accept "true", "1", and "on" (HTML checkbox default)
                keep_original = matches!(
                    field.text().await.ok().as_deref(),
                    Some("true") | Some("1") | Some("on")
                );
            }
            Some("format") => {
                if let Ok(v) = field.text().await { post_format = v; }
            }
            Some("source") => {
                // Only accept known source values; default to "admin"
                if let Ok(v) = field.text().await {
                    source = if v == "gallery" { v } else { "admin".to_string() };
                }
            }
            Some("image") => {
                let content_type = field.content_type()
                    .unwrap_or("application/octet-stream")
                    .to_string();

                if !matches!(content_type.as_str(), "image/jpeg" | "image/png" | "image/webp") {
                    return (StatusCode::BAD_REQUEST, Html("Invalid image type".to_string()))
                        .into_response();
                }

                let bytes = field.bytes().await.unwrap_or_default();

                if bytes.len() > MAX_IMAGE_BYTES {
                    return (StatusCode::PAYLOAD_TOO_LARGE, Html("Image too large (max 35MB)".to_string()))
                        .into_response();
                }

                let ext = match validate_magic_bytes(&bytes) {
                    Some(ext) => ext,
                    None => return (StatusCode::BAD_REQUEST, Html("Invalid image file".to_string())).into_response(),
                };

                image_data = Some((bytes.to_vec(), format!("image/{ext}")));
            }
            _ => {}
        }
    }

    let caption = caption.unwrap_or_default();
    let (bytes, content_type) = match image_data {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Html("Missing image".to_string())).into_response(),
    };

    // Compress if above threshold and user did not opt out
    let (final_bytes, final_content_type) = if !keep_original && bytes.len() > MAX_COMPRESS_BYTES {
        match compress_to_webp(&bytes) {
            Ok(webp) => {
                tracing::info!("compressed {} bytes -> {} bytes as webp", bytes.len(), webp.len());
                (webp, "image/webp".to_string())
            }
            Err(e) => {
                tracing::warn!("compression failed, storing original: {e}");
                (bytes, content_type)
            }
        }
    } else {
        (bytes, content_type)
    };

    let file_size_bytes = final_bytes.len() as i64;
    let ext = final_content_type.split('/').last().unwrap_or("jpg");
    let key = format!("{}.{}", uuid::Uuid::new_v4(), ext);

    let image_url = match state.storage.upload(&key, final_bytes, &final_content_type).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("storage upload error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Upload failed".to_string())).into_response();
        }
    };

    let post = crate::db::insert_post(
        &state.pool, caption.trim(), &image_url, &post_format, file_size_bytes,
    ).await;
    tracing::info!("post created: id={}, key={key}, size={file_size_bytes} bytes, format={post_format}", post.id);

    let card_html = if source == "gallery" {
        crate::routes::feed::post_card_html(&post)
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

pub fn validate_magic_bytes(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) { return Some("jpeg"); }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") { return Some("png"); }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") { return Some("webp"); }
    None
}

fn admin_post_card_html(post: &crate::models::Post) -> String {
    let caption_html = if post.caption.is_empty() {
        String::new()
    } else {
        format!("    <p>{}</p>\n", html_escape(&post.caption))
    };
    format!(
        r##"<div class="admin-post" id="admin-post-{}">
  <img src="{}" alt="">
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

    #[test]
    fn test_compress_to_webp_returns_webp_bytes() {
        // Minimal valid 1x1 red PNG (generated with correct CRCs)
        let png: Vec<u8> = vec![
            0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A,
            0x00,0x00,0x00,0x0D,0x49,0x48,0x44,0x52,
            0x00,0x00,0x00,0x01,0x00,0x00,0x00,0x01,
            0x08,0x02,0x00,0x00,0x00,0x90,0x77,0x53,0xDE,
            0x00,0x00,0x00,0x0C,0x49,0x44,0x41,0x54,
            0x78,0x9C,0x63,0xF8,0xCF,0xC0,0x00,0x00,
            0x03,0x01,0x01,0x00,0xC9,0xFE,0x92,0xEF,
            0x00,0x00,0x00,0x00,0x49,0x45,0x4E,0x44,
            0xAE,0x42,0x60,0x82,
        ];
        let result = compress_to_webp(&png);
        assert!(result.is_ok(), "compression should succeed: {:?}", result.err());
        assert_eq!(&result.unwrap()[0..4], b"RIFF", "output should be a WebP (RIFF) file");
    }
}
