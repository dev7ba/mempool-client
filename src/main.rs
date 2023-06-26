use anyhow::{anyhow, Context, Result};
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth, Client};
use bytes::Buf;
use bytes::Bytes;
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
    if let Some(urlp) = env::args().nth(1) {
        // println!("url: {}", urlp);
        let url = Url::parse(urlp.as_str())?;
        if let Ok(settings) = get_settings() {
            // println!("Settings loaded");
            if let Ok(bcc) = get_client(&settings) {
                // println!("Client Ok");
                let mut err_vec: Vec<String> = Vec::new();
                let mut last_mpc = do_get(&url, &bcc, &mut err_vec).await?;
                // println!("Last mpc: {}", last_mpc);
                loop {
                    let mpc = do_get_from(&url, &bcc, &last_mpc, &mut err_vec).await?;
                    // println!("mpc: {}", mpc);
                    if mpc == last_mpc || mpc == u64::MAX {
                        break;
                    }
                    last_mpc = mpc;
                }
                // println!("Errors inserting txs: #{}", err_vec.len());
                // err_vec.iter().for_each(|err| println!("{}", err));
                println!("Finished loading transactions into mempool.");
            } else {
                println!("Client error, verify client address and authentication method in config.toml file");
            }
        } else {
            println!("Settings error");
        }
    } else {
        println!("Usage: mempool-client url");
    }
    Ok(())
}

fn get_settings() -> anyhow::Result<BitcoindClient> {
    let settings = match Settings::new() {
        Ok(settings) => settings,
        Err(e) => {
            println!("Error, cannot load all necessary settings from config.toml or environment variables: {}",e);
            return Err(anyhow!("error:{}", e));
        }
    };
    // println!("{:#?}", &settings);

    Ok(settings.bitcoind_client)
}

fn get_client(bcc: &BitcoindClient) -> anyhow::Result<Client, anyhow::Error> {
    let client = if let Some(path) = &bcc.cookie_auth_path {
        get_client_cookie(&bcc.ip_addr, path.clone())?
    } else {
        if bcc.user.is_some() && bcc.passwd.is_some() {
            get_client_user_passw(
                &bcc.ip_addr,
                bcc.user.as_ref().unwrap().clone(),
                bcc.passwd.as_ref().unwrap().clone(),
            )?
        } else {
            println!("Configuration error, no cookie_auth_path or user & password");
            return Err(anyhow!(
                "Configuration error, no cookie_auth_path or user & password"
            ));
        }
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

async fn do_get(url_base: &Url, bcc: &Client, err_vec: &mut Vec<String>) -> Result<u64> {
    let url = url_base
        .join("/mempoolServer/txsdata")
        .context("Error parsing url")?;
    println!("Getting data from: {}", url);
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
    println!("Getting data from: {}", url);

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
