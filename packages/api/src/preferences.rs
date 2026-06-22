use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{auth::require_claims, db::DbPool, models};

#[derive(sqlx::FromRow)]
struct UserPrefsRow {
    default_model: String,
    preferred_models: Vec<String>,
    updated_at: chrono::DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct UserPrefsResponse {
    pub default_model: String,
    pub preferred_models: Vec<String>,
    pub last_updated_at: chrono::DateTime<Utc>,
}

#[derive(Deserialize)]
pub struct UpdateUserPrefsRequest {
    pub default_model: String,
    pub preferred_models: Vec<String>,
}

#[derive(Serialize)]
struct ErrorResponse {
    success: bool,
    error: String,
}

pub async fn get_user_prefs(State(pool): State<DbPool>, headers: HeaderMap) -> Response {
    let claims = match require_claims(&headers) {
        Ok(claims) => claims,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    match fetch_or_create_user_prefs(&pool, claims.sub).await {
        Ok(prefs) => Json(prefs).into_response(),
        Err(error) => {
            tracing::error!(error = %error, "Failed to fetch user preferences");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

pub async fn update_user_prefs(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(request): Json<UpdateUserPrefsRequest>,
) -> Response {
    let claims = match require_claims(&headers) {
        Ok(claims) => claims,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    if !models::is_available_model(&request.default_model) {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                success: false,
                error: "default_model is not available".to_string(),
            }),
        )
            .into_response();
    }

    let preferred_models: Vec<String> = request
        .preferred_models
        .into_iter()
        .filter(|model| models::is_available_model(model))
        .collect();

    let updated = sqlx::query_as::<_, UserPrefsRow>(
        r#"
        INSERT INTO user_preferences (user_id, default_model, preferred_models, updated_at)
        VALUES ($1, $2, $3, NOW())
        ON CONFLICT (user_id) DO UPDATE SET
            default_model = EXCLUDED.default_model,
            preferred_models = EXCLUDED.preferred_models,
            updated_at = NOW()
        RETURNING default_model, preferred_models, updated_at
        "#,
    )
    .bind(claims.sub)
    .bind(&request.default_model)
    .bind(&preferred_models)
    .fetch_one(&pool)
    .await;

    match updated {
        Ok(row) => Json(row.into_response()).into_response(),
        Err(error) => {
            tracing::error!(error = %error, "Failed to update user preferences");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn fetch_or_create_user_prefs(
    pool: &DbPool,
    user_id: Uuid,
) -> anyhow::Result<UserPrefsResponse> {
    let preferred_models = Vec::<String>::new();
    let row = sqlx::query_as::<_, UserPrefsRow>(
        r#"
        INSERT INTO user_preferences (user_id, default_model, preferred_models)
        VALUES ($1, $2, $3)
        ON CONFLICT (user_id) DO UPDATE SET user_id = user_preferences.user_id
        RETURNING default_model, preferred_models, updated_at
        "#,
    )
    .bind(user_id)
    .bind(models::default_model_id())
    .bind(preferred_models)
    .fetch_one(pool)
    .await?;

    Ok(row.into_response())
}

impl UserPrefsRow {
    fn into_response(self) -> UserPrefsResponse {
        UserPrefsResponse {
            default_model: self.default_model,
            preferred_models: self.preferred_models,
            last_updated_at: self.updated_at,
        }
    }
}
