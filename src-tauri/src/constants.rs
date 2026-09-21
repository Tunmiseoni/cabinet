use std::time::Duration;

pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);
pub const PROBE_DEFAULT_TIMEOUT_MS: u64 = 3_000;
pub const PROBE_TIMEOUT_MIN_MS: u64 = 100;
pub const PROBE_TIMEOUT_MAX_MS: u64 = 30_000;
pub const PORT_PROBE_ATTEMPT_TIMEOUT: Duration = Duration::from_millis(750);
pub const PORT_PROBE_RETRY_INTERVAL: Duration = Duration::from_millis(250);
pub const HOST_WAIT_TIMEOUT: Duration = Duration::from_secs(20);
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(4);
pub const MONITOR_INTERVAL: Duration = Duration::from_millis(300);
pub const TAILSCALE_STATUS_CACHE_TTL: Duration = Duration::from_millis(500);
pub const SESSION_KEEP: usize = 20;
pub const DIAGNOSTICS_SESSION_LIST_LIMIT: usize = 10;
pub const DIAGNOSTICS_LOG_TAIL_LINES: usize = 200;
pub const RETROARCH_DEFAULT_PORT: u16 = 55435;
