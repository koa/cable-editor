use cable_editor_common::limits::is_uid;
use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::{net::IpAddr, sync::LazyLock, time::Duration};

#[derive(Deserialize)]
pub struct Settings {
    auth_client_id: Box<str>,
    auth_issuer: Box<str>,
    user_info_url: Option<Box<str>>,
    auth_scopes: Option<Box<str>>,
    planner_groups: Option<Box<str>>,
    admin_groups: Option<Box<str>>,

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
    pub fn auth_scopes(&self) -> Box<[Box<str>]> {
        self.auth_scopes
            .as_deref()
            .unwrap_or("openid profile groups")
            .split_whitespace()
            .map(Box::from)
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
    url: Box<str>,
    token: Box<str>,
    provider_id: i64,
    type_id: i64,
    sync_interval_h: Option<f64>,
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
    /// The automatic sync runs at least this often (`sync_interval_h`, default 4 hours), so it
    /// also undoes changes made in Netbox by hand.
    pub fn sync_interval(&self) -> Duration {
        let hours = self.sync_interval_h.filter(|h| *h > 0.0).unwrap_or(4.0);
        Duration::from_secs_f64(hours * 3600.0)
    }
}

/// Delivery to the Leitungskataster (SIA405 LKMap, see docs/leitungskataster.md); optional, only
/// the export needs it.
#[derive(Deserialize)]
pub struct LkmapSettings {
    datenlieferant_uid: Box<str>,
    oid_prefix: Box<str>,
    perimeter_puffer_m: Option<f64>,
}

impl LkmapSettings {
    /// UID of whoever delivers the data (`Datenlieferant`), the same for all owners.
    pub fn datenlieferant_uid(&self) -> &str {
        &self.datenlieferant_uid
    }
    /// The first 8 characters of every `STANDARDOID`.
    pub fn oid_prefix(&self) -> &str {
        &self.oid_prefix
    }
    /// Metres around the convex hull of an owner's delivered ducts.
    pub fn perimeter_puffer_m(&self) -> f64 {
        self.perimeter_puffer_m.unwrap_or(10.0)
    }

    fn validated(self) -> Result<Self, ConfigError> {
        if !is_uid(&self.datenlieferant_uid) {
            return Err(ConfigError::Message(format!(
                "lkmap.datenlieferant_uid {:?} is no UID like CHE-123.456.789",
                self.datenlieferant_uid
            )));
        }
        // STANDARDOID: 8 characters prefix + 8 characters, an XML id (starts with a letter)
        let prefix = self.oid_prefix.as_bytes();
        if prefix.len() != 8
            || !prefix[0].is_ascii_alphabetic()
            || !prefix.iter().all(u8::is_ascii_alphanumeric)
        {
            return Err(ConfigError::Message(format!(
                "lkmap.oid_prefix {:?} must be 8 letters or digits, starting with a letter",
                self.oid_prefix
            )));
        }
        if self.perimeter_puffer_m().is_nan() || self.perimeter_puffer_m() <= 0.0 {
            return Err(ConfigError::Message(
                "lkmap.perimeter_puffer_m must be positive".into(),
            ));
        }
        Ok(self)
    }
}

fn create_lkmap_settings() -> Result<Option<LkmapSettings>, ConfigError> {
    match read_cfg()?.get::<LkmapSettings>("lkmap") {
        Ok(settings) => settings.validated().map(Some),
        Err(ConfigError::NotFound(_)) => Ok(None),
        Err(e) => Err(e),
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
/// Missing without the section `lkmap`; an invalid one stops the start (`main.rs` reads it early).
pub static LKMAP_CONFIG: LazyLock<Option<LkmapSettings>> =
    LazyLock::new(|| create_lkmap_settings().expect("Invalid section lkmap in config.yaml"));
