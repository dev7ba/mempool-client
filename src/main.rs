use anyhow::{anyhow, Context};
use bitcoincore_rpc::RpcApi;
use bitcoincore_rpc::{Auth, Client};
use bytes::Buf;
use bytes::Bytes;
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest;
use reqwest::Result;
use settings::{BitcoindClient, Settings};
use std::env;
use std::path::PathBuf;

mod settings;

#[tokio::main]
async fn main() -> Result<()> {
    if let Some(url) = env::args().nth(1) {
        println!("url: {}", url);
        if let Ok(settings) = get_settings() {
            println!("Settings good");
            if let Ok(bcc) = get_client(&settings) {
                println!("Client ok");
                do_get(url, bcc).await?;
            } else {
                println!("Client bad");
            }
        } else {
            println!("Settings bad");
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
    println!("{:#?}", &settings);

    Ok(settings.bitcoind_client)
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

async fn do_get(url: String, bcc: Client) -> Result<u64> {
    let mut stream = reqwest::get(url).await?.bytes_stream();
    let mut buf = Bytes::new();
    let mut tx_size: u32 = 0;
    let mut size_hint: u32 = 0;
    let mut mp_counter: u64 = u64::MAX;
    let mut opb: Option<ProgressBar> = Option::None;
    let mut err_vec: Vec<String> = Vec::new();

    while let Some(item) = stream.next().await {
        let chunk = item?;
        let buf2 = buf.copy_to_bytes(buf.remaining());
        let mut buf2 = buf2.chain(chunk);
        buf = buf2.copy_to_bytes(buf2.remaining());

        loop {
            if size_hint == 0 && buf.len() as u32 >= u32::BITS / 8 {
                size_hint = buf.get_u32(); //Big endian
                println!("Size hint: {}", size_hint);
                let style = ProgressStyle::with_template(
                    "[{elapsed_precise}] {wide_bar} {pos:>7}/{len:7} ",
                )
                .unwrap();
                opb = Some(ProgressBar::new(size_hint as u64).with_style(style));
            }
            if mp_counter == u64::MAX && buf.len() as u32 >= u64::BITS / 8 {
                mp_counter = buf.get_u64(); //Big endian
                println!("Mempool counter: {}", mp_counter);
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
    err_vec.iter().for_each(|err| println!("{}", err));

    Ok(mp_counter)
}
