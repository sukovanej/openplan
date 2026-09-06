use serde::{Deserialize, Serialize};

// `cached_input_tokens` is the part of `input_tokens` that a cache served, and `reasoning_tokens`
// the part of `output_tokens` the model spent thinking. Claude Code reports cache reads as a
// separate bucket and Codex reports them as a subset, so both backends normalise to subsets here
// and `total_tokens` stays a sum of the three billed buckets.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub cached_input_tokens: u64,
    pub cache_write_tokens: u64,
    pub output_tokens: u64,
    pub reasoning_tokens: u64,
    pub cost_usd: Option<f64>,
}

impl Usage {
    pub fn total_tokens(&self) -> u64 {
        self.input_tokens + self.cache_write_tokens + self.output_tokens
    }

    pub fn uncached_input_tokens(&self) -> u64 {
        self.input_tokens.saturating_sub(self.cached_input_tokens)
    }

    pub fn add(&mut self, turn: &Self) {
        self.input_tokens += turn.input_tokens;
        self.cached_input_tokens += turn.cached_input_tokens;
        self.cache_write_tokens += turn.cache_write_tokens;
        self.output_tokens += turn.output_tokens;
        self.reasoning_tokens += turn.reasoning_tokens;
        if let Some(cost) = turn.cost_usd {
            self.cost_usd = Some(self.cost_usd.unwrap_or_default() + cost);
        }
    }
}
