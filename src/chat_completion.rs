use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::server_state::ServerState;

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    pub messages: Option<Vec<Value>>,
    pub model: Option<String>,
    #[serde(flatten)]
    pub _other: Value,
}

#[derive(Serialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Usage,
}

#[derive(Serialize)]
pub struct Choice {
    pub index: u32,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Serialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

pub async fn chat_completions(
    state: State<ServerState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> Result<(HeaderMap, Json<Value>), (StatusCode, HeaderMap, Json<Value>)> {
    state.increment_request_count();

    if let Some(error_code) = state.should_return_error() {
        let headers = state.get_rate_limit_headers();
        return Err((
            StatusCode::from_u16(error_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            headers,
            Json(json!({
                "error": {
                    "message": format!("Simulated error with code {}", error_code),
                    "type": "api_error",
                    "code": error_code.to_string()
                }
            })),
        ));
    }

    let response_length = state.get_response_length();

    if response_length == 0 {
        let headers = state.get_rate_limit_headers();
        return Err((StatusCode::NO_CONTENT, headers, Json(json!({}))));
    }

    let content = state.generate_lorem_content(response_length);

    let prompt_text = payload
        .messages
        .as_ref()
        .map(|msgs| serde_json::to_string(msgs).unwrap_or_default())
        .unwrap_or_default();

    let prompt_tokens = state.count_tokens(&prompt_text);
    let completion_tokens = state.count_tokens(&content);
    let total_tokens = prompt_tokens + completion_tokens;

    state.add_token_usage(total_tokens);

    let response = ChatCompletionResponse {
        id: format!("chatcmpl-{}", rand::thread_rng().gen::<u32>()),
        object: "chat.completion".to_string(),
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        model: payload.model.unwrap_or_else(|| "gpt-3.5-turbo".to_string()),
        choices: vec![Choice {
            index: 0,
            message: Message {
                role: "assistant".to_string(),
                content,
            },
            finish_reason: "stop".to_string(),
        }],
        usage: Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        },
    };

    let headers = state.get_rate_limit_headers();
    Ok((headers, Json(json!(response))))
}