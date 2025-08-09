use axum::http::HeaderMap;
use rand::Rng;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tiktoken_rs::cl100k_base;

use crate::Args;

#[derive(Clone)]
pub struct ServerState {
    args: Args,
    request_count: Arc<Mutex<u32>>,
    token_count: Arc<Mutex<u32>>,
    request_reset_time: Arc<Mutex<SystemTime>>,
    token_reset_time: Arc<Mutex<SystemTime>>,
}

impl ServerState {
    pub fn new(args: Args) -> Self {
        let now = SystemTime::now();
        let request_reset = now + Duration::from_secs(args.x_ratelimit_reset_requests);
        let token_reset = now + Duration::from_secs(args.x_ratelimit_reset_tokens * 60);

        Self {
            args,
            request_count: Arc::new(Mutex::new(0)),
            token_count: Arc::new(Mutex::new(0)),
            request_reset_time: Arc::new(Mutex::new(request_reset)),
            token_reset_time: Arc::new(Mutex::new(token_reset)),
        }
    }

    pub fn should_return_error(&self) -> Option<u16> {
        if let (Some(code), Some(rate)) = (self.args.error_code, self.args.error_rate) {
            let mut rng = rand::thread_rng();
            if rng.gen_range(0..100) < rate {
                return Some(code);
            }
        }
        None
    }

    pub fn get_response_length(&self) -> usize {
        match &self.args.response_length {
            Some(length_str) => {
                if let Some(pos) = length_str.find(':') {
                    let min: usize = length_str[..pos].parse().unwrap_or(0);
                    let max: usize = length_str[pos + 1..].parse().unwrap_or(100);
                    rand::thread_rng().gen_range(min..=max)
                } else {
                    length_str.parse().unwrap_or(0)
                }
            }
            None => 0,
        }
    }

    pub fn generate_lorem_content(&self, length: usize) -> String {
        if length == 0 {
            return String::new();
        }

        let mut content = String::new();
        while content.len() < length {
            content.push_str(&lipsum::lipsum(10));
            content.push(' ');
        }
        content.truncate(length);
        content
    }

    pub fn count_tokens(&self, text: &str) -> u32 {
        let bpe = cl100k_base().unwrap();
        bpe.encode_with_special_tokens(text).len() as u32
    }

    pub fn increment_request_count(&self) {
        let now = SystemTime::now();
        let mut count = self.request_count.lock().unwrap();
        let mut reset_time = self.request_reset_time.lock().unwrap();

        if now >= *reset_time {
            *count = 0;
            *reset_time = now + Duration::from_secs(self.args.x_ratelimit_reset_requests);
        }

        *count += 1;
    }

    pub fn add_token_usage(&self, tokens: u32) {
        let now = SystemTime::now();
        let mut count = self.token_count.lock().unwrap();
        let mut reset_time = self.token_reset_time.lock().unwrap();

        if now >= *reset_time {
            *count = 0;
            *reset_time = now + Duration::from_secs(self.args.x_ratelimit_reset_tokens * 60);
        }

        *count += tokens;
    }

    pub fn get_rate_limit_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        let request_count = *self.request_count.lock().unwrap();
        let token_count = *self.token_count.lock().unwrap();
        let request_reset = *self.request_reset_time.lock().unwrap();
        let token_reset = *self.token_reset_time.lock().unwrap();

        let now = SystemTime::now();
        let request_reset_seconds = request_reset
            .duration_since(now)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let token_reset_seconds = token_reset
            .duration_since(now)
            .unwrap_or(Duration::ZERO)
            .as_secs();

        headers.insert(
            "x-ratelimit-limit-requests",
            self.args
                .x_ratelimit_limit_requests
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            (self
                .args
                .x_ratelimit_limit_requests
                .saturating_sub(request_count))
            .to_string()
            .parse()
            .unwrap(),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            format!("{}s", request_reset_seconds).parse().unwrap(),
        );

        headers.insert(
            "x-ratelimit-limit-tokens",
            self.args
                .x_ratelimit_limit_tokens
                .to_string()
                .parse()
                .unwrap(),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            (self
                .args
                .x_ratelimit_limit_tokens
                .saturating_sub(token_count))
            .to_string()
            .parse()
            .unwrap(),
        );
        headers.insert(
            "x-ratelimit-reset-tokens",
            format!("{}m0s", token_reset_seconds / 60).parse().unwrap(),
        );

        headers
    }
}