use axum::Json;
use serde::Serialize;

#[derive(Clone, Serialize)]
pub struct AIModel {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub status: ModelStatus,
    pub replacement_id: Option<String>,
}

#[derive(Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    Available,
    Deprecated,
}

#[derive(Serialize)]
pub struct ModelsResponse {
    pub models: Vec<AIModel>,
}

pub const DEFAULT_MODEL_ID: &str = "@cf/zai-org/glm-4.7-flash";

pub fn catalog() -> Vec<AIModel> {
    vec![
        available(
            "@cf/zai-org/glm-4.7-flash",
            "GLM 4.7 Flash",
            "Cloudflare Workers AI",
        ),
        available(
            "@cf/google/gemma-4-26b-a4b-it",
            "Gemma 4 26B",
            "Cloudflare Workers AI",
        ),
        available(
            "@cf/moonshotai/kimi-k2.6",
            "Kimi K2.6",
            "Cloudflare Workers AI",
        ),
        available(
            "@cf/meta/llama-3.3-70b-instruct-fp8-fast",
            "Llama 3.3 70B Instruct",
            "Cloudflare Workers AI",
        ),
        deprecated(
            "gpt-4o-mini",
            "GPT-4o mini",
            "Legacy OpenAI",
            Some(DEFAULT_MODEL_ID),
        ),
        deprecated(
            "claude-3.5-sonnet",
            "Claude 3.5 Sonnet",
            "Legacy Anthropic",
            Some(DEFAULT_MODEL_ID),
        ),
        deprecated(
            "mistral-nemo",
            "Mistral Nemo",
            "Legacy",
            Some(DEFAULT_MODEL_ID),
        ),
    ]
}

pub fn default_model_id() -> &'static str {
    DEFAULT_MODEL_ID
}

pub fn is_known_model(id: &str) -> bool {
    catalog().iter().any(|model| model.id == id)
}

pub fn is_available_model(id: &str) -> bool {
    catalog()
        .iter()
        .any(|model| model.id == id && model.status == ModelStatus::Available)
}

pub async fn list_models() -> Json<ModelsResponse> {
    Json(ModelsResponse { models: catalog() })
}

fn available(id: &str, display_name: &str, provider: &str) -> AIModel {
    AIModel {
        id: id.to_string(),
        display_name: display_name.to_string(),
        provider: provider.to_string(),
        status: ModelStatus::Available,
        replacement_id: None,
    }
}

fn deprecated(
    id: &str,
    display_name: &str,
    provider: &str,
    replacement_id: Option<&str>,
) -> AIModel {
    AIModel {
        id: id.to_string(),
        display_name: display_name.to_string(),
        provider: provider.to_string(),
        status: ModelStatus::Deprecated,
        replacement_id: replacement_id.map(str::to_string),
    }
}
