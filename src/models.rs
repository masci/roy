// Copyright 2025 Massimiliano Pippi
// SPDX-License-Identifier: MIT

use serde::Serialize;

#[derive(Serialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
