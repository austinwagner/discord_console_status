mod file;

use std::io;
use std::fs::File;
use std::time::Duration;
use std::collections::HashMap;
use self::file::ConfigFile;
use serde_hjson::Value as HJsonValue;

use HJsonObject;
use serde_hjson;

quick_error! {
    #[derive(Debug)]
    pub enum ConfigError {
        Io(err: io::Error) {
            from()
            description("io error")
            display("I/O error: {}", err)
            cause(err)
        }
        Json(err: serde_hjson::Error) {
            from()
            description("hjson parse error")
            display("HJSON parsing error: {}", err)
            cause(err)
        }
        Invalid {
            description("invalid config")
            display("Invalid config")
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
    pub discord_token: String,
    pub update_interval: Duration,
    pub title_settings: HashMap<String, TitleSetting>,
    pub json: HJsonObject,
}

impl PresenceMonitorConfig {
    pub fn from_file(path: &str) -> Result<PresenceMonitorConfig, ConfigError> {
        let json = {
            let file = File::open(path)?;
            match serde_hjson::from_reader(file)? {
                HJsonValue::Object(o) => o,
                _ => return Err(ConfigError::Invalid),
            }
        };

        let config: ConfigFile = {
            let file = File::open(path)?;
            serde_hjson::from_reader(file)?
        };

        let mut title_settings: HashMap<String, TitleSetting> = HashMap::new();
        for pair in config.title_settings.iter().flat_map(|ref x| x.iter()) {
            title_settings.insert(pair.0.clone(),
                                  PresenceMonitorConfig::convert_title_setting(pair.1));
        }

        Ok(PresenceMonitorConfig {
            discord_token: config.discord_token.clone(),
            update_interval: Duration::from_secs(config.update_interval.unwrap_or(30u64)),
            title_settings: title_settings,
            json: json,
        })
    }

    fn convert_title_setting(string: &str) -> TitleSetting {
        match string {
            "ignore" => TitleSetting::Ignore,
            "name-only" => TitleSetting::NameOnly,
            _ => TitleSetting::Full,
        }
    }
}
