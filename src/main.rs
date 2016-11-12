#[macro_use]
extern crate hyper;

#[macro_use]
extern crate quick_error;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

extern crate clap;
extern crate serde_json;
extern crate env_logger;
extern crate discord;
extern crate rpassword;

mod xbl;
mod psn;
mod sigint;
mod config;

use std::io::{self, Write};
use std::error;
use std::thread;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Condvar, Mutex};
use std::collections::HashMap;
use std::time::Duration;
use discord::model::Game;
use clap::{Arg, App, SubCommand};
use config::{PresenceMonitorConfig, TitleSetting};
use std::any::TypeId;

#[derive(Debug)]
struct PresenceDetail {
    device: String,
    game: String,
    extended_info: Option<String>,
}

type Presence = Option<PresenceDetail>;

struct PresenceProviderType {
    id: TypeId,
    name: &'static str,
}

trait PresenceProvider: std::marker::Send {
    fn get_presence(&mut self) -> Result<Presence, Box<error::Error>>;
    fn provider_type(&self) -> PresenceProviderType;
}

struct PresenceMonitor {
    config: PresenceMonitorConfig,
    last_status: Option<String>,
    discord: discord::Discord,
    last_statuses: HashMap<TypeId, Option<String>>,
}

impl PresenceMonitor {
    fn new(config: PresenceMonitorConfig) -> PresenceMonitor {
        PresenceMonitor {
            discord: discord::Discord::new(&config.discord_username, &config.discord_password)
                .unwrap(),
            config: config,
            last_status: None,
            last_statuses: HashMap::new(),
        }
    }

    fn update_loop(update_interval: Duration,
                   mut provider: Box<PresenceProvider>,
                   sender: Sender<(PresenceProviderType, Presence)>,
                   canceller: Arc<Condvar>) {
        debug!("update_loop - {} - start", provider.provider_type().name);

        let dummy_mutex = Mutex::new(0u8);

        loop {
            match provider.get_presence() {
                Err(e) => {
                    error!("update_loop - {} - {}", provider.provider_type().name, e);
                }
                Ok(presence) => {
                    let _ = sender.send((provider.provider_type(), presence));
                }
            }

            let dummy_lock = dummy_mutex.lock().unwrap();
            let _ = (*canceller).wait_timeout(dummy_lock, update_interval).unwrap();
            if sigint::cancelled() {
                debug!("update_loop - {} - exit",
                       provider.provider_type().name);
                return;
            }
        }
    }

    fn spawn_threads(&mut self,
                     canceller: Arc<Condvar>,
                     mut providers: Vec<Box<PresenceProvider>>)
                     -> Receiver<(PresenceProviderType, Presence)> {
        let (sender, receiver) = channel::<(PresenceProviderType, Presence)>();
        for provider in providers.drain(0..) {
            self.last_statuses.insert(provider.provider_type().id, None);
            let update_interval = self.config.update_interval;
            let sender_clone = sender.clone();
            let canceller_clone = canceller.clone();
            thread::spawn(move || {
                PresenceMonitor::update_loop(update_interval,
                                             provider,
                                             sender_clone,
                                             canceller_clone);
            });
        }

        receiver
    }

    fn make_status_string(&self, presence: &Presence) -> Option<String> {
        match *presence {
            None => None,
            Some(ref detail) => {
                let title_setting = match self.config.title_settings.get(&detail.game) {
                    Some(s) => *s,
                    None => TitleSetting::Full,
                };

                if title_setting == TitleSetting::Ignore {
                    info!("Skipping '{}' dues to 'ignore'", detail.game);
                    return None;
                }

                let mut new_status = format!("{}: {}", detail.device, detail.game);
                if let Some(ref extended_info) = detail.extended_info {
                    if title_setting == TitleSetting::NameOnly {
                        info!("Skipping extended info for '{}' due to 'name-only'",
                              detail.game);
                    } else {
                        new_status = format!("{} {}", new_status, extended_info);
                    }
                }

                Some(new_status)
            }
        }
    }

    fn run_loop(&mut self,
                receiver: Receiver<(PresenceProviderType, Presence)>,
                connection: &discord::Connection) {
        for (provider_type, presence) in receiver.iter() {
            let last_status = (*(self.last_statuses.get(&provider_type.id).unwrap())).clone();

            let new_status = self.make_status_string(&presence);
            if last_status != new_status || self.last_status == None {
                let game = match new_status {
                    None => {
                        info!("Clearing status");
                        None
                    }
                    Some(ref s) => {
                        info!("Updating status to '{}'", s);
                        Some(Game::playing(s.clone()))
                    }
                };

                connection.set_game(game);
            } else {
                info!("Status unchanged");
            }

            self.last_statuses.insert(provider_type.id, new_status.clone());
            self.last_status = new_status;
        }
    }

    fn make_providers(&self) -> Vec<Box<PresenceProvider>> {
        let mut providers: Vec<Box<PresenceProvider>> = Vec::new();
        if self.config.xbl_id != "" {
            providers.push(Box::new(xbl::XblPresenceProvider::new(&self.config.xbl_id,
                                                                  &self.config.xbl_api_key)));
        }

        if self.config.psn_id != "" {
            providers.push(Box::new(psn::PsnPresenceProvider::new(&self.config.psn_id,
                                                                  &self.config.psn_refresh_token)));
        }

        providers
    }

    fn run(&mut self, providers: Vec<Box<PresenceProvider>>) {
        let (connection, ready_event) = self.discord.connect().unwrap();
        info!("Discord logged in as {}", ready_event.user.username);
        let canceller = Arc::new(Condvar::new());

        sigint::set_ctrlc_handler(&*canceller);

        let receiver = self.spawn_threads(canceller.clone(), providers);

        self.run_loop(receiver, &connection);

        info!("Cleaning up and resetting status");
        connection.set_game(None);
    }
}

struct DummyProvider;
impl PresenceProvider for DummyProvider {
    fn get_presence(&mut self) -> Result<Presence, Box<error::Error>> {
        Ok(None)
    }

    fn provider_type(&self) -> PresenceProviderType {
        PresenceProviderType {
            id: TypeId::of::<Self>(),
            name: "dummy",
        }
    }
}

fn try_main(config_path: &str) -> Result<(), Box<error::Error>> {
    let config = PresenceMonitorConfig::from_file(config_path)?;
    let mut monitor = PresenceMonitor::new(config);

    monitor.run(monitor.make_providers());
    Ok(())
}

fn get_psn_token() -> Result<(), Box<error::Error>> {
    let mut stdout = io::stdout();
    write!(stdout, "Username: ").unwrap();
    stdout.flush().unwrap();

    let stdin = io::stdin();
    let mut username = String::new();
    stdin.read_line(&mut username).unwrap();

    let password = rpassword::prompt_password_stdout("Password: ").unwrap();

    let (_, refresh_token) = psn::PsnPresenceProvider::perform_login(&username, &password)?;
    println!("{}", refresh_token);
    Ok(())
}

fn main() {
    env_logger::init().unwrap();

    let matches = App::new("discord_console_status")
        .version("1.0")
        .author("Austin Wagner <austinwagner@gmail.com>")
        .about("Lets Discord chat show when you are playing Xbox or Playstation games")
        .arg(Arg::with_name("config")
            .short("c")
            .long("config")
            .value_name("FILE")
            .help("Overrides the default config file path")
            .takes_value(true))
        .subcommand(SubCommand::with_name("get-psn-token")
            .about("Retrieves a refresh token to enter into the configuration file for \
                    connecting to Playstation Network"))
        .get_matches();

    let result =
        if let Some(_) = matches.subcommand_matches("get-psn-token") {
            get_psn_token()
        } else {
            let config = matches.value_of("config").unwrap_or("config.json");
            try_main(&config)
        };

    if let Err(e) = result {
        error!("{}", e);
    }
}
