use anyhow::{anyhow, Context, Result};
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth, Client};
use bytes::Buf;
use bytes::Bytes;
use colored::*;
use config::ConfigError;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest;
use settings::{BitcoindClient, Settings};
use std::env;
use std::path::PathBuf;
use url::Url;

mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    let mut url: Url = Url::parse("https://mempoolexplorer.com").unwrap();
    if let Some(urlp) = env::args().nth(1) {
        if urlp == "--help" || urlp == "-h" {
            print_cmd_help();
            return Ok(());
        } else {
            url = Url::parse(urlp.as_str())?;
        }
    }
    let mut perr = false;
    if let Some(print_err) = env::args().nth(2) {
        if print_err == "-e" || print_err == "--errors" {
            perr = true;
        } else {
            println!("Unknown option: {}", print_err);
            print_cmd_help();
        }
    }
    match check_settings(Settings::new()) {
        Ok(settings) => match check_client(get_client(&settings.bitcoind_client)) {
            Ok(bcc) => {
                let mut err_vec: Vec<String> = Vec::new();
                let mut last_mpc = do_get(&url, &bcc, &mut err_vec).await?;
                println!("Now inserting additional transactions received by the server while sending data...");
                loop {
                    let mpc = do_get_from(&url, &bcc, &last_mpc, &mut err_vec).await?;
                    if mpc == last_mpc || mpc == u64::MAX {
                        break;
                    }
                    last_mpc = mpc;
                }
                if perr {
                    println!("Errors inserting txs: #{}", err_vec.len());
                    err_vec.iter().for_each(|err| println!("{}", err));
                }
                println!("Finished loading transactions into mempool.");
                Ok(())
            }
            Err(e) => {
                print_client_error_advice(e);
                Ok(())
            }
        },
        Err(e) => {
            println!(
                "{}{}",
                "Error, cannot load all necessary settings from config.toml: ".red(),
                e.to_string().red()
            );
            print_conf_toml_template();
            Ok(())
        }
    }
}

fn print_cmd_help() {
    println!("Usage: mempool-client url [options]\n");
    println!("Options:");
    println!(
        "{}{}{}{}",
        "  -e".dimmed(),
        ", ",
        "--errors".dimmed(),
        ": prints errors while inserting txs into bitcoind node.\n"
    );
    println!("Default url: https://mempoolexplorer.com\n");
}

fn check_client(res: Result<Client>) -> Result<Client> {
    match res {
        Ok(client) => match client.get_mempool_info() {
            Ok(_) => Ok(client),
            Err(err) => Err(anyhow!("Cannot access to bitcoind node, Error: {}", err)),
        },
        Err(err) => Err(err),
    }
}

// Checks if optional settings are ok since library can't do it for us.
fn check_settings(res: Result<Settings, ConfigError>) -> Result<Settings, ConfigError> {
    match res {
        Ok(settings) => {
            if settings.bitcoind_client.cookie_auth_path.is_some() {
                Ok(settings)
            } else if settings.bitcoind_client.user.is_some()
                && settings.bitcoind_client.passwd.is_some()
            {
                Ok(settings)
            } else {
                Err(ConfigError::NotFound(
                    "cookie_auth_path or user & password".to_string(),
                ))
            }
        }
        Err(e) => Err(e),
    }
}

fn get_client(bcc: &BitcoindClient) -> anyhow::Result<Client, anyhow::Error> {
    let client = if let Some(path) = &bcc.cookie_auth_path {
        get_client_cookie(&bcc.ip_addr, path.clone())?
    } else {
        get_client_user_passw(
            &bcc.ip_addr,
            bcc.user.as_ref().unwrap().clone(),
            bcc.passwd.as_ref().unwrap().clone(),
        )?
    };
    Ok(client)
}

fn get_client_cookie(ip: &str, path: PathBuf) -> anyhow::Result<Client> {
    Client::new(ip, Auth::CookieFile(path))
        .with_context(|| format!("Can't connect to bitcoind node: {}", ip))
}

fn get_client_user_passw(ip: &str, user_name: String, passwd: String) -> anyhow::Result<Client> {
    Client::new(ip, Auth::UserPass(user_name, passwd))
        .with_context(|| format!("Can't connect to bitcoind node: {}", ip))
}

fn print_client_error_advice(e: anyhow::Error) {
    println!(
        "{}{}{}",
        "\nClient error: ".red().bold(),
        e.to_string().red().bold(),
        "\nCreate or verify client address and authentication method in config.toml file"
            .red()
            .bold(),
    );
    print_conf_toml_template();
    println!(
        "Be aware that you may have to check ~/.bitcoin/bitcoin.conf file to add these fields:"
    );
    println!(
        "{}",
        "
#Your interface for rpc calls to bitcoind
rpcbind=ip_address_here
# Choose between cookieauthpath or user & passwd authentication. Default is cookieauthpath.
rpccookiefile=/home/your_user/.bitcoin/.cookie
rpcuser=your_user
rpcpassword=your_password
#Ip address of machine executing mempool-client
rpcallowip=ip_address_here\n"
            .dimmed()
    );
    println!(
        "Check {} for more info.\n",
        "https://github.com/dev7ba/mempool-client/blob/master/README.md"
            .blue()
            .dimmed()
    );
}

fn print_conf_toml_template() {
    println!("\nExample of config.toml file (in same path as this executable):");
    println!(
        "{}",
        "\
# File comments starts with #
[bitcoindclient]
    # Choose between cookieauthpath or user & passwd authentication.
    # cookieauthpath takes precedence over user & passwd.
    cookieauthpath = \"/home/myuser/.bitcoin/.cookie\" 
    #user = \"my_user\"
    #passwd = \"my_passwd\"
    # Ip address where bitcoind instance is running, localhost by default.
    ipaddr = \"localhost\"
        "
        .dimmed()
    );
}

async fn do_get(url_base: &Url, bcc: &Client, err_vec: &mut Vec<String>) -> Result<u64> {
    let url = url_base
        .join("/mempoolServer/txsdata")
        .context("Error parsing url")?;
    println!("Getting data from: {}", url_base);
    let mut stream = reqwest::get(url.to_string()).await?.bytes_stream();
    let mut buf = Bytes::new();
    let mut magic: u64 = 0;
    let mut tx_size: u32 = 0;
    let mut size_hint: u32 = 0;
    let mut mp_counter: u64 = u64::MAX;
    let mut opb: Option<ProgressBar> = Option::None;

    while let Some(item) = stream.next().await {
        let chunk = item?;
        let buf2 = buf.copy_to_bytes(buf.remaining());
        let mut buf2 = buf2.chain(chunk);
        buf = buf2.copy_to_bytes(buf2.remaining());

        loop {
            if magic == 0 && buf.len() as u32 >= u64::BITS / 8 {
                magic = buf.get_u64();
                if magic != u64::MAX {
                    return Err(anyhow!("Invalid stream, check url."));
                }
            }
            if size_hint == 0 && buf.len() as u32 >= u32::BITS / 8 {
                size_hint = buf.get_u32(); //Big endian
                let style = ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos:>7}/{len:7} ",
                )
                .unwrap();
                opb = Some(ProgressBar::new(size_hint as u64).with_style(style));
            }
            if mp_counter == u64::MAX && buf.len() as u32 >= u64::BITS / 8 {
                mp_counter = buf.get_u64(); //Big endian
            }
            if tx_size == 0 {
                if buf.len() as u32 >= u32::BITS / 8 {
                    tx_size = buf.get_u32(); //Big endian
                } else {
                    break;
                }
            } else {
                if buf.remaining() as u32 >= tx_size {
                    let tx = buf.copy_to_bytes(tx_size as usize);
                    match bcc.send_raw_transaction(tx.chunk()) {
                        Ok(_) => (),
                        Err(err) => {
                            err_vec.push(err.to_string());
                        }
                    }
                    tx_size = 0;
                    if let Some(pb) = &opb {
                        pb.inc(1);
                    }
                } else {
                    break;
                }
            }
        }
    }
    if let Some(pb) = &opb {
        pb.finish();
    }

    Ok(mp_counter)
}

async fn do_get_from(
    url_base: &Url,
    bcc: &Client,
    from: &u64,
    err_vec: &mut Vec<String>,
) -> Result<u64> {
    let urlstr = format!("/mempoolServer/txsdatafrom/{}", from);
    let url = url_base.join(&urlstr).context("Error parsing url")?;
    println!("Getting data from the mempool sequence: {}", from);

    let mut stream = reqwest::get(url).await?.bytes_stream();

    let mut buf = Bytes::new();
    let mut magic: u64 = 0;
    let mut tx_size: u32 = 0;
    let mut mp_counter: u64 = u64::MAX;

    while let Some(item) = stream.next().await {
        let chunk = item?;
        let buf2 = buf.copy_to_bytes(buf.remaining());
        let mut buf2 = buf2.chain(chunk);
        buf = buf2.copy_to_bytes(buf2.remaining());

        loop {
            if magic == 0 && buf.len() as u32 >= u64::BITS / 8 {
                magic = buf.get_u64();
                if magic != u64::MAX {
                    return Err(anyhow!("Invalid stream, check url."));
                }
            }
            if mp_counter == u64::MAX && buf.len() as u32 >= u64::BITS / 8 {
                mp_counter = buf.get_u64(); //Big endian
            }
            if tx_size == 0 {
                if buf.len() as u32 >= u32::BITS / 8 {
                    tx_size = buf.get_u32(); //Big endian
                } else {
                    break;
                }
            } else {
                if buf.remaining() as u32 >= tx_size {
                    let tx = buf.copy_to_bytes(tx_size as usize);
                    match bcc.send_raw_transaction(tx.chunk()) {
                        Ok(_) => (),
                        Err(err) => {
                            err_vec.push(err.to_string());
                        }
                    }
                    tx_size = 0;
                } else {
                    break;
                }
            }
        }
    }

    Ok(mp_counter)
}
