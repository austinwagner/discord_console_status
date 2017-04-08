use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct ConfigFile {
    pub discord_token: String,
    pub title_settings: Option<HashMap<String, String>>,
    pub update_interval: Option<u64>,
}
