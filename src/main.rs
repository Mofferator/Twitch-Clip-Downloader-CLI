use clap::{arg, command, Parser, Subcommand};
use dateparser::parse;
use indicatif::MultiProgress;
use serde::{Deserialize, Serialize};
use twdl::{download_clips, get_video_source_files};
use twitch_api::{twitch_oauth2::AppAccessToken, types::{Timestamp, TimestampRef, UserId}};
use std::{borrow::Cow, path::PathBuf, process, str::FromStr, sync::Arc};
use regex::Regex;
use tokio::fs::read;
use log::{error, info};

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

        #[arg(short = 's', long = "start", help = "Start of datetime range (If no end provided, defaults to 1 week)")]
        start_timestamp: Option<String>,

        #[arg(short = 'e', long = "end", help = "End of datetime range, requires a start time")]
        end_timestamp: Option<String>,

        #[arg(short = 'C', long = "chunk-size", help = "Number of clips fetched per page, default=20 max=100")]
        chunk_size: Option<usize>
    }
}

#[derive(Deserialize, Serialize, Debug)]
struct TwitchCredentials {
    client_id: String,
    client_secret: String
}

fn exit_with_error_msg(msg: &str, code: Option<i32>) -> ! {
    error!("{msg}");
    process::exit(code.unwrap_or(1));
}

fn output_file_or_cwd(output_file: &Option<String>, slug: &String) -> Result<PathBuf, std::convert::Infallible> {
    match output_file {
        Some(path) => PathBuf::from_str(path),
        None => PathBuf::from_str(&format!("./{slug}.mp4"))
    }
}

async fn login_or_id(id: &Option<u32>, login: &Option<String>, token: &AppAccessToken) -> UserId {
    match (id, login) {
        (None, None) => {
            error!("Either broadcaster login or id is required");
            process::exit(1);
        }
        (Some(id), _) => id.to_string().into(),
        (None, Some(login)) => {
            match twdl::twitch_utils::get_broadcaster_id(login, token).await {
                Ok(Some(id)) => id,
                _ => {
                    error!("Error finding user with that login");
                    process::exit(1);
                }
            }
        }
    }
}

fn interpret_datetimes(start: Option<String>, end: Option<String>) 
    -> (Option<Cow<'static, TimestampRef>>, Option<Cow<'static, TimestampRef>>) {
        if let (None, Some(_)) = (&start, &end) {
            error!("Start datetime must be provided with end datetime");
            process::exit(1); 
        } else {
            fn interpret_date(date: Option<String>) -> Option<Cow<'static, TimestampRef>> {
                match date {
                    Some(s) => match parse(&s) {
                        Ok(date) => {
                            // Convert into owned `Timestamp`
                            let owned: Timestamp = match Timestamp::from_str(&date.to_rfc3339()) {
                                Ok(ts) => ts,
                                Err(err) 
                                    => exit_with_error_msg(&format!("Failed to interpret datetime: {err}"), Some(1)),
                            };
                            Some(Cow::Owned(owned))
                        }
                        Err(err) => exit_with_error_msg(&format!("Failed to interpret datetime: {err}"), Some(1)),
                    },
                    None => None,
                }
            }
            (interpret_date(start), interpret_date(end))
        }
}

async fn handle_clip_subcommand(output: Option<String>, clip: String) {
    let re = Regex::new(r"(?:https?://(?:www\.)?twitch\.tv/[^/]+/clip/|https?://clips\.twitch\.tv/)?([A-Za-z0-9_-]+)")
        .expect("Failed to parse regex string");

    let slug = if let Some(caps) = re.captures(&clip) {
        if let Some(m) = caps.get(1) {
            m.as_str().to_string()
        } else {
            error!("No clip slug found in URL");
            process::exit(1);
        }
    } else {
        error!("Invalid Clip URL format");
        process::exit(1);
    };

    let path = match output_file_or_cwd(&output, &slug) {
        Ok(path) => path,
        Err(err) => {
            error!("Invalid path: {err}");
            return;
        }
    };
    let files = match get_video_source_files(&slug).await {
        Ok(files) => files,
        Err(_) => {
            error!("Failed to get clips for slug {slug}");
            return;
        }
    };

    let best = match files.iter().max() {
        Some(best) => best,
        None => {
            error!("No Source files found");
            return;
        }
    };

    twdl::download_file(best.url.clone(), &path).await;

}

async fn handle_channel_subcommand(credentials: String, output: String,
                                    broadcaster_id: Option<u32>, broadcaster_login: Option<String>,
                                    start_timestamp: Option<String>, end_timestamp: Option<String>,
                                    chunk_size: Option<usize>, multi: Arc<MultiProgress>) -> () {
    let path = match PathBuf::from_str(&credentials) {
        Ok(path) => path,
        Err(_) => {
            error!("Invalid credentials path: {credentials}");
            process::exit(1);
        }
    };
    let contents = match read(path).await {
        Ok(contents) => {
            match String::from_utf8(contents) {
                Ok(str) => str,
                Err(_) => {
                    error!("Failed to interpret creds file as text");
                    process::exit(1);
                }
            }
        }
        Err(_) => {
            error!("Failed to read from credentials file");
            process::exit(1);
        }
    };
    let creds: TwitchCredentials = match serde_json::from_str(&contents) {
        Ok(creds) => creds,
        Err(err) => {
            error!("json file has invalid formatting: {err}");
            process::exit(1);
        }
    };
    let token = match twdl::twitch_utils::get_token(&creds.client_id, &creds.client_secret).await {
        Ok(token) => token,
        Err(err) => {
            error!("Failed to fetch application token: {err}");
            process::exit(1);
        }
    };
    let id = login_or_id(&broadcaster_id, &broadcaster_login, &token).await;
    let (start, end) = interpret_datetimes(start_timestamp, end_timestamp);
    let output_path = match PathBuf::from_str(&output) {
        Ok(path) => path,
        Err(_) => {
            error!("Invalid path");
            process::exit(1);
        }
    };
    let clips = match twdl::twitch_utils::get_clips(multi.clone(), &id, &token, start, end, Some(100)).await {
        Ok(clips) => clips,
        Err(err) => {
            error!("Failed to fetch clips: {err}");
            process::exit(1);
        }
    };
    info!("Fetched {} clips, starting download", clips.len());
    download_clips(multi, clips, &output_path, chunk_size.unwrap_or(10), false).await;

}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let multi = Arc::new(MultiProgress::new());

    {
        let multi_for_logs = multi.clone();
        env_logger::Builder::new()
            .format(move |buf, record| {
                let ts = buf.timestamp();
                let msg = format!("{} [{}] {}", ts, record.level(), record.args());
                multi_for_logs.println(msg).unwrap();
                Ok(())
            })
            .filter_level(log::LevelFilter::Info)
            .init();
    }

    match args.command {
        Commands::Clip { output, clip } => {
            handle_clip_subcommand(output, clip).await
        }
        Commands::Channel { credentials, output, broadcaster_id, 
            broadcaster_name, start_timestamp, end_timestamp, 
            chunk_size } => {
            handle_channel_subcommand(credentials, output, broadcaster_id, 
                broadcaster_name, start_timestamp, end_timestamp, 
                chunk_size, multi).await
        }
    }

}