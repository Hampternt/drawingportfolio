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
/// Built once at the handler edge from `OptionalAuth` and the `?visitor=1`
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

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Session {
    pub id: String,
    pub expires_at: String,
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

#[derive(Debug, Clone)]
pub struct MealEntryWithFood {
    pub entry_id: i64,
    pub food_item_id: i64,
    pub food_name: String,
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
