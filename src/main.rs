use clap::{arg, command, Parser, Subcommand};
use serde::{Deserialize, Serialize};
use twdl::get_video_source_files;
use twitch_api::{twitch_oauth2::AppAccessToken, types::UserId};
use std::{path::PathBuf, process, str::FromStr};
use regex::Regex;
use tokio::fs::read;
use futures_util::future::join_all;

#[derive(Parser, Debug)]
#[command(name = "tw-dl", version, about = "Downloads twitch clips")]
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Clip {
        #[arg(short = 'o', long = "output", help = "Output file to download clip to")]
        output: Option<String>,

        clip: String
    },

    Channel {
        #[arg(short = 'o', long = "output", default_value_t = String::from("."), help = "Path to directory to store the clips")]
        output: String,

        #[arg(short = 'c', long = "credentials", help = "Path to a json file containing client_id and client_secret")]
        credentials: String,

        #[arg(short = 'i', long = "broadcaster-id", help = "Numeric broadcaster ID")]
        broadcaster_id: Option<u32>,

        #[arg(short = 'l', long = "broadcaster-login", help = "Broadcaster login")]
        broadcaster_name: Option<String>,
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TwitchCredentials {
    client_id: String,
    client_secret: String
}

fn output_file_or_cwd(output_file: &Option<String>, slug: &String) -> Result<PathBuf, std::convert::Infallible> {
    match output_file {
        Some(path) => PathBuf::from_str(&path),
        None => PathBuf::from_str(&format!("./{slug}.mp4"))
    }
}

async fn login_or_id(id: &Option<u32>, login: &Option<String>, token: &AppAccessToken) -> UserId {
    match (id, login) {
        (None, None) => {
            eprintln!("Either broadcaster login or id is required");
            process::exit(1);
        }
        (Some(id), _) => id.to_string().into(),
        (None, Some(login)) => {
            match twdl::twitch_utils::get_broadcaster_id(login, token).await {
                Ok(Some(id)) => id,
                _ => {
                    eprintln!("Error finding user with that login");
                    process::exit(1);
                }
            }
        }
    }
}

async fn handle_clip_subcommand(output: Option<String>, clip: String) {
    let re = Regex::new(r"(?:https?://(?:www\.)?twitch\.tv/[^/]+/clip/|https?://clips\.twitch\.tv/)?([A-Za-z0-9_-]+)")
        .expect("Failed to parse regex string");

    let slug = if let Some(caps) = re.captures(&clip) {
        if let Some(m) = caps.get(1) {
            m.as_str().to_string()
        } else {
            eprintln!("No clip slug found in URL");
            process::exit(1);
        }
    } else {
        eprintln!("Invalid Clip URL format");
        process::exit(1);
    };

    let path = match output_file_or_cwd(&output, &slug) {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Invalid path: {err}");
            return;
        }
    };
    let files = match get_video_source_files(&slug).await {
        Ok(files) => files,
        Err(_) => {
            eprintln!("Failed to get clips for slug {slug}");
            return;
        }
    };

    let best = match files.iter().max() {
        Some(best) => best,
        None => {
            eprintln!("No Source files found");
            return;
        }
    };

    twdl::download_file(best.url.clone(), &path).await;

}

async fn handle_channel_subcommand(credentials: String, output: String,
                                    broadcaster_id: Option<u32>, broadcaster_login: Option<String>) -> () {
    let path = match PathBuf::from_str(&credentials) {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Invalid credentials path: {credentials}");
            process::exit(1);
        }
    };
    let contents = match read(path).await {
        Ok(contents) => {
            match String::from_utf8(contents) {
                Ok(str) => str,
                Err(_) => {
                    eprintln!("Failed to interpret creds file as text");
                    process::exit(1);
                }
            }
        }
        Err(_) => {
            eprintln!("Failed to read from credentials file");
            process::exit(1);
        }
    };
    let creds: TwitchCredentials = match serde_json::from_str(&contents) {
        Ok(creds) => creds,
        Err(err) => {
            eprintln!("json file has invalid formatting: {err}");
            process::exit(1);
        }
    };
    let token = match twdl::twitch_utils::get_token(&creds.client_id, &creds.client_secret).await {
        Ok(token) => token,
        Err(err) => {
            eprintln!("Failed to fetch application token: {err}");
            process::exit(1);
        }
    };
    let id = login_or_id(&broadcaster_id, &broadcaster_login, &token).await;
    let output_path = match PathBuf::from_str(&output) {
        Ok(path) => path,
        Err(_) => {
            eprintln!("Invalid path");
            process::exit(1);
        }
    };
    let clips = match twdl::twitch_utils::get_clips(&id, &token).await {
        Ok(clips) => clips,
        Err(err) => {
            eprintln!("Failed to fetch clips: {err}");
            process::exit(1);
        }
    };
    let mut futures = Vec::new();
    for clip in clips {
        futures.push(twdl::download_clip(clip, &output_path));
    }
    join_all(futures).await;

}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::Clip { output, clip } => {
            handle_clip_subcommand(output, clip).await
        }
        Commands::Channel { credentials, output, broadcaster_id, broadcaster_name } => {
            handle_channel_subcommand(credentials, output, broadcaster_id, broadcaster_name).await
        }
    }

}