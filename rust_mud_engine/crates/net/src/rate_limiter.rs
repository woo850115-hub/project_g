use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Configuration for rate limiting.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub max_connections_total: usize,
    pub max_connections_per_ip: usize,
    pub max_commands_per_second: u32,
    pub max_input_length: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_connections_total: 1000,
            max_connections_per_ip: 5,
            max_commands_per_second: 20,
            max_input_length: 4096,
        }
    }
}

/// Reason a connection was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RateLimitRejection {
    TotalLimitReached,
    IpLimitReached,
}

impl std::fmt::Display for RateLimitRejection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TotalLimitReached => write!(f, "server at max connections"),
            Self::IpLimitReached => write!(f, "too many connections from this IP"),
        }
    }
}

/// Tracks connection counts per IP and total.
/// Shared across server tasks via Arc<Mutex>.
#[derive(Debug)]
pub struct ConnectionLimiter {
    config: RateLimitConfig,
    total: usize,
    per_ip: BTreeMap<IpAddr, usize>,
}

impl ConnectionLimiter {
    pub fn new(config: RateLimitConfig) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            config,
            total: 0,
            per_ip: BTreeMap::new(),
        }))
    }

    /// Try to admit a new connection. Returns Ok(()) on success.
    pub fn try_admit(&mut self, ip: IpAddr) -> Result<(), RateLimitRejection> {
        if self.total >= self.config.max_connections_total {
            return Err(RateLimitRejection::TotalLimitReached);
        }
        let count = self.per_ip.entry(ip).or_insert(0);
        if *count >= self.config.max_connections_per_ip {
            return Err(RateLimitRejection::IpLimitReached);
        }
        *count += 1;
        self.total += 1;
        Ok(())
    }

    /// Release a connection slot when a client disconnects.
    pub fn release(&mut self, ip: IpAddr) {
        if let Some(count) = self.per_ip.get_mut(&ip) {
            *count = count.saturating_sub(1);
            if *count == 0 {
                self.per_ip.remove(&ip);
            }
        }
        self.total = self.total.saturating_sub(1);
    }

    pub fn total_connections(&self) -> usize {
        self.total
    }
}

/// Per-session token-bucket command throttle.
pub struct CommandThrottle {
    max_per_second: u32,
    tokens: u32,
    last_refill: Instant,
}

impl CommandThrottle {
    pub fn new(max_per_second: u32) -> Self {
        Self {
            max_per_second,
            tokens: max_per_second,
            last_refill: Instant::now(),
        }
    }

    /// Try to consume one command token. Returns true if allowed.
    pub fn try_consume(&mut self) -> bool {
        self.refill();
        if self.tokens > 0 {
            self.tokens -= 1;
            true
        } else {
            false
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill);
        let new_tokens = (elapsed.as_secs_f64() * self.max_per_second as f64) as u32;
        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.max_per_second);
            self.last_refill = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn connection_limiter_admits_within_limit() {
        let limiter = ConnectionLimiter::new(RateLimitConfig {
            max_connections_total: 10,
            max_connections_per_ip: 3,
            ..Default::default()
        });
        let ip = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
        let mut l = limiter.lock().unwrap();
        assert!(l.try_admit(ip).is_ok());
        assert!(l.try_admit(ip).is_ok());
        assert!(l.try_admit(ip).is_ok());
        assert_eq!(l.total_connections(), 3);
    }

    #[test]
    fn connection_limiter_rejects_per_ip() {
        let limiter = ConnectionLimiter::new(RateLimitConfig {
            max_connections_total: 100,
            max_connections_per_ip: 2,
            ..Default::default()
        });
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));
        let mut l = limiter.lock().unwrap();
        assert!(l.try_admit(ip).is_ok());
        assert!(l.try_admit(ip).is_ok());
        assert_eq!(l.try_admit(ip), Err(RateLimitRejection::IpLimitReached));
    }

    #[test]
    fn connection_limiter_rejects_total() {
        let limiter = ConnectionLimiter::new(RateLimitConfig {
            max_connections_total: 2,
            max_connections_per_ip: 10,
            ..Default::default()
        });
        let mut l = limiter.lock().unwrap();
        let ip1 = IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(2, 0, 0, 2));
        let ip3 = IpAddr::V4(Ipv4Addr::new(3, 0, 0, 3));
        assert!(l.try_admit(ip1).is_ok());
        assert!(l.try_admit(ip2).is_ok());
        assert_eq!(l.try_admit(ip3), Err(RateLimitRejection::TotalLimitReached));
    }

    #[test]
    fn connection_limiter_release() {
        let limiter = ConnectionLimiter::new(RateLimitConfig {
            max_connections_total: 10,
            max_connections_per_ip: 2,
            ..Default::default()
        });
        let ip = IpAddr::V4(Ipv4Addr::new(5, 5, 5, 5));
        let mut l = limiter.lock().unwrap();
        assert!(l.try_admit(ip).is_ok());
        assert!(l.try_admit(ip).is_ok());
        assert_eq!(l.try_admit(ip), Err(RateLimitRejection::IpLimitReached));
        l.release(ip);
        assert!(l.try_admit(ip).is_ok());
        assert_eq!(l.total_connections(), 2);
    }

    #[test]
    fn command_throttle_allows_burst() {
        let mut throttle = CommandThrottle::new(5);
        for _ in 0..5 {
            assert!(throttle.try_consume());
        }
        // 6th should fail (no time passed to refill)
        assert!(!throttle.try_consume());
    }

    #[test]
    fn command_throttle_refills_over_time() {
        let mut throttle = CommandThrottle::new(10);
        // Exhaust tokens
        for _ in 0..10 {
            assert!(throttle.try_consume());
        }
        assert!(!throttle.try_consume());
        // Simulate time passing
        throttle.last_refill = Instant::now() - std::time::Duration::from_secs(1);
        assert!(throttle.try_consume());
    }

    #[test]
    fn input_length_check() {
        let config = RateLimitConfig {
            max_input_length: 10,
            ..Default::default()
        };
        let short_input = "hello";
        let long_input = "this is way too long for the limit";
        assert!(short_input.len() <= config.max_input_length);
        assert!(long_input.len() > config.max_input_length);
    }
}
