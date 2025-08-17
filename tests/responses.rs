// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        routing::post,
        Router,
    };
    use clap_verbosity_flag::Verbosity;
    use roy_cli::{responses, server_state::ServerState, Args};
    use tower::ServiceExt; // for `oneshot`

    #[tokio::test]
    async fn test_responses() {
        let args = Args {
            verbosity: Verbosity::new(0, 0),
            port: 8000,
            address: "127.0.0.1".parse().unwrap(),
            response_length: Some("10".to_string()),
            error_code: None,
            error_rate: None,
            rpm: 60,
            tpm: 150000,
            slowdown: Some("0:100".to_string()),
            timeout: None,
        };
        let state = ServerState::new(args);
        let app = Router::new()
            .route("/v1/responses", post(responses::responses))
            .with_state(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/responses")
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"model":"gpt-4.1","input":"Hello"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
