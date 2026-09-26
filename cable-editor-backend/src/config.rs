use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::{net::IpAddr, sync::LazyLock};

#[derive(Deserialize)]
pub struct Settings {
    auth_client_id: String,
    auth_issuer: String,
    user_info_url: Option<String>,
    auth_scopes: Option<String>,
    planner_groups: Option<String>,
    admin_groups: Option<String>,

    server_port: Option<u16>,
    server_mgmt_port: Option<u16>,
    server_bind_address: Option<IpAddr>,
}
impl Settings {
    pub fn auth_client_id(&self) -> &str {
        &self.auth_client_id
    }
    /// Without trailing slash, it is appended to for discovery and must match the token `iss`.
    pub fn auth_issuer(&self) -> &str {
        self.auth_issuer.trim_end_matches('/')
    }

    /// OIDC scopes the frontend requests, space separated like the `scope` parameter.
    /// `groups` needs a matching scope at the provider.
    pub fn auth_scopes(&self) -> Vec<String> {
        self.auth_scopes
            .as_deref()
            .unwrap_or("openid profile groups")
            .split_whitespace()
            .map(String::from)
            .collect()
    }
    /// OIDC groups whose members may plan and change the master data, space separated.
    pub fn planner_groups(&self) -> impl Iterator<Item = &str> {
        self.planner_groups
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
    }
    /// OIDC groups whose members may also implement plans, sync them to Netbox and delete
    /// cables, space separated.
    pub fn admin_groups(&self) -> impl Iterator<Item = &str> {
        self.admin_groups
            .as_deref()
            .unwrap_or_default()
            .split_whitespace()
    }
    pub fn server_port(&self) -> u16 {
        self.server_port.unwrap_or(8080)
    }
    pub fn server_mgmt_port(&self) -> u16 {
        self.server_mgmt_port
            .unwrap_or_else(|| self.server_port() + 1000)
    }
    pub fn server_bind_address(&self) -> IpAddr {
        self.server_bind_address
            .unwrap_or_else(|| IpAddr::from([0u8; 16]))
    }
    pub fn user_info_url(&self) -> Option<&str> {
        self.user_info_url.as_deref()
    }
}
#[derive(Deserialize)]
pub struct NetboxSettings {
    url: String,
    token: String,
    provider_id: i64,
    type_id: i64,
}

impl NetboxSettings {
    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn provider_id(&self) -> i64 {
        self.provider_id
    }
    pub fn type_id(&self) -> i64 {
        self.type_id
    }
}

fn create_netbox_settings() -> Result<NetboxSettings, ConfigError> {
    let cfg = read_cfg()?;
    cfg.get("netbox")
}

fn create_settings() -> Result<Settings, ConfigError> {
    let cfg = read_cfg()?;
    cfg.get("oauth")
}

fn read_cfg() -> Result<Config, ConfigError> {
    let cfg = Config::builder()
        .add_source(File::with_name("config.yaml").required(false))
        .add_source(Environment::with_prefix("app").separator("__"))
        .build()?;
    Ok(cfg)
}

pub static CONFIG: LazyLock<Settings> =
    LazyLock::new(|| create_settings().expect("Cannot load config.yaml"));
pub static NETBOX_CONFIG: LazyLock<NetboxSettings> =
    LazyLock::new(|| create_netbox_settings().expect("Cannot load config.yaml"));
