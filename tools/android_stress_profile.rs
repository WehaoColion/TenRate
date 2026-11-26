//! The baseline and extreme workloads share the same probe and domain gates.
#[derive(Clone, Copy, Debug)]
pub struct StressProfile {
    pub name: &'static str,
    pub history_rows: usize,
    pub edit_rounds: usize,
    pub burst_rounds: usize,
    pub sticky_rounds: usize,
}

pub const BASELINE: StressProfile = StressProfile {
    name: "baseline",
    history_rows: 500_000,
    edit_rounds: 12,
    burst_rounds: 30,
    sticky_rounds: 24,
};

pub const EXTREME: StressProfile = StressProfile {
    name: "extreme",
    history_rows: 1_000_000,
    edit_rounds: 48,
    burst_rounds: 90,
    sticky_rounds: 48,
};

pub const ACTIVE: StressProfile = if cfg!(tenrate_extreme_history) { EXTREME } else { BASELINE };
pub const HISTORY_PADDING_BYTES: usize = 16 * 1024;
pub const SAVE_BUDGET_MS: u128 = 8_000;
pub const RECOVERY_BUDGET_SECONDS: u64 = 600;
