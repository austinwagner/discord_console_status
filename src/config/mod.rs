extern crate serde_json;

mod file;

use std::io;
use std::fs::File;
use std::time::Duration;
use std::collections::HashMap;
use self::file::ConfigFile;

quick_error! {
    #[derive(Debug)]
    pub enum ConfigError {
        Io(err: io::Error) {
            from()
            description("io error")
            display("I/O error: {}", err)
            cause(err)
        }
        Json(err: serde_json::Error) {
            from()
            description("json parse error")
            display("JSON parsing error: {}", err)
            cause(err)
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum TitleSetting {
    Ignore,
    NameOnly,
    Full,
}

pub struct PresenceMonitorConfig {
    pub discord_username: String,
    pub discord_password: String,
    pub xbl_id: String,
    pub xbl_api_key: String,
    pub psn_id: String,
    pub psn_refresh_token: String,
    pub update_interval: Duration,
    pub title_settings: HashMap<String, TitleSetting>,
}

impl PresenceMonitorConfig {
    pub fn from_file(path: &str) -> Result<PresenceMonitorConfig, ConfigError> {
        let file = File::open(path)?;
        let config_file: ConfigFile = serde_json::from_reader(file)?;

        let mut config = PresenceMonitorConfig {
            discord_username: config_file.discord_username.clone(),
            discord_password: config_file.discord_password.clone(),
            xbl_id: "".to_owned(),
            xbl_api_key: "".to_owned(),
            psn_id: "".to_owned(),
            psn_refresh_token: "".to_owned(),
            update_interval: Duration::from_secs(config_file.update_interval.unwrap_or(30u64)),
            title_settings: HashMap::new(),
        };

        if let Some(ref xbl) = config_file.xbl {
            config.xbl_id = xbl.id.clone();
            config.xbl_api_key = xbl.api_key.clone();
        }

        if let Some(ref psn) = config_file.psn {
            config.psn_id = psn.id.clone();
            config.psn_refresh_token = psn.refresh_token.clone();
        }

        {
            let ref mut title_settings = config.title_settings;
            for pair in config_file.title_settings.iter().flat_map(|ref x| x.iter()) {
                title_settings.insert(pair.0.clone(), PresenceMonitorConfig::convert_title_setting(pair.1));
            }
        }

        Ok(config)
    }

    fn convert_title_setting(string: &str) -> TitleSetting {
        match string {
            "ignore" => TitleSetting::Ignore,
            "name-only" => TitleSetting::NameOnly,
            _ => TitleSetting::Full,
        }
    }
}
