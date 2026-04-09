use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Post {
    pub id: i64,
    pub caption: String,
    pub image_url: String,
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
    pub image_url: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MealEntry {
    pub id: i64,
    pub food_item_id: i64,
    pub date: String,
    pub grams: f64,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct MealEntryWithFood {
    pub entry_id: i64,
    pub food_name: String,
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
