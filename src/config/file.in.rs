use std::collections::HashMap;

#[derive(Deserialize, Debug)]
pub struct ConfigFile {
    pub discord_username: String,
    pub discord_password: String,
    pub xbl: Option<Xbl>,
    pub psn: Option<Psn>,
    pub title_settings: Option<HashMap<String, String>>,
    pub update_interval: Option<u64>,
}

#[derive(Deserialize, Debug)]
pub struct Xbl {
    pub id: String,
    pub api_key: String,
}

#[derive(Deserialize, Debug)]
pub struct Psn {
    pub id: String,
    pub refresh_token: String,
}
