// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use crate::server_state::ServerState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Json, Sse},
};
use rand::distributions::Alphanumeric;
use rand::Rng;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::time::sleep;

#[derive(Deserialize)]
pub struct ResponsesRequest {
    pub model: Option<String>,
    pub input: Option<String>,
    pub instructions: Option<String>,
    pub stream: Option<bool>,
    #[serde(flatten)]
    pub _other: Value,
}

// Helper to generate random IDs
fn generate_id(prefix: &str) -> String {
    format!("{}_{:x}", prefix, rand::thread_rng().gen::<u128>())
}

// Data Models

#[derive(Serialize, Clone, Default)]
struct ResponseFormatText {
    #[serde(rename = "type")]
    _type: String,
}

#[derive(Serialize, Clone, Default)]
struct ResponseTextConfig {
    format: ResponseFormatText,
    verbosity: String,
}

#[derive(Serialize, Clone, Default)]
struct Reasoning {
    effort: String,
    generate_summary: Option<bool>,
    summary: Option<String>,
}

#[derive(Serialize, Clone, Debug)]
struct ResponseReasoningItem {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    summary: Vec<Value>,
    content: Option<Value>,
    encrypted_content: Option<Value>,
    status: Option<String>,
}

#[derive(Serialize, Clone, Debug, Default)]
struct ResponseOutputText {
    #[serde(rename = "type")]
    _type: String,
    text: String,
    annotations: Vec<Value>,
    logprobs: Vec<Value>,
}

#[derive(Serialize, Clone, Debug)]
struct ResponseOutputMessage {
    id: String,
    #[serde(rename = "type")]
    _type: String,
    content: Vec<ResponseOutputText>,
    role: String,
    status: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(untagged)]
enum ResponseOutputItem {
    Reasoning(ResponseReasoningItem),
    Message(ResponseOutputMessage),
}

#[derive(Serialize, Clone)]
struct InputTokensDetails {
    cached_tokens: u32,
}

#[derive(Serialize, Clone)]
struct OutputTokensDetails {
    reasoning_tokens: u32,
}

#[derive(Serialize, Clone)]
struct ResponseUsage {
    input_tokens: u32,
    input_tokens_details: InputTokensDetails,
    output_tokens: u32,
    output_tokens_details: OutputTokensDetails,
    total_tokens: u32,
}

#[derive(Serialize, Clone, Default)]
struct Response {
    id: String,
    created_at: f64,
    error: Option<Value>,
    incomplete_details: Option<Value>,
    instructions: Option<String>,
    metadata: Value,
    model: String,
    object: String,
    output: Vec<ResponseOutputItem>,
    parallel_tool_calls: bool,
    temperature: f32,
    tool_choice: String,
    tools: Vec<Value>,
    top_p: f32,
    background: bool,
    max_output_tokens: Option<u32>,
    max_tool_calls: Option<u32>,
    previous_response_id: Option<String>,
    prompt: Option<String>,
    prompt_cache_key: Option<String>,
    reasoning: Reasoning,
    safety_identifier: Option<String>,
    service_tier: String,
    status: String,
    text: ResponseTextConfig,
    top_logprobs: u32,
    truncation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    usage: Option<ResponseUsage>,
    user: Option<String>,
    store: bool,
}

// SSE

#[derive(Serialize)]
struct ResponseEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    response: Response,
}

#[derive(Serialize)]
#[serde(untagged)]
enum OutputItem {
    Reasoning(ResponseReasoningItem),
    Message(ResponseOutputMessage),
}

#[derive(Serialize)]
struct ResponseOutputItemAddedEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item: OutputItem,
}

#[derive(Serialize)]
struct ResponseOutputItemDoneEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item: OutputItem,
}

#[derive(Serialize)]
struct ResponseContentPartAddedEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item_id: String,
    content_index: u32,
    part: ResponseOutputText,
}

#[derive(Serialize)]
struct ResponseTextDeltaEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item_id: String,
    content_index: u32,
    delta: String,
    logprobs: Vec<Value>,
    obfuscation: String,
}

#[derive(Serialize)]
struct ResponseTextDoneEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item_id: String,
    content_index: u32,
    text: String,
    logprobs: Vec<Value>,
}

#[derive(Serialize)]
struct ResponseContentPartDoneEvent {
    #[serde(rename = "type")]
    _type: String,
    sequence_number: u32,
    output_index: u32,
    item_id: String,
    content_index: u32,
    part: ResponseOutputText,
}

pub async fn responses(
    state: State<ServerState>,
    Json(payload): Json<ResponsesRequest>,
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

    let prompt_text = payload.input.clone().unwrap_or_else(|| "".to_string());
    let prompt_tokens = state.count_tokens(&prompt_text).unwrap_or(0) as u32;
    let completion_tokens = state.count_tokens(&content).unwrap_or(0) as u32;
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

    let headers = state.get_rate_limit_headers();
    let model = payload
        .model
        .clone()
        .unwrap_or_else(|| "gpt-5-2025-08-07".to_string());
    let response_id = generate_id("resp");
    let message_id = generate_id("msg");
    let created_at = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("should be able to get duration")
        .as_secs_f64();

    let stream_response = payload.stream.unwrap_or(false);
    if stream_response {
        let reasoning_item_id = generate_id("rs");
        let stream = async_stream::stream! {
            let mut sequence_number = 0;
            let mut response = Response {
                id: response_id.clone(),
                object: "response".to_string(),
                created_at,
                model: model.clone(),
                status: "in_progress".to_string(),
                instructions: payload.instructions.clone(),
                parallel_tool_calls: true,
                temperature: 1.0,
                tool_choice: "auto".to_string(),
                top_p: 1.0,
                background: false,
                reasoning: Reasoning {
                    effort: "medium".to_string(),
                    ..Default::default()
                },
                service_tier: "auto".to_string(),
                text: ResponseTextConfig {
                    format: ResponseFormatText { _type: "text".to_string() },
                    verbosity: "medium".to_string(),
                },
                top_logprobs: 0,
                truncation: "disabled".to_string(),
                store: false,
                ..Default::default()
            };

            // 1. response.created
            let created_event = ResponseEvent {
                _type: "response.created".to_string(),
                sequence_number,
                response: response.clone(),
            };
            yield Ok::<_, Infallible>(Event::default().event("response.created").data(serde_json::to_string(&created_event).unwrap()));
            sequence_number += 1;

            // 2. response.in_progress
            let in_progress_event = ResponseEvent {
                _type: "response.in_progress".to_string(),
                sequence_number,
                response: response.clone(),
            };
            yield Ok::<_, Infallible>(Event::default().event("response.in_progress").data(serde_json::to_string(&in_progress_event).unwrap()));
            sequence_number += 1;

            // 3. response.output_item.added (simulate a reasoning event)
            let reasoning_item = ResponseReasoningItem {
                id: reasoning_item_id.clone(),
                _type: "reasoning".to_string(),
                summary: vec![],
                content: None,
                encrypted_content: None,
                status: None,
            };
            let output_item_added_event = ResponseOutputItemAddedEvent {
                _type: "response.output_item.added".to_string(),
                sequence_number,
                output_index: 0,
                item: OutputItem::Reasoning(reasoning_item.clone()),
            };
            response.output.push(ResponseOutputItem::Reasoning(reasoning_item.clone()));
            yield Ok::<_, Infallible>(Event::default().event("response.output_item.added").data(serde_json::to_string(&output_item_added_event).unwrap()));
            sequence_number += 1;

            // 4. response.output_item.done (reasoning)
            let output_item_done_event = ResponseOutputItemDoneEvent {
                _type: "response.output_item.done".to_string(),
                sequence_number,
                output_index: 0,
                item: OutputItem::Reasoning(reasoning_item.clone()),
            };
            yield Ok::<_, Infallible>(Event::default().event("response.output_item.done").data(serde_json::to_string(&output_item_done_event).unwrap()));
            sequence_number += 1;

            // 5. response.output_item.added (message)
            let message_item = ResponseOutputMessage {
                id: message_id.clone(),
                _type: "message".to_string(),
                content: vec![],
                role: "assistant".to_string(),
                status: "in_progress".to_string(),
            };
            let output_item_added_event = ResponseOutputItemAddedEvent {
                _type: "response.output_item.added".to_string(),
                sequence_number,
                output_index: 1,
                item: OutputItem::Message(message_item.clone()),
            };
            response.output.push(ResponseOutputItem::Message(message_item.clone()));
            yield Ok::<_, Infallible>(Event::default().event("response.output_item.added").data(serde_json::to_string(&output_item_added_event).unwrap()));
            sequence_number += 1;

            // 6. response.content_part.added
            let part = ResponseOutputText {
                _type: "output_text".to_string(),
                text: "".to_string(),
                annotations: vec![],
                logprobs: vec![],
            };
            let content_part_added_event = ResponseContentPartAddedEvent {
                _type: "response.content_part.added".to_string(),
                sequence_number,
                output_index: 1,
                item_id: message_id.clone(),
                content_index: 0,
                part: part.clone(),
            };
            if let Some(ResponseOutputItem::Message(msg)) = response.output.get_mut(1) {
                msg.content.push(part);
            }
            yield Ok::<_, Infallible>(Event::default().event("response.content_part.added").data(serde_json::to_string(&content_part_added_event).unwrap()));
            sequence_number += 1;

            // 7. response.output_text.delta
            let chunks = content.as_bytes().chunks(5);
            for chunk in chunks {
                let delta = String::from_utf8_lossy(chunk).to_string();
                let obfuscation: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(10)
                    .map(char::from)
                    .collect();
                let delta_event = ResponseTextDeltaEvent {
                    _type: "response.output_text.delta".to_string(),
                    sequence_number,
                    output_index: 1,
                    item_id: message_id.clone(),
                    content_index: 0,
                    delta,
                    logprobs: vec![],
                    obfuscation,
                };
                yield Ok::<_, Infallible>(Event::default().event("response.output_text.delta").data(serde_json::to_string(&delta_event).unwrap()));
                sequence_number += 1;
                sleep(Duration::from_millis(10)).await;
            }

            // 8. response.output_text.done
            let text_done_event = ResponseTextDoneEvent {
                _type: "response.output_text.done".to_string(),
                sequence_number,
                output_index: 1,
                item_id: message_id.clone(),
                content_index: 0,
                text: content.clone(),
                logprobs: vec![],
            };
            if let Some(ResponseOutputItem::Message(msg)) = response.output.get_mut(1) {
                if let Some(p) = msg.content.get_mut(0) {
                    p.text = content.clone();
                }
            }
            yield Ok::<_, Infallible>(Event::default().event("response.output_text.done").data(serde_json::to_string(&text_done_event).unwrap()));
            sequence_number += 1;

            // 9. response.content_part.done
            let part = ResponseOutputText {
                _type: "output_text".to_string(),
                text: content.clone(),
                annotations: vec![],
                logprobs: vec![],
            };
            let content_part_done_event = ResponseContentPartDoneEvent {
                _type: "response.content_part.done".to_string(),
                sequence_number,
                output_index: 1,
                item_id: message_id.clone(),
                content_index: 0,
                part,
            };
            yield Ok::<_, Infallible>(Event::default().event("response.content_part.done").data(serde_json::to_string(&content_part_done_event).unwrap()));
            sequence_number += 1;

            // 10. response.output_item.done (message)
            let final_message_item = ResponseOutputMessage {
                id: message_id.clone(),
                _type: "message".to_string(),
                content: vec![ResponseOutputText {
                    _type: "output_text".to_string(),
                    text: content.clone(),
                    annotations: vec![],
                    logprobs: vec![],
                }],
                role: "assistant".to_string(),
                status: "completed".to_string(),
            };
            let output_item_done_event = ResponseOutputItemDoneEvent {
                _type: "response.output_item.done".to_string(),
                sequence_number,
                output_index: 1,
                item: OutputItem::Message(final_message_item.clone()),
            };
            if let Some(ResponseOutputItem::Message(msg)) = response.output.get_mut(1) {
                *msg = final_message_item;
            }
            yield Ok::<_, Infallible>(Event::default().event("response.output_item.done").data(serde_json::to_string(&output_item_done_event).unwrap()));
            sequence_number += 1;

            // 11. response.completed
            response.status = "completed".to_string();
            response.usage = Some(ResponseUsage {
                input_tokens: prompt_tokens,
                input_tokens_details: InputTokensDetails { cached_tokens: 0 },
                output_tokens: completion_tokens + 128, // mock reasoning tokens
                output_tokens_details: OutputTokensDetails { reasoning_tokens: 128 },
                total_tokens: total_tokens + 128,
            });
            let completed_event = ResponseEvent {
                _type: "response.completed".to_string(),
                sequence_number,
                response: response.clone(),
            };
            yield Ok::<_, Infallible>(Event::default().event("response.completed").data(serde_json::to_string(&completed_event).unwrap()));

            // End of stream
            yield Ok::<_, Infallible>(Event::default().data("[DONE]"));
        };

        return Sse::new(stream).into_response();
    } else {
        let output_text = ResponseOutputText {
            _type: "output_text".to_string(),
            text: content.clone(),
            ..Default::default()
        };

        let message_item = ResponseOutputMessage {
            id: message_id,
            _type: "message".to_string(),
            content: vec![output_text],
            role: "assistant".to_string(),
            status: "completed".to_string(),
        };

        let response = Response {
            id: response_id,
            object: "response".to_string(),
            created_at,
            model,
            status: "completed".to_string(),
            output: vec![ResponseOutputItem::Message(message_item)],
            usage: Some(ResponseUsage {
                input_tokens: prompt_tokens,
                input_tokens_details: InputTokensDetails { cached_tokens: 0 },
                output_tokens: completion_tokens,
                output_tokens_details: OutputTokensDetails {
                    reasoning_tokens: 0,
                },
                total_tokens,
            }),
            instructions: payload.instructions,
            parallel_tool_calls: true,
            temperature: 1.0,
            tool_choice: "auto".to_string(),
            top_p: 1.0,
            background: false,
            reasoning: Reasoning {
                effort: "medium".to_string(),
                ..Default::default()
            },
            service_tier: "auto".to_string(),
            text: ResponseTextConfig {
                format: ResponseFormatText {
                    _type: "text".to_string(),
                },
                verbosity: "medium".to_string(),
            },
            top_logprobs: 0,
            truncation: "disabled".to_string(),
            store: false,
            ..Default::default()
        };

        return (headers, Json(json!(response))).into_response();
    }
}
