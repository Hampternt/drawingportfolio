use crate::models::{
    AuthChallengeState, Collection, CollectionWithCount, CreateCollectionError,
    DrawingTaskWithImage, FoodItem, MealEntryWithFood, PasskeyCredential, Post, PostCounts,
    PostFilter, Session, TagWithCount, Targets, TaskImage, UserId, Viewer, Visibility,
};
use sqlx::{
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
    Row, SqlitePool,
};
use std::str::FromStr;

pub type DbPool = SqlitePool;

pub async fn connect(database_url: &str) -> DbPool {
    let options = SqliteConnectOptions::from_str(database_url)
        .expect("invalid DATABASE_URL")
        .create_if_missing(true);
    SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await
        .expect("failed to connect to SQLite")
}

pub async fn run_migrations(pool: &DbPool) {
    sqlx::query(include_str!("../migrations/001_initial.sql"))
        .execute(pool)
        .await
        .expect("failed to run migrations");

    // Migration 002: idempotent — errors on duplicate column are intentionally ignored
    let _ = sqlx::query(include_str!("../migrations/002_add_post_fields.sql"))
        .execute(pool)
        .await;

    // Migration 003: nutrition tracker tables
    let _ = sqlx::query(include_str!("../migrations/003_nutrition.sql"))
        .execute(pool)
        .await;

    // Migration 004: image variant URLs (webp_url, avif_url)
    let _ = sqlx::query(include_str!("../migrations/004_add_image_variants.sql"))
        .execute(pool)
        .await;

    // Migration 005: package size for food items
    let _ = sqlx::query(include_str!("../migrations/005_add_package_size.sql"))
        .execute(pool)
        .await;

    // Migration 006: custom portion sizes for food items
    let _ = sqlx::query(include_str!("../migrations/006_add_custom_portions.sql"))
        .execute(pool)
        .await;

    // Migration 007: drawing task images and tasks
    sqlx::query(include_str!("../migrations/007_drawing_tasks.sql"))
        .execute(pool)
        .await
        .expect("failed to run drawing tasks migration");

    // Migration 008: meal slot per entry (breakfast/lunch/dinner/snack/other)
    let _ = sqlx::query(include_str!("../migrations/008_meal_slots.sql"))
        .execute(pool)
        .await;

    // Migration 009: daily nutrition targets (single row, single user)
    let _ = sqlx::query(include_str!("../migrations/009_targets.sql"))
        .execute(pool)
        .await;

    // Migration 010: food metadata — category, favourite flag, default portion
    let _ = sqlx::query(include_str!("../migrations/010_food_meta.sql"))
        .execute(pool)
        .await;

    // Migration 011: weight log + saved meals (recipes)
    let _ = sqlx::query(include_str!("../migrations/011_weights_recipes.sql"))
        .execute(pool)
        .await;

    // Migration 012: intrinsic image dimensions.
    //
    // The art feed lays cards out in a CSS multi-column masonry. Without
    // width/height on the <img>, each image reserves no height until it loads
    // and the column reflows under it. Storing the real ratio lets the browser
    // reserve the box up front.
    //
    // Existing rows keep 0 — no backfill. The originals are in object storage
    // and re-fetching them at startup would stall the boot for a layout
    // optimisation; post_card.html omits both attributes when either is 0, so
    // legacy rows render exactly as they did before.
    let _ = sqlx::query(include_str!("../migrations/012_image_dimensions.sql"))
        .execute(pool)
        .await;

    // Migration 013: visibility model — public / unlisted / hidden.
    //
    // Existing rows default to 'public'. Slice 1 took 012 for image dimensions,
    // so this is 013 even though the design handoff reserved 012 for it —
    // migration numbers follow ship order, not the order a document listed them.
    let _ = sqlx::query(include_str!("../migrations/013_post_visibility.sql"))
        .execute(pool)
        .await;

    // Migration 014: collections + tags (slice 3). Four new tables and two
    // indexes; no existing row is touched. The REFERENCES clauses are
    // documentation — the pool does not enable foreign_keys, so deletes clean
    // their own join rows in Rust.
    let _ = sqlx::query(include_str!("../migrations/014_collections_tags.sql"))
        .execute(pool)
        .await;

    // Migration 015: users. Creates the table, the one-owner index and the
    // seeded owner row. CREATE ... IF NOT EXISTS and INSERT OR IGNORE make the
    // whole file re-runnable, so this one gets `.expect()` — a failure here is
    // real, not the duplicate-column noise the ALTER migrations shrug off.
    sqlx::query(include_str!("../migrations/015_users.sql"))
        .execute(pool)
        .await
        .expect("failed to run users migration");

    // Migration 016: sessions.user_id + passkey_credentials.user_id.
    // ALTER-only, so `let _ =` for the duplicate-column error on re-run.
    let _ = sqlx::query(include_str!("../migrations/016_session_identity.sql"))
        .execute(pool)
        .await;

    // Migration 017: meal_entries.user_id + recipes.user_id. ALTER-only.
    let _ = sqlx::query(include_str!("../migrations/017_nutrition_user_id.sql"))
        .execute(pool)
        .await;

    // Migrations 018-020 rebuild tables rather than adding to them, so each is
    // guarded rather than `let _ =`.
    //
    // A rebuild is create/copy/drop/rename. A second pass over an
    // already-migrated table does not *error* — it copies every user's rows
    // back out as user 1 and drops the original. The duplicate-column
    // tolerance the ALTER migrations lean on is no protection, because there
    // is no duplicate column to trip over. The column check is what makes each
    // file run exactly once.
    //
    // **One guard per table, deliberately.** DDL statements auto-commit
    // individually — there is no enclosing transaction — so a single file that
    // renamed `weights` and then failed before renaming `targets` would leave
    // the next boot skipping the whole file over a half-migrated schema, with
    // the guard cheerfully reporting "already done" and the targets queries
    // failing at runtime against a baked `.sqlx` cache. Each guard therefore
    // checks exactly the table its own file rebuilds.
    if !column_exists(pool, "weights", "user_id").await {
        sqlx::query(include_str!("../migrations/018_weights_rebuild.sql"))
            .execute(pool)
            .await
            .expect("failed to rebuild weights for multi-user");
    }

    // The old targets table keys on `id`, the new one on `user_id`, so this is
    // an exact discriminator between the two schemas.
    if !column_exists(pool, "targets", "user_id").await {
        sqlx::query(include_str!("../migrations/019_targets_rebuild.sql"))
            .execute(pool)
            .await
            .expect("failed to rebuild targets for multi-user");
    }

    // Migration 020: per-user food preferences. Same reasoning — the three
    // DROP COLUMNs are not re-runnable, and the copy would re-attribute
    // everyone's preferences to the owner.
    //
    // `DROP COLUMN` requires SQLite 3.35+ (2021); sqlx bundles a far newer one,
    // so this is not a practical worry. Worth naming anyway: it runs under
    // `.expect()` on the startup path, so an older SQLite would be a boot
    // panic rather than a degraded page.
    if column_exists(pool, "food_items", "is_favourite").await {
        sqlx::query(include_str!("../migrations/020_user_food_prefs.sql"))
            .execute(pool)
            .await
            .expect("failed to split user food preferences");
    }

    // Migration 021: PIN lockout counters. ALTER-only, so `let _ =`.
    let _ = sqlx::query(include_str!("../migrations/021_pin_lockout.sql"))
        .execute(pool)
        .await;

    // Migration 022: food_items.base_name. ALTER-only, so `let _ =` — a second
    // run fails on the duplicate column and that failure is the idempotence.
    let _ = sqlx::query(include_str!("../migrations/022_food_base_name.sql"))
        .execute(pool)
        .await;
}

/// Whether `table` currently has a column named `column`.
///
/// The idempotence guard for migrations that rebuild a table rather than add to
/// it. `PRAGMA table_info` is the only portable way to ask SQLite this, and
/// asking is cheaper than the alternative — a rebuild that re-runs does not
/// fail loudly, it quietly re-attributes every row to one user.
async fn column_exists(pool: &DbPool, table: &str, column: &str) -> bool {
    let rows = sqlx::query(&format!("PRAGMA table_info({table})"))
        .fetch_all(pool)
        .await
        .unwrap_or_default();
    rows.iter()
        .filter_map(|r| r.try_get::<String, _>("name").ok())
        .any(|name| name == column)
}

/// Builds the `LIKE` pattern for a caption search.
///
/// Escapes the escape character first, then LIKE's two wildcards, then wraps the
/// result in `%…%`. The order is not a style choice: escaping `%` before `\`
/// would send the second pass back over the backslashes the first one just
/// introduced and double them.
///
/// Without this, a search for `100%` becomes the pattern `%100%%`, which matches
/// every row in the table.
pub fn like_pattern(q: &str) -> String {
    let escaped = q
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("%{escaped}%")
}

/// Normalizes a comma-separated tag list from a form field: trims, lowercases,
/// drops empties, dedupes preserving first occurrence, drops tags over 40
/// chars, and caps the result at 20 tags — all silently, no error path.
pub fn normalize_tags(raw: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let tag = part.trim().to_lowercase();
        if tag.is_empty() || tag.chars().count() > 40 || out.contains(&tag) {
            continue; // empties, over-length and duplicates are silently dropped
        }
        out.push(tag);
        if out.len() == 20 {
            break; // max 20 tags per post — excess silently dropped
        }
    }
    out
}

/// Turns a collection name into a URL-safe slug: lowercase, runs of
/// non-alphanumerics collapsed to a single '-', with no leading or trailing
/// dash.
pub fn slugify(name: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for c in name.chars() {
        if c.is_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            for lc in c.to_lowercase() {
                slug.push(lc);
            }
        } else {
            pending_dash = true; // runs of non-alphanumerics collapse to one '-'
        }
    }
    slug // leading/trailing '-' never appear by construction
}

/// One page of posts, filtered by `PostFilter`.
///
/// Asks for 21 rows to answer "is there another page?" without a COUNT; the
/// caller drops the 21st. SQLite's `LIKE` is ASCII-case-insensitive by default,
/// which is the behaviour the search wants — no `COLLATE NOCASE` needed.
///
/// One `query_as!` macro answers every combination of search, collection, tags
/// and (admin-only) visibility subset — each clause is an `(?n IS NULL OR …)`
/// guard rather than a branch. That used to be two hand-written branches (see
/// the git history for why: sqlx's SQLite macro was thought not to support
/// reusing a numbered placeholder), but the tag and visibility filters turn
/// "two branches" into "sixteen branches" the moment they cross each other, so
/// this rewrite proves the numbered-placeholder reuse works after all — `json_each`
/// on a `NULL` bind yields zero rows, which is what lets the `IS NULL` guard win
/// outright and keeps every filter compile-time checked, list params included.
///
/// The **viewer** is a bool bind (`?1`) for the same reason it always was:
/// crossing it with every other filter as literal branches would multiply out
/// of hand, not just double.
///
/// Migration 014 shipped `idx_posts_visibility_created`, so the caveat that used
/// to live here — no index over `(visibility, created_at DESC)` because the OR
/// predicate couldn't use one — no longer applies.
pub async fn get_posts_page(
    pool: &DbPool,
    filter: &PostFilter,
    page: i64,
    viewer: Viewer,
) -> Vec<Post> {
    let all = viewer.is_admin(); // ?1
    let pattern = filter.q.as_deref().map(like_pattern); // ?2
    let collection = filter.collection.as_deref(); // ?3
    let tags_json = if filter.tags.is_empty() {
        // ?4
        None
    } else {
        Some(serde_json::to_string(&filter.tags).unwrap())
    };
    let vis_json = filter
        .vis
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap()); // ?5
    let offset = page * 20; // ?6

    sqlx::query_as!(
        Post,
        r#"SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes,
       created_at, image_width, image_height, visibility
FROM posts WHERE
    (?1 OR visibility = 'public')
AND (?2 IS NULL OR caption LIKE ?2 ESCAPE '\')
AND (?3 IS NULL OR id IN
     (SELECT post_id FROM post_collections pc
      JOIN collections c ON c.id = pc.collection_id WHERE c.slug = ?3))
AND (?4 IS NULL OR id IN
     (SELECT post_id FROM post_tags pt
      JOIN tags t ON t.id = pt.tag_id
      WHERE t.name IN (SELECT value FROM json_each(?4))
      GROUP BY post_id
      HAVING COUNT(DISTINCT t.id) = json_array_length(?4)))
AND (?5 IS NULL OR visibility IN (SELECT value FROM json_each(?5)))
ORDER BY created_at DESC LIMIT 21 OFFSET ?6"#,
        all,
        pattern,
        collection,
        tags_json,
        vis_json,
        offset
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// The real total for the page head.
///
/// Called only on a full page render — never on HTMX pagination, where the head
/// is not re-rendered and the COUNT would be wasted work on every Load more.
///
/// The `AS "n: i64"` override is load-bearing: sqlx infers SQLite's `COUNT(*)`
/// as `i32`.
///
/// Note there is **no viewer branch in the SQL at all**. The query counts every
/// state and the viewer decides only what `total` means, which keeps the
/// viewer-dependence to one line of Rust instead of a second pair of branches.
pub async fn count_posts(pool: &DbPool, filter: &PostFilter, viewer: Viewer) -> PostCounts {
    struct Row {
        visibility: String,
        n: i64,
    }

    let pattern = filter.q.as_deref().map(like_pattern); // ?1
    let collection = filter.collection.as_deref(); // ?2
    let tags_json = if filter.tags.is_empty() {
        // ?3
        None
    } else {
        Some(serde_json::to_string(&filter.tags).unwrap())
    };
    let vis_json = filter
        .vis
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap()); // ?4

    let rows: Vec<Row> = sqlx::query_as!(
        Row,
        r#"SELECT visibility AS "visibility!: String", COUNT(*) AS "n: i64"
FROM posts WHERE
    (?1 IS NULL OR caption LIKE ?1 ESCAPE '\')
AND (?2 IS NULL OR id IN
     (SELECT post_id FROM post_collections pc
      JOIN collections c ON c.id = pc.collection_id WHERE c.slug = ?2))
AND (?3 IS NULL OR id IN
     (SELECT post_id FROM post_tags pt
      JOIN tags t ON t.id = pt.tag_id
      WHERE t.name IN (SELECT value FROM json_each(?3))
      GROUP BY post_id
      HAVING COUNT(DISTINCT t.id) = json_array_length(?3)))
AND (?4 IS NULL OR visibility IN (SELECT value FROM json_each(?4)))
GROUP BY visibility"#,
        pattern,
        collection,
        tags_json,
        vis_json
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    let mut counts = PostCounts::default();
    // GROUP BY returns no row for a state with zero posts, so accumulate into
    // defaults rather than indexing the result. A portfolio with nothing hidden
    // is the normal case, not an edge one.
    for row in rows {
        match Visibility::from_row(&row.visibility) {
            Visibility::Public => counts.public = row.n,
            Visibility::Unlisted => counts.unlisted = row.n,
            Visibility::Hidden => counts.hidden = row.n,
        }
    }
    counts.total = if viewer.is_admin() {
        counts.public + counts.unlisted + counts.hidden
    } else {
        counts.public
    };
    counts
}

/// One post, or `None` when this viewer may not have it.
///
/// A missing id and a hidden post are the same answer on purpose: the caller
/// turns both into a 404, so from outside they are indistinguishable. A
/// distinguishable error would confirm the row exists, which is the one fact
/// hiding it is meant to withhold.
///
/// `unlisted` is deliberately **not** filtered here — reachable by permalink
/// while absent from the feed is the entire definition of the state.
pub async fn get_post_by_id(pool: &DbPool, id: i64, viewer: Viewer) -> Option<Post> {
    let post = sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height, visibility FROM posts WHERE id = ?", id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()?;

    match (Visibility::from_row(&post.visibility), viewer) {
        (Visibility::Hidden, Viewer::Visitor) => None,
        _ => Some(post),
    }
}

/// Sets one post's state. `false` means no such id, which the route turns into
/// a 404.
pub async fn set_post_visibility(pool: &DbPool, id: i64, visibility: Visibility) -> bool {
    let value = visibility.as_str();
    sqlx::query!("UPDATE posts SET visibility = ? WHERE id = ?", value, id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() == 1)
        .unwrap_or(false)
}

// ── Collections & tags (migration 014) ──────────────────────────────────────
//
// The pool never sets the foreign_keys pragma, so nothing here relies on
// ON DELETE CASCADE — every delete cleans its own join rows in a transaction,
// following `delete_task_image` (below, ~line 1080).

/// Every collection with its post count, ordered by name.
///
/// Viewer-aware: an admin's count is every member post; a visitor's is public
/// members only. `LEFT JOIN` on purpose — an admin sees an empty collection
/// (count 0) they just created, because deleting it needs a row to click. A
/// visitor never sees a collection whose visible count is 0.
pub async fn list_collections_with_counts(
    pool: &DbPool,
    viewer: Viewer,
) -> Vec<CollectionWithCount> {
    if viewer.is_admin() {
        sqlx::query_as!(
            CollectionWithCount,
            r#"SELECT c.id AS "id!: i64", c.name, c.slug, COUNT(pc.post_id) AS "count!: i64"
               FROM collections c
               LEFT JOIN post_collections pc ON pc.collection_id = c.id
               GROUP BY c.id, c.name, c.slug
               ORDER BY c.name ASC"#
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            CollectionWithCount,
            r#"SELECT c.id AS "id!: i64", c.name, c.slug, COUNT(p.id) AS "count!: i64"
               FROM collections c
               LEFT JOIN post_collections pc ON pc.collection_id = c.id
               LEFT JOIN posts p ON p.id = pc.post_id AND p.visibility = 'public'
               GROUP BY c.id, c.name, c.slug
               HAVING COUNT(p.id) > 0
               ORDER BY c.name ASC"#
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    }
}

/// Every tag with its post count, ordered by name.
///
/// `INNER JOIN` through `post_tags` to `posts` on purpose — an orphan tag (no
/// posts left carrying it) naturally disappears for everyone, no `HAVING`
/// needed. Viewer-aware: a visitor additionally loses tags whose public count
/// is 0, via the `WHERE` clause on `posts.visibility`.
pub async fn list_tags_with_counts(pool: &DbPool, viewer: Viewer) -> Vec<TagWithCount> {
    if viewer.is_admin() {
        sqlx::query_as!(
            TagWithCount,
            r#"SELECT t.name, COUNT(*) AS "count!: i64"
               FROM tags t
               INNER JOIN post_tags pt ON pt.tag_id = t.id
               INNER JOIN posts p ON p.id = pt.post_id
               GROUP BY t.id, t.name
               ORDER BY t.name ASC"#
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    } else {
        sqlx::query_as!(
            TagWithCount,
            r#"SELECT t.name, COUNT(*) AS "count!: i64"
               FROM tags t
               INNER JOIN post_tags pt ON pt.tag_id = t.id
               INNER JOIN posts p ON p.id = pt.post_id
               WHERE p.visibility = 'public'
               GROUP BY t.id, t.name
               ORDER BY t.name ASC"#
        )
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    }
}

/// Creates a collection, slugging the trimmed name for the URL/dedup key
/// while storing the trimmed name as typed (display case preserved).
///
/// A slug that comes out empty (the name was pure punctuation/whitespace)
/// fails fast with `InvalidName` before touching the database. Otherwise the
/// insert is attempted directly — detecting the UNIQUE violation on `slug`
/// rather than pre-checking, since a pre-check races a concurrent insert of
/// the same slug. On that violation, the existing row is re-read by slug and
/// its name is returned so the caller can say "you already have one".
pub async fn create_collection(
    pool: &DbPool,
    name: &str,
) -> Result<Collection, CreateCollectionError> {
    let trimmed = name.trim();
    let slug = slugify(trimmed);
    if slug.is_empty() {
        return Err(CreateCollectionError::InvalidName);
    }

    let inserted = sqlx::query!(
        "INSERT INTO collections (name, slug) VALUES (?, ?) RETURNING id",
        trimmed,
        slug
    )
    .fetch_one(pool)
    .await;

    let id = match inserted {
        Ok(row) => row.id,
        Err(e) => {
            let is_unique_violation = e
                .as_database_error()
                .map(|db_err| db_err.is_unique_violation())
                .unwrap_or(false);
            if !is_unique_violation {
                return Err(CreateCollectionError::InvalidName);
            }
            let existing = sqlx::query_as!(
                Collection,
                r#"SELECT id AS "id!: i64", name, slug, created_at FROM collections WHERE slug = ?"#,
                slug
            )
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
            return match existing {
                Some(c) => Err(CreateCollectionError::DuplicateSlug(c.name)),
                None => Err(CreateCollectionError::InvalidName),
            };
        }
    };

    let collection = sqlx::query_as!(
        Collection,
        "SELECT id, name, slug, created_at FROM collections WHERE id = ?",
        id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted collection");
    Ok(collection)
}

/// Deletes a collection and its membership rows, leaving member posts intact.
/// Returns a bool; the route ignores this and re-renders the rail fragment either way (idempotent by contract).
pub async fn delete_collection(pool: &DbPool, id: i64) -> bool {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return false,
    };

    if sqlx::query!("DELETE FROM post_collections WHERE collection_id = ?", id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return false;
    }

    let result = sqlx::query!("DELETE FROM collections WHERE id = ?", id)
        .execute(&mut *tx)
        .await;

    match result {
        Ok(r) if r.rows_affected() == 1 => {
            tx.commit().await.ok();
            true
        }
        _ => false,
    }
}

/// Replaces a post's tag set wholesale. Callers pass already-normalized tags
/// (`normalize_tags` is applied once, at the form/PATCH edge) — this function
/// does not re-normalize. `false` if the post does not exist; the `tags`
/// table gains no new row in that case.
pub async fn set_post_tags(pool: &DbPool, post_id: i64, tags: &[String]) -> bool {
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(_) => return false,
    };

    let post_exists = sqlx::query!("SELECT id FROM posts WHERE id = ?", post_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()
        .is_some();
    if !post_exists {
        return false;
    }

    if sqlx::query!("DELETE FROM post_tags WHERE post_id = ?", post_id)
        .execute(&mut *tx)
        .await
        .is_err()
    {
        return false;
    }

    for tag in tags {
        if sqlx::query!("INSERT OR IGNORE INTO tags (name) VALUES (?)", tag)
            .execute(&mut *tx)
            .await
            .is_err()
        {
            return false;
        }
        let tag_id = match sqlx::query!("SELECT id FROM tags WHERE name = ?", tag)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
        {
            Some(row) => row.id,
            None => return false,
        };
        if sqlx::query!(
            "INSERT INTO post_tags (post_id, tag_id) VALUES (?, ?)",
            post_id,
            tag_id
        )
        .execute(&mut *tx)
        .await
        .is_err()
        {
            return false;
        }
    }

    tx.commit().await.is_ok()
}

/// Plain caption update. `false` for an unknown id.
pub async fn update_post_caption(pool: &DbPool, post_id: i64, caption: &str) -> bool {
    sqlx::query!(
        "UPDATE posts SET caption = ? WHERE id = ?",
        caption,
        post_id
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() == 1)
    .unwrap_or(false)
}

/// Adds a post to a collection, idempotently. `false` if either the post or
/// the collection is missing — the FK pragma is off, so a dangling insert
/// would otherwise succeed silently. A second call with the same pair is
/// still `true`, and still leaves exactly one join row (`INSERT OR IGNORE`
/// on the composite primary key).
pub async fn add_post_to_collection(pool: &DbPool, post_id: i64, collection_id: i64) -> bool {
    let post_exists = sqlx::query!("SELECT id FROM posts WHERE id = ?", post_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some();
    if !post_exists {
        return false;
    }
    let collection_exists = sqlx::query!("SELECT id FROM collections WHERE id = ?", collection_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some();
    if !collection_exists {
        return false;
    }

    sqlx::query!(
        "INSERT OR IGNORE INTO post_collections (post_id, collection_id) VALUES (?, ?)",
        post_id,
        collection_id
    )
    .execute(pool)
    .await
    .is_ok()
}

/// Removes a post from a collection, idempotently — the route re-renders the
/// checklist either way, so `true` regardless of whether a row was there.
pub async fn remove_post_from_collection(pool: &DbPool, post_id: i64, collection_id: i64) -> bool {
    sqlx::query!(
        "DELETE FROM post_collections WHERE post_id = ? AND collection_id = ?",
        post_id,
        collection_id
    )
    .execute(pool)
    .await
    .ok();
    true
}

/// A post's tags, name-ordered.
pub async fn get_post_tags(pool: &DbPool, post_id: i64) -> Vec<String> {
    sqlx::query!(
        "SELECT t.name FROM post_tags pt JOIN tags t ON t.id = pt.tag_id WHERE pt.post_id = ? ORDER BY t.name",
        post_id
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| row.name)
    .collect()
}

/// A post's collection memberships, id-ordered.
pub async fn get_post_collection_ids(pool: &DbPool, post_id: i64) -> Vec<i64> {
    sqlx::query!(
        "SELECT collection_id FROM post_collections WHERE post_id = ? ORDER BY collection_id",
        post_id
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| row.collection_id)
    .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn insert_post(
    pool: &DbPool,
    caption: &str,
    image_url: &str,
    webp_url: &str,
    avif_url: &str,
    format: &str,
    file_size_bytes: i64,
    image_width: i64,
    image_height: i64,
    visibility: Visibility,
) -> Post {
    let visibility = visibility.as_str();
    let id = sqlx::query!(
        "INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes, image_width, image_height, visibility) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        caption, image_url, webp_url, avif_url, format, file_size_bytes, image_width, image_height, visibility
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert post")
    .id;

    sqlx::query_as!(Post,
        "SELECT id, caption, image_url, webp_url, avif_url, format, file_size_bytes, created_at, image_width, image_height, visibility FROM posts WHERE id = ?", id
    )
    .fetch_one(pool)
    .await
    .expect("failed to fetch inserted post")
}

pub async fn update_post_avif_url(
    pool: &DbPool,
    id: i64,
    avif_url: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE posts SET avif_url = ? WHERE id = ?", avif_url, id)
        .execute(pool)
        .await?;
    Ok(())
}

pub struct PostUrls {
    pub image_url: String,
    pub webp_url: String,
    pub avif_url: String,
}

pub async fn delete_post_and_get_urls(pool: &DbPool, id: i64) -> Option<PostUrls> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!(
        "SELECT image_url, webp_url, avif_url FROM posts WHERE id = ?",
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    if let Some(r) = row {
        // The pool never sets the foreign_keys pragma, so ON DELETE CASCADE
        // never fires — join rows are cleaned explicitly before the post row
        // goes, or they'd dangle and corrupt every tag/collection count.
        // Orphaned tags themselves are left in place (the spec's recorded
        // trade-off); only join rows are not.
        sqlx::query!("DELETE FROM post_tags WHERE post_id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        sqlx::query!("DELETE FROM post_collections WHERE post_id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        sqlx::query!("DELETE FROM posts WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        tx.commit().await.ok();
        Some(PostUrls {
            image_url: r.image_url,
            webp_url: r.webp_url,
            avif_url: r.avif_url,
        })
    } else {
        tx.rollback().await.ok();
        None
    }
}

pub async fn create_session(pool: &DbPool, id: &str, expires_at: &str, user_id: i64) {
    sqlx::query!(
        "INSERT INTO sessions (id, expires_at, user_id) VALUES (?, ?, ?)",
        id,
        expires_at,
        user_id
    )
    .execute(pool)
    .await
    .expect("failed to create session");
}

/// Loads a live session together with its user's identity and flags.
///
/// The `JOIN` is deliberate: this runs on every authenticated request, and the
/// admin flags have to be answered in the same round-trip. An INNER JOIN also
/// makes an orphaned session — one whose user was deleted — read as logged out
/// rather than as a session with no permissions, which is the safe direction.
pub async fn get_session(pool: &DbPool, id: &str) -> Option<Session> {
    sqlx::query_as!(Session,
        r#"SELECT s.id as "id!", s.expires_at as "expires_at!", s.user_id as "user_id!",
                  u.name as "user_name!", u.is_owner as "is_owner!: bool", u.is_admin as "is_admin!: bool"
           FROM sessions s
           JOIN users u ON u.id = s.user_id
           WHERE s.id = ? AND s.expires_at > datetime('now')"#,
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// The id of the single owner row.
///
/// Looked up rather than hardcoded to 1: migration 015 seeds it there, but the
/// one-owner index — not the literal id — is what the invariant rests on.
pub async fn get_owner_user_id(pool: &DbPool) -> Option<i64> {
    sqlx::query_scalar!(r#"SELECT id as "id!" FROM users WHERE is_owner = 1"#)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
}

// ── Users ─────────────────────────────────────────────────────────────────────
//
// The owner invariants live here, in SQL, not in the management template. A
// hidden button is a UI convenience; `AND is_owner = 0` is the rule. Every
// destructive user operation carries it, so the owner cannot be deleted or
// demoted even by a hand-made request.

/// Every account, owner first then alphabetical — the management list.
pub async fn list_users(pool: &DbPool) -> Vec<crate::models::UserRow> {
    sqlx::query_as!(
        crate::models::UserRow,
        r#"SELECT id as "id!", name as "name!",
                  is_owner as "is_owner!: bool", is_admin as "is_admin!: bool",
                  (pin_hash IS NOT NULL) as "has_pin!: bool",
                  (locked_until IS NOT NULL AND locked_until > datetime('now')) as "is_locked!: bool",
                  created_at as "created_at!"
           FROM users ORDER BY is_owner DESC, name COLLATE NOCASE ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Looks a user up by name for the PIN login path.
///
/// `users.name` is `UNIQUE COLLATE NOCASE`, so this matches case-insensitively
/// and someone who typed "Alex" logs in as "alex". `pin_hash` collapses to an
/// empty string when unset, which `verify_pin` rejects — an account with no PIN
/// (the owner, on a passkey) must not be reachable with an empty one.
pub async fn get_user_by_name(pool: &DbPool, name: &str) -> Option<crate::models::UserAuth> {
    sqlx::query_as!(
        crate::models::UserAuth,
        r#"SELECT id as "id!",
                  COALESCE(pin_hash, '') as "pin_hash!: String",
                  failed_pin_attempts as "failed_pin_attempts!",
                  (locked_until IS NOT NULL AND locked_until > datetime('now')) as "is_locked!: bool"
           FROM users WHERE name = ?"#,
        name
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

/// Creates a member account. Never an owner and never an admin — the owner is
/// seeded by migration 015 and admin is granted separately and deliberately.
pub async fn create_user(pool: &DbPool, name: &str, pin_hash: &str) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"INSERT INTO users (name, pin_hash, is_owner, is_admin) VALUES (?, ?, 0, 0) RETURNING id as "id!""#,
        name,
        pin_hash
    )
    .fetch_one(pool)
    .await
}

/// Sets a user's PIN and clears any lockout — used both for the owner
/// resetting a forgotten PIN and for a member changing their own.
pub async fn set_user_pin(pool: &DbPool, id: i64, pin_hash: &str) -> bool {
    sqlx::query!(
        "UPDATE users SET pin_hash = ?, failed_pin_attempts = 0, locked_until = NULL WHERE id = ?",
        pin_hash,
        id
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

/// Records a wrong PIN, locking the account once the budget is spent.
///
/// The lock is written as an absolute timestamp rather than a countdown so it
/// survives restarts — an attacker who can bounce the process must not be able
/// to reset their own budget.
pub async fn record_failed_pin(pool: &DbPool, id: i64) {
    let max = crate::pin::MAX_PIN_ATTEMPTS;
    let mins = format!("+{} minutes", crate::pin::LOCKOUT_MINUTES);
    sqlx::query!(
        "UPDATE users
         SET failed_pin_attempts = failed_pin_attempts + 1,
             locked_until = CASE WHEN failed_pin_attempts + 1 >= ?
                                 THEN datetime('now', ?) ELSE locked_until END
         WHERE id = ?",
        max,
        mins,
        id
    )
    .execute(pool)
    .await
    .ok();
}

/// Clears the failure budget after a successful login.
pub async fn clear_failed_pins(pool: &DbPool, id: i64) {
    sqlx::query!(
        "UPDATE users SET failed_pin_attempts = 0, locked_until = NULL WHERE id = ?",
        id
    )
    .execute(pool)
    .await
    .ok();
}

/// Grants or revokes art-portfolio admin. Refuses to touch the owner.
///
/// The owner already reads as an effective admin without the flag, so writing
/// it would be a no-op at best; refusing outright means "revoke admin" can
/// never be aimed at the one account that must keep it.
pub async fn set_user_admin(pool: &DbPool, id: i64, is_admin: bool) -> bool {
    let flag = i64::from(is_admin);
    sqlx::query!(
        "UPDATE users SET is_admin = ? WHERE id = ? AND is_owner = 0",
        flag,
        id
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

pub async fn rename_user(pool: &DbPool, id: i64, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("UPDATE users SET name = ? WHERE id = ?", name, id)
        .execute(pool)
        .await
        .map(|_| ())
}

/// Deletes a member and everything of theirs. Refuses the owner.
///
/// The deletes are explicit because the pool runs with `foreign_keys` off, so
/// nothing cascades on its own. Order matters only for readability — none of
/// these constrain each other — but *completeness* matters a great deal: a
/// missed table leaves rows keyed to an id that `AUTOINCREMENT` will eventually
/// hand to somebody else, and the next member to be created would inherit a
/// stranger's food log.
pub async fn delete_user(pool: &DbPool, id: i64) -> bool {
    // Guard first: if this is the owner, touch nothing at all.
    let deleted = sqlx::query!("DELETE FROM users WHERE id = ? AND is_owner = 0", id)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
        .unwrap_or(false);
    if !deleted {
        return false;
    }

    for stmt in [
        "DELETE FROM sessions WHERE user_id = ?",
        "DELETE FROM passkey_credentials WHERE user_id = ?",
        "DELETE FROM meal_entries WHERE user_id = ?",
        "DELETE FROM weights WHERE user_id = ?",
        "DELETE FROM targets WHERE user_id = ?",
        "DELETE FROM user_food_prefs WHERE user_id = ?",
        "DELETE FROM recipe_items WHERE recipe_id IN (SELECT id FROM recipes WHERE user_id = ?)",
        "DELETE FROM recipes WHERE user_id = ?",
    ] {
        sqlx::query(stmt).bind(id).execute(pool).await.ok();
    }
    true
}

/// Resolves a passkey credential id to the user that registered it.
///
/// This is what makes a passkey login mean "you are this person" rather than
/// "someone with a valid passkey".
pub async fn get_credential_user_id(pool: &DbPool, cred_id: &str) -> Option<i64> {
    sqlx::query_scalar!(
        r#"SELECT user_id as "user_id!" FROM passkey_credentials WHERE id = ?"#,
        cred_id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn delete_session(pool: &DbPool, id: &str) {
    sqlx::query!("DELETE FROM sessions WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn cleanup_expired(pool: &DbPool) {
    let sessions = sqlx::query!("DELETE FROM sessions WHERE expires_at <= datetime('now')")
        .execute(pool)
        .await
        .ok();
    let challenges =
        sqlx::query!("DELETE FROM auth_challenge_state WHERE expires_at <= datetime('now')")
            .execute(pool)
            .await
            .ok();

    let session_rows = sessions.map(|r| r.rows_affected()).unwrap_or(0);
    let challenge_rows = challenges.map(|r| r.rows_affected()).unwrap_or(0);
    tracing::info!(
        "cleanup: removed {session_rows} expired sessions, {challenge_rows} expired challenges"
    );
}

pub async fn get_all_credentials(pool: &DbPool) -> Vec<PasskeyCredential> {
    sqlx::query_as!(
        PasskeyCredential,
        r#"SELECT id as "id!", passkey_json as "passkey_json!" FROM passkey_credentials"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn save_credential(pool: &DbPool, id: &str, passkey_json: &str, user_id: i64) {
    sqlx::query!(
        "INSERT OR REPLACE INTO passkey_credentials (id, passkey_json, user_id) VALUES (?, ?, ?)",
        id,
        passkey_json,
        user_id
    )
    .execute(pool)
    .await
    .expect("failed to save credential");
}

pub async fn save_challenge(pool: &DbPool, id: &str, state_json: &str, expires_at: &str) {
    sqlx::query!(
        "INSERT INTO auth_challenge_state (id, state_json, expires_at) VALUES (?, ?, ?)",
        id,
        state_json,
        expires_at
    )
    .execute(pool)
    .await
    .expect("failed to save challenge");
}

pub async fn take_challenge(pool: &DbPool, id: &str) -> Option<AuthChallengeState> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query_as!(AuthChallengeState,
        r#"SELECT id as "id!", state_json as "state_json!" FROM auth_challenge_state WHERE id = ? AND expires_at > datetime('now')"#,
        id
    )
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    if row.is_some() {
        sqlx::query!("DELETE FROM auth_challenge_state WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        tx.commit().await.ok();
    } else {
        tx.rollback().await.ok();
    }

    row
}

/// The food catalog is shared; the opinions about it are not.
///
/// `food_items` rows are common property — that is the point of one catalog,
/// and it is what makes a barcode scan by one person useful to everyone. But
/// "is this a favourite", "what portion do I usually take" and "what custom
/// portions do I want offered" are personal, so they live in `user_food_prefs`
/// and arrive by LEFT JOIN.
///
/// The join is LEFT and the values are COALESCEd because *no preference row* is
/// the normal state: a food you have never favourited or sized simply has no
/// row, and must read as "not a favourite, no custom portions" rather than
/// vanishing from the catalog — which is exactly what an INNER JOIN would do.
///
/// The column list is repeated across the four readers rather than shared in a
/// constant: `query_as!` checks its SQL against the live schema at compile
/// time, and it can only do that for a string literal. A shared `const` would
/// buy tidiness by giving up the check that makes these queries safe to change.
pub async fn get_food_items(pool: &DbPool, user: UserId) -> Vec<FoodItem> {
    let uid = user.get();
    sqlx::query_as!(
        FoodItem,
        r#"SELECT
            fi.id, fi.name, fi.brand, fi.barcode, fi.calories, fi.protein, fi.carbs,
            fi.fat, fi.fiber, fi.sugar, fi.sodium, fi.saturated_fat, fi.package_size,
            COALESCE(p.custom_portions, '') as "custom_portions!",
            fi.image_url, fi.category,
            COALESCE(p.is_favourite, 0) as "is_favourite!",
            p.default_portion_g, fi.created_at
        FROM food_items fi
        LEFT JOIN user_food_prefs p ON p.food_item_id = fi.id AND p.user_id = ?
        ORDER BY fi.name ASC"#,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn search_food_items(pool: &DbPool, q: &str, user: UserId) -> Vec<FoodItem> {
    let pattern = format!("%{}%", q);
    let uid = user.get();
    sqlx::query_as!(
        FoodItem,
        r#"SELECT
            fi.id, fi.name, fi.brand, fi.barcode, fi.calories, fi.protein, fi.carbs,
            fi.fat, fi.fiber, fi.sugar, fi.sodium, fi.saturated_fat, fi.package_size,
            COALESCE(p.custom_portions, '') as "custom_portions!",
            fi.image_url, fi.category,
            COALESCE(p.is_favourite, 0) as "is_favourite!",
            p.default_portion_g, fi.created_at
        FROM food_items fi
        LEFT JOIN user_food_prefs p ON p.food_item_id = fi.id AND p.user_id = ?
        WHERE fi.name LIKE ? OR fi.brand LIKE ?
        ORDER BY fi.name ASC LIMIT 20"#,
        uid,
        pattern,
        pattern
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

pub async fn insert_food_item(
    pool: &DbPool,
    name: &str,
    brand: &str,
    barcode: Option<&str>,
    calories: f64,
    protein: f64,
    carbs: f64,
    fat: f64,
    fiber: f64,
    sugar: f64,
    sodium: f64,
    saturated_fat: f64,
    package_size: Option<f64>,
    custom_portions: &str,
    image_url: &str,
    user: UserId,
) -> FoodItem {
    let uid = user.get();
    let id = sqlx::query!(
        "INSERT INTO food_items (name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) RETURNING id",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url
    )
    .fetch_one(pool)
    .await
    .expect("failed to insert food item")
    .id;

    // The food joins the shared catalog; the custom portions the adder typed in
    // are *their* preference and land in their own row. Skipped when empty so
    // the prefs table only holds actual opinions.
    if !custom_portions.is_empty() {
        sqlx::query!(
            "INSERT INTO user_food_prefs (user_id, food_item_id, custom_portions) VALUES (?, ?, ?)
             ON CONFLICT(user_id, food_item_id) DO UPDATE SET custom_portions = excluded.custom_portions",
            uid, id, custom_portions
        )
        .execute(pool)
        .await
        .ok();
    }

    get_food_item(pool, id, user)
        .await
        .expect("failed to fetch inserted food item")
}

/// Deletes a food from the **shared** catalog, for everybody.
///
/// Deliberately not scoped to a user: there is one catalog, so there is one
/// delete. It takes no [`UserId`] because there is no per-user variant of this
/// to get wrong — the entry rows and preference rows it orphans belong to
/// whoever logged them, and are cleaned regardless of who pressed the button.
///
/// That the button is available to any signed-in user is a deliberate carry-
/// over of the single-user behaviour, and the one place where members can
/// affect each other's experience. Restricting it is a pack 4 question.
pub async fn delete_food_item(pool: &DbPool, id: i64) -> Option<String> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url FROM food_items WHERE id = ?", id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

    if let Some(r) = row {
        sqlx::query!("DELETE FROM food_items WHERE id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        // Preference rows do not cascade — the pool has foreign_keys off, so
        // they are cleared here or they linger and re-attach to whatever id
        // AUTOINCREMENT hands out next.
        sqlx::query!("DELETE FROM user_food_prefs WHERE food_item_id = ?", id)
            .execute(&mut *tx)
            .await
            .ok();
        tx.commit().await.ok();
        Some(r.image_url)
    } else {
        tx.rollback().await.ok();
        None
    }
}

pub async fn get_food_item(pool: &DbPool, id: i64, user: UserId) -> Option<FoodItem> {
    let uid = user.get();
    sqlx::query_as!(
        FoodItem,
        r#"SELECT
            fi.id, fi.name, fi.brand, fi.barcode, fi.calories, fi.protein, fi.carbs,
            fi.fat, fi.fiber, fi.sugar, fi.sodium, fi.saturated_fat, fi.package_size,
            COALESCE(p.custom_portions, '') as "custom_portions!",
            fi.image_url, fi.category,
            COALESCE(p.is_favourite, 0) as "is_favourite!",
            p.default_portion_g, fi.created_at
        FROM food_items fi
        LEFT JOIN user_food_prefs p ON p.food_item_id = fi.id AND p.user_id = ?
        WHERE fi.id = ?"#,
        uid,
        id
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

#[allow(clippy::too_many_arguments)]
pub async fn update_food_item(
    pool: &DbPool,
    id: i64,
    name: &str,
    brand: &str,
    barcode: Option<&str>,
    calories: f64,
    protein: f64,
    carbs: f64,
    fat: f64,
    fiber: f64,
    sugar: f64,
    sodium: f64,
    saturated_fat: f64,
    package_size: Option<f64>,
    custom_portions: &str,
    image_url: &str,
    category: &str,
    is_favourite: bool,
    default_portion_g: Option<f64>,
    user: UserId,
) {
    let uid = user.get();
    let fav = if is_favourite { 1i64 } else { 0i64 };

    // The nutrition facts are a property of the food and go to the shared row —
    // one person correcting a wrong calorie count fixes it for everyone.
    sqlx::query!(
        "UPDATE food_items SET name = ?, brand = ?, barcode = ?, calories = ?, protein = ?, carbs = ?, fat = ?, fiber = ?, sugar = ?, sodium = ?, saturated_fat = ?, package_size = ?, image_url = ?, category = ? WHERE id = ?",
        name, brand, barcode, calories, protein, carbs, fat, fiber, sugar, sodium, saturated_fat, package_size, image_url, category, id
    )
    .execute(pool)
    .await
    .ok();

    // Favourite, usual portion and custom portions are opinions and go to the
    // editor's own row. Upsert rather than update: most foods have no
    // preference row until someone expresses one.
    sqlx::query!(
        "INSERT INTO user_food_prefs (user_id, food_item_id, is_favourite, default_portion_g, custom_portions)
         VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id, food_item_id) DO UPDATE SET
            is_favourite = excluded.is_favourite,
            default_portion_g = excluded.default_portion_g,
            custom_portions = excluded.custom_portions",
        uid, id, fav, default_portion_g, custom_portions
    )
    .execute(pool)
    .await
    .ok();
}

/// Flips *this user's* favourite flag for a food, leaving everyone else's alone.
pub async fn toggle_food_favourite(pool: &DbPool, id: i64, user: UserId) {
    let uid = user.get();
    sqlx::query!(
        "INSERT INTO user_food_prefs (user_id, food_item_id, is_favourite) VALUES (?, ?, 1)
         ON CONFLICT(user_id, food_item_id) DO UPDATE SET is_favourite = 1 - is_favourite",
        uid,
        id
    )
    .execute(pool)
    .await
    .ok();
}

/// This user's logging history for one food — how much of it *they* ate, per day.
///
/// Scoped even though it is addressed by food id rather than entry id: the food
/// is shared, but "when did I last eat this and how much" is not, and an
/// unscoped `SUM(grams)` would quietly total the whole household's intake into
/// one person's history chart.
pub async fn get_item_log_history(
    pool: &DbPool,
    id: i64,
    start: &str,
    end: &str,
    user: UserId,
) -> Vec<(String, f64)> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT date as "date!", SUM(grams) as "grams!: f64" FROM meal_entries
        WHERE food_item_id = ? AND date >= ? AND date <= ? AND user_id = ?
        GROUP BY date ORDER BY date ASC"#,
        id,
        start,
        end,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.grams))
    .collect()
}

pub async fn get_meal_entries_for_date(
    pool: &DbPool,
    date: &str,
    user: UserId,
) -> Vec<MealEntryWithFood> {
    let uid = user.get();
    // The LEFT JOIN carries the *viewer's* portion preference, not the food's:
    // `default_portion_g` moved to user_food_prefs in migration 020 precisely
    // so two people could disagree about what a usual serving is. A missing
    // row reads as "no opinion", which the basis fallback handles.
    let rows = sqlx::query!(
        r#"SELECT
            me.id as "entry_id!",
            me.food_item_id,
            fi.name as food_name,
            fi.brand as "brand!",
            fi.image_url as "image_url!",
            fi.package_size,
            fi.base_name as "base_name!",
            p.default_portion_g,
            me.grams,
            me.slot as "slot!",
            fi.calories as base_calories,
            fi.protein as base_protein,
            fi.carbs as base_carbs,
            fi.fat as base_fat,
            fi.fiber as base_fiber,
            fi.sugar as base_sugar,
            fi.sodium as base_sodium,
            fi.saturated_fat as base_saturated_fat
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        LEFT JOIN user_food_prefs p
               ON p.food_item_id = fi.id AND p.user_id = ?
        WHERE me.date = ? AND me.user_id = ?
        ORDER BY me.created_at ASC"#,
        uid,
        date,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| {
            let factor = r.grams / 100.0;
            let base_grams = crate::models::basis_grams(r.package_size, r.default_portion_g);
            MealEntryWithFood {
                entry_id: r.entry_id,
                food_item_id: r.food_item_id,
                food_name: r.food_name,
                brand: r.brand,
                image_url: r.image_url,
                base_grams,
                base_name: r.base_name,
                slot: r.slot,
                grams: r.grams,
                calories: r.base_calories * factor,
                protein: r.base_protein * factor,
                carbs: r.base_carbs * factor,
                fat: r.base_fat * factor,
                fiber: r.base_fiber * factor,
                sugar: r.base_sugar * factor,
                sodium: r.base_sodium * factor,
                saturated_fat: r.base_saturated_fat * factor,
            }
        })
        .collect()
}

/// Every food this user has logged, mapped to the grams they last used.
///
/// This is the whole of the design's `last_grams[food_id]`. It is deliberately
/// *not* a column: the entry log already records every amount ever chosen, so a
/// stored copy would be a second source of truth that drifts the first time an
/// entry is edited or deleted.
///
/// One query per render rather than one per row — the Today screen asks about
/// every row it draws, and the day's rows can easily be a dozen foods.
pub async fn get_last_grams_map(
    pool: &DbPool,
    user: UserId,
) -> std::collections::HashMap<i64, f64> {
    let uid = user.get();
    // Bare columns beside MAX() resolve to the max row's values in SQLite, so
    // this picks each food's most recent entry. The tiebreak on id matters:
    // created_at has one-second resolution and logging a saved meal writes
    // several rows inside the same second.
    sqlx::query!(
        r#"SELECT food_item_id as "food_item_id!", grams as "grams!: f64",
                  MAX(created_at || '-' || printf('%012d', id)) as "latest!: String"
           FROM meal_entries
           WHERE user_id = ?
           GROUP BY food_item_id"#,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.food_item_id, r.grams))
    .collect()
}

/// What this user usually eats at a given slot: their most-*frequent* foods
/// there, with the amount they last took.
///
/// Deliberately not `get_recent_foods` filtered by slot. Most-recent answers
/// "what did I just eat", which is already the log-again chips; this answers
/// "what do I always have at breakfast", and the two disagree exactly when it
/// matters — the morning after a one-off.
///
/// Ties break on recency, so a food eaten five times last year sits below one
/// eaten five times this month.
pub async fn get_usual_for_slot(
    pool: &DbPool,
    slot: &str,
    limit: i64,
    user: UserId,
) -> Vec<crate::models::UsualFood> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT me.food_item_id as "food_item_id!", fi.name as "name!",
                  fi.image_url as "image_url!",
                  fi.protein as "protein!: f64", fi.carbs as "carbs!: f64",
                  fi.fat as "fat!: f64",
                  COUNT(*) as "times!: i64",
                  MAX(me.created_at || '-' || printf('%012d', me.id)) as "latest!: String",
                  me.grams as "last_grams!: f64"
           FROM meal_entries me
           JOIN food_items fi ON fi.id = me.food_item_id
           WHERE me.user_id = ? AND me.slot = ?
           GROUP BY me.food_item_id
           ORDER BY 7 DESC, 8 DESC
           LIMIT ?"#,
        uid,
        slot,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::UsualFood {
        food_item_id: r.food_item_id,
        name: r.name,
        image_url: r.image_url,
        protein: r.protein,
        carbs: r.carbs,
        fat: r.fat,
        last_grams: r.last_grams,
    })
    .collect()
}

/// Logs a food to a user's day.
///
/// `food_item_id` and the user id are both bare integers on the way in, which
/// is precisely why [`UserId`] is a newtype — transposing them here would file
/// the entry under a user id that happens to be a food id.
pub async fn insert_meal_entry(
    pool: &DbPool,
    food_item_id: i64,
    date: &str,
    grams: f64,
    slot: &str,
    user: UserId,
) -> Result<i64, sqlx::Error> {
    let uid = user.get();
    let id = sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot, user_id) VALUES (?, ?, ?, ?, ?) RETURNING id",
        food_item_id,
        date,
        grams,
        slot,
        uid
    )
    .fetch_one(pool)
    .await?
    .id;
    id.ok_or(sqlx::Error::RowNotFound)
}

pub async fn get_recent_foods(
    pool: &DbPool,
    limit: i64,
    user: UserId,
) -> Vec<crate::models::RecentFood> {
    let uid = user.get();
    // Bare columns + MAX() in SQLite resolve to the max row's values; the
    // annotated alias can't be referenced by name, so ORDER BY ordinal 5.
    sqlx::query!(
        r#"SELECT me.food_item_id as "food_item_id!", fi.name as "name!",
                  me.grams as "last_grams!: f64", me.slot as "last_slot!",
                  MAX(me.created_at || '-' || printf('%012d', me.id)) as "latest!: String"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.user_id = ?
        GROUP BY me.food_item_id
        ORDER BY 5 DESC
        LIMIT ?"#,
        uid,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecentFood {
        food_item_id: r.food_item_id,
        name: r.name,
        last_grams: r.last_grams,
        last_slot: r.last_slot,
    })
    .collect()
}

pub async fn get_food_item_by_barcode(
    pool: &DbPool,
    barcode: &str,
    user: UserId,
) -> Option<FoodItem> {
    let uid = user.get();
    sqlx::query_as!(
        FoodItem,
        r#"SELECT
            fi.id, fi.name, fi.brand, fi.barcode, fi.calories, fi.protein, fi.carbs,
            fi.fat, fi.fiber, fi.sugar, fi.sodium, fi.saturated_fat, fi.package_size,
            COALESCE(p.custom_portions, '') as "custom_portions!",
            fi.image_url, fi.category,
            COALESCE(p.is_favourite, 0) as "is_favourite!",
            p.default_portion_g, fi.created_at
        FROM food_items fi
        LEFT JOIN user_food_prefs p ON p.food_item_id = fi.id AND p.user_id = ?
        WHERE fi.barcode = ?"#,
        uid,
        barcode
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn upsert_weight(pool: &DbPool, date: &str, kg: f64, user: UserId) {
    let uid = user.get();
    sqlx::query!(
        "INSERT INTO weights (user_id, date, kg) VALUES (?, ?, ?) ON CONFLICT(user_id, date) DO UPDATE SET kg = excluded.kg",
        uid,
        date,
        kg
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_weights_since(pool: &DbPool, start: &str, user: UserId) -> Vec<(String, f64)> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT date as "date!", kg as "kg!: f64" FROM weights WHERE date >= ? AND user_id = ? ORDER BY date ASC"#,
        start,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.kg))
    .collect()
}

pub async fn get_latest_weight(pool: &DbPool, user: UserId) -> Option<(String, f64)> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT date as "date!", kg as "kg!: f64" FROM weights WHERE user_id = ? ORDER BY date DESC LIMIT 1"#,
        uid
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .map(|r| (r.date, r.kg))
}

// The five aggregates below are the easiest to get wrong: none of them mentions
// an entry id, so an unscoped version reads as a perfectly sensible query and
// silently totals the whole household into one person's charts and streaks.

pub async fn get_protein_by_date_range(
    pool: &DbPool,
    start: &str,
    end: &str,
    user: UserId,
) -> Vec<(String, f64)> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.protein) as "protein!: f64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ? AND me.user_id = ?
        GROUP BY me.date ORDER BY me.date ASC"#,
        start,
        end,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.protein))
    .collect()
}

pub async fn get_logged_dates_desc(pool: &DbPool, limit: i64, user: UserId) -> Vec<String> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT DISTINCT date as "date!" FROM meal_entries WHERE user_id = ? ORDER BY date DESC LIMIT ?"#,
        uid,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.date)
    .collect()
}

pub async fn get_most_logged_between(
    pool: &DbPool,
    start: &str,
    end: &str,
    limit: i64,
    user: UserId,
) -> Vec<(String, i64)> {
    let uid = user.get();
    // ORDER BY ordinal 2: the annotated count alias can't be named in ORDER BY
    sqlx::query!(
        r#"SELECT fi.name as "name!", COUNT(*) as "n!: i64"
        FROM meal_entries me JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ? AND me.user_id = ?
        GROUP BY me.food_item_id ORDER BY 2 DESC, fi.name ASC LIMIT ?"#,
        start,
        end,
        uid,
        limit
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.name, r.n))
    .collect()
}

pub async fn create_recipe_from_slot(
    pool: &DbPool,
    name: &str,
    date: &str,
    slot: &str,
    user: UserId,
) -> Option<i64> {
    let uid = user.get();
    let mut tx = pool.begin().await.ok()?;
    let rows = sqlx::query!(
        "SELECT food_item_id, grams FROM meal_entries WHERE date = ? AND slot = ? AND user_id = ?",
        date,
        slot,
        uid
    )
    .fetch_all(&mut *tx)
    .await
    .ok()?;
    if rows.is_empty() {
        return None;
    }
    let rid = sqlx::query!(
        "INSERT INTO recipes (name, user_id) VALUES (?, ?) RETURNING id",
        name,
        uid
    )
    .fetch_one(&mut *tx)
    .await
    .ok()?
    .id;
    for r in rows {
        sqlx::query!(
            "INSERT INTO recipe_items (recipe_id, food_item_id, grams) VALUES (?, ?, ?)",
            rid,
            r.food_item_id,
            r.grams
        )
        .execute(&mut *tx)
        .await
        .ok()?;
    }
    tx.commit().await.ok()?;
    Some(rid)
}

pub async fn get_recipes_with_totals(
    pool: &DbPool,
    user: UserId,
) -> Vec<crate::models::RecipeWithTotals> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT r.id as "id!", r.name as "name!",
                  COUNT(ri.id) as "item_count!: i64",
                  COALESCE(SUM(ri.grams / 100.0 * fi.calories), 0) as "total_cal!: f64"
        FROM recipes r
        LEFT JOIN recipe_items ri ON ri.recipe_id = r.id
        LEFT JOIN food_items fi ON fi.id = ri.food_item_id
        WHERE r.user_id = ?
        GROUP BY r.id ORDER BY r.name ASC"#,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| crate::models::RecipeWithTotals {
        id: r.id,
        name: r.name,
        item_count: r.item_count,
        total_cal: r.total_cal,
    })
    .collect()
}

/// Logs a saved recipe into a day.
///
/// The `r.user_id = ?` in the SELECT is the access check: logging a recipe id
/// belonging to someone else matches no rows and inserts nothing, rather than
/// copying their meal into your day. The new entries are stamped with the
/// logger's id, not the recipe owner's — they are the same person here, but
/// writing it explicitly keeps that true if the ownership rule ever loosens.
/// Logs every item of a saved meal, returning the entry ids it created.
///
/// The ids are the point: a meal is one action to the person tapping it, so its
/// rows must flag together and undo together. `RETURNING` gives them straight
/// from the insert — reading them back afterwards would race a second tab
/// logging the same meal.
pub async fn log_recipe(pool: &DbPool, id: i64, date: &str, slot: &str, user: UserId) -> Vec<i64> {
    let uid = user.get();
    sqlx::query!(
        r#"INSERT INTO meal_entries (food_item_id, date, grams, slot, user_id)
         SELECT ri.food_item_id, ?, ri.grams, ?, ?
         FROM recipe_items ri
         JOIN recipes r ON r.id = ri.recipe_id
         WHERE ri.recipe_id = ? AND r.user_id = ?
         RETURNING id as "id!""#,
        date,
        slot,
        uid,
        id,
        uid
    )
    .fetch_all(pool)
    .await
    .map(|rows| rows.into_iter().map(|r| r.id).collect())
    .unwrap_or_default()
}

/// Creates a saved meal from an explicit list of foods and amounts.
///
/// The sibling of `create_recipe_from_slot`, which can only capture what is
/// already logged. The builder needs to compose a meal you have not eaten yet.
pub async fn create_recipe_from_items(
    pool: &DbPool,
    name: &str,
    items: &[(i64, f64)],
    user: UserId,
) -> Option<i64> {
    if items.is_empty() {
        return None;
    }
    let uid = user.get();
    let mut tx = pool.begin().await.ok()?;
    let rid = sqlx::query!(
        "INSERT INTO recipes (name, user_id) VALUES (?, ?) RETURNING id",
        name,
        uid
    )
    .fetch_one(&mut *tx)
    .await
    .ok()?
    .id;
    for (food_item_id, grams) in items {
        sqlx::query!(
            "INSERT INTO recipe_items (recipe_id, food_item_id, grams) VALUES (?, ?, ?)",
            rid,
            food_item_id,
            grams
        )
        .execute(&mut *tx)
        .await
        .ok()?;
    }
    tx.commit().await.ok()?;
    Some(rid)
}

/// Deletes one of this user's recipes. Someone else's id deletes nothing.
///
/// The child rows go first but are gated on the parent's ownership via
/// `EXISTS` — otherwise passing another user's recipe id would strip its items
/// while leaving the recipe itself standing, which is worse than either
/// deleting it or refusing.
pub async fn delete_recipe(pool: &DbPool, id: i64, user: UserId) -> bool {
    let uid = user.get();
    sqlx::query!(
        "DELETE FROM recipe_items WHERE recipe_id = ?
         AND EXISTS (SELECT 1 FROM recipes r WHERE r.id = ? AND r.user_id = ?)",
        id,
        id,
        uid
    )
    .execute(pool)
    .await
    .ok();
    sqlx::query!("DELETE FROM recipes WHERE id = ? AND user_id = ?", id, uid)
        .execute(pool)
        .await
        .map(|r| r.rows_affected() > 0)
        .unwrap_or(false)
}

/// Copies a day's entries onto another date, within one user's log.
///
/// Scoped on both ends: the `WHERE` picks up only the copier's rows, and the
/// inserted rows carry their id. Unscoped, this was the sharpest of the
/// aggregate-shaped functions — it would have copied *everyone's* meals for
/// that date into one person's day.
pub async fn copy_day_entries(pool: &DbPool, from_date: &str, to_date: &str, user: UserId) -> u64 {
    let uid = user.get();
    sqlx::query!(
        "INSERT INTO meal_entries (food_item_id, date, grams, slot, user_id)
         SELECT food_item_id, ?, grams, slot, ? FROM meal_entries WHERE date = ? AND user_id = ?",
        to_date,
        uid,
        from_date,
        uid
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected())
    .unwrap_or(0)
}

pub async fn get_calories_by_date_range(
    pool: &DbPool,
    start: &str,
    end: &str,
    user: UserId,
) -> Vec<(String, f64)> {
    let uid = user.get();
    sqlx::query!(
        r#"SELECT me.date as "date!", SUM(me.grams / 100.0 * fi.calories) as "cal!: f64"
        FROM meal_entries me
        JOIN food_items fi ON fi.id = me.food_item_id
        WHERE me.date >= ? AND me.date <= ? AND me.user_id = ?
        GROUP BY me.date
        ORDER BY me.date ASC"#,
        start,
        end,
        uid
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| (r.date, r.cal))
    .collect()
}

// The three functions below are addressed by entry id, and entry ids are
// sequential integers. `AND user_id = ?` is not decoration — without it,
// `PUT /api/nutrition/entries/41` edits whoever's entry 41 happens to be, and
// the ids worth trying are the ones either side of your own.
//
// The two mutations report whether they touched anything. The HTMX handlers
// deliberately ignore that and re-render the caller's own day either way: on a
// fragment endpoint a 404 would leave the UI stale after an ordinary double
// click, and the response is the caller's unchanged day regardless, so a probe
// learns nothing from it. The bool is what lets the tests assert the no-op
// actually happened rather than inferring it from a lack of visible change.
// `get_meal_entry` returns `None`, which `entry_edit_form` turns into the same
// 404 an unknown id gets.

pub async fn update_meal_entry(
    pool: &DbPool,
    id: i64,
    grams: f64,
    slot: &str,
    user: UserId,
) -> bool {
    let uid = user.get();
    sqlx::query!(
        "UPDATE meal_entries SET grams = ?, slot = ? WHERE id = ? AND user_id = ?",
        grams,
        slot,
        id,
        uid
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

pub async fn get_meal_entry(
    pool: &DbPool,
    id: i64,
    user: UserId,
) -> Option<crate::models::MealEntry> {
    let uid = user.get();
    sqlx::query_as!(
        crate::models::MealEntry,
        r#"SELECT id, food_item_id, date, grams, slot as "slot!", created_at FROM meal_entries WHERE id = ? AND user_id = ?"#,
        id,
        uid
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
}

pub async fn delete_meal_entry(pool: &DbPool, id: i64, user: UserId) -> bool {
    let uid = user.get();
    sqlx::query!(
        "DELETE FROM meal_entries WHERE id = ? AND user_id = ?",
        id,
        uid
    )
    .execute(pool)
    .await
    .map(|r| r.rows_affected() > 0)
    .unwrap_or(false)
}

/// This user's macro targets, or the house defaults if they have not set any.
///
/// The fallback is what makes migration 018 safe to ship without seeding a row
/// per user: a new account has no `targets` row and reads the same defaults the
/// single-user version shipped with, rather than zeroes that would divide the
/// progress rings by nothing.
pub async fn get_targets(pool: &DbPool, user: UserId) -> Targets {
    let uid = user.get();
    sqlx::query_as!(Targets,
        r#"SELECT calories as "calories!", protein as "protein!", carbs as "carbs!", fat as "fat!" FROM targets WHERE user_id = ?"#,
        uid
    )
    .fetch_optional(pool)
    .await
    .ok()
    .flatten()
    .unwrap_or(Targets { calories: 2400.0, protein: 165.0, carbs: 260.0, fat: 72.0 })
}

pub async fn set_targets(
    pool: &DbPool,
    calories: f64,
    protein: f64,
    carbs: f64,
    fat: f64,
    user: UserId,
) {
    let uid = user.get();
    sqlx::query!(
        "INSERT INTO targets (user_id, calories, protein, carbs, fat) VALUES (?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET calories = excluded.calories, protein = excluded.protein,
         carbs = excluded.carbs, fat = excluded.fat",
        uid,
        calories,
        protein,
        carbs,
        fat
    )
    .execute(pool)
    .await
    .ok();
}

// ── Drawing tasks ─────────────────────────────────────────────────────────────

/// Filter/sort parameters for the /tasks page. Empty string = "no filter".
/// `sort` is one of: "newest" (default), "oldest", "easiest", "hardest".
pub struct TaskFilters {
    pub subject: String,
    pub difficulty: String,
    pub task_type: String,
    pub sort: String,
}

pub async fn insert_task_image(pool: &DbPool, title: &str, image_url: &str) {
    sqlx::query!(
        "INSERT INTO task_images (title, image_url) VALUES (?, ?)",
        title,
        image_url
    )
    .execute(pool)
    .await
    .expect("failed to insert task image");
}

pub async fn get_task_images(pool: &DbPool) -> Vec<TaskImage> {
    sqlx::query_as!(TaskImage,
        "SELECT id, title, image_url, created_at FROM task_images ORDER BY created_at DESC, id DESC"
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
}

/// Deletes a reference image and all tasks attached to it, returning the
/// image URL so the caller can remove the object from S3.
/// Returns Some only after the transaction has committed — the caller
/// deletes the S3 object, which must never happen if the rows survived.
/// (Tasks are deleted explicitly rather than via ON DELETE CASCADE so the
/// behavior doesn't depend on the connection's foreign_keys pragma.)
pub async fn delete_task_image(pool: &DbPool, id: i64) -> Option<String> {
    let mut tx = pool.begin().await.ok()?;

    let row = sqlx::query!("SELECT image_url FROM task_images WHERE id = ?", id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()?;

    sqlx::query!("DELETE FROM drawing_tasks WHERE image_id = ?", id)
        .execute(&mut *tx)
        .await
        .ok()?;
    sqlx::query!("DELETE FROM task_images WHERE id = ?", id)
        .execute(&mut *tx)
        .await
        .ok()?;
    tx.commit().await.ok()?;
    Some(row.image_url)
}

/// Returns false if the insert is rejected — e.g. the FK on image_id fails
/// because the reference image was deleted after the form was rendered.
pub async fn insert_drawing_task(
    pool: &DbPool,
    image_id: i64,
    title: &str,
    prompt: &str,
    subject: &str,
    difficulty: &str,
    task_type: &str,
) -> bool {
    sqlx::query!(
        "INSERT INTO drawing_tasks (image_id, title, prompt, subject, difficulty, task_type) VALUES (?, ?, ?, ?, ?, ?)",
        image_id, title, prompt, subject, difficulty, task_type
    )
    .execute(pool)
    .await
    .is_ok()
}

pub async fn delete_drawing_task(pool: &DbPool, id: i64) {
    sqlx::query!("DELETE FROM drawing_tasks WHERE id = ?", id)
        .execute(pool)
        .await
        .ok();
}

pub async fn toggle_task_completed(pool: &DbPool, id: i64) {
    sqlx::query!(
        "UPDATE drawing_tasks SET completed = 1 - completed WHERE id = ?",
        id
    )
    .execute(pool)
    .await
    .ok();
}

pub async fn get_tasks_filtered(pool: &DbPool, f: &TaskFilters) -> Vec<DrawingTaskWithImage> {
    let rows = sqlx::query!(
        r#"SELECT
            dt.id as "id!",
            dt.image_id as "image_id!",
            dt.title as "title!",
            dt.prompt as "prompt!",
            dt.subject as "subject!",
            dt.difficulty as "difficulty!",
            dt.task_type as "task_type!",
            dt.completed as "completed!",
            dt.created_at as "created_at!",
            ti.title as "image_title!",
            ti.image_url as "image_url!"
        FROM drawing_tasks dt
        JOIN task_images ti ON ti.id = dt.image_id
        WHERE (?1 = '' OR dt.subject = ?1)
          AND (?2 = '' OR dt.difficulty = ?2)
          AND (?3 = '' OR dt.task_type = ?3)
        ORDER BY
            CASE WHEN ?4 = 'easiest' THEN
                CASE dt.difficulty WHEN 'easy' THEN 0 WHEN 'medium' THEN 1 WHEN 'hard' THEN 2 ELSE 3 END
            END ASC,
            CASE WHEN ?4 = 'hardest' THEN
                CASE dt.difficulty WHEN 'hard' THEN 0 WHEN 'medium' THEN 1 WHEN 'easy' THEN 2 ELSE 3 END
            END ASC,
            CASE WHEN ?4 = 'oldest' THEN dt.id END ASC,
            dt.id DESC"#,
        f.subject, f.difficulty, f.task_type, f.sort
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();

    rows.into_iter()
        .map(|r| DrawingTaskWithImage {
            id: r.id,
            image_id: r.image_id,
            title: r.title,
            prompt: r.prompt,
            subject: r.subject,
            difficulty: r.difficulty,
            task_type: r.task_type,
            completed: r.completed != 0,
            created_at: r.created_at,
            image_title: r.image_title,
            image_url: r.image_url,
        })
        .collect()
}

/// Distinct non-empty subjects, for the filter dropdown and admin datalist.
pub async fn get_task_subjects(pool: &DbPool) -> Vec<String> {
    sqlx::query!(
        r#"SELECT DISTINCT subject as "subject!" FROM drawing_tasks WHERE subject != '' ORDER BY subject ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.subject)
    .collect()
}

/// Distinct non-empty task types, for the filter dropdown and admin datalist.
pub async fn get_task_types(pool: &DbPool) -> Vec<String> {
    sqlx::query!(
        r#"SELECT DISTINCT task_type as "task_type!" FROM drawing_tasks WHERE task_type != '' ORDER BY task_type ASC"#
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|r| r.task_type)
    .collect()
}

#[cfg(test)]
mod tests {
    /// The seeded owner. Every pre-multi-user test is written from their
    /// point of view, so they keep asserting exactly what they asserted before.
    const OWNER: UserId = UserId(1);

    use super::*;

    /// Creates a second account and returns its [`UserId`].
    async fn other_user(pool: &DbPool, name: &str) -> UserId {
        let id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, is_owner, is_admin) VALUES (?, 0, 0) RETURNING id",
        )
        .bind(name)
        .fetch_one(pool)
        .await
        .unwrap();
        UserId(id)
    }

    /// Adds a food to the shared catalog, returning its id.
    async fn seed_food(pool: &DbPool, name: &str, cal: f64, protein: f64, by: UserId) -> i64 {
        insert_food_item(
            pool, name, "Generic", None, cal, protein, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, None, "", "",
            by,
        )
        .await
        .id
    }

    async fn test_pool() -> DbPool {
        let pool = SqlitePoolOptions::new()
            .connect("sqlite::memory:")
            .await
            .unwrap();
        run_migrations(&pool).await;
        pool
    }

    #[tokio::test]
    async fn test_insert_and_get_post() {
        let pool = test_pool().await;
        let post = insert_post(
            &pool,
            "test caption",
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
        assert_eq!(post.caption, "test caption");
        let posts = get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].id, post.id);
    }

    #[tokio::test]
    async fn test_delete_post() {
        let pool = test_pool().await;
        let post = insert_post(
            &pool,
            "to delete",
            "https://example.com/img.jpg",
            "https://example.com/img-webp.webp",
            "https://example.com/img-avif.avif",
            crate::models::PostFormat::Single.as_str(),
            0,
            0,
            0,
            crate::models::Visibility::Public,
        )
        .await;
        let urls = delete_post_and_get_urls(&pool, post.id).await;
        assert!(urls.is_some());
        let urls = urls.unwrap();
        assert_eq!(urls.image_url, "https://example.com/img.jpg");
        assert_eq!(urls.webp_url, "https://example.com/img-webp.webp");
        assert_eq!(urls.avif_url, "https://example.com/img-avif.avif");
        assert!(
            get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_session_lifecycle() {
        let pool = test_pool().await;
        let id = "test-session-id";
        create_session(&pool, id, "2099-01-01T00:00:00", 1).await;
        assert!(get_session(&pool, id).await.is_some());
        delete_session(&pool, id).await;
        assert!(get_session(&pool, id).await.is_none());
    }

    #[tokio::test]
    async fn test_expired_session_rejected() {
        let pool = test_pool().await;
        create_session(&pool, "expired-id", "2000-01-01T00:00:00", 1).await;
        assert!(get_session(&pool, "expired-id").await.is_none());
    }

    // ── Accounts (pack 3) ────────────────────────────────────────────────────

    /// The owner cannot be demoted or deleted — by the management page, or by
    /// anything else.
    ///
    /// The template hides the controls, but that is a courtesy. The rule is
    /// `AND is_owner = 0` on every destructive statement, which is what still
    /// holds when the request is hand-made. Losing this means losing the only
    /// account that can grant admin.
    #[tokio::test]
    async fn test_owner_cannot_be_demoted_or_deleted() {
        let pool = test_pool().await;
        let owner = get_owner_user_id(&pool).await.unwrap();

        assert!(
            !set_user_admin(&pool, owner, false).await,
            "the owner's admin flag must not be revocable"
        );
        assert!(
            !delete_user(&pool, owner).await,
            "the owner must not be deletable"
        );

        // Still there, still an owner, still an admin.
        create_session(&pool, "s", "2099-01-01T00:00:00", owner).await;
        let s = get_session(&pool, "s").await.unwrap();
        assert!(s.is_owner);
        assert!(s.is_effective_admin());
    }

    /// Granting and revoking admin works on a member, and is exactly what
    /// `is_effective_admin` reports.
    #[tokio::test]
    async fn test_grant_and_revoke_admin_on_a_member() {
        let pool = test_pool().await;
        let id = create_user(&pool, "alex", "hash").await.unwrap();
        create_session(&pool, "s", "2099-01-01T00:00:00", id).await;

        assert!(!get_session(&pool, "s").await.unwrap().is_effective_admin());
        assert!(set_user_admin(&pool, id, true).await);
        assert!(get_session(&pool, "s").await.unwrap().is_effective_admin());
        assert!(set_user_admin(&pool, id, false).await);
        assert!(!get_session(&pool, "s").await.unwrap().is_effective_admin());
    }

    /// A created account is a member — never an owner, never an admin.
    #[tokio::test]
    async fn test_created_users_are_plain_members() {
        let pool = test_pool().await;
        let id = create_user(&pool, "alex", "hash").await.unwrap();
        let row = list_users(&pool)
            .await
            .into_iter()
            .find(|u| u.id == id)
            .unwrap();
        assert!(!row.is_owner);
        assert!(!row.is_admin);
        assert!(row.has_pin);

        // Names are unique case-insensitively, so "Alex" cannot shadow "alex".
        assert!(create_user(&pool, "ALEX", "hash").await.is_err());
    }

    /// Deleting a member takes every trace of them with it.
    ///
    /// Nothing cascades — the pool runs with `foreign_keys` off — so each table
    /// is cleared by hand, and a missed one would leave rows keyed to an id
    /// that `AUTOINCREMENT` eventually reissues. The next member created would
    /// open their tracker onto a stranger's food log.
    #[tokio::test]
    async fn test_deleting_a_member_removes_all_their_data() {
        let pool = test_pool().await;
        let alex = UserId(create_user(&pool, "alex", "hash").await.unwrap());
        let food = seed_food(&pool, "Oats", 380.0, 13.0, OWNER).await;

        insert_meal_entry(&pool, food, "2026-08-16", 80.0, "breakfast", alex)
            .await
            .unwrap();
        upsert_weight(&pool, "2026-08-16", 61.0, alex).await;
        set_targets(&pool, 1800.0, 120.0, 180.0, 60.0, alex).await;
        toggle_food_favourite(&pool, food, alex).await;
        create_recipe_from_slot(&pool, "Alex's breakfast", "2026-08-16", "breakfast", alex)
            .await
            .unwrap();
        create_session(&pool, "alex-session", "2099-01-01T00:00:00", alex.get()).await;
        save_credential(&pool, "alex-cred", "{}", alex.get()).await;

        assert!(delete_user(&pool, alex.get()).await);

        for (table, n) in [
            ("meal_entries", 0),
            ("weights", 0),
            ("targets", 0),
            ("user_food_prefs", 0),
            ("recipes", 0),
            ("recipe_items", 0),
            ("sessions", 0),
            ("passkey_credentials", 0),
        ] {
            let count: i64 =
                sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table} WHERE user_id = ?"))
                    .bind(alex.get())
                    .fetch_one(&pool)
                    .await
                    .unwrap_or(0);
            // recipe_items has no user_id; checked separately below.
            if table != "recipe_items" {
                assert_eq!(count, n, "{table} still holds rows for the deleted user");
            }
        }
        let orphan_items: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM recipe_items")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(orphan_items, 0, "recipe_items orphaned by the delete");

        // The session is gone, so the cookie no longer authenticates.
        assert!(get_session(&pool, "alex-session").await.is_none());

        // And the shared catalog is untouched — deleting a person must not
        // delete the food everybody uses.
        assert_eq!(get_food_items(&pool, OWNER).await.len(), 1);
    }

    /// Wrong PINs lock the account, and the lock is absolute-timestamped so a
    /// restart cannot reset an attacker's budget.
    #[tokio::test]
    async fn test_pin_lockout_after_repeated_failures() {
        let pool = test_pool().await;
        let id = create_user(&pool, "alex", "hash").await.unwrap();

        for i in 1..crate::pin::MAX_PIN_ATTEMPTS {
            record_failed_pin(&pool, id).await;
            let u = get_user_by_name(&pool, "alex").await.unwrap();
            assert_eq!(u.failed_pin_attempts, i);
            assert!(!u.is_locked, "locked early, after {i} attempts");
        }

        record_failed_pin(&pool, id).await;
        let u = get_user_by_name(&pool, "alex").await.unwrap();
        assert!(
            u.is_locked,
            "not locked after {} attempts",
            crate::pin::MAX_PIN_ATTEMPTS
        );

        // Setting a PIN clears the lock — this is how the owner rescues someone
        // who locked themselves out.
        assert!(set_user_pin(&pool, id, "newhash").await);
        let u = get_user_by_name(&pool, "alex").await.unwrap();
        assert!(!u.is_locked);
        assert_eq!(u.failed_pin_attempts, 0);
    }

    /// A successful login clears the failure budget.
    #[tokio::test]
    async fn test_successful_login_clears_failed_attempts() {
        let pool = test_pool().await;
        let id = create_user(&pool, "alex", "hash").await.unwrap();
        record_failed_pin(&pool, id).await;
        record_failed_pin(&pool, id).await;
        assert_eq!(
            get_user_by_name(&pool, "alex")
                .await
                .unwrap()
                .failed_pin_attempts,
            2
        );
        clear_failed_pins(&pool, id).await;
        assert_eq!(
            get_user_by_name(&pool, "alex")
                .await
                .unwrap()
                .failed_pin_attempts,
            0
        );
    }

    /// Lookup by name is case-insensitive, matching the UNIQUE COLLATE NOCASE
    /// index — someone who typed "Alex" logs in as "alex".
    #[tokio::test]
    async fn test_user_lookup_is_case_insensitive() {
        let pool = test_pool().await;
        create_user(&pool, "alex", "hash").await.unwrap();
        assert!(get_user_by_name(&pool, "ALEX").await.is_some());
        assert!(get_user_by_name(&pool, "Alex").await.is_some());
        assert!(get_user_by_name(&pool, "alexx").await.is_none());
    }

    /// The seeded owner has no PIN, and their empty hash must never verify —
    /// otherwise an empty PIN would log in as the owner.
    #[tokio::test]
    async fn test_owner_without_pin_cannot_be_reached_by_an_empty_pin() {
        let pool = test_pool().await;
        let owner = get_user_by_name(&pool, "admin").await.expect("owner row");
        assert_eq!(owner.pin_hash, "", "the seeded owner has no PIN");
        assert!(!crate::pin::verify_pin("", &owner.pin_hash));
        assert!(!crate::pin::verify_pin("1234", &owner.pin_hash));
    }

    // ── Multi-user isolation (pack 2) ────────────────────────────────────────

    /// The pack's headline claim: two people logging the same day see only
    /// their own food, weight and targets.
    #[tokio::test]
    async fn test_two_users_do_not_see_each_others_day() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let chicken = seed_food(&pool, "Chicken", 165.0, 31.0, OWNER).await;
        let oats = seed_food(&pool, "Oats", 380.0, 13.0, OWNER).await;

        insert_meal_entry(&pool, chicken, "2026-08-16", 200.0, "lunch", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, oats, "2026-08-16", 80.0, "breakfast", alex)
            .await
            .unwrap();

        let owner_day = get_meal_entries_for_date(&pool, "2026-08-16", OWNER).await;
        let alex_day = get_meal_entries_for_date(&pool, "2026-08-16", alex).await;
        assert_eq!(owner_day.len(), 1);
        assert_eq!(alex_day.len(), 1);
        assert_eq!(owner_day[0].food_name, "Chicken");
        assert_eq!(alex_day[0].food_name, "Oats");

        // Weights: the old schema had `date` as the primary key, so this pair
        // of writes would have been one row before the rebuild.
        upsert_weight(&pool, "2026-08-16", 82.0, OWNER).await;
        upsert_weight(&pool, "2026-08-16", 61.5, alex).await;
        assert_eq!(get_latest_weight(&pool, OWNER).await.unwrap().1, 82.0);
        assert_eq!(get_latest_weight(&pool, alex).await.unwrap().1, 61.5);

        // Targets: likewise a single `CHECK (id = 1)` row before.
        set_targets(&pool, 3000.0, 200.0, 300.0, 90.0, OWNER).await;
        set_targets(&pool, 1800.0, 120.0, 180.0, 60.0, alex).await;
        assert_eq!(get_targets(&pool, OWNER).await.calories, 3000.0);
        assert_eq!(get_targets(&pool, alex).await.calories, 1800.0);
    }

    /// A user with no targets row gets the house defaults, not zeroes.
    ///
    /// Migration 018 seeds no row per user, so this is what a brand-new account
    /// reads on its first visit — zeroes would divide the progress rings by
    /// nothing and render the day as infinitely over target.
    #[tokio::test]
    async fn test_new_user_gets_default_targets() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let t = get_targets(&pool, alex).await;
        assert_eq!(t.calories, 2400.0);
        assert_eq!(t.protein, 165.0);
    }

    /// **The IDOR test.** Entry ids are sequential, so the ids worth trying are
    /// the ones either side of your own.
    ///
    /// Accepting a `UserId` is not the same as using it: each of these
    /// functions would compile, typecheck and pass every single-user test with
    /// the owner filter missing from its `WHERE` clause. This is the test that
    /// notices.
    #[tokio::test]
    async fn test_entry_addressed_by_id_is_not_reachable_by_another_user() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let chicken = seed_food(&pool, "Chicken", 165.0, 31.0, OWNER).await;

        let victim = insert_meal_entry(&pool, chicken, "2026-08-16", 200.0, "lunch", OWNER)
            .await
            .unwrap();

        // Read: invisible.
        assert!(
            get_meal_entry(&pool, victim, alex).await.is_none(),
            "another user's entry must not be readable by id"
        );
        assert!(get_meal_entry(&pool, victim, OWNER).await.is_some());

        // Update: refused, and genuinely unchanged.
        assert!(
            !update_meal_entry(&pool, victim, 999.0, "dinner", alex).await,
            "another user's entry must not be editable by id"
        );
        let after = get_meal_entry(&pool, victim, OWNER).await.unwrap();
        assert_eq!(after.grams, 200.0, "the victim's entry was modified");
        assert_eq!(after.slot, "lunch");

        // Delete: refused, and still there.
        assert!(
            !delete_meal_entry(&pool, victim, alex).await,
            "another user's entry must not be deletable by id"
        );
        assert!(get_meal_entry(&pool, victim, OWNER).await.is_some());

        // The owner can still do all three — the gate is ownership, not a
        // blanket refusal.
        assert!(update_meal_entry(&pool, victim, 250.0, "dinner", OWNER).await);
        assert!(delete_meal_entry(&pool, victim, OWNER).await);
        assert!(get_meal_entry(&pool, victim, OWNER).await.is_none());
    }

    /// Recipes are private; logging or deleting someone else's does nothing.
    #[tokio::test]
    async fn test_recipes_are_private_to_their_owner() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let chicken = seed_food(&pool, "Chicken", 165.0, 31.0, OWNER).await;
        insert_meal_entry(&pool, chicken, "2026-08-16", 200.0, "lunch", OWNER)
            .await
            .unwrap();

        let rid = create_recipe_from_slot(&pool, "Owner's lunch", "2026-08-16", "lunch", OWNER)
            .await
            .expect("recipe created");

        assert_eq!(get_recipes_with_totals(&pool, OWNER).await.len(), 1);
        assert!(
            get_recipes_with_totals(&pool, alex).await.is_empty(),
            "recipes must not be listed to other users"
        );

        // Logging someone else's recipe inserts nothing.
        assert!(
            log_recipe(&pool, rid, "2026-08-17", "dinner", alex)
                .await
                .is_empty(),
            "another user's recipe must not be loggable"
        );
        assert!(get_meal_entries_for_date(&pool, "2026-08-17", alex)
            .await
            .is_empty());

        // Deleting someone else's does nothing, and leaves it *wholly* intact.
        //
        // The item_count assertion is the load-bearing one and is here on
        // purpose: `delete_recipe` deletes the child rows first, gated on the
        // parent's ownership by an `EXISTS`. Drop that `EXISTS` and the recipe
        // row survives — so `get_recipes_with_totals(..).len() == 1` still
        // passes — while its items are silently gone. Checking only the count
        // would let the subtlest SQL in the pack fail unnoticed.
        assert!(!delete_recipe(&pool, rid, alex).await);
        let survivors = get_recipes_with_totals(&pool, OWNER).await;
        assert_eq!(survivors.len(), 1);
        assert_eq!(
            survivors[0].item_count, 1,
            "another user's delete stripped the recipe's items"
        );
        assert!(survivors[0].total_cal > 0.0);

        // The owner's own calls work.
        assert_eq!(
            log_recipe(&pool, rid, "2026-08-17", "dinner", OWNER)
                .await
                .len(),
            1
        );
        assert!(delete_recipe(&pool, rid, OWNER).await);
    }

    /// The catalog is shared; the opinions about it are not.
    #[tokio::test]
    async fn test_catalog_is_shared_but_preferences_are_not() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let oats = seed_food(&pool, "Oats", 380.0, 13.0, OWNER).await;

        // Shared: a food added by one is visible to the other.
        assert_eq!(get_food_items(&pool, alex).await.len(), 1);
        assert_eq!(search_food_items(&pool, "Oat", alex).await.len(), 1);

        // Not shared: favouriting is personal.
        toggle_food_favourite(&pool, oats, OWNER).await;
        assert_eq!(
            get_food_item(&pool, oats, OWNER)
                .await
                .unwrap()
                .is_favourite,
            1
        );
        assert_eq!(
            get_food_item(&pool, oats, alex).await.unwrap().is_favourite,
            0,
            "one user's favourite must not become everyone's"
        );

        // Alex favourites it too; both are true independently, and un-
        // favouriting one leaves the other alone.
        toggle_food_favourite(&pool, oats, alex).await;
        assert_eq!(
            get_food_item(&pool, oats, alex).await.unwrap().is_favourite,
            1
        );
        toggle_food_favourite(&pool, oats, alex).await;
        assert_eq!(
            get_food_item(&pool, oats, alex).await.unwrap().is_favourite,
            0
        );
        assert_eq!(
            get_food_item(&pool, oats, OWNER)
                .await
                .unwrap()
                .is_favourite,
            1,
            "the owner's favourite survived another user toggling theirs"
        );

        // A food with no preference row at all reads as "no opinion" rather
        // than dropping out of the catalog — the LEFT JOIN, not an INNER one.
        let fresh = seed_food(&pool, "Rice", 130.0, 2.7, OWNER).await;
        let seen = get_food_item(&pool, fresh, alex)
            .await
            .expect("still listed");
        assert_eq!(seen.is_favourite, 0);
        assert_eq!(seen.custom_portions, "");
        assert_eq!(seen.default_portion_g, None);
    }

    /// Editing a food writes shared facts to the catalog and personal ones to
    /// the editor's own preference row.
    #[tokio::test]
    async fn test_food_edit_splits_shared_facts_from_personal_opinions() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let oats = seed_food(&pool, "Oats", 380.0, 13.0, OWNER).await;

        update_food_item(
            &pool,
            oats,
            "Rolled Oats",
            "Quaker",
            None,
            370.0,
            13.5,
            60.0,
            7.0,
            10.0,
            1.0,
            5.0,
            2.0,
            None,
            "40,80",
            "",
            "grains",
            true,
            Some(80.0),
            OWNER,
        )
        .await;

        // Shared: Alex sees the corrected nutrition facts and the new name.
        let as_alex = get_food_item(&pool, oats, alex).await.unwrap();
        assert_eq!(as_alex.name, "Rolled Oats");
        assert_eq!(as_alex.calories, 370.0);
        assert_eq!(as_alex.category, "grains");

        // Personal: the favourite flag and portions stayed with the owner.
        assert_eq!(as_alex.is_favourite, 0);
        assert_eq!(as_alex.custom_portions, "");
        assert_eq!(as_alex.default_portion_g, None);

        let as_owner = get_food_item(&pool, oats, OWNER).await.unwrap();
        assert_eq!(as_owner.is_favourite, 1);
        assert_eq!(as_owner.custom_portions, "40,80");
        assert_eq!(as_owner.default_portion_g, Some(80.0));
    }

    /// The aggregates — the ones with no entry id in sight, which read as
    /// perfectly sensible queries while silently totalling the household.
    #[tokio::test]
    async fn test_aggregates_and_copy_day_are_scoped() {
        let pool = test_pool().await;
        let alex = other_user(&pool, "alex").await;
        let chicken = seed_food(&pool, "Chicken", 165.0, 31.0, OWNER).await;
        let oats = seed_food(&pool, "Oats", 380.0, 13.0, OWNER).await;

        insert_meal_entry(&pool, chicken, "2026-08-16", 100.0, "lunch", OWNER)
            .await
            .unwrap();
        for d in ["2026-08-14", "2026-08-15", "2026-08-16"] {
            insert_meal_entry(&pool, oats, d, 100.0, "breakfast", alex)
                .await
                .unwrap();
        }

        // Calories and protein by range: each sees only their own totals.
        let owner_cal = get_calories_by_date_range(&pool, "2026-08-10", "2026-08-20", OWNER).await;
        assert_eq!(owner_cal.len(), 1);
        assert_eq!(owner_cal[0].1, 165.0);
        let alex_cal = get_calories_by_date_range(&pool, "2026-08-10", "2026-08-20", alex).await;
        assert_eq!(alex_cal.len(), 3);
        assert_eq!(alex_cal[0].1, 380.0);

        let owner_protein =
            get_protein_by_date_range(&pool, "2026-08-10", "2026-08-20", OWNER).await;
        assert_eq!(owner_protein.len(), 1);
        assert_eq!(owner_protein[0].1, 31.0);

        // Logged dates drive the streak: the owner logged one day, not three.
        assert_eq!(get_logged_dates_desc(&pool, 10, OWNER).await.len(), 1);
        assert_eq!(get_logged_dates_desc(&pool, 10, alex).await.len(), 3);

        // Most-logged and recents.
        let top = get_most_logged_between(&pool, "2026-08-10", "2026-08-20", 5, OWNER).await;
        assert_eq!(top, vec![("Chicken".to_string(), 1)]);
        let alex_recent = get_recent_foods(&pool, 10, alex).await;
        assert_eq!(alex_recent.len(), 1);
        assert_eq!(alex_recent[0].name, "Oats");

        // Per-food history is personal even though the food is shared.
        let hist = get_item_log_history(&pool, oats, "2026-08-10", "2026-08-20", OWNER).await;
        assert!(
            hist.is_empty(),
            "the owner never ate Oats — Alex's grams must not appear in their history"
        );

        // Copy-day copies only the copier's own rows.
        let copied = copy_day_entries(&pool, "2026-08-16", "2026-08-20", OWNER).await;
        assert_eq!(copied, 1, "copy-day must not sweep up other users' entries");
        let dest = get_meal_entries_for_date(&pool, "2026-08-20", OWNER).await;
        assert_eq!(dest.len(), 1);
        assert_eq!(dest[0].food_name, "Chicken");
        assert!(get_meal_entries_for_date(&pool, "2026-08-20", alex)
            .await
            .is_empty());
    }

    /// `run_migrations` runs on every boot, and 015 is one of the few that
    /// `.expect()`s rather than shrugging its error off. Every other test calls
    /// it once, so nothing else exercises the second-boot path — and if
    /// `INSERT OR IGNORE` or the partial index ever misbehaved on re-run, the
    /// symptom would be the site failing to start after a deploy restart.
    #[tokio::test]
    async fn test_migrations_are_idempotent_across_boots() {
        let pool = test_pool().await; // first boot
        run_migrations(&pool).await; // second
        run_migrations(&pool).await; // third, for good measure

        let owners: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE is_owner = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(
            owners, 1,
            "re-running migrations must not duplicate the owner"
        );

        // And the identity columns still work — not clobbered by a re-ALTER.
        create_session(&pool, "post-reboot", "2099-01-01T00:00:00", 1).await;
        assert!(get_session(&pool, "post-reboot").await.is_some());
    }

    /// Migration 015 seeds exactly one owner, and the partial unique index is
    /// what stops a second one existing. Without this the "owner cannot be
    /// demoted" rule would rest on nothing but the management page's UI.
    #[tokio::test]
    async fn test_single_owner_invariant() {
        let pool = test_pool().await;

        let owner_id = get_owner_user_id(&pool)
            .await
            .expect("owner must be seeded");
        assert_eq!(owner_id, 1);

        let second_owner = sqlx::query("INSERT INTO users (name, is_owner) VALUES ('impostor', 1)")
            .execute(&pool)
            .await;
        assert!(second_owner.is_err(), "a second owner must be rejected");

        // Non-owners are unconstrained — the index is on `is_owner = 1` only.
        for name in ["member-a", "member-b"] {
            sqlx::query("INSERT INTO users (name, is_owner) VALUES (?, 0)")
                .bind(name)
                .execute(&pool)
                .await
                .expect("non-owner insert must succeed");
        }
    }

    /// The session carries its user's flags because `get_session` joins
    /// `users`. A member's session must not report admin.
    #[tokio::test]
    async fn test_session_carries_user_flags() {
        let pool = test_pool().await;

        create_session(&pool, "owner-sess", "2099-01-01T00:00:00", 1).await;
        let owner = get_session(&pool, "owner-sess")
            .await
            .expect("owner session");
        assert!(owner.is_owner);
        assert!(owner.is_effective_admin());

        let member_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, is_owner, is_admin) VALUES ('member', 0, 0) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        create_session(&pool, "member-sess", "2099-01-01T00:00:00", member_id).await;
        let member = get_session(&pool, "member-sess")
            .await
            .expect("member session");
        assert!(!member.is_owner);
        assert!(
            !member.is_effective_admin(),
            "a plain member must never read as admin"
        );

        // A granted admin is an effective admin without being the owner.
        sqlx::query("UPDATE users SET is_admin = 1 WHERE id = ?")
            .bind(member_id)
            .execute(&pool)
            .await
            .unwrap();
        let granted = get_session(&pool, "member-sess")
            .await
            .expect("granted session");
        assert!(!granted.is_owner);
        assert!(granted.is_effective_admin());
    }

    /// An orphaned session — user deleted out from under it — must read as
    /// logged out rather than as a session with no permissions. The INNER JOIN
    /// in `get_session` is what makes that the default.
    #[tokio::test]
    async fn test_orphaned_session_reads_as_logged_out() {
        let pool = test_pool().await;
        let member_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO users (name, is_owner) VALUES ('doomed', 0) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        create_session(&pool, "orphan", "2099-01-01T00:00:00", member_id).await;
        assert!(get_session(&pool, "orphan").await.is_some());

        sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(member_id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(get_session(&pool, "orphan").await.is_none());
    }

    /// A passkey resolves to the user that registered it — the fact that turns
    /// "somebody valid authenticated" into "this person is logged in".
    #[tokio::test]
    async fn test_credential_resolves_to_its_user() {
        let pool = test_pool().await;
        save_credential(&pool, "cred-owner", "{}", 1).await;
        assert_eq!(get_credential_user_id(&pool, "cred-owner").await, Some(1));
        assert_eq!(get_credential_user_id(&pool, "no-such-cred").await, None);
    }

    #[tokio::test]
    async fn test_cleanup_removes_expired() {
        let pool = test_pool().await;
        create_session(&pool, "old-session", "2000-01-01T00:00:00", 1).await;
        save_challenge(&pool, "old-challenge", "{}", "2000-01-01T00:00:00").await;
        cleanup_expired(&pool).await;
        assert!(get_session(&pool, "old-session").await.is_none());
        assert!(take_challenge(&pool, "old-challenge").await.is_none());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_post_returns_none() {
        let pool = test_pool().await;
        let result = delete_post_and_get_urls(&pool, 99999).await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_insert_post_stores_format_and_filesize() {
        let pool = test_pool().await;
        let fmt = crate::models::PostFormat::Single.as_str();
        let post = insert_post(
            &pool,
            "hello",
            "https://example.com/img.jpg",
            "",
            "",
            fmt,
            12345,
            0,
            0,
            crate::models::Visibility::Public,
        )
        .await;
        assert_eq!(post.format, "single");
        assert_eq!(post.file_size_bytes, 12345);
    }

    #[tokio::test]
    async fn test_insert_post_empty_caption() {
        let pool = test_pool().await;
        let fmt = crate::models::PostFormat::Single.as_str();
        let post = insert_post(
            &pool,
            "",
            "https://example.com/img.jpg",
            "",
            "",
            fmt,
            0,
            0,
            0,
            crate::models::Visibility::Public,
        )
        .await;
        assert_eq!(post.caption, "");
    }

    #[tokio::test]
    async fn test_migrations_are_idempotent() {
        let pool = test_pool().await;
        // test_pool() has already run them once; a second pass must not panic.
        // SQLite has no ADD COLUMN IF NOT EXISTS, so every re-run returns a
        // duplicate-column error that the `let _ =` in run_migrations discards.
        run_migrations(&pool).await;
        assert!(
            get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn test_insert_post_persists_dimensions() {
        let pool = test_pool().await;
        let post = insert_post(
            &pool,
            "dimensioned",
            "https://example.com/img.jpg",
            "",
            "",
            crate::models::PostFormat::Single.as_str(),
            0,
            1600,
            900,
            crate::models::Visibility::Public,
        )
        .await;
        assert_eq!(post.image_width, 1600);
        assert_eq!(post.image_height, 900);

        let fetched = &get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin).await[0];
        assert_eq!(
            fetched.image_width, 1600,
            "dimensions survive the insert/select round trip"
        );
        assert_eq!(fetched.image_height, 900);
    }

    #[tokio::test]
    async fn test_legacy_rows_read_back_as_zero_dimensions() {
        let pool = test_pool().await;
        // A pre-012 row: inserted without touching the new columns, exactly as
        // the old insert_post would have. NOT NULL DEFAULT 0 on an ALTER over an
        // existing table is worth pinning down — a NULL here would make Post's
        // i64 fail to decode at runtime rather than at compile time.
        sqlx::query(
            "INSERT INTO posts (caption, image_url, webp_url, avif_url, format, file_size_bytes) \
             VALUES ('old', 'https://example.com/old.jpg', '', '', 'single', 0)",
        )
        .execute(&pool)
        .await
        .expect("legacy-shaped insert succeeds");

        let post = &get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin).await[0];
        assert_eq!(post.image_width, 0, "legacy rows read back as 0, not NULL");
        assert_eq!(post.image_height, 0);
    }

    /// Inserts a post carrying only a caption — the field caption search reads.
    async fn seed_caption(pool: &DbPool, caption: &str) {
        insert_post(
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
    }

    /// A `PostFilter` with only `q` set — the shape every caption-search test
    /// below used before the filter grew a `q`-only path through `PostFilter`.
    fn q_filter(q: &str) -> PostFilter {
        PostFilter {
            q: Some(q.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_like_pattern_escapes_wildcards() {
        assert_eq!(like_pattern("100%"), "%100\\%%");
        assert_eq!(like_pattern("a_b"), "%a\\_b%");
        // The escape character itself is doubled first, so it survives as a
        // literal rather than escaping whatever follows it.
        assert_eq!(like_pattern("c:\\x"), "%c:\\\\x%");
        assert_eq!(like_pattern("loomis"), "%loomis%");
    }

    #[test]
    fn test_normalize_tags_trims_and_lowercases() {
        assert_eq!(
            normalize_tags(" Ink , PERSPECTIVE "),
            vec!["ink".to_string(), "perspective".to_string()]
        );
    }

    #[test]
    fn test_normalize_tags_drops_empties_and_dupes() {
        assert_eq!(
            normalize_tags("ink,,Ink, ink ,wash"),
            vec!["ink".to_string(), "wash".to_string()]
        );
    }

    #[test]
    fn test_normalize_tags_drops_over_40_chars() {
        let long_tag = "a".repeat(41);
        let raw = format!("{long_tag},ok");
        assert_eq!(normalize_tags(&raw), vec!["ok".to_string()]);
    }

    #[test]
    fn test_normalize_tags_caps_at_20() {
        let raw = (1..=25)
            .map(|i| format!("t{i}"))
            .collect::<Vec<_>>()
            .join(",");
        let result = normalize_tags(&raw);
        assert_eq!(result.len(), 20);
        let expected: Vec<String> = (1..=20).map(|i| format!("t{i}")).collect();
        assert_eq!(result, expected);
    }

    #[test]
    fn test_normalize_tags_empty_input() {
        assert_eq!(normalize_tags(""), Vec::<String>::new());
    }

    #[test]
    fn test_slugify_basic() {
        assert_eq!(slugify("Figure Studies"), "figure-studies");
    }

    #[test]
    fn test_slugify_collapses_runs_and_trims() {
        assert_eq!(slugify("  Ink & Wash!  "), "ink-wash");
    }

    #[test]
    fn test_slugify_leading_trailing() {
        assert_eq!(slugify("--Inks--"), "inks");
    }

    #[test]
    fn test_slugify_all_junk_is_empty() {
        assert_eq!(slugify("!!!"), "");
    }

    #[tokio::test]
    async fn test_migration_014_is_idempotent() {
        let pool = test_pool().await;
        // test_pool() has already run migrations (including 014) once.
        run_migrations(&pool).await;
        sqlx::query("INSERT INTO tags (name) VALUES ('x')")
            .execute(&pool)
            .await
            .expect("insert into tags should succeed after idempotent re-run");
    }

    #[tokio::test]
    async fn test_get_posts_page_unfiltered_keeps_the_n_plus_1_probe() {
        let pool = test_pool().await;
        for i in 0..21 {
            seed_caption(&pool, &format!("caption {i}")).await;
        }
        // 21 rows come back on page 0: 20 to render plus the has_more probe,
        // which the caller truncates. Page 1 holds the single leftover.
        assert_eq!(
            get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin)
                .await
                .len(),
            21
        );
        assert_eq!(
            get_posts_page(&pool, &PostFilter::default(), 1, Viewer::Admin)
                .await
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn test_get_posts_page_filters_captions_case_insensitively() {
        let pool = test_pool().await;
        seed_caption(&pool, "Loomis head").await;
        seed_caption(&pool, "figure drawing").await;

        let hits = get_posts_page(&pool, &q_filter("loomis"), 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].caption, "Loomis head");
    }

    #[tokio::test]
    async fn test_search_for_a_literal_percent_does_not_match_every_row() {
        let pool = test_pool().await;
        seed_caption(&pool, "100% cotton paper").await;
        seed_caption(&pool, "graphite study").await;

        // Unescaped, the pattern would be %100%% — two wildcards around a
        // literal, which matches every row in the table.
        let hits = get_posts_page(&pool, &q_filter("100%"), 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 1, "a literal % must not act as a wildcard");
        assert_eq!(hits[0].caption, "100% cotton paper");
    }

    #[tokio::test]
    async fn test_search_for_a_literal_underscore_is_not_a_wildcard() {
        let pool = test_pool().await;
        seed_caption(&pool, "study_01").await;
        seed_caption(&pool, "studyA01").await;

        let hits = get_posts_page(&pool, &q_filter("study_0"), 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 1, "_ must match itself, not any character");
        assert_eq!(hits[0].caption, "study_01");
    }

    #[tokio::test]
    async fn test_count_posts_agrees_with_the_filtered_result() {
        let pool = test_pool().await;
        seed_caption(&pool, "gesture study").await;
        seed_caption(&pool, "hand study").await;
        seed_caption(&pool, "colour thumbnail").await;

        assert_eq!(
            count_posts(&pool, &PostFilter::default(), Viewer::Admin)
                .await
                .total,
            3
        );
        assert_eq!(
            count_posts(&pool, &q_filter("study"), Viewer::Admin)
                .await
                .total,
            2
        );
        assert_eq!(
            count_posts(&pool, &q_filter("nothing here"), Viewer::Admin)
                .await
                .total,
            0
        );
        assert_eq!(
            count_posts(&pool, &q_filter("study"), Viewer::Admin)
                .await
                .total as usize,
            get_posts_page(&pool, &q_filter("study"), 0, Viewer::Admin)
                .await
                .len(),
            "the head count and the rendered page must not disagree"
        );
    }

    #[tokio::test]
    async fn test_insert_and_get_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Chicken Breast",
            "Generic",
            None,
            165.0,
            31.0,
            0.0,
            3.6,
            0.0,
            0.0,
            74.0,
            1.0,
            None,
            "",
            "",
            OWNER,
        )
        .await;
        assert_eq!(item.name, "Chicken Breast");
        assert_eq!(item.calories, 165.0);
        assert!(item.barcode.is_none());
        let items = get_food_items(&pool, OWNER).await;
        assert_eq!(items.len(), 1);
    }

    #[tokio::test]
    async fn test_search_food_items() {
        let pool = test_pool().await;
        insert_food_item(
            &pool,
            "Chicken Breast",
            "Generic",
            None,
            165.0,
            31.0,
            0.0,
            3.6,
            0.0,
            0.0,
            74.0,
            1.0,
            None,
            "",
            "",
            OWNER,
        )
        .await;
        insert_food_item(
            &pool,
            "Brown Rice",
            "Generic",
            None,
            112.0,
            2.6,
            23.5,
            0.9,
            1.8,
            0.0,
            5.0,
            0.2,
            None,
            "",
            "",
            OWNER,
        )
        .await;
        let results = search_food_items(&pool, "chicken", OWNER).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "Chicken Breast");
    }

    #[tokio::test]
    async fn test_delete_food_item() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Test Item",
            "",
            None,
            100.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            None,
            "",
            "https://example.com/img.jpg",
            OWNER,
        )
        .await;
        let url = delete_food_item(&pool, item.id).await;
        assert_eq!(url, Some("https://example.com/img.jpg".to_string()));
        assert!(get_food_items(&pool, OWNER).await.is_empty());
    }

    #[tokio::test]
    async fn test_insert_meal_entry_and_get_for_date() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "White Rice",
            "",
            None,
            130.0,
            2.7,
            28.6,
            0.3,
            0.4,
            0.0,
            1.0,
            0.1,
            None,
            "",
            "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-04-09", 200.0, "other", OWNER)
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09", OWNER).await;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].food_name, "White Rice");
        assert_eq!(entries[0].grams, 200.0);
        assert!((entries[0].calories - 260.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_delete_meal_entry() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Apple", "", None, 52.0, 0.3, 14.0, 0.2, 2.4, 10.0, 1.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        let entry_id = insert_meal_entry(&pool, item.id, "2026-04-09", 150.0, "other", OWNER)
            .await
            .unwrap();
        delete_meal_entry(&pool, entry_id, OWNER).await;
        assert!(get_meal_entries_for_date(&pool, "2026-04-09", OWNER)
            .await
            .is_empty());
    }

    fn no_filters() -> TaskFilters {
        TaskFilters {
            subject: String::new(),
            difficulty: String::new(),
            task_type: String::new(),
            sort: "newest".to_string(),
        }
    }

    async fn seed_image(pool: &DbPool) -> i64 {
        insert_task_image(pool, "Test model", "https://example.com/model.jpg").await;
        get_task_images(pool).await[0].id
    }

    #[tokio::test]
    async fn test_insert_task_and_get() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(
            &pool,
            img_id,
            "Draw the hands",
            "Focus on the hands only",
            "anatomy",
            "hard",
            "focus study",
        )
        .await;
        let tasks = get_tasks_filtered(&pool, &no_filters()).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "Draw the hands");
        assert_eq!(tasks[0].image_url, "https://example.com/model.jpg");
        assert_eq!(tasks[0].image_title, "Test model");
        assert!(!tasks[0].completed);
    }

    #[tokio::test]
    async fn test_multiple_tasks_on_one_image() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(
            &pool,
            img_id,
            "Focus on hands",
            "",
            "anatomy",
            "hard",
            "focus study",
        )
        .await;
        insert_drawing_task(
            &pool,
            img_id,
            "Redraw in ink style",
            "",
            "style",
            "medium",
            "style study",
        )
        .await;
        insert_drawing_task(
            &pool,
            img_id,
            "Change the lighting",
            "",
            "lighting",
            "easy",
            "modification",
        )
        .await;
        let tasks = get_tasks_filtered(&pool, &no_filters()).await;
        assert_eq!(tasks.len(), 3);
        assert!(tasks.iter().all(|t| t.image_id == img_id));
    }

    #[tokio::test]
    async fn test_task_filters() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "A", "", "anatomy", "hard", "focus study").await;
        insert_drawing_task(&pool, img_id, "B", "", "style", "easy", "style study").await;

        let mut f = no_filters();
        f.subject = "anatomy".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "A");

        let mut f = no_filters();
        f.difficulty = "easy".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "B");

        let mut f = no_filters();
        f.task_type = "style study".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0].title, "B");
    }

    #[tokio::test]
    async fn test_task_sort_by_difficulty() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "hard one", "", "", "hard", "").await;
        insert_drawing_task(&pool, img_id, "easy one", "", "", "easy", "").await;
        insert_drawing_task(&pool, img_id, "medium one", "", "", "medium", "").await;

        let mut f = no_filters();
        f.sort = "easiest".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        let difficulties: Vec<&str> = tasks.iter().map(|t| t.difficulty.as_str()).collect();
        assert_eq!(difficulties, vec!["easy", "medium", "hard"]);

        f.sort = "hardest".to_string();
        let tasks = get_tasks_filtered(&pool, &f).await;
        let difficulties: Vec<&str> = tasks.iter().map(|t| t.difficulty.as_str()).collect();
        assert_eq!(difficulties, vec!["hard", "medium", "easy"]);
    }

    #[tokio::test]
    async fn test_toggle_task_completed() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "toggle me", "", "", "medium", "").await;
        let id = get_tasks_filtered(&pool, &no_filters()).await[0].id;

        toggle_task_completed(&pool, id).await;
        assert!(get_tasks_filtered(&pool, &no_filters()).await[0].completed);
        toggle_task_completed(&pool, id).await;
        assert!(!get_tasks_filtered(&pool, &no_filters()).await[0].completed);
    }

    #[tokio::test]
    async fn test_insert_task_with_missing_image_fails_cleanly() {
        let pool = test_pool().await;
        // No task_images row with id 999 — the FK (enforced: sqlx enables
        // PRAGMA foreign_keys by default) must reject this without panicking.
        assert!(!insert_drawing_task(&pool, 999, "orphan", "", "", "medium", "").await);
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_task_image_removes_tasks() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "orphan-to-be", "", "", "medium", "").await;

        let url = delete_task_image(&pool, img_id).await;
        assert_eq!(url, Some("https://example.com/model.jpg".to_string()));
        assert!(get_task_images(&pool).await.is_empty());
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_drawing_task() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "delete me", "", "", "medium", "").await;
        let id = get_tasks_filtered(&pool, &no_filters()).await[0].id;
        delete_drawing_task(&pool, id).await;
        assert!(get_tasks_filtered(&pool, &no_filters()).await.is_empty());
        // image stays
        assert_eq!(get_task_images(&pool).await.len(), 1);
    }

    #[tokio::test]
    async fn test_task_filter_options() {
        let pool = test_pool().await;
        let img_id = seed_image(&pool).await;
        insert_drawing_task(&pool, img_id, "A", "", "anatomy", "hard", "focus study").await;
        insert_drawing_task(&pool, img_id, "B", "", "style", "easy", "style study").await;
        insert_drawing_task(&pool, img_id, "C", "", "", "easy", "").await;

        assert_eq!(get_task_subjects(&pool).await, vec!["anatomy", "style"]);
        assert_eq!(
            get_task_types(&pool).await,
            vec!["focus study", "style study"]
        );
    }

    #[tokio::test]
    async fn test_targets_default_and_set() {
        let pool = test_pool().await;
        let t = get_targets(&pool, OWNER).await;
        assert_eq!(t.calories, 2400.0);
        assert_eq!(t.protein, 165.0);
        set_targets(&pool, 2200.0, 170.0, 240.0, 70.0, OWNER).await;
        let t = get_targets(&pool, OWNER).await;
        assert_eq!(t.calories, 2200.0);
        assert_eq!(t.fat, 70.0);
    }

    #[tokio::test]
    async fn test_favourite_and_category_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool,
            "Skyr",
            "Arla",
            None,
            63.0,
            11.0,
            4.0,
            0.2,
            0.0,
            4.0,
            45.0,
            0.1,
            Some(450.0),
            "",
            "",
            OWNER,
        )
        .await;
        assert_eq!(item.category, "");
        assert_eq!(item.is_favourite, 0);
        toggle_food_favourite(&pool, item.id, OWNER).await;
        assert_eq!(
            get_food_item(&pool, item.id, OWNER)
                .await
                .unwrap()
                .is_favourite,
            1
        );
        update_food_item(
            &pool,
            item.id,
            "Skyr",
            "Arla",
            None,
            63.0,
            11.0,
            4.0,
            0.2,
            0.0,
            4.0,
            45.0,
            0.1,
            Some(450.0),
            "",
            "",
            "Dairy & eggs",
            true,
            Some(170.0),
            OWNER,
        )
        .await;
        let item = get_food_item(&pool, item.id, OWNER).await.unwrap();
        assert_eq!(item.category, "Dairy & eggs");
        assert_eq!(item.is_favourite, 1);
        assert_eq!(item.default_portion_g, Some(170.0));
        toggle_food_favourite(&pool, item.id, OWNER).await;
        assert_eq!(
            get_food_item(&pool, item.id, OWNER)
                .await
                .unwrap()
                .is_favourite,
            0
        );
    }

    #[tokio::test]
    async fn test_item_log_history() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-30", 80.0, "breakfast", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-30", 40.0, "snack", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast", OWNER)
            .await
            .unwrap();
        let hist = get_item_log_history(&pool, item.id, "2026-07-18", "2026-07-31", OWNER).await;
        assert_eq!(hist.len(), 2);
        assert_eq!(hist[0], ("2026-07-30".to_string(), 120.0));
    }

    #[tokio::test]
    async fn test_recent_foods_dedup_and_order() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "", OWNER,
        )
        .await;
        let b = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 250.0, "breakfast", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 80.0, "breakfast", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, a.id, "2026-08-01", 300.0, "snack", OWNER)
            .await
            .unwrap();
        let recent = get_recent_foods(&pool, 8, OWNER).await;
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].name, "Skyr"); // most recently logged first
        assert_eq!(recent[0].last_grams, 300.0); // grams of the latest log
        assert_eq!(recent[0].last_slot, "snack");
    }

    #[tokio::test]
    async fn test_get_food_item_by_barcode() {
        let pool = test_pool().await;
        insert_food_item(
            &pool,
            "Bar",
            "Barebells",
            Some("5060123456789"),
            200.0,
            20.0,
            16.0,
            8.0,
            0.0,
            0.0,
            0.0,
            0.0,
            Some(55.0),
            "",
            "",
            OWNER,
        )
        .await;
        assert!(get_food_item_by_barcode(&pool, "5060123456789", OWNER)
            .await
            .is_some());
        assert!(get_food_item_by_barcode(&pool, "0000000000000", OWNER)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn test_weight_upsert_and_range() {
        let pool = test_pool().await;
        upsert_weight(&pool, "2026-07-30", 82.7, OWNER).await;
        upsert_weight(&pool, "2026-07-31", 82.4, OWNER).await;
        upsert_weight(&pool, "2026-07-31", 82.5, OWNER).await; // same-day overwrite
        let all = get_weights_since(&pool, "2026-07-01", OWNER).await;
        assert_eq!(all.len(), 2);
        assert_eq!(all[1], ("2026-07-31".to_string(), 82.5));
        assert_eq!(
            get_latest_weight(&pool, OWNER).await,
            Some(("2026-07-31".to_string(), 82.5))
        );
    }

    #[tokio::test]
    async fn test_recipe_create_and_log() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.0, 60.0, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        let b = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 0.0, 0.0, 0.0, None, "", "", OWNER,
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-31", 80.0, "breakfast", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, b.id, "2026-07-31", 250.0, "breakfast", OWNER)
            .await
            .unwrap();
        assert!(
            create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "dinner", OWNER)
                .await
                .is_none()
        ); // empty slot
        let rid =
            create_recipe_from_slot(&pool, "Overnight oats", "2026-07-31", "breakfast", OWNER)
                .await
                .unwrap();
        let recipes = get_recipes_with_totals(&pool, OWNER).await;
        assert_eq!(recipes.len(), 1);
        assert_eq!(recipes[0].item_count, 2);
        assert!((recipes[0].total_cal - (379.0 * 0.8 + 63.0 * 2.5)).abs() < 0.1);
        let inserted = log_recipe(&pool, rid, "2026-08-01", "snack", OWNER).await;
        assert_eq!(inserted.len(), 2);
        // The ids are what the toast's Undo removes, so they must be the real
        // entry ids and not, say, row ordinals.
        let entry_ids: Vec<i64> = get_meal_entries_for_date(&pool, "2026-08-01", OWNER)
            .await
            .iter()
            .map(|e| e.entry_id)
            .collect();
        assert!(
            inserted.iter().all(|id| entry_ids.contains(id)),
            "log_recipe must return the ids it actually inserted"
        );
        let entries = get_meal_entries_for_date(&pool, "2026-08-01", OWNER).await;
        assert!(entries.iter().all(|e| e.slot == "snack"));
        delete_recipe(&pool, rid, OWNER).await;
        assert!(get_recipes_with_totals(&pool, OWNER).await.is_empty());
    }

    #[tokio::test]
    async fn test_protein_range_and_logged_dates() {
        let pool = test_pool().await;
        let a = insert_food_item(
            &pool, "Chicken", "", None, 165.0, 31.0, 0.0, 3.6, 0.0, 0.0, 0.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, a.id, "2026-07-30", 200.0, "lunch", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, a.id, "2026-07-31", 100.0, "lunch", OWNER)
            .await
            .unwrap();
        let prot = get_protein_by_date_range(&pool, "2026-07-30", "2026-07-31", OWNER).await;
        assert_eq!(prot.len(), 2);
        assert!((prot[0].1 - 62.0).abs() < 0.01);
        assert_eq!(
            get_logged_dates_desc(&pool, 10, OWNER).await,
            vec!["2026-07-31", "2026-07-30"]
        );
        let most = get_most_logged_between(&pool, "2026-07-27", "2026-08-02", 5, OWNER).await;
        assert_eq!(most[0], ("Chicken".to_string(), 2));
    }

    #[tokio::test]
    async fn test_copy_day_entries() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Oats", "", None, 379.0, 13.2, 60.1, 6.5, 0.0, 0.0, 0.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-31", 80.0, "breakfast", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-31", 120.0, "lunch", OWNER)
            .await
            .unwrap();
        let copied = copy_day_entries(&pool, "2026-07-31", "2026-08-01", OWNER).await;
        assert_eq!(copied, 2);
        let entries = get_meal_entries_for_date(&pool, "2026-08-01", OWNER).await;
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[1].grams, 120.0);
    }

    #[tokio::test]
    async fn test_calories_by_date_range() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Rice", "", None, 100.0, 2.0, 20.0, 1.0, 0.0, 0.0, 0.0, 0.0, None, "", "", OWNER,
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-07-27", 100.0, "lunch", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-27", 50.0, "dinner", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-07-29", 200.0, "lunch", OWNER)
            .await
            .unwrap();
        insert_meal_entry(&pool, item.id, "2026-08-05", 100.0, "lunch", OWNER)
            .await
            .unwrap(); // outside range
        let rows = get_calories_by_date_range(&pool, "2026-07-26", "2026-08-01", OWNER).await;
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "2026-07-27");
        assert!((rows[0].1 - 150.0).abs() < 0.01);
        assert!((rows[1].1 - 200.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_meal_entry_slot_roundtrip() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Skyr", "", None, 63.0, 11.0, 4.0, 0.2, 0.0, 4.0, 45.0, 0.1, None, "", "", OWNER,
        )
        .await;
        let id = insert_meal_entry(&pool, item.id, "2026-08-01", 250.0, "breakfast", OWNER)
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-08-01", OWNER).await;
        assert_eq!(entries[0].slot, "breakfast");
        assert_eq!(entries[0].food_item_id, item.id);
        update_meal_entry(&pool, id, 300.0, "lunch", OWNER).await;
        let entries = get_meal_entries_for_date(&pool, "2026-08-01", OWNER).await;
        assert_eq!(entries[0].grams, 300.0);
        assert_eq!(entries[0].slot, "lunch");
        let raw = get_meal_entry(&pool, id, OWNER).await.unwrap();
        assert_eq!(raw.slot, "lunch");
    }

    #[tokio::test]
    async fn test_meal_entry_wrong_date_not_returned() {
        let pool = test_pool().await;
        let item = insert_food_item(
            &pool, "Banana", "", None, 89.0, 1.1, 23.0, 0.3, 2.6, 12.0, 1.0, 0.0, None, "", "",
            OWNER,
        )
        .await;
        insert_meal_entry(&pool, item.id, "2026-04-08", 100.0, "other", OWNER)
            .await
            .unwrap();
        let entries = get_meal_entries_for_date(&pool, "2026-04-09", OWNER).await;
        assert!(entries.is_empty());
    }

    // ===== Visibility model (migration 013) =====

    /// Inserts a post and sets its state.
    ///
    /// Two steps because `insert_post` does not take a visibility yet — the
    /// upload's field arrives with the PATCH route. Every post therefore starts
    /// `public` and is moved, which also exercises `set_post_visibility` on the
    /// way to every other assertion.
    async fn post_with(pool: &DbPool, caption: &str, visibility: Visibility) -> Post {
        let post = insert_post(
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
        set_post_visibility(pool, post.id, visibility).await;
        post
    }

    async fn one_of_each(pool: &DbPool) {
        post_with(pool, "shown", Visibility::Public).await;
        post_with(pool, "linked", Visibility::Unlisted).await;
        post_with(pool, "secret", Visibility::Hidden).await;
    }

    #[tokio::test]
    async fn test_get_posts_page_visitor_sees_only_public() {
        let pool = test_pool().await;
        one_of_each(&pool).await;
        let posts = get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Visitor).await;
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].caption, "shown");
    }

    #[tokio::test]
    async fn test_get_posts_page_admin_sees_all() {
        let pool = test_pool().await;
        one_of_each(&pool).await;
        assert_eq!(
            get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin)
                .await
                .len(),
            3
        );
    }

    #[tokio::test]
    async fn test_get_posts_page_visitor_filter_applies_with_search() {
        let pool = test_pool().await;
        post_with(&pool, "a cat study", Visibility::Public).await;
        post_with(&pool, "a cat secret", Visibility::Hidden).await;
        let hits = get_posts_page(&pool, &q_filter("cat"), 0, Viewer::Visitor).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].caption, "a cat study");
    }

    #[tokio::test]
    async fn test_count_posts_visitor_total_is_public_only() {
        let pool = test_pool().await;
        post_with(&pool, "shown", Visibility::Public).await;
        post_with(&pool, "linked", Visibility::Unlisted).await;
        post_with(&pool, "secret one", Visibility::Hidden).await;
        post_with(&pool, "secret two", Visibility::Hidden).await;
        let counts = count_posts(&pool, &PostFilter::default(), Viewer::Visitor).await;
        assert_eq!(counts.total, 1);
        assert_eq!(counts.public, 1);
    }

    #[tokio::test]
    async fn test_count_posts_admin_total_is_everything() {
        let pool = test_pool().await;
        post_with(&pool, "shown", Visibility::Public).await;
        post_with(&pool, "linked", Visibility::Unlisted).await;
        post_with(&pool, "secret one", Visibility::Hidden).await;
        post_with(&pool, "secret two", Visibility::Hidden).await;
        let counts = count_posts(&pool, &PostFilter::default(), Viewer::Admin).await;
        assert_eq!(counts.total, 4);
        assert_eq!(counts.public, 1);
        assert_eq!(counts.unlisted, 1);
        assert_eq!(counts.hidden, 2);
    }

    /// `GROUP BY` returns no row for a state with zero posts, so the struct has
    /// to accumulate into defaults rather than index the result. A portfolio
    /// with nothing hidden is the normal case, not an edge one.
    #[tokio::test]
    async fn test_count_posts_absent_states_are_zero() {
        let pool = test_pool().await;
        post_with(&pool, "one", Visibility::Public).await;
        post_with(&pool, "two", Visibility::Public).await;
        let counts = count_posts(&pool, &PostFilter::default(), Viewer::Admin).await;
        assert_eq!(counts.public, 2);
        assert_eq!(counts.unlisted, 0);
        assert_eq!(counts.hidden, 0);
        assert_eq!(counts.total, 2);
    }

    #[tokio::test]
    async fn test_get_post_by_id_hidden_is_none_for_visitor() {
        let pool = test_pool().await;
        let post = post_with(&pool, "secret", Visibility::Hidden).await;
        assert!(get_post_by_id(&pool, post.id, Viewer::Visitor)
            .await
            .is_none());
        assert!(get_post_by_id(&pool, post.id, Viewer::Admin)
            .await
            .is_some());
    }

    /// The whole point of the state: out of the feed, still served by its
    /// permalink.
    #[tokio::test]
    async fn test_get_post_by_id_unlisted_is_some_for_visitor() {
        let pool = test_pool().await;
        let post = post_with(&pool, "linked", Visibility::Unlisted).await;
        assert!(get_post_by_id(&pool, post.id, Viewer::Visitor)
            .await
            .is_some());
        assert!(get_post_by_id(&pool, post.id, Viewer::Admin)
            .await
            .is_some());
    }

    #[tokio::test]
    async fn test_get_post_by_id_unknown_id_is_none() {
        let pool = test_pool().await;
        assert!(get_post_by_id(&pool, 9999, Viewer::Admin).await.is_none());
    }

    #[tokio::test]
    async fn test_visibility_from_row_fails_closed() {
        assert_eq!(Visibility::from_row("bogus"), Visibility::Hidden);
        assert_eq!(Visibility::from_row(""), Visibility::Hidden);
        assert_eq!(Visibility::from_row("public"), Visibility::Public);
    }

    #[tokio::test]
    async fn test_visibility_from_str_rejects_unknown() {
        assert!(Visibility::from_str("bogus").is_none());
        assert_eq!(Visibility::from_str("unlisted"), Some(Visibility::Unlisted));
    }

    #[tokio::test]
    async fn test_set_post_visibility_round_trip() {
        let pool = test_pool().await;
        let post = post_with(&pool, "moves", Visibility::Public).await;
        assert!(set_post_visibility(&pool, post.id, Visibility::Hidden).await);
        let stored = get_post_by_id(&pool, post.id, Viewer::Admin).await.unwrap();
        assert_eq!(Visibility::from_row(&stored.visibility), Visibility::Hidden);
    }

    #[tokio::test]
    async fn test_set_post_visibility_unknown_id_is_false() {
        let pool = test_pool().await;
        assert!(!set_post_visibility(&pool, 9999, Visibility::Hidden).await);
    }

    /// Guards `admin.rs`'s dashboard call site. Passing `Viewer::Visitor` there
    /// still compiles and still renders — it just silently stops listing the
    /// posts an admin most needs to see.
    #[tokio::test]
    async fn test_admin_dashboard_query_sees_all_states() {
        let pool = test_pool().await;
        one_of_each(&pool).await;
        assert_eq!(
            get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin)
                .await
                .len(),
            3
        );
    }

    // ===== Collections & tags (migration 014) =====

    #[tokio::test]
    async fn test_create_collection_slugs_the_name() {
        let pool = test_pool().await;
        let collection = create_collection(&pool, "Figure Studies")
            .await
            .expect("valid name creates a collection");
        assert_eq!(collection.slug, "figure-studies");
        assert_eq!(collection.name, "Figure Studies");
    }

    #[tokio::test]
    async fn test_create_collection_duplicate_slug() {
        let pool = test_pool().await;
        create_collection(&pool, "Figure Studies").await.unwrap();
        let result = create_collection(&pool, "figure  studies!").await;
        match result {
            Err(CreateCollectionError::DuplicateSlug(name)) => {
                assert_eq!(name, "Figure Studies");
            }
            other => panic!("expected DuplicateSlug(\"Figure Studies\"), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_create_collection_junk_name() {
        let pool = test_pool().await;
        let result = create_collection(&pool, "!!!").await;
        assert!(matches!(result, Err(CreateCollectionError::InvalidName)));
    }

    #[tokio::test]
    async fn test_delete_collection_unlinks_but_keeps_posts() {
        let pool = test_pool().await;
        let post = post_with(&pool, "keeper", Visibility::Public).await;
        let collection = create_collection(&pool, "Figure Studies").await.unwrap();
        assert!(add_post_to_collection(&pool, post.id, collection.id).await);

        assert!(delete_collection(&pool, collection.id).await);
        assert!(get_post_by_id(&pool, post.id, Viewer::Admin)
            .await
            .is_some());
        assert!(get_post_collection_ids(&pool, post.id).await.is_empty());
    }

    #[tokio::test]
    async fn test_delete_collection_unknown_id() {
        let pool = test_pool().await;
        assert!(!delete_collection(&pool, 9999).await);
    }

    #[tokio::test]
    async fn test_set_post_tags_replaces() {
        let pool = test_pool().await;
        let post = post_with(&pool, "tagged", Visibility::Public).await;
        assert!(set_post_tags(&pool, post.id, &["ink".to_string(), "wash".to_string()]).await);
        assert!(set_post_tags(&pool, post.id, &["wash".to_string(), "pencil".to_string()]).await);
        assert_eq!(
            get_post_tags(&pool, post.id).await,
            vec!["pencil".to_string(), "wash".to_string()]
        );
    }

    #[tokio::test]
    async fn test_set_post_tags_empty_clears() {
        let pool = test_pool().await;
        let post = post_with(&pool, "tagged", Visibility::Public).await;
        assert!(set_post_tags(&pool, post.id, &["ink".to_string()]).await);
        assert!(set_post_tags(&pool, post.id, &[]).await);
        assert!(get_post_tags(&pool, post.id).await.is_empty());
    }

    #[tokio::test]
    async fn test_set_post_tags_unknown_post() {
        let pool = test_pool().await;
        assert!(!set_post_tags(&pool, 9999, &["ink".to_string()]).await);
        let tag_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tags")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(tag_count, 0, "an unknown post must not create a tag row");
    }

    #[tokio::test]
    async fn test_add_post_to_collection_idempotent() {
        let pool = test_pool().await;
        let post = post_with(&pool, "member", Visibility::Public).await;
        let collection = create_collection(&pool, "Figure Studies").await.unwrap();
        assert!(add_post_to_collection(&pool, post.id, collection.id).await);
        assert!(add_post_to_collection(&pool, post.id, collection.id).await);
        assert_eq!(get_post_collection_ids(&pool, post.id).await.len(), 1);
    }

    #[tokio::test]
    async fn test_add_post_to_unknown_collection() {
        let pool = test_pool().await;
        let post = post_with(&pool, "lonely", Visibility::Public).await;
        assert!(!add_post_to_collection(&pool, post.id, 999).await);
        assert!(get_post_collection_ids(&pool, post.id).await.is_empty());
    }

    #[tokio::test]
    async fn test_remove_post_from_collection() {
        let pool = test_pool().await;
        let post = post_with(&pool, "member", Visibility::Public).await;
        let collection = create_collection(&pool, "Figure Studies").await.unwrap();
        assert!(add_post_to_collection(&pool, post.id, collection.id).await);
        assert!(remove_post_from_collection(&pool, post.id, collection.id).await);
        assert!(get_post_collection_ids(&pool, post.id).await.is_empty());
    }

    #[tokio::test]
    async fn test_update_post_caption_round_trip() {
        let pool = test_pool().await;
        let post = post_with(&pool, "old caption", Visibility::Public).await;
        assert!(update_post_caption(&pool, post.id, "new caption").await);
        let fetched = get_post_by_id(&pool, post.id, Viewer::Admin).await.unwrap();
        assert_eq!(fetched.caption, "new caption");
        assert!(!update_post_caption(&pool, 9999, "whatever").await);
    }

    #[tokio::test]
    async fn test_list_collections_counts_are_viewer_aware() {
        let pool = test_pool().await;
        let public_post = post_with(&pool, "shown", Visibility::Public).await;
        let hidden_post = post_with(&pool, "secret", Visibility::Hidden).await;
        let collection = create_collection(&pool, "Figure Studies").await.unwrap();
        assert!(add_post_to_collection(&pool, public_post.id, collection.id).await);
        assert!(add_post_to_collection(&pool, hidden_post.id, collection.id).await);

        let visitor_list = list_collections_with_counts(&pool, Viewer::Visitor).await;
        assert_eq!(visitor_list.len(), 1);
        assert_eq!(visitor_list[0].count, 1);

        let admin_list = list_collections_with_counts(&pool, Viewer::Admin).await;
        assert_eq!(admin_list.len(), 1);
        assert_eq!(admin_list[0].count, 2);
    }

    #[tokio::test]
    async fn test_list_collections_empty_hidden_from_visitors() {
        let pool = test_pool().await;
        create_collection(&pool, "Empty").await.unwrap();

        let visitor_list = list_collections_with_counts(&pool, Viewer::Visitor).await;
        assert!(visitor_list.is_empty());

        let admin_list = list_collections_with_counts(&pool, Viewer::Admin).await;
        assert_eq!(admin_list.len(), 1);
        assert_eq!(admin_list[0].count, 0);
    }

    #[tokio::test]
    async fn test_list_tags_counts_are_viewer_aware() {
        let pool = test_pool().await;
        let public_post = post_with(&pool, "shown", Visibility::Public).await;
        let hidden_post = post_with(&pool, "secret", Visibility::Hidden).await;
        let hidden_only_post = post_with(&pool, "shadow", Visibility::Hidden).await;

        assert!(set_post_tags(&pool, public_post.id, &["ink".to_string()]).await);
        assert!(set_post_tags(&pool, hidden_post.id, &["ink".to_string()]).await);
        assert!(set_post_tags(&pool, hidden_only_post.id, &["charcoal".to_string()]).await);

        let visitor_tags = list_tags_with_counts(&pool, Viewer::Visitor).await;
        let ink = visitor_tags.iter().find(|t| t.name == "ink");
        assert_eq!(ink.map(|t| t.count), Some(1));
        assert!(
            visitor_tags.iter().all(|t| t.name != "charcoal"),
            "a tag on hidden posts only must not appear for a visitor"
        );

        let admin_tags = list_tags_with_counts(&pool, Viewer::Admin).await;
        let ink = admin_tags.iter().find(|t| t.name == "ink").unwrap();
        assert_eq!(ink.count, 2);
        let charcoal = admin_tags.iter().find(|t| t.name == "charcoal").unwrap();
        assert_eq!(charcoal.count, 1);
    }

    #[tokio::test]
    async fn test_delete_post_cleans_join_rows() {
        let pool = test_pool().await;
        let post = post_with(&pool, "doomed", Visibility::Public).await;
        let collection = create_collection(&pool, "Figure Studies").await.unwrap();
        assert!(set_post_tags(&pool, post.id, &["ink".to_string()]).await);
        assert!(add_post_to_collection(&pool, post.id, collection.id).await);

        assert!(delete_post_and_get_urls(&pool, post.id).await.is_some());

        assert!(get_post_tags(&pool, post.id).await.is_empty());
        assert!(get_post_collection_ids(&pool, post.id).await.is_empty());
    }

    // ===== PostFilter: the one-macro filter query (Task 3) ==================

    fn tag_filter(tags: &[&str]) -> PostFilter {
        PostFilter {
            tags: tags.iter().map(|s| s.to_string()).collect(),
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_filter_multi_tag_is_and() {
        let pool = test_pool().await;
        let post_a = post_with(&pool, "post a", Visibility::Public).await;
        let post_b = post_with(&pool, "post b", Visibility::Public).await;
        assert!(set_post_tags(&pool, post_a.id, &["ink".to_string()]).await);
        assert!(
            set_post_tags(
                &pool,
                post_b.id,
                &["ink".to_string(), "perspective".to_string()]
            )
            .await
        );

        let both = get_posts_page(
            &pool,
            &tag_filter(&["ink", "perspective"]),
            0,
            Viewer::Admin,
        )
        .await;
        assert_eq!(both.len(), 1, "AND semantics: only the post with both tags");
        assert_eq!(both[0].id, post_b.id);

        let ink_only = get_posts_page(&pool, &tag_filter(&["ink"]), 0, Viewer::Admin).await;
        assert_eq!(ink_only.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_unknown_collection_is_empty() {
        let pool = test_pool().await;
        post_with(&pool, "one", Visibility::Public).await;
        post_with(&pool, "two", Visibility::Public).await;

        let filter = PostFilter {
            collection: Some("no-such-slug".to_string()),
            ..Default::default()
        };
        let hits = get_posts_page(&pool, &filter, 0, Viewer::Admin).await;
        assert!(
            hits.is_empty(),
            "unknown slug must not error, just match nothing"
        );
    }

    #[tokio::test]
    async fn test_filter_collection_scopes() {
        let pool = test_pool().await;
        let post_a = post_with(&pool, "in the collection", Visibility::Public).await;
        let post_b = post_with(&pool, "not in the collection", Visibility::Public).await;
        let collection = create_collection(&pool, "Studies").await.unwrap();
        assert_eq!(collection.slug, "studies");
        assert!(add_post_to_collection(&pool, post_a.id, collection.id).await);

        let filter = PostFilter {
            collection: Some("studies".to_string()),
            ..Default::default()
        };
        let hits = get_posts_page(&pool, &filter, 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, post_a.id);
        assert!(hits.iter().all(|p| p.id != post_b.id));
    }

    #[tokio::test]
    async fn test_filter_like_escape_with_tags() {
        let pool = test_pool().await;
        let matching = post_with(&pool, "100% ink study", Visibility::Public).await;
        let other = post_with(&pool, "loose gesture", Visibility::Public).await;
        assert!(set_post_tags(&pool, matching.id, &["ink".to_string()]).await);
        assert!(set_post_tags(&pool, other.id, &["ink".to_string()]).await);

        let filter = PostFilter {
            q: Some("100%".to_string()),
            tags: vec!["ink".to_string()],
            ..Default::default()
        };
        let hits = get_posts_page(&pool, &filter, 0, Viewer::Admin).await;
        assert_eq!(
            hits.len(),
            1,
            "the LIKE escape must still hold once combined with the tag filter"
        );
        assert_eq!(hits[0].id, matching.id);
    }

    #[tokio::test]
    async fn test_filter_visitor_stays_public_with_tags() {
        let pool = test_pool().await;
        let public_post = post_with(&pool, "public tagged", Visibility::Public).await;
        let hidden_post = post_with(&pool, "hidden tagged", Visibility::Hidden).await;
        assert!(set_post_tags(&pool, public_post.id, &["ink".to_string()]).await);
        assert!(set_post_tags(&pool, hidden_post.id, &["ink".to_string()]).await);

        let filter = tag_filter(&["ink"]);
        let visitor_hits = get_posts_page(&pool, &filter, 0, Viewer::Visitor).await;
        assert_eq!(visitor_hits.len(), 1);
        assert_eq!(visitor_hits[0].id, public_post.id);

        let admin_hits = get_posts_page(&pool, &filter, 0, Viewer::Admin).await;
        assert_eq!(admin_hits.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_vis_subset() {
        let pool = test_pool().await;
        one_of_each(&pool).await; // public "shown", unlisted "linked", hidden "secret"

        let hidden_only = PostFilter {
            vis: Some(vec!["hidden".to_string()]),
            ..Default::default()
        };
        let hits = get_posts_page(&pool, &hidden_only, 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].caption, "secret");

        // Safety net for the self-review item: even if a `vis` subset reached
        // this function for a non-admin viewer — which Task 4's parser is
        // supposed to prevent — clause 1's `(?1 OR visibility = 'public')`
        // still wins, because `?1` is false for a visitor. No hidden row leaks.
        let visitor_attempt = get_posts_page(&pool, &hidden_only, 0, Viewer::Visitor).await;
        assert!(
            visitor_attempt.is_empty(),
            "a visitor must never see a non-public row, whatever vis is passed"
        );

        let public_and_unlisted = PostFilter {
            vis: Some(vec!["public".to_string(), "unlisted".to_string()]),
            ..Default::default()
        };
        let hits = get_posts_page(&pool, &public_and_unlisted, 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 2);
    }

    #[tokio::test]
    async fn test_filter_default_matches_slice2_behaviour() {
        let pool = test_pool().await;
        post_with(&pool, "shown", Visibility::Public).await;
        post_with(&pool, "secret", Visibility::Hidden).await;

        let filter = PostFilter::default();
        assert_eq!(
            get_posts_page(&pool, &filter, 0, Viewer::Visitor)
                .await
                .len(),
            1
        );
        assert_eq!(
            get_posts_page(&pool, &filter, 0, Viewer::Admin).await.len(),
            2
        );
    }

    #[tokio::test]
    async fn test_filter_keeps_n_plus_1_probe() {
        let pool = test_pool().await;
        for i in 0..22 {
            seed_caption(&pool, &format!("post {i}")).await;
        }
        let hits = get_posts_page(&pool, &PostFilter::default(), 0, Viewer::Admin).await;
        assert_eq!(hits.len(), 21);
    }

    /// Seeds 2 public posts tagged `ink`, 1 public post untagged, and 1 hidden
    /// post tagged `ink` — the shared scenario the next two tests both read.
    async fn seed_tag_and_vis_scenario(pool: &DbPool) {
        let public_a = post_with(pool, "public a", Visibility::Public).await;
        let public_b = post_with(pool, "public b", Visibility::Public).await;
        post_with(pool, "public untagged", Visibility::Public).await;
        let hidden = post_with(pool, "hidden tagged", Visibility::Hidden).await;
        assert!(set_post_tags(pool, public_a.id, &["ink".to_string()]).await);
        assert!(set_post_tags(pool, public_b.id, &["ink".to_string()]).await);
        assert!(set_post_tags(pool, hidden.id, &["ink".to_string()]).await);
    }

    #[tokio::test]
    async fn test_count_posts_reflects_tag_filter() {
        let pool = test_pool().await;
        seed_tag_and_vis_scenario(&pool).await;

        let filter = tag_filter(&["ink"]);
        let visitor_counts = count_posts(&pool, &filter, Viewer::Visitor).await;
        assert_eq!(visitor_counts.total, 2);

        let admin_counts = count_posts(&pool, &filter, Viewer::Admin).await;
        assert_eq!(admin_counts.total, 3);
        assert_eq!(admin_counts.hidden, 1);
    }

    #[tokio::test]
    async fn test_count_posts_reflects_vis_subset() {
        let pool = test_pool().await;
        seed_tag_and_vis_scenario(&pool).await;

        let filter = PostFilter {
            vis: Some(vec!["hidden".to_string()]),
            ..Default::default()
        };
        let counts = count_posts(&pool, &filter, Viewer::Admin).await;
        assert_eq!(counts.total, 1);
        assert_eq!(counts.public, 0);
    }
}
