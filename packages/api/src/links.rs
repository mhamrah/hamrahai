use axum::{
    Json,
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::auth::require_claims;
use crate::db::DbPool;

#[derive(Serialize, sqlx::FromRow)]
pub struct Link {
    pub id: Uuid,
    pub user_id: Uuid,
    pub url: String,
    pub title: Option<String>,
    pub state: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Deserialize)]
pub struct CreateLinkRequest {
    pub url: String,
    pub title: Option<String>,
    pub client_id: Option<String>,
    pub shared_text: Option<String>,
    pub source_app: Option<String>,
    pub shared_at: Option<String>,
}

#[derive(Deserialize)]
pub struct ListLinksQuery {
    pub since: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Serialize)]
pub struct LinkDeltaResponse {
    pub links: Vec<ServerLink>,
    pub next_cursor: Option<String>,
}

#[derive(Serialize)]
pub struct ServerLink {
    pub id: String,
    pub original_url: String,
    pub canonical_url: String,
    pub title: Option<String>,
    pub snippet: Option<String>,
    pub summary_short: Option<String>,
    pub summary_long: Option<String>,
    pub lang: Option<String>,
    pub tags: Vec<String>,
    pub save_count: i32,
    pub status: String,
    pub shared_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct CreateLinkResponse {
    pub id: String,
    pub canonical_url: String,
}

pub async fn list_links(State(pool): State<DbPool>, headers: HeaderMap) -> impl IntoResponse {
    list_links_with_query(
        State(pool),
        headers,
        Query(ListLinksQuery {
            since: None,
            limit: None,
        }),
    )
    .await
}

pub async fn list_links_with_query(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Query(query): Query<ListLinksQuery>,
) -> impl IntoResponse {
    let claims = match require_claims(&headers) {
        Ok(c) => c,
        Err(_) => return axum::http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 100);
    let since = query
        .since
        .as_deref()
        .and_then(|raw| DateTime::parse_from_rfc3339(raw).ok())
        .map(|date| date.with_timezone(&Utc));

    let rows = sqlx::query_as::<_, Link>(
        r#"
        SELECT id, user_id, url, title, state, created_at
        FROM links
        WHERE user_id = $1 AND ($2::timestamptz IS NULL OR created_at > $2)
        ORDER BY created_at ASC
        LIMIT $3
        "#,
    )
    .bind(claims.sub)
    .bind(since)
    .bind(limit)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    let next_cursor = rows.last().map(|link| link.created_at.to_rfc3339());
    let links = rows.into_iter().map(ServerLink::from).collect();

    Json(LinkDeltaResponse { links, next_cursor }).into_response()
}

pub async fn create_link(
    State(pool): State<DbPool>,
    headers: HeaderMap,
    Json(req): Json<CreateLinkRequest>,
) -> impl IntoResponse {
    let claims = match require_claims(&headers) {
        Ok(c) => c,
        Err(_) => return axum::http::StatusCode::UNAUTHORIZED.into_response(),
    };
    let id = Uuid::new_v4();
    let link = sqlx::query_as::<_, Link>(
        r#"INSERT INTO links (id, user_id, url, title, state)
           VALUES ($1, $2, $3, $4, 'new')
           RETURNING id, user_id, url, title, state, created_at"#,
    )
    .bind(id)
    .bind(claims.sub)
    .bind(&req.url)
    .bind(&req.title)
    .fetch_one(&pool)
    .await;
    match link {
        Ok(link) => Json(CreateLinkResponse {
            id: link.id.to_string(),
            canonical_url: link.url,
        })
        .into_response(),
        Err(error) => {
            tracing::error!(error = %error, "Failed to create link");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({"success": false, "error": "Failed to create link"})),
            )
                .into_response()
        }
    }
}

impl From<Link> for ServerLink {
    fn from(link: Link) -> Self {
        ServerLink {
            id: link.id.to_string(),
            original_url: link.url.clone(),
            canonical_url: link.url,
            title: link.title,
            snippet: None,
            summary_short: None,
            summary_long: None,
            lang: None,
            tags: Vec::new(),
            save_count: 1,
            status: "synced".to_string(),
            shared_at: link.created_at,
            created_at: link.created_at,
        }
    }
}
