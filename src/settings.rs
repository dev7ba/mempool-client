use config::{Config, ConfigError, File};
use dirs;
use serde::Deserialize;
use std::default::Default;
use std::fmt;
use std::{env, path::PathBuf};

#[derive(Deserialize)]
#[allow(unused)]
pub struct BitcoindClient {
    ///cookie_auth_path takes precedence over user/passwd authentication.
    #[serde(rename = "cookieauthpath")]
    pub cookie_auth_path: Option<PathBuf>,
    #[serde(rename = "ipaddr")]
    pub ip_addr: String,
    pub user: Option<String>,
    pub passwd: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(unused)]
pub struct Settings {
    #[serde(rename = "bitcoindclient")]
    pub bitcoind_client: BitcoindClient,
}

///Settings can be loaded from config.toml file located in the executable directory
/// Note that toml must have have all variable names in lowercase without '_' separators
/// ```
/// [bitcoindclient]
/// 	cookieauthpath = "/home/ba/.bitcoin/.cookie"
///   ipaddr = "localhost"
///   user = "anon"
///   passwd = "anon"
/// ```
impl Settings {
    pub fn new() -> Result<Self, ConfigError> {
        let mut path = env::current_exe().unwrap();
        path.pop();
        path.push("config.toml");
        if path.exists() {
            let s = Config::builder()
                .add_source(File::with_name(path.to_str().unwrap()).required(false))
                .build()?;
            s.try_deserialize()
        } else {
            Ok(Settings::default())
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        let mut path = dirs::home_dir().unwrap();
        path.push(".bitcoin/.cookie");

        Settings {
            bitcoind_client: BitcoindClient {
                cookie_auth_path: Some(path),
                ip_addr: String::from("localhost"),
                user: None,
                passwd: None,
            },
        }
    }
}

//Manually implemented Debug to avoid password leak
impl fmt::Debug for BitcoindClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BitcoindClient")
            .field("cookie_auth_path", &self.cookie_auth_path)
            .field("ip_addr", &self.ip_addr)
            .field("user", &"****")
            .field("passwd", &"****")
            .field("user", &self.user)
            .field("passwd", &self.passwd)
            .finish()
    }
}
