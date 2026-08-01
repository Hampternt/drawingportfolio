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
