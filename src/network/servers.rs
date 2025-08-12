use crate::core::error::{Result, SpeedKarmaError};
use crate::data::models::SpeedtestServer;
use reqwest::{Client, ClientBuilder};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

/// Speedtest.net server configuration response
#[derive(Debug, Deserialize)]
struct SpeedtestConfig {
    servers: Vec<SpeedtestServerResponse>,
}

/// Individual server from speedtest.net API
#[derive(Debug, Deserialize)]
struct SpeedtestServerResponse {
    id: String,
    host: String,
    name: String,
    country: String,
    sponsor: String,
    #[serde(rename = "lat")]
    latitude: f64,
    #[serde(rename = "lon")]
    longitude: f64,
}

/// Connection health status for a server
#[derive(Debug, Clone)]
pub struct ConnectionHealth {
    pub server_id: String,
    pub is_connected: bool,
    pub last_ping: Option<Instant>,
    pub consecutive_failures: u32,
    pub average_latency_ms: Option<f64>,
    pub connection_established_at: Option<Instant>,
}

/// Persistent connection to a speedtest server
#[derive(Debug)]
pub struct ServerConnection {
    pub server: SpeedtestServer,
    pub client: Client,
    pub health: ConnectionHealth,
    pub last_activity: Instant,
}

/// Pool of speedtest servers with connection management
pub struct ServerPool {
    #[cfg(test)]
    pub servers: Vec<SpeedtestServer>,
    #[cfg(not(test))]
    pub(crate) servers: Vec<SpeedtestServer>,
    connections: Arc<RwLock<HashMap<String, ServerConnection>>>,
    client: Client,
    current_index: usize,
    user_location: Option<(f64, f64)>, // (latitude, longitude)
}

impl ServerPool {
    pub fn new() -> Result<Self> {
        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(|e| SpeedKarmaError::NetworkUnavailable(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            servers: Vec::new(),
            connections: Arc::new(RwLock::new(HashMap::new())),
            client,
            current_index: 0,
            user_location: None,
        })
    }

    /// Loads servers from speedtest.net API with regional prioritization
    pub async fn load_servers(&mut self) -> Result<()> {
        info!("Loading speedtest servers from API");
        
        // First try to get server list from speedtest.net
        let servers = self.fetch_servers_from_api().await
            .or_else(|_| self.load_fallback_servers())
            .map_err(|e| SpeedKarmaError::NetworkUnavailable(format!("Failed to load servers: {}", e)))?;

        // Calculate distances if we have user location
        let mut processed_servers = servers;
        if let Some((user_lat, user_lon)) = self.user_location {
            for server in &mut processed_servers {
                if let (Some(lat), Some(lon)) = (server.distance, server.latency) {
                    server.distance = Some(self.calculate_distance(user_lat, user_lon, lat, lon));
                }
            }
        }

        // Prioritize servers for Sri Lanka and regional optimization
        processed_servers.sort_by(|a, b| {
            let a_priority = self.get_server_priority(a);
            let b_priority = self.get_server_priority(b);
            b_priority.partial_cmp(&a_priority).unwrap_or(std::cmp::Ordering::Equal)
        });

        self.servers = processed_servers;
        info!("Loaded {} speedtest servers", self.servers.len());
        
        Ok(())
    }

    /// Fetch servers from speedtest.net API
    async fn fetch_servers_from_api(&self) -> Result<Vec<SpeedtestServer>> {
        debug!("Fetching servers from speedtest.net API");
        
        let response = self.client
            .get("https://www.speedtest.net/api/js/servers?engine=js")
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(SpeedKarmaError::NetworkUnavailable(
                format!("Speedtest API returned status: {}", response.status())
            ));
        }

        let servers_data: Vec<SpeedtestServerResponse> = response.json().await?;
        
        let servers: Vec<SpeedtestServer> = servers_data
            .into_iter()
            .map(|server_data| {
                SpeedtestServer::new(
                    server_data.id,
                    server_data.host,
                    8080, // Default speedtest port
                    server_data.name,
                    server_data.country,
                    server_data.sponsor,
                )
            })
            .collect();

        debug!("Fetched {} servers from API", servers.len());
        Ok(servers)
    }

    /// Load fallback servers for when API is unavailable
    fn load_fallback_servers(&self) -> Result<Vec<SpeedtestServer>> {
        warn!("Using fallback server list");
        
        // Hardcoded list of reliable servers for Sri Lanka region
        let fallback_servers = vec![
            SpeedtestServer::new(
                "21541".to_string(),
                "speedtest.dialog.lk".to_string(),
                8080,
                "Dialog Axiata".to_string(),
                "Sri Lanka".to_string(),
                "Dialog Axiata PLC".to_string(),
            ),
            SpeedtestServer::new(
                "24037".to_string(),
                "speedtest-sin1.digitalocean.com".to_string(),
                8080,
                "Singapore".to_string(),
                "Singapore".to_string(),
                "DigitalOcean".to_string(),
            ),
            SpeedtestServer::new(
                "13623".to_string(),
                "speedtest.slt.lk".to_string(),
                8080,
                "Sri Lanka Telecom".to_string(),
                "Sri Lanka".to_string(),
                "Sri Lanka Telecom PLC".to_string(),
            ),
            SpeedtestServer::new(
                "28910".to_string(),
                "speedtest-blr1.digitalocean.com".to_string(),
                8080,
                "Bangalore".to_string(),
                "India".to_string(),
                "DigitalOcean".to_string(),
            ),
            SpeedtestServer::new(
                "15322".to_string(),
                "lg-sin.fdcservers.net".to_string(),
                8080,
                "Singapore".to_string(),
                "Singapore".to_string(),
                "FDC Servers".to_string(),
            ),
        ];

        Ok(fallback_servers)
    }

    /// Calculate priority score for server selection (higher is better)
    fn get_server_priority(&self, server: &SpeedtestServer) -> f64 {
        let mut priority = 0.0;

        // Prioritize regional servers for Sri Lanka
        let country_lower = server.country.to_lowercase();
        if country_lower.contains("sri lanka") {
            priority += 100.0;
        } else if country_lower.contains("singapore") {
            priority += 80.0;
        } else if country_lower.contains("india") {
            priority += 70.0;
        } else if country_lower.contains("malaysia") || country_lower.contains("thailand") {
            priority += 60.0;
        }

        // Prefer servers with lower distance
        if let Some(distance) = server.distance {
            priority += (1000.0 - distance.min(1000.0)) / 10.0;
        }

        // Prefer active servers
        if server.is_active {
            priority += 10.0;
        }

        priority
    }

    /// Calculate distance between two points using Haversine formula
    fn calculate_distance(&self, lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
        let r = 6371.0; // Earth's radius in kilometers
        let d_lat = (lat2 - lat1).to_radians();
        let d_lon = (lon2 - lon1).to_radians();
        let a = (d_lat / 2.0).sin().powi(2)
            + lat1.to_radians().cos() * lat2.to_radians().cos() * (d_lon / 2.0).sin().powi(2);
        let c = 2.0 * a.sqrt().atan2((1.0 - a).sqrt());
        r * c
    }

    /// Establish persistent connection to a server
    pub async fn connect_to_server(&self, server: &SpeedtestServer) -> Result<()> {
        debug!("Establishing connection to server: {} ({})", server.name, server.host);

        let client = ClientBuilder::new()
            .timeout(Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()?;

        // Test connection with a lightweight request
        let test_url = format!("http://{}:{}/speedtest/latency.txt", server.host, server.port);
        let start_time = Instant::now();
        
        let response = client
            .get(&test_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await;

        let latency = start_time.elapsed().as_millis() as f64;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let connection = ServerConnection {
                    server: server.clone(),
                    client,
                    health: ConnectionHealth {
                        server_id: server.server_id.clone(),
                        is_connected: true,
                        last_ping: Some(Instant::now()),
                        consecutive_failures: 0,
                        average_latency_ms: Some(latency),
                        connection_established_at: Some(Instant::now()),
                    },
                    last_activity: Instant::now(),
                };

                let mut connections = self.connections.write().await;
                connections.insert(server.server_id.clone(), connection);
                
                info!("Successfully connected to server: {} (latency: {:.1}ms)", server.name, latency);
                Ok(())
            }
            Ok(resp) => {
                error!("Server responded with error status: {} for {}", resp.status(), server.host);
                Err(SpeedKarmaError::NetworkUnavailable(
                    format!("Server {} returned status {}", server.host, resp.status())
                ))
            }
            Err(e) => {
                error!("Failed to connect to server {}: {}", server.host, e);
                Err(SpeedKarmaError::NetworkUnavailable(
                    format!("Connection failed to {}: {}", server.host, e)
                ))
            }
        }
    }

    /// Establish connections to multiple servers for redundancy
    pub async fn establish_connection_pool(&self, target_connections: usize) -> Result<usize> {
        info!("Establishing connection pool with {} target connections", target_connections);
        
        let mut successful_connections = 0;
        let max_attempts = (target_connections * 2).min(self.servers.len());

        for server in self.servers.iter().take(max_attempts) {
            if successful_connections >= target_connections {
                break;
            }

            match self.connect_to_server(server).await {
                Ok(()) => {
                    successful_connections += 1;
                }
                Err(e) => {
                    warn!("Failed to connect to {}: {}", server.host, e);
                    // Continue trying other servers
                }
            }

            // Small delay between connection attempts to avoid overwhelming servers
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        if successful_connections == 0 {
            return Err(SpeedKarmaError::NetworkUnavailable(
                "Failed to establish any server connections".to_string()
            ));
        }

        info!("Established {} out of {} target connections", successful_connections, target_connections);
        Ok(successful_connections)
    }

    /// Monitor connection health and reconnect if needed
    pub async fn monitor_connections(&self) -> Result<()> {
        let mut connections = self.connections.write().await;
        let mut reconnect_needed = Vec::new();

        for (server_id, connection) in connections.iter_mut() {
            // Check if connection needs health check
            if connection.last_activity.elapsed() > Duration::from_secs(60) {
                match self.ping_server(connection).await {
                    Ok(latency) => {
                        connection.health.is_connected = true;
                        connection.health.last_ping = Some(Instant::now());
                        connection.health.consecutive_failures = 0;
                        connection.health.average_latency_ms = Some(latency);
                        connection.last_activity = Instant::now();
                        debug!("Health check passed for {}: {:.1}ms", connection.server.name, latency);
                    }
                    Err(_) => {
                        connection.health.consecutive_failures += 1;
                        connection.health.is_connected = false;
                        
                        if connection.health.consecutive_failures >= 3 {
                            warn!("Server {} failed health check {} times, marking for reconnection", 
                                  connection.server.name, connection.health.consecutive_failures);
                            reconnect_needed.push(server_id.clone());
                        }
                    }
                }
            }
        }

        // Remove failed connections and attempt reconnection
        for server_id in reconnect_needed {
            if let Some(connection) = connections.remove(&server_id) {
                drop(connections); // Release lock before async operation
                
                info!("Attempting to reconnect to {}", connection.server.name);
                if let Err(e) = self.connect_to_server(&connection.server).await {
                    error!("Reconnection failed for {}: {}", connection.server.name, e);
                }
                
                connections = self.connections.write().await; // Re-acquire lock
            }
        }

        Ok(())
    }

    /// Ping a server to check connection health
    async fn ping_server(&self, connection: &ServerConnection) -> Result<f64> {
        let ping_url = format!("http://{}:{}/speedtest/latency.txt", connection.server.host, connection.server.port);
        let start_time = Instant::now();
        
        let response = connection.client
            .get(&ping_url)
            .timeout(Duration::from_secs(5))
            .send()
            .await?;

        if response.status().is_success() {
            Ok(start_time.elapsed().as_millis() as f64)
        } else {
            Err(SpeedKarmaError::NetworkUnavailable(
                format!("Ping failed with status: {}", response.status())
            ))
        }
    }

    /// Get the next server in rotation
    pub fn next_server(&mut self) -> Option<&SpeedtestServer> {
        if self.servers.is_empty() {
            return None;
        }
        
        let server = &self.servers[self.current_index];
        self.current_index = (self.current_index + 1) % self.servers.len();
        Some(server)
    }
    
    /// Gets servers filtered by region/country
    pub fn get_regional_servers(&self, country: &str) -> Vec<&SpeedtestServer> {
        self.servers
            .iter()
            .filter(|server| server.country.eq_ignore_ascii_case(country))
            .collect()
    }
    
    /// Gets the closest servers by distance
    pub fn get_closest_servers(&self, count: usize) -> Vec<&SpeedtestServer> {
        let mut servers = self.servers.iter().collect::<Vec<_>>();
        servers.sort_by(|a, b| {
            match (a.distance, b.distance) {
                (Some(a_dist), Some(b_dist)) => a_dist.partial_cmp(&b_dist).unwrap_or(std::cmp::Ordering::Equal),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
        servers.into_iter().take(count).collect()
    }

    /// Get currently connected servers
    pub async fn get_connected_servers(&self) -> Vec<String> {
        let connections = self.connections.read().await;
        connections
            .values()
            .filter(|conn| conn.health.is_connected)
            .map(|conn| conn.server.name.clone())
            .collect()
    }

    /// Get connection health statistics
    pub async fn get_connection_stats(&self) -> ConnectionStats {
        let connections = self.connections.read().await;
        let total_connections = connections.len();
        let healthy_connections = connections.values()
            .filter(|conn| conn.health.is_connected)
            .count();
        
        let average_latency = connections.values()
            .filter_map(|conn| conn.health.average_latency_ms)
            .collect::<Vec<_>>();
        
        let avg_latency = if !average_latency.is_empty() {
            Some(average_latency.iter().sum::<f64>() / average_latency.len() as f64)
        } else {
            None
        };

        ConnectionStats {
            total_connections,
            healthy_connections,
            average_latency_ms: avg_latency,
            servers_available: self.servers.len(),
        }
    }

    /// Set user location for distance calculations
    pub fn set_user_location(&mut self, latitude: f64, longitude: f64) {
        self.user_location = Some((latitude, longitude));
        info!("User location set to: {:.4}, {:.4}", latitude, longitude);
    }

    /// Get servers count (for testing)
    pub fn servers_count(&self) -> usize {
        self.servers.len()
    }

    /// Set servers directly (for testing)
    pub fn set_servers(&mut self, servers: Vec<SpeedtestServer>) {
        self.servers = servers;
    }
}

/// Connection statistics for monitoring
#[derive(Debug, Clone)]
pub struct ConnectionStats {
    pub total_connections: usize,
    pub healthy_connections: usize,
    pub average_latency_ms: Option<f64>,
    pub servers_available: usize,
}