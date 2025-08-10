use axum::http::HeaderMap;
use humantime;
use rand::Rng;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime},
};
use tiktoken_rs::cl100k_base;

use crate::Args;

#[derive(Clone)]
pub struct ServerState {
    args: Args,
    request_timestamps: Arc<Mutex<VecDeque<SystemTime>>>,
    token_usage_timestamps: Arc<Mutex<VecDeque<(SystemTime, u32)>>>,
}

impl ServerState {
    pub fn new(args: Args) -> Self {
        Self {
            args,
            request_timestamps: Arc::new(Mutex::new(VecDeque::new())),
            token_usage_timestamps: Arc::new(Mutex::new(VecDeque::new())),
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
        let word_count = length / 5;
        let mut content = lipsum::lipsum(word_count);
        content.truncate(length);
        content
    }

    pub fn count_tokens(&self, text: &str) -> anyhow::Result<u32> {
        let bpe = cl100k_base()?;
        Ok(bpe.encode_with_special_tokens(text).len() as u32)
    }

    pub fn increment_request_count(&self) {
        let mut timestamps = self.request_timestamps.lock().unwrap();
        let now = SystemTime::now();
        let sixty_seconds_ago = now - Duration::from_secs(60);
        while let Some(front) = timestamps.front() {
            if *front < sixty_seconds_ago {
                timestamps.pop_front();
            } else {
                break;
            }
        }
        timestamps.push_back(now);
    }

    pub fn add_token_usage(&self, tokens: u32) {
        let mut timestamps = self.token_usage_timestamps.lock().unwrap();
        let now = SystemTime::now();
        let sixty_seconds_ago = now - Duration::from_secs(60);
        while let Some((t, _)) = timestamps.front() {
            if *t < sixty_seconds_ago {
                timestamps.pop_front();
            } else {
                break;
            }
        }
        timestamps.push_back((now, tokens));
    }

    pub fn get_rate_limit_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        let now = SystemTime::now();

        // Requests logic
        let mut timestamps = self.request_timestamps.lock().unwrap();
        let sixty_seconds_ago = now - Duration::from_secs(60);

        while let Some(front) = timestamps.front() {
            if *front < sixty_seconds_ago {
                timestamps.pop_front();
            } else {
                break;
            }
        }

        let request_count = timestamps.len() as u32;
        let limit = self.args.rpm;
        let remaining = limit.saturating_sub(request_count);

        let reset_duration = if request_count < limit {
            Duration::ZERO
        } else {
            if let Some(oldest) = timestamps.front() {
                (*oldest + Duration::from_secs(60))
                    .duration_since(now)
                    .unwrap_or(Duration::ZERO)
            } else {
                Duration::ZERO
            }
        };
        let reset_duration_rounded = Duration::from_secs(reset_duration.as_secs());

        headers.insert(
            "x-ratelimit-limit-requests",
            limit.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-remaining-requests",
            remaining.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            humantime::format_duration(reset_duration_rounded)
                .to_string()
                .parse()
                .expect("x-ratelimit-reset-requests must be a valid header value"),
        );

        // Tokens logic
        let mut token_timestamps = self.token_usage_timestamps.lock().unwrap();
        while let Some((t, _)) = token_timestamps.front() {
            if *t < sixty_seconds_ago {
                token_timestamps.pop_front();
            } else {
                break;
            }
        }

        let current_token_usage: u32 = token_timestamps.iter().map(|(_, tokens)| tokens).sum();
        let token_limit = self.args.tpm;
        let remaining_tokens = token_limit.saturating_sub(current_token_usage);

        let token_reset_duration = if current_token_usage < token_limit {
            Duration::ZERO
        } else {
            if let Some((oldest_ts, _)) = token_timestamps.front() {
                (*oldest_ts + Duration::from_secs(60))
                    .duration_since(now)
                    .unwrap_or(Duration::ZERO)
            } else {
                Duration::ZERO
            }
        };
        let token_reset_duration_rounded = Duration::from_secs(token_reset_duration.as_secs());

        headers.insert(
            "x-ratelimit-limit-tokens",
            token_limit.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            remaining_tokens.to_string().parse().unwrap(),
        );
        headers.insert(
            "x-ratelimit-reset-tokens",
            humantime::format_duration(token_reset_duration_rounded)
                .to_string()
                .parse()
                .expect("x-ratelimit-reset-tokens must be a valid header value"),
        );

        headers
    }
}
