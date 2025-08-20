// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Sse},
    Json,
};
use futures_util::stream;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::server_state::ServerState;

#[derive(Serialize, Debug)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

#[derive(Deserialize)]
pub struct ChatCompletionRequest {
    pub messages: Option<Vec<Value>>,
    pub model: Option<String>,
    #[serde(default)]
    pub stream: Option<bool>,
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

#[derive(Serialize, Debug)]
pub struct ChatCompletionChunk {
    pub id: String,
    pub object: String,
    pub created: u64,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<Usage>,
}

#[derive(Serialize, Debug)]
pub struct ChunkChoice {
    pub index: u32,
    pub delta: ChoiceDelta,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
}

#[derive(Serialize, Debug, Default)]
pub struct ChoiceDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
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

pub async fn chat_completions(
    state: State<ServerState>,
    Json(payload): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    if state.check_request_limit_exceeded() {
        let headers = state.get_rate_limit_headers();
        let error_body = json!({
            "error": {
                "message": "Too many requests",
                "type": "rate_limit_error",
                "code": "rate_limit_exceeded"
            }
        });
        return (StatusCode::TOO_MANY_REQUESTS, headers, Json(error_body)).into_response();
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

        return (status_code, headers, Json(error_body)).into_response();
    }

    let response_length = state.get_response_length();

    if response_length == 0 {
        let headers = state.get_rate_limit_headers();
        return (StatusCode::NO_CONTENT, headers, Json(json!({}))).into_response();
    }

    let content = state.generate_lorem_content(response_length);

    let prompt_text = payload
        .messages
        .as_ref()
        .map(|msgs| serde_json::to_string(msgs).unwrap_or_default())
        .unwrap_or_default();

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
        return (StatusCode::TOO_MANY_REQUESTS, headers, Json(error_body)).into_response();
    }
    state.add_token_usage(total_tokens);

    let stream_response = payload.stream.unwrap_or(false);
    if stream_response {
        let id = format!("chatcmpl-{}", rand::thread_rng().gen::<u32>());
        let created = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("should be able to get duration")
            .as_secs();
        let model = payload
            .model
            .clone()
            .unwrap_or_else(|| "gpt-3.5-turbo".to_string());
        let words = content
            .split_whitespace()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let mut events = vec![];

        // 1. First chunk with role
        let first_chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: ChoiceDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
            usage: None,
        };
        events.push(Ok::<_, Infallible>(
            Event::default().data(serde_json::to_string(&first_chunk).unwrap()),
        ));

        // 2. Content chunks
        for word in words {
            let chunk = ChatCompletionChunk {
                id: id.clone(),
                object: "chat.completion.chunk".to_string(),
                created,
                model: model.clone(),
                choices: vec![ChunkChoice {
                    index: 0,
                    delta: ChoiceDelta {
                        role: None,
                        content: Some(format!("{} ", word)),
                    },
                    finish_reason: None,
                }],
                usage: None,
            };
            events.push(Ok(
                Event::default().data(serde_json::to_string(&chunk).unwrap())
            ));
        }

        // 3. Final chunk with finish_reason
        let final_chunk = ChatCompletionChunk {
            id: id.clone(),
            object: "chat.completion.chunk".to_string(),
            created,
            model: model.clone(),
            choices: vec![ChunkChoice {
                index: 0,
                delta: Default::default(),
                finish_reason: Some("stop".to_string()),
            }],
            usage: Some(Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            }),
        };
        events.push(Ok(
            Event::default().data(serde_json::to_string(&final_chunk).unwrap())
        ));

        // 4. Done message
        events.push(Ok(Event::default().data("[DONE]")));

        let stream = stream::iter(events);

        return Sse::new(stream).into_response();
    }

    let response = ChatCompletionResponse {
        id: format!("chatcmpl-{}", rand::thread_rng().gen::<u32>()),
        object: "chat.completion".to_string(),
        created: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("should be able to get duration")
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
    (headers, Json(json!(response))).into_response()
}
