use std::time::Duration;

pub const DISCOVERY_PROBE_TIMEOUT: Duration = Duration::from_millis(400);
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
pub const PROBE_DEFAULT_TIMEOUT_MS: u64 = 3_000;
pub const PROBE_TIMEOUT_MIN_MS: u64 = 100;
pub const PROBE_TIMEOUT_MAX_MS: u64 = 30_000;
pub const HOST_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(4);
pub const MONITOR_INTERVAL: Duration = Duration::from_millis(300);
