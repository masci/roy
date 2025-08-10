use axum::http::HeaderMap;
use humantime;
use rand::Rng;
use std::{
    sync::{Arc, Mutex, MutexGuard},
    time::{Duration, SystemTime},
};
use tiktoken_rs::cl100k_base;

use crate::Args;

#[derive(Clone)]
pub struct ServerState {
    args: Args,
    request_count: Arc<Mutex<u32>>,
    token_count: Arc<Mutex<u32>>,
    requests_per_minute: Arc<Mutex<u32>>,
    tokens_per_minute: Arc<Mutex<u32>>,
    request_reset_time: Arc<Mutex<SystemTime>>,
    token_reset_time: Arc<Mutex<SystemTime>>,
}

impl ServerState {
    pub fn new(args: Args) -> Self {
        let now = SystemTime::now();
        let rpm = args.rpm;
        let tpm = args.tpm;
        Self {
            args,
            request_count: Arc::new(Mutex::new(0)),
            token_count: Arc::new(Mutex::new(0)),
            requests_per_minute: Arc::new(Mutex::new(rpm)),
            tokens_per_minute: Arc::new(Mutex::new(tpm)),
            request_reset_time: Arc::new(Mutex::new(now)),
            token_reset_time: Arc::new(Mutex::new(now)),
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

        // Approximate number of words needed. Adjust the division factor as needed for better accuracy.
        let word_count = length / 5;
        let mut content = lipsum::lipsum(word_count);
        content.truncate(length);
        content
    }

    pub fn count_tokens(&self, text: &str) -> anyhow::Result<u32> {
        let bpe = cl100k_base()?;
        Ok(bpe.encode_with_special_tokens(text).len() as u32)
    }

    fn reset_if_needed(
        &self,
        count: &mut MutexGuard<u32>,
        reset_time: &mut MutexGuard<SystemTime>,
        reset_duration: Duration,
    ) {
        let now = SystemTime::now();
        if now >= **reset_time {
            **count = 0;
            **reset_time = now + reset_duration;
        }
    }

    pub fn increment_request_count(&self) {
        let mut count = self
            .request_count
            .lock()
            .expect("Failed to lock request_count mutex");
        *count += 1;
    }

    pub fn add_token_usage(&self, tokens: u32) {
        let mut count = self
            .token_count
            .lock()
            .expect("Failed to lock token_count mutex");
        *count += tokens;
    }

    pub fn get_rate_limit_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        let request_count = *self
            .request_count
            .lock()
            .expect("Failed to lock request_count mutex");
        let token_count = *self
            .token_count
            .lock()
            .expect("Failed to lock token_count mutex");
        let request_reset = *self
            .request_reset_time
            .lock()
            .expect("Failed to lock request_reset_time mutex");
        let token_reset = *self
            .token_reset_time
            .lock()
            .expect("Failed to lock token_reset_time mutex");

        let now = SystemTime::now();
        let request_reset_duration = request_reset.duration_since(now).unwrap_or(Duration::ZERO);
        let token_reset_duration = token_reset.duration_since(now).unwrap_or(Duration::ZERO);

        let request_reset_duration_rounded = Duration::from_secs(request_reset_duration.as_secs());
        let token_reset_duration_rounded = Duration::from_secs(token_reset_duration.as_secs());

        headers.insert("x-ratelimit-limit-requests", "0".parse().unwrap());
        headers.insert(
            "x-ratelimit-remaining-requests",
            (self.args.rpm.saturating_sub(request_count))
                .to_string()
                .parse()
                .expect("x-ratelimit-remaining-requests must be a valid header value"),
        );
        headers.insert(
            "x-ratelimit-reset-requests",
            humantime::format_duration(request_reset_duration_rounded)
                .to_string()
                .parse()
                .expect("x-ratelimit-reset-requests must be a valid header value"),
        );

        headers.insert(
            "x-ratelimit-limit-tokens",
            self.args
                .tpm
                .to_string()
                .parse()
                .expect("x-ratelimit-limit-tokens must be a valid header value"),
        );
        headers.insert(
            "x-ratelimit-remaining-tokens",
            (self.args.tpm.saturating_sub(token_count))
                .to_string()
                .parse()
                .expect("x-ratelimit-remaining-tokens must be a valid header value"),
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
