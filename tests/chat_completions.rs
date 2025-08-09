#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use clap_verbosity_flag::Verbosity;
    use roy::{chat_completion, server_state::ServerState, Args};
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn test_chat_completions() {
        let args = Args {
            verbosity: Verbosity::new(0, 0),
            port: 8000,
            address: "127.0.0.1".parse().unwrap(),
            response_length: Some("10".to_string()),
            error_code: None,
            error_rate: None,
            x_ratelimit_limit_requests: 60,
            x_ratelimit_reset_requests: 1,
            x_ratelimit_limit_tokens: 150000,
            x_ratelimit_reset_tokens: 6,
        };
        let state = ServerState::new(args);
        let app = Router::new()
            .route(
                "/v1/chat/completions",
                post(chat_completion::chat_completions),
            )
            .with_state(state);

        let response = app
            .oneshot(Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Content-Type", "application/json")
                .body(Body::from(
                    r#"{"messages":[{"role":"user","content":"Hello"}],"model":"gpt-3.5-turbo"}"#,
                ))
                .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
