#[derive(Deserialize, Debug)]
pub struct SsoCookie {
    pub npsso: String,
}

#[derive(Deserialize, Debug)]
pub struct Authorization {
    pub access_token: Option<String>,
    pub account_uuid: Option<String>,
    pub expires_in: Option<i32>,
    pub id_token: Option<String>,
    pub scope: Option<String>,
    pub token_type: Option<String>,
    pub refresh_token: Option<String>,
    pub error_code: Option<i32>,
    pub error_description: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ProfileWrapper {
    pub profile: Profile,
}

#[derive(Deserialize, Debug)]
pub struct Profile {
    pub presences: Vec<Presence>,
}

#[derive(Deserialize, Debug)]
pub struct Presence {
    #[serde(rename = "onlineStatus")]
    pub online_status: String,
    pub platform: Option<String>,
    #[serde(rename = "titleName")]
    pub title_name: Option<String>,
}
