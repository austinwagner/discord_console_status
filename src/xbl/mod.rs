extern crate serde_json;

mod responses;

use hyper::client::Client as HttpClient;
use hyper::header::Headers;
use std::io::Read;
use std::iter::Iterator;
use std::any::TypeId;
use PresenceProvider;
use Presence;
use PresenceDetail;
use PresenceProviderType;

use std::io;
use std::error;
use hyper;

header! { (XAuth, "X-AUTH") => [String] }

const BASE_URL: &'static str = "https://xboxapi.com";

pub struct XblPresenceProvider {
    xbl_id: String,
    api_key: String,
}

quick_error! {
    #[derive(Debug)]
    pub enum XblError {
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
            display("Xbox API error {}: {}", code, msg)
        }
        MissingField(name: &'static str) {
            description("missing response field")
            display("Response missing field: {}", name)
        }
    }
}

impl XblPresenceProvider {
    fn get(&self, url: &str) -> Result<responses::Presence, XblError> {
        info!("Requesting data from Xbox API");

        let mut headers = Headers::new();
        headers.set(XAuth(self.api_key.clone()));

        let client = HttpClient::new();
        let req = client.get(url);
        let req = req.headers(headers);

        let mut resp = req.send()?;
        let mut json = String::new();
        resp.read_to_string(&mut json)?;

        debug!("Xbox API response: {}", json);

        let presence: responses::Presence = serde_json::from_str(&json)?;
        match presence.error_code {
            None => Ok(presence),
            Some(e) => Err(XblError::Api(e, presence.error_message.unwrap_or("Unknown error".to_owned()))),
        }
    }

    pub fn new(xbl_id: &str, api_key: &str) -> XblPresenceProvider {
        XblPresenceProvider {
            xbl_id: xbl_id.to_owned(),
            api_key: api_key.to_owned(),
        }
    }
}

impl PresenceProvider for XblPresenceProvider {
    fn provider_type(&self) -> PresenceProviderType {
        PresenceProviderType {
            id: TypeId::of::<Self>(),
            name: "xbl",
        }
    }

    fn get_presence(&mut self) -> Result<Presence, Box<error::Error>> {
        let presence_url = format!("{}/v2/{}/presence", BASE_URL, self.xbl_id);
        let resp = self.get(&presence_url)?;

        let devices = resp.devices.unwrap_or(Vec::new());
        if resp.state.ok_or(XblError::MissingField("state"))? != "Online" || devices.len() == 0 {
            return Ok(None);
        }

        let ref device = devices[0];
        let ref titles = device.titles;
        let title = match titles.into_iter()
            .find(|ref x| x.placement != "Background" && x.state == "Active") {
            Some(s) => s,
            None => return Ok(None),
        };

        let rich_presence = match title.activity {
            Some(ref s) => {
                match s.rich_presence {
                    Some(ref r) => Some(r.clone()),
                    None => None,
                }
            }
            None => None,
        };

        let device_type = match device.name.as_ref() {
            "XboxOne" => "XB1".to_owned(),
            "Xbox360" => "360".to_owned(),
            _ => device.name.to_owned(),
        };

        Ok(Some(PresenceDetail {
            device: device_type,
            game: title.name.clone(),
            extended_info: rich_presence,
        }))
    }
}
