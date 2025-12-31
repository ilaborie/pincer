//! Client configuration types.

use std::time::Duration;

/// Configuration for the HTTP client.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Request timeout duration.
    pub timeout: Duration,
    /// Connection timeout duration.
    pub connect_timeout: Duration,
    /// Maximum idle connections per host.
    pub pool_idle_per_host: usize,
    /// Idle connection timeout.
    pub pool_idle_timeout: Duration,
    /// Whether to retry on connection errors.
    pub retry_on_connection_failure: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            pool_idle_per_host: 32,
            pool_idle_timeout: Duration::from_secs(90),
            retry_on_connection_failure: true,
        }
    }
}

impl ClientConfig {
    /// Create a new configuration builder.
    #[must_use]
    pub fn builder() -> ClientConfigBuilder {
        ClientConfigBuilder::default()
    }
}

/// Builder for [`ClientConfig`].
#[derive(Debug, Clone, Default)]
pub struct ClientConfigBuilder {
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    pool_idle_per_host: Option<usize>,
    pool_idle_timeout: Option<Duration>,
    retry_on_connection_failure: Option<bool>,
}

impl ClientConfigBuilder {
    /// Set the request timeout.
    #[must_use]
    pub const fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the connection timeout.
    #[must_use]
    pub const fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set the maximum idle connections per host.
    #[must_use]
    pub const fn pool_idle_per_host(mut self, count: usize) -> Self {
        self.pool_idle_per_host = Some(count);
        self
    }

    /// Set the idle connection timeout.
    #[must_use]
    pub const fn pool_idle_timeout(mut self, timeout: Duration) -> Self {
        self.pool_idle_timeout = Some(timeout);
        self
    }

    /// Set whether to retry on connection failures.
    #[must_use]
    pub const fn retry_on_connection_failure(mut self, retry: bool) -> Self {
        self.retry_on_connection_failure = Some(retry);
        self
    }

    /// Build the configuration.
    #[must_use]
    pub fn build(self) -> ClientConfig {
        let defaults = ClientConfig::default();
        ClientConfig {
            timeout: self.timeout.unwrap_or(defaults.timeout),
            connect_timeout: self.connect_timeout.unwrap_or(defaults.connect_timeout),
            pool_idle_per_host: self
                .pool_idle_per_host
                .unwrap_or(defaults.pool_idle_per_host),
            pool_idle_timeout: self.pool_idle_timeout.unwrap_or(defaults.pool_idle_timeout),
            retry_on_connection_failure: self
                .retry_on_connection_failure
                .unwrap_or(defaults.retry_on_connection_failure),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = ClientConfig::default();
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert_eq!(config.connect_timeout, Duration::from_secs(10));
        assert_eq!(config.pool_idle_per_host, 32);
    }

    #[test]
    fn builder_overrides() {
        let config = ClientConfig::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(5))
            .pool_idle_per_host(16)
            .build();

        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.connect_timeout, Duration::from_secs(5));
        assert_eq!(config.pool_idle_per_host, 16);
    }
}
