// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use crate::models::Usage;
use crate::server_state::ServerState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::Json,
};
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Deserialize)]
pub struct ResponsesRequest {
    pub model: Option<String>,
    pub input: Option<String>,
    pub instructions: Option<String>,
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub _other: Value,
}

#[derive(Serialize)]
pub struct ResponsesOutputContent {
    #[serde(rename = "type")]
    pub _type: String,
    pub text: String,
}

#[derive(Serialize)]
pub struct ResponsesOutput {
    #[serde(rename = "type")]
    pub _type: String,
    pub id: String,
    content: Vec<ResponsesOutputContent>,
}

#[derive(Serialize)]
pub struct ResponsesResponse {
    pub id: String,
    pub object: String,
    pub created_at: u64,
    pub model: String,
    pub usage: Usage,
    pub output: Vec<ResponsesOutput>,
}

#[derive(Serialize)]
pub struct ResponsesStreamResponse {
    pub id: String,
}

pub async fn responses(
    state: State<ServerState>,
    Json(payload): Json<ResponsesRequest>,
) -> Result<(HeaderMap, Json<Value>), (StatusCode, HeaderMap, Json<Value>)> {
    if state.check_request_limit_exceeded() {
        let headers = state.get_rate_limit_headers();
        let error_body = json!({
            "error": {
                "message": "Too many requests",
                "type": "rate_limit_error",
                "code": "rate_limit_exceeded"
            }
        });
        return Err((StatusCode::TOO_MANY_REQUESTS, headers, Json(error_body)));
    }
    state.increment_request_count();

    if let Some(error_code) = state.should_return_error() {
        let headers = state.get_rate_limit_headers();
        let status_code =
            StatusCode::from_u16(error_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

        let error_body = json!({
            "error": {
                "message": format!("Simulated error with code {}", error_code),
                "type": "api_error",
                "code": error_code.to_string()
            }
        });

        return Err((status_code, headers, Json(error_body)));
    }

    let response_length = state.get_response_length();

    if response_length == 0 {
        let headers = state.get_rate_limit_headers();
        return Err((StatusCode::NO_CONTENT, headers, Json(json!({}))));
    }

    let content = state.generate_lorem_content(response_length);

    let prompt_text = payload.input.unwrap_or_else(|| "".to_string());
    let prompt_tokens = state.count_tokens(&prompt_text).unwrap_or(0);
    let completion_tokens = state.count_tokens(&content).unwrap_or(0);
    let total_tokens = prompt_tokens + completion_tokens;

    if state.check_token_limit_exceeded(total_tokens) {
        let headers = state.get_rate_limit_headers();
        let error_body = json!({
            "error": {
                "message": "You have exceeded your token quota.",
                "type": "rate_limit_error",
                "code": "rate_limit_exceeded"
            }
        });
        return Err((StatusCode::TOO_MANY_REQUESTS, headers, Json(error_body)));
    }
    state.add_token_usage(total_tokens);

    let headers = state.get_rate_limit_headers();

    let stream_response = payload.stream.unwrap_or(false);
    if stream_response {
        // FIXME
        let response = ResponsesStreamResponse {
            id: format!("resp_{}", rand::thread_rng().gen::<u32>()),
        };
        return Ok((headers, Json(json!(response))));
    } else {
        let response = ResponsesResponse {
            id: format!("resp_{}", rand::thread_rng().gen::<u32>()),
            object: "response".to_string(),
            created_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("should be able to get duration")
                .as_secs(),
            model: payload.model.unwrap_or_else(|| "gpt-5".to_string()),
            usage: Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
            output: vec![ResponsesOutput {
                _type: "message".to_string(),
                id: format!("msg_{}", rand::thread_rng().gen::<u32>()),
                content: vec![ResponsesOutputContent {
                    _type: "output_text".to_string(),
                    text: content,
                }],
            }],
        };
        return Ok((headers, Json(json!(response))));
    }
}
