use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
    pub webp_url: String,
    pub avif_url: String,
    pub format: String,
    pub file_size_bytes: i64,
    pub created_at: String,
    /// Intrinsic pixel dimensions, from migration 012. `0` means unknown —
    /// either a pre-012 row or a header the `image` crate could not parse.
    /// `post_card.html` emits width/height only when both are non-zero, since
    /// `width="0"` would collapse the image.
    pub image_width: i64,
    pub image_height: i64,
    /// The raw `visibility` column text, from migration 013 — parse it with
    /// [`Visibility::from_row`] wherever behaviour depends on it.
    ///
    /// A `String` rather than the enum because six queries select the posts
    /// columns and each would otherwise need a sqlx type override. It is also
    /// `Serialize`, so it appears in the JSON API — harmless, since that route
    /// serves a visitor nothing but `public` rows.
    pub visibility: String,
}

/// One month's worth of the feed, computed in the handler.
///
/// Grouping lives in Rust rather than the template because a month is also a
/// layout unit: each group renders its own `columns` block, which is what makes
/// a full-bleed divider composable with a CSS multi-column masonry.
#[derive(Debug, Clone)]
pub struct MonthGroup {
    /// `YYYY-MM` — the first seven characters of the ISO8601 `created_at`.
    pub label: String,
    /// Posts in **this page's** slice of the month, not the month's total. A
    /// month straddling the page boundary leaves its divider reading `5` above
    /// 8 cards once Load more appends the rest, because the divider is not
    /// re-rendered. That is inherent to append-only pagination; a per-month
    /// COUNT is not in this slice's scope.
    pub count: usize,
    /// False when the previous page already rendered this month's divider. The
    /// label survives suppression, because the next page's `last_month`
    /// parameter is built from it.
    pub show_divider: bool,
    pub posts: Vec<Post>,
}

/// Extensibility hook: add new variants here as post formats are implemented.
#[derive(Debug, Clone, PartialEq)]
pub enum PostFormat {
    Single,
}

impl PostFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Single => "single",
        }
    }
}

impl Default for PostFormat {
    fn default() -> Self {
        Self::Single
    }
}

/// A post's visibility state.
///
/// `public` is listed everywhere; `unlisted` is excluded from the feed and the
/// JSON API but still served by its permalink; `hidden` is served to nobody but
/// an admin.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Unlisted,
    Hidden,
}

impl Visibility {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Unlisted => "unlisted",
            Self::Hidden => "hidden",
        }
    }

    /// Strict parse, for input arriving from outside — a PATCH body or a
    /// multipart field. `None` is a 400 at the call site, never a default:
    /// silently coercing a typo to some state would look like the request
    /// succeeded.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "public" => Some(Self::Public),
            "unlisted" => Some(Self::Unlisted),
            "hidden" => Some(Self::Hidden),
            _ => None,
        }
    }

    /// Lenient parse, for a value read back out of the database — and it fails
    /// **closed**. A corrupt row, or one written by a future version that knows
    /// a state this build does not, must not render to the public.
    pub fn from_row(s: &str) -> Self {
        Self::from_str(s).unwrap_or(Self::Hidden)
    }
}

impl Default for Visibility {
    fn default() -> Self {
        Self::Public
    }
}

/// Who is asking.
///
/// Built once at the handler edge from `OptionalAdmin` and the `?visitor=1`
/// preview flag, then used for **both** the db call and the template flags.
/// Deriving those two separately is how a preview ends up rendering a visitor's
/// post set with admin badges and controls over it — getting wrong precisely the
/// thing the preview exists to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Viewer {
    Visitor,
    Admin,
}

impl Viewer {
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin)
    }
}

/// The page head's numbers.
///
/// `total` is viewer-dependent: an admin's total is every post, a visitor's is
/// the public count. The other three are rendered for an admin only.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PostCounts {
    pub total: i64,
    pub public: i64,
    pub unlisted: i64,
    pub hidden: i64,
}

/// The feed's one filter, threaded through `get_posts_page` and `count_posts`.
///
/// `tags` arrive already normalized — `normalize_tags` is applied once, at the
/// form/query edge, same discipline as `set_post_tags`. `vis` is `None` for
/// every visitor call: only an admin may request a visibility subset, and
/// Task 4's query-string parser is what enforces that — this struct itself
/// places no restriction on the field. `q` is the raw, unescaped search term;
/// `like_pattern` is applied inside `db.rs`, never by the caller.
#[derive(Debug, Clone, Default)]
pub struct PostFilter {
    pub q: Option<String>,
    pub tags: Vec<String>,          // normalized; empty = no tag filter
    pub collection: Option<String>, // slug
    pub vis: Option<Vec<String>>,   // None = viewer default; Some = admin subset
}

/// A row of the `collections` table.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub created_at: String,
}

/// A collection joined with its post count.
///
/// `count` is viewer-aware: it is computed by Task 2's queries, scoped to
/// whatever posts the current viewer (admin vs. visitor) is allowed to see.
#[derive(Debug, Clone)]
pub struct CollectionWithCount {
    pub id: i64,
    pub name: String,
    pub slug: String,
    pub count: i64,
}

/// A tag name joined with its post count.
///
/// `count` is viewer-aware: it is computed by Task 2's queries, scoped to
/// whatever posts the current viewer (admin vs. visitor) is allowed to see.
#[derive(Debug, Clone)]
pub struct TagWithCount {
    pub name: String,
    pub count: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateCollectionError {
    InvalidName,
    DuplicateSlug(String), // payload: existing name
}

/// A live session joined to the user it belongs to.
///
/// The user's flags ride along because `get_session` joins `users` — the auth
/// extractors run on every request, including each HTMX fragment swap on the
/// fitness page, and a second round-trip per swap to answer "is this person an
/// admin?" would be paid on every interaction.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub expires_at: String,
    pub user_id: i64,
    pub user_name: String,
    pub is_owner: bool,
    pub is_admin: bool,
}

impl Session {
    /// The only admin question anything should ask.
    ///
    /// The owner is an admin without carrying the grant flag, so nothing reads
    /// `is_admin` directly — reading the raw column is how the owner ends up
    /// locked out of their own portfolio the first time someone revokes a grant.
    pub fn is_effective_admin(&self) -> bool {
        self.is_owner || self.is_admin
    }
}

/// A user as the management page lists them.
///
/// `has_pin` and `is_locked` are derived in SQL rather than exposing the hash
/// or the timestamp: the page needs to say "PIN set" and "locked", and nothing
/// that renders a template should be holding a credential.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserRow {
    pub id: i64,
    pub name: String,
    pub is_owner: bool,
    pub is_admin: bool,
    pub has_pin: bool,
    pub is_locked: bool,
    pub created_at: String,
}

/// What the PIN login path needs, and nothing more.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct UserAuth {
    pub id: i64,
    /// Empty when the user has no PIN — `verify_pin` rejects that, so a
    /// passkey-only account cannot be reached by supplying an empty PIN.
    pub pin_hash: String,
    pub failed_pin_attempts: i64,
    pub is_locked: bool,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct PasskeyCredential {
    pub id: String,
    pub passkey_json: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AuthChallengeState {
    pub id: String,
    pub state_json: String,
}

/// Whose nutrition data a query touches.
///
/// A newtype rather than a bare `i64`, for the same reason [`Viewer`] exists on
/// the post side: it makes the wrong call a *compile* error rather than a
/// silent read of someone else's food log. Two properties earn its keep —
///
/// 1. **It cannot be omitted.** Every nutrition `db.rs` function takes one, so
///    a query written without an owner does not compile.
/// 2. **It cannot be transposed.** These functions take several integers —
///    `insert_meal_entry` alone has a food id and a user id, `get_recent_foods`
///    has a user id and a limit. As bare `i64`s the compiler would happily
///    accept them in the wrong order and the bug would surface as one user's
///    entries appearing in another's day.
///
/// Accepting one is not the same as *using* it: for anything addressed by row
/// id, the id must appear in the `WHERE` clause beside the row's own id, or a
/// guessed sequential id still reaches another user's row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserId(pub i64);

impl UserId {
    /// The raw id, for binding into a query.
    pub fn get(self) -> i64 {
        self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct FoodItem {
    pub id: i64,
    pub name: String,
    pub brand: String,
    pub barcode: Option<String>,
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
    pub fiber: f64,
    pub sugar: f64,
    pub sodium: f64,
    pub saturated_fat: f64,
    pub package_size: Option<f64>,
    pub custom_portions: String,
    pub image_url: String,
    pub category: String,
    pub is_favourite: i64,
    pub default_portion_g: Option<f64>,
    pub created_at: String,
}

/// A reference image that drawing tasks are attached to.
/// One image can have many tasks (different focus, style, modification, …).
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TaskImage {
    pub id: i64,
    pub title: String,
    pub image_url: String,
    pub created_at: String,
}

/// A drawing task joined with its reference image — what the /tasks page lists.
/// `subject`, `difficulty` and `task_type` are the three filterable axes.
#[derive(Debug, Clone)]
pub struct DrawingTaskWithImage {
    pub id: i64,
    pub image_id: i64,
    pub title: String,
    pub prompt: String,
    pub subject: String,
    pub difficulty: String,
    pub task_type: String,
    pub completed: bool,
    pub created_at: String,
    pub image_title: String,
    pub image_url: String,
}

#[derive(Debug, Clone)]
pub struct RecipeWithTotals {
    pub id: i64,
    pub name: String,
    pub item_count: i64,
    pub total_cal: f64,
}

#[derive(Debug, Clone)]
pub struct RecentFood {
    pub food_item_id: i64,
    pub name: String,
    pub last_grams: f64,
    pub last_slot: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MealEntry {
    pub id: i64,
    pub food_item_id: i64,
    pub date: String,
    pub grams: f64,
    pub slot: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Targets {
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
}

/// A food this user habitually eats at one slot — the "usual at breakfast"
/// card. Macros ride along per 100 g so the thumbnail can wear the same
/// dominance ring the logged row uses.
#[derive(Debug, Clone)]
pub struct UsualFood {
    pub food_item_id: i64,
    pub name: String,
    pub image_url: String,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
    pub last_grams: f64,
}

/// How many grams the food's basis is: the pack it comes in, else the amount
/// this user usually takes, else 100 g.
///
/// Lives here rather than beside either caller because both the row's fraction
/// buttons and the day query need the same answer, and a basis that disagreed
/// between them would label a button "½ pack" while logging half of something
/// else. A zero or negative package size is missing data, not a 0 g pack.
pub fn basis_grams(package_size: Option<f64>, usual: Option<f64>) -> f64 {
    package_size
        .filter(|g| *g > 0.0)
        .or(usual.filter(|g| *g > 0.0))
        .unwrap_or(100.0)
}

#[derive(Debug, Clone)]
pub struct MealEntryWithFood {
    pub entry_id: i64,
    pub food_item_id: i64,
    pub food_name: String,
    /// Rendered after the name as `Skyr natural · Arla`; empty for own-brand.
    pub brand: String,
    /// The row's thumbnail when the food has one. Empty falls back to the
    /// letter tile — a real state, not a placeholder, since most catalog rows
    /// are typed in rather than scanned.
    pub image_url: String,
    /// Grams the food is packaged or served in, and what to call that unit.
    /// The row's fraction buttons are computed from these.
    pub base_grams: f64,
    pub base_name: String,
    pub slot: String,
    pub grams: f64,
    pub calories: f64,
    pub protein: f64,
    pub carbs: f64,
    pub fat: f64,
    pub fiber: f64,
    pub sugar: f64,
    pub sodium: f64,
    pub saturated_fat: f64,
}
