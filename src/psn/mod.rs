extern crate regex;
extern crate select;

mod responses;

use hyper::client::Client as HttpClient;
use hyper::header::{Headers, UserAgent, Origin, ContentType, ContentLength, Authorization,
                    CacheControl, Bearer, Cookie, CookiePair, Location, Host, SetCookie,
                    CacheDirective};
use hyper::mime::{Mime, TopLevel, SubLevel};
use std::io::Read;
use std::iter::Iterator;
use std::any::TypeId;
use serde_hjson::Value as HJsonValue;

use HJsonObject;
use PresenceProvider;
use Presence;
use PresenceDetail;
use PresenceProviderType;
use serde_json;

use std::io;
use std::error;
use std::fmt::Write;
use hyper;

header! { (XRequestedWith, "X-Requested-With") => [String] }

const BASE_URL: &'static str = "https://auth.api.sonyentertainmentnetwork.com";
const SERVICE_ENTITY: &'static str = "urn:service-entity:psn";
const STATE: &'static str = "x";
const REDIRECT_URL: &'static str = "com.scee.psxandroid.scecompcall://redirect";
const CLIENT_ID1: &'static str = "71a7beb8-f21a-47d9-a604-2e71bee24fe0";
const CLIENT_ID2: &'static str = "b0d0d7ad-bb99-4ab1-b25e-afa0c76577b0";
const CLIENT_SECRET1: &'static str = "xSk2YI8qJqZfeLQv";
const CLIENT_SECRET2: &'static str = "Zo4y8eGIa3oazIEp";
const DUID: &'static str = "00000007000801a800000000000000ace468c6910815113a20202020476f6f676c653a2020202020506978656c00000000000000000000000000000000";
const REQUESTED_WITH: &'static str = "com.scee.psxandroid";

const SCOPES1: &'static str =
    "openid user:account.core.get user:account.languages.get kamaji:get_account_hash \
     user:account.address.create user:account.address.update user:account.address.get \
     user:account.communication.update user:account.communication.get user:account.onlineId.update";
const SCOPES2: &'static str = "psn:sceapp,user:account.get,user:account.settings.privacy.get,user:\
                               account.settings.privacy.update,user:account.realName.get,user:\
                               account.realName.update,kamaji:get_account_hash,kamaji:ugc:\
                               distributor,oauth:manage_device_usercodes,kamaji:game_list,kamaji:\
                               get_internal_entitlements,capone:report_submission";
lazy_static! {
    static ref USER_AGENT: String = format!(
        "Mozilla/5.0 (Linux; U; Android 4.3; {0}; C6502 Build/10.4.1.B.0.101) AppleWebKit/534.30 (KHTML, like Gecko) Version/4.0 Mobile Safari/534.30 PlayStation App/2.55.8/{0}/{0}",
        "en");
}

lazy_static! {
    static ref CODE_REGEX: regex::Regex = regex::Regex::new(r"code=(.{6})").unwrap();
}

pub struct PsnPresenceProvider {
    psn_id: String,
    refresh_token: String,
    access_token: String,
}

quick_error! {
    #[derive(Debug)]
    pub enum PsnError {
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
        Http(err: hyper::Error) {
            from()
            description("http error")
            display("HTTP error: {}", err)
            cause(err)
        }
        Api(code: i32, msg: String) {
            description("xbox api error")
            display("PSN API error {}: {}", code, msg)
        }
        InvalidResponse(msg: &'static str) {
            description("invalid api response")
            display("Invalid API response: {}", msg)
        }
        MissingField(name: &'static str) {
            description("missing response field")
            display("Response missing field: {}", name)
        }
    }
}


fn urlencode(string: &str) -> String {
    let mut result = String::new();
    for c in string.bytes() {
        if ('0' as u8 <= c && c <= '9' as u8) || ('a' as u8 <= c && c <= 'z' as u8) ||
           ('A' as u8 <= c && c <= 'Z' as u8) || c == '.' as u8 || c == '-' as u8 ||
           c == '*' as u8 || c == '_' as u8 {
            write!(&mut result, "{}", c as char).unwrap();
        } else {
            write!(&mut result, "%{:X}", c).unwrap();
        }
    }

    result
}

fn make_url_query(data: &[(&str, &str)]) -> String {
    data.iter()
        .map(|ref x| format!("{}={}", urlencode(x.0), urlencode(x.1)))
        .collect::<Vec<_>>()
        .join("&")
}


impl PsnPresenceProvider {
    fn default_headers() -> Headers {
        let mut headers = Headers::new();
        headers.set(UserAgent(USER_AGENT.to_owned()));
        headers.set(Origin {
            scheme: "https".to_owned(),
            host: Host {
                hostname: "id.sonyentertainmentnetwork.com".to_owned(),
                port: None,
            },
        });
        headers.set(XRequestedWith(REQUESTED_WITH.to_owned()));
        headers
    }

    fn default_post_headers(body: &str) -> Headers {
        let mut headers = PsnPresenceProvider::default_headers();
        headers.set(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, vec![])));
        headers.set(ContentLength(body.len() as u64));
        headers
    }

    pub fn new(psn_id: &str, refresh_token: &str) -> PsnPresenceProvider {
        PsnPresenceProvider {
            psn_id: psn_id.to_owned(),
            refresh_token: refresh_token.to_owned(),
            access_token: "".to_owned(),
        }
    }

    pub fn from_config(config: &HJsonObject) -> Option<PsnPresenceProvider> {
        let psn_obj = json!(opt!(config.get("psn")), HJsonValue::Object);
        let id = json!(opt!(psn_obj.get("id")), HJsonValue::String);
        let refresh_token = json!(opt!(psn_obj.get("refresh_token")), HJsonValue::String);

        Some(PsnPresenceProvider::new(&id, &refresh_token))
    }

    pub fn refresh(&mut self) -> Result<(), PsnError> {
        self.access_token = "".to_owned();
        let mut client = HttpClient::new();
        let tokens = PsnPresenceProvider::refresh_access_token(&mut client, &self.refresh_token)?;
        self.access_token = tokens.0;
        self.refresh_token = tokens.1;
        Ok(())
    }

    fn get_profile(&mut self) -> Result<responses::Profile, PsnError> {
        info!("Requesting presence data from PSN");
        let mut headers = Headers::new();
        headers.set(UserAgent(USER_AGENT.to_owned()));
        headers.set(Origin {
            scheme: "http".to_owned(),
            host: Host {
                hostname: "psapp.dl.playstation.net".to_owned(),
                port: None,
            },
        });
        headers.set(XRequestedWith(REQUESTED_WITH.to_owned()));
        headers.set(Authorization(Bearer { token: self.access_token.clone() }));
        headers.set(CacheControl(vec![CacheDirective::NoCache]));

        // there's a good chance the request will work without most of these
        // should check at some point
        let data = make_url_query(&[("fields", "presences(@titleInfo)"),
                                    ("avatarSizes", "m"),
                                    ("profilePictureSizes", "m"),
                                    ("languagesUsedLanguageSet", "set3"),
                                    ("psVitaTitleIcon", "circled"),
                                    ("titleIconSize", "s")]);

        let url = format!("https://us-prof.np.community.playstation.\
                           net/userProfile/v1/users/{}/profile2?{}",
                          self.psn_id,
                          data);

        let client = HttpClient::new();
        let req = client.get(&url)
            .headers(headers);

        let mut resp = req.send()?;
        let mut json = String::new();
        resp.read_to_string(&mut json)?;

        debug!("{}", json);

        let profile_wrapper = serde_json::from_str::<responses::ProfileWrapper>(&json)?;
        Ok(profile_wrapper.profile)
    }

    fn request_ssocookie(client: &mut HttpClient,
                         username: &str,
                         password: &str)
                         -> Result<Vec<CookiePair>, PsnError> {
        info!("Requesting SSO cookie from PSN");
        let url = format!("{}/2.0/ssocookie", BASE_URL);
        let data = make_url_query(&[("authentication_type", "password"),
                                    ("username", username),
                                    ("password", password),
                                    ("client_id", CLIENT_ID1)]);
        let headers = PsnPresenceProvider::default_post_headers(&data);

        let req = client.post(&url)
            .headers(headers)
            .body(&data);

        let mut resp = req.send()?;

        let mut resp_body = String::new();
        resp.read_to_string(&mut resp_body)?;
        debug!("{}", resp_body);

        if let Ok(err) = serde_json::from_str::<responses::GenericError>(&resp_body) {
            if let Some(error_code) = err.error_code {
                return Err(PsnError::Api(error_code, err.error_description.unwrap_or("Unknown error".to_owned())));
            }
        }

        match resp.headers.get::<SetCookie>() {
            Some(s) => Ok(s.0.clone()),
            None => Err(PsnError::InvalidResponse("Missing response cookie.")),
        }
    }

    fn exchange_ssocookie_for_access_token(client: &mut HttpClient,
                                           cookies: &Vec<CookiePair>)
                                           -> Result<String, PsnError> {
        info!("Requesting access token from PSN");
        let url = format!("{}/2.0/oauth/token", BASE_URL);
        let data = make_url_query(&[("grant_type", "sso_cookie"),
                                    ("scope", SCOPES1),
                                    ("client_id", CLIENT_ID1),
                                    ("client_secret", CLIENT_SECRET1)]);
        let mut headers = PsnPresenceProvider::default_post_headers(&data);
        headers.set(Cookie(cookies.clone()));

        let req = client.post(&url)
            .headers(headers)
            .body(&data);

        let mut resp = req.send()?;
        let mut resp_body = String::new();
        resp.read_to_string(&mut resp_body)?;
        debug!("{}", resp_body);
        let authorization: responses::Authorization = serde_json::from_str(&resp_body)?;
        authorization.access_token.ok_or(PsnError::MissingField("access_token"))
    }

    fn request_login_code(client: &mut HttpClient,
                          cookies: &Vec<CookiePair>)
                          -> Result<String, PsnError> {
        info!("Requesting login code from PSN");
        let url = format!("{}/2.0/oauth/authorize", BASE_URL);
        let data = make_url_query(&[("state", STATE),
                                    ("duid", DUID),
                                    ("ui", "pr"),
                                    ("service_entity", SERVICE_ENTITY),
                                    ("service_logo", "ps"),
                                    ("app_context", "inapp_aos"),
                                    ("client_id", CLIENT_ID2),
                                    ("device_base_font_size", "20"),
                                    ("device_profile", "mobile"),
                                    ("redirect_uri", REDIRECT_URL),
                                    ("response_type", "code"),
                                    ("scope", SCOPES2),
                                    ("smcid", "psapp:signin"),
                                    ("support_scheme", "sneiprls"),
                                    ("tp_psn", "true")]);
        let mut headers = PsnPresenceProvider::default_headers();
        headers.set(Cookie(cookies.clone()));

        let url = format!("{}?{}", url, data);
        let req = client.get(&url)
            .headers(headers);

        let resp = req.send()?;
        match resp.headers.get::<Location>() {
            Some(l) => {
                match CODE_REGEX.captures(&l) {
                    Some(c) => Ok(c.at(1).unwrap().to_owned()),
                    None => Err(PsnError::InvalidResponse("Missing login code.")),
                }
            }
            None => Err(PsnError::InvalidResponse("Missing redirect location.")),
        }
    }

    pub fn request_full_token(client: &mut HttpClient,
                              cookies: &Vec<CookiePair>,
                              login_code: &str)
                              -> Result<(String, String), PsnError> {
        info!("Requesting full token from PSN");
        let url = format!("{}/2.0/oauth/token", BASE_URL);
        let data = make_url_query(&[("grant_type", "authorization_code"),
                                    ("client_id", CLIENT_ID2),
                                    ("client_secret", CLIENT_SECRET2),
                                    ("redirect_uri", REDIRECT_URL),
                                    ("scope", SCOPES2),
                                    ("code", login_code),
                                    ("service_entity", SERVICE_ENTITY),
                                    ("duid", DUID)]);

        let mut headers = Headers::new();
        headers.set(UserAgent("com.sony.snei.np.android.sso.share.oauth.versa.USER_AGENT".to_owned()));
        headers.set(Authorization("Basic YjBkMGQ3YWQtYmI5OS00YWIxLWIyNWUtYWZhMGM3NjU3N2IwOlpvNHk4ZUdJYTNvYXpJRXA=".to_owned()));
        headers.set(Cookie(cookies.clone()));
        headers.set(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, vec![])));
        headers.set(ContentLength(data.len() as u64));

        let req = client.post(&url)
            .headers(headers)
            .body(&data);

        let mut resp = req.send()?;
        let mut resp_body = String::new();
        resp.read_to_string(&mut resp_body)?;
        debug!("{}", resp_body);

        PsnPresenceProvider::unpack_authorization(&resp_body)
    }

    pub fn refresh_access_token(client: &mut HttpClient,
                                refresh_token: &str)
                                -> Result<(String, String), PsnError> {
        info!("Refreshing access token from PSN");
        let url = format!("{}/2.0/oauth/token", BASE_URL);
        let data = make_url_query(&[("grant_type", "refresh_token"),
                                    ("client_id", CLIENT_ID2),
                                    ("client_secret", CLIENT_SECRET2),
                                    ("redirect_uri", REDIRECT_URL),
                                    ("scope", SCOPES2),
                                    ("refresh_token", refresh_token),
                                    ("service_entity", SERVICE_ENTITY),
                                    ("duid", DUID)]);

        let mut headers = Headers::new();
        headers.set(UserAgent("com.sony.snei.np.android.sso.share.oauth.versa.USER_AGENT".to_owned()));
        headers.set(Authorization("Basic YjBkMGQ3YWQtYmI5OS00YWIxLWIyNWUtYWZhMGM3NjU3N2IwOlpvNHk4ZUdJYTNvYXpJRXA=".to_owned()));
        headers.set(ContentType(Mime(TopLevel::Application, SubLevel::WwwFormUrlEncoded, vec![])));
        headers.set(ContentLength(data.len() as u64));

        let req = client.post(&url)
            .headers(headers)
            .body(&data);

        let mut resp = req.send()?;
        let mut resp_body = String::new();
        resp.read_to_string(&mut resp_body)?;
        debug!("{}", resp_body);

        PsnPresenceProvider::unpack_authorization(&resp_body)
    }

    fn unpack_authorization(json: &str) -> Result<(String, String), PsnError> {
        let authorization: responses::Authorization = serde_json::from_str(json)?;

        match authorization.error_code {
            Some(e) => Err(PsnError::Api(e, authorization.error_description.unwrap_or("Unknown error".to_owned()))),
            None => Ok((authorization.access_token.ok_or(PsnError::MissingField("access_token"))?,
                        authorization.refresh_token.ok_or(PsnError::MissingField("refresh_token"))?))
        }
    }

    pub fn perform_login(username: &str,
                         password: &str)
                         -> Result<(String, String), Box<error::Error>> {
        let mut client = HttpClient::new();
        client.set_redirect_policy(hyper::client::RedirectPolicy::FollowNone);
        let cookies = PsnPresenceProvider::request_ssocookie(&mut client, &username, &password)?;
        let _ = PsnPresenceProvider::exchange_ssocookie_for_access_token(&mut client, &cookies)?;
        let login_code = PsnPresenceProvider::request_login_code(&mut client, &cookies)?;
        debug!("login_code: {}", login_code);
        let token_pair =
            PsnPresenceProvider::request_full_token(&mut client, &cookies, &login_code)?;
        Ok(token_pair)
    }
}

impl PresenceProvider for PsnPresenceProvider {
    fn provider_type(&self) -> PresenceProviderType {
        PresenceProviderType {
            id: TypeId::of::<Self>(),
            name: "psn",
        }
    }

    fn get_presence(&mut self) -> Result<Presence, Box<error::Error>> {
        let profile = match self.get_profile() {
            Ok(p) => p,
            Err(_) => {
                self.refresh()?;
                self.get_profile()?
            }
        };

        match profile.presences.iter().find(|ref x| x.online_status == "online") {
            None => Ok(None),
            Some(p) => {
                Ok(Some(PresenceDetail {
                    device: p.platform.clone().ok_or(PsnError::MissingField("platform"))?,
                    game: p.title_name.clone().ok_or(PsnError::MissingField("title_name"))?,
                    extended_info: None,
                }))
            }
        }
    }
}
