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

async fn encode_as_webp(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let mut buf = std::io::Cursor::new(Vec::new());
        img.write_to(&mut buf, image::ImageFormat::WebP)
            .map_err(|e| format!("webp encode failed: {e}"))?;
        Ok(buf.into_inner())
    })
    .await
    .map_err(|e| format!("spawn_blocking panicked: {e}"))?
}

async fn encode_as_avif(bytes: Vec<u8>) -> Result<Vec<u8>, String> {
    tokio::task::spawn_blocking(move || {
        let img = image::load_from_memory(&bytes)
            .map_err(|e| format!("decode failed: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let pixels: Vec<rgb::RGBA8> = rgba
            .pixels()
            .map(|p| rgb::RGBA8 { r: p[0], g: p[1], b: p[2], a: p[3] })
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
    let mut image_data = None::<(Vec<u8>, String, &'static str)>; // (bytes, content_type, ext)
    let mut post_format = crate::models::PostFormat::Single.as_str().to_string();
    let mut source = "admin".to_string();

    while let Ok(Some(field)) = multipart.next_field().await {
        match field.name() {
            Some("caption") => {
                caption = field.text().await.ok();
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

                image_data = Some((bytes.to_vec(), format!("image/{ext}"), ext));
            }
            _ => {}
        }
    }

    let caption = caption.unwrap_or_default();
    let (bytes, content_type, ext) = match image_data {
        Some(d) => d,
        None => return (StatusCode::BAD_REQUEST, Html("Missing image".to_string())).into_response(),
    };

    // Generate WebP and AVIF variants concurrently; both encode in a spawn_blocking thread
    // so they don't block the async executor. Failures are non-fatal — the post will still
    // be created, but those variant URLs will be empty (graceful fallback in <picture>).
    let (webp_result, avif_result) = tokio::join!(
        encode_as_webp(bytes.clone()),
        encode_as_avif(bytes.clone()),
    );

    let webp_bytes = webp_result.unwrap_or_else(|e| { tracing::warn!("webp encode failed: {e}"); Vec::new() });
    let avif_bytes = avif_result.unwrap_or_else(|e| { tracing::warn!("avif encode failed: {e}"); Vec::new() });

    let file_size_bytes = bytes.len() as i64;
    let uuid = uuid::Uuid::new_v4().to_string();
    // Suffix variants with -webp/-avif to avoid collision when original ext is also .webp
    let original_key = format!("{uuid}.{ext}");
    let webp_key     = format!("{uuid}-webp.webp");
    let avif_key     = format!("{uuid}-avif.avif");

    let image_url = match state.storage.upload(&original_key, bytes, &content_type).await {
        Ok(url) => url,
        Err(e) => {
            tracing::error!("storage upload error: {e}");
            return (StatusCode::INTERNAL_SERVER_ERROR, Html("Upload failed".to_string())).into_response();
        }
    };

    let webp_url = if !webp_bytes.is_empty() {
        state.storage.upload(&webp_key, webp_bytes, "image/webp").await
            .unwrap_or_else(|e| { tracing::error!("webp upload failed: {e}"); String::new() })
    } else {
        String::new()
    };

    let avif_url = if !avif_bytes.is_empty() {
        state.storage.upload(&avif_key, avif_bytes, "image/avif").await
            .unwrap_or_else(|e| { tracing::error!("avif upload failed: {e}"); String::new() })
    } else {
        String::new()
    };

    let post = crate::db::insert_post(
        &state.pool, caption.trim(), &image_url, &webp_url, &avif_url, &post_format, file_size_bytes,
    ).await;
    tracing::info!("post created: id={}, key={original_key}, size={file_size_bytes} bytes, format={post_format}", post.id);

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
    if let Some(urls) = crate::db::delete_post_and_get_urls(&state.pool, id).await {
        tracing::info!("deleting post id={id}");
        for url in [&urls.image_url, &urls.webp_url, &urls.avif_url] {
            if url.is_empty() { continue; } // old posts may have no variants
            if let Err(e) = state.storage.delete_by_url(url).await {
                tracing::error!("storage delete failed for post id={id} url={url}: {e}");
            }
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
}

#[cfg(test)]
mod tests {
    use super::*;

    // Minimal valid 1x1 red PNG (generated with correct CRCs)
    fn test_png() -> Vec<u8> {
        vec![
            0x89,0x50,0x4E,0x47,0x0D,0x0A,0x1A,0x0A,
            0x00,0x00,0x00,0x0D,0x49,0x48,0x44,0x52,
            0x00,0x00,0x00,0x01,0x00,0x00,0x00,0x01,
            0x08,0x02,0x00,0x00,0x00,0x90,0x77,0x53,0xDE,
            0x00,0x00,0x00,0x0C,0x49,0x44,0x41,0x54,
            0x78,0x9C,0x63,0xF8,0xCF,0xC0,0x00,0x00,
            0x03,0x01,0x01,0x00,0xC9,0xFE,0x92,0xEF,
            0x00,0x00,0x00,0x00,0x49,0x45,0x4E,0x44,
            0xAE,0x42,0x60,0x82,
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
    async fn test_encode_as_webp_returns_webp_bytes() {
        let result = encode_as_webp(test_png()).await;
        assert!(result.is_ok(), "encode should succeed: {:?}", result.err());
        assert_eq!(&result.unwrap()[0..4], b"RIFF", "output should be a WebP (RIFF) file");
    }

    #[tokio::test]
    async fn test_encode_as_avif_returns_nonempty_bytes() {
        let result = encode_as_avif(test_png()).await;
        assert!(result.is_ok(), "avif encode should succeed: {:?}", result.err());
        let bytes = result.unwrap();
        assert!(!bytes.is_empty());
        // AVIF is an ISOBMFF container; bytes 4..8 are the 'ftyp' box type
        assert_eq!(&bytes[4..8], b"ftyp", "output should be an AVIF/ISOBMFF file");
    }
}
