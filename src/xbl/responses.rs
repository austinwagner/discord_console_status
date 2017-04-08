#[derive(Deserialize, Debug)]
pub struct Activity {
    #[serde(rename = "richPresence")]
    pub rich_presence: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct Title {
    pub name: String,
    pub placement: String,
    pub state: String,
    pub activity: Option<Activity>,
}

#[derive(Deserialize, Debug)]
pub struct Device {
    #[serde(rename = "type")]
    pub name: String,
    pub titles: Vec<Title>,
}

#[derive(Deserialize, Debug)]
pub struct Presence {
    pub error_code: Option<i32>,
    pub error_message: Option<String>,
    pub devices: Option<Vec<Device>>,
    pub state: Option<String>,
}
