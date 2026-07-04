# Fast Image Upload Design

**Date:** 2026-04-09  
**Status:** Approved

## Problem

Image uploads take ~39 seconds because the server encodes both WebP and AVIF synchronously inside the HTTP request handler. The cx23 server (shared vCPU) is the bottleneck. Additionally, if the user navigates away before the response arrives, the async future is dropped and the upload is cancelled entirely.

## Goals

- Upload response returns in ~2-3s (just S3 PUT latency)
- User can navigate away immediately after clicking Upload
- WebP and AVIF variants are still generated and stored (AVIF is an intentional feature showcase)
- No new dependencies

## Approach: Client WebP + Detached AVIF Background Task

### Client-side WebP conversion (`templates/admin.html`)

The HTMX form attributes (`hx-post`, `hx-encoding`, `hx-target`, `hx-swap`) are replaced with a plain `<form>` and a JS submit handler (~25 lines, no dependencies):

1. Intercepts the form submit event
2. Draws the selected image onto an off-screen `<canvas>`
3. Calls `canvas.toBlob('image/webp', 0.85)` — native browser API, universally supported
4. Replaces the `image` field in a `FormData` with the converted WebP blob
5. Updates `#upload-status` with "Converting… / Uploading…" feedback, disables submit button to prevent double-submit
6. `fetch`-posts the multipart form to `/api/admin/posts`
7. On response: injects the returned card HTML into `#posts-list` via `insertAdjacentHTML`, resets the form

**Fallback:** if `canvas.toBlob` returns null (extremely rare — IE11 only), the original file is sent unchanged, preserving the old behaviour.

### Server: skip WebP encoding, detach AVIF (`src/routes/admin.rs`)

- `encode_as_webp` function and its call site are removed — the uploaded file is already WebP
- `image_url` stores the uploaded WebP; `webp_url` is set to the same value so the existing `<picture>` element logic in both feed and admin is unchanged
- `encode_as_avif` is retained but moved into a **detached** `tokio::spawn` that is not awaited — it runs independently of the HTTP response
- The spawned task receives `Arc<AppState>` and the post ID; it encodes AVIF, uploads to S3, then calls `db::update_post_avif_url` to backfill the URL on the already-inserted post row
- Errors in the background task are logged but do not affect the user-facing response

### DB: one new function (`src/db.rs`)

```rust
pub async fn update_post_avif_url(pool: &SqlitePool, id: i64, avif_url: &str)
```

Simple `UPDATE posts SET avif_url = ? WHERE id = ?`. No migration required — the `avif_url` column already exists from migration 002.

## Files Changed

| File | Change |
|---|---|
| `templates/admin.html` | Replace HTMX form with JS fetch handler + canvas WebP conversion |
| `src/routes/admin.rs` | Remove `encode_as_webp`, detach AVIF into `tokio::spawn` |
| `src/db.rs` | Add `update_post_avif_url` |

## Out of Scope

- Image resizing / dimension capping (not requested)
- WASM client-side AVIF encoding (Option 3, deferred)
- Progress reporting for AVIF background task
