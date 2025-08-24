use clap::{arg, command, Parser, Subcommand, Args};
use dateparser::parse;
use futures_util::future::join_all;
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
    Clip(ClipCommandArgs),

    Channel(ChannelCommandArgs)
}

#[derive(Args, Debug)]
struct ClipCommandArgs {
     #[arg(short = 'o', long = "output", default_value_t = String::from("."), help = "Output dir to download clip to")]
    output: String,

    #[arg(short = 'L', long = "link", help = "Skip download and print the source file URL")]
    link: bool,

    #[arg(short = 'm', long = "metadata", help = "Download json metadata alongside the clip")]
    metadata: bool,

    #[arg(short = 'c', long = "credentials", help = "Path to a json file containing client_id and client_secret")]
    credentials: Option<String>,

    clip: String
}

#[derive(Args, Debug)]
struct ChannelCommandArgs {
    #[arg(short = 'o', long = "output", default_value_t = String::from("."), help = "Path to directory to store the clips")]
    output: String,

    #[arg(short = 'c', long = "credentials", help = "Path to a json file containing client_id and client_secret")]
    credentials: String,

    #[arg(short = 'i', long = "broadcaster-id", help = "Numeric broadcaster ID")]
    broadcaster_id: Option<u32>,

    #[arg(short = 'l', long = "broadcaster-login", help = "Broadcaster login")]
    broadcaster_login: Option<String>,

    #[arg(short = 's', long = "start", help = "Start of datetime range (If no end provided, defaults to 1 week)")]
    start_timestamp: Option<String>,

    #[arg(short = 'e', long = "end", help = "End of datetime range, requires a start time")]
    end_timestamp: Option<String>,

    #[arg(short = 'C', long = "chunk-size", help = "Number of clips fetched per page, default=20 max=100")]
    chunk_size: Option<usize>,

    #[arg(short = 'L', long = "link", help = "Skip downloads and print the source file URLs")]
    link: bool,

    #[arg(short = 'm', long = "metadata", help = "Download json metadata alongside the clip")]
    metadata: bool
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
                _ => exit_with_error_msg("Error finding user with that login", Some(1))
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

async fn load_credentials(creds: String) -> TwitchCredentials {
    let path = match PathBuf::from_str(&creds) {
        Ok(path) => path,
        Err(_) => exit_with_error_msg(&format!("Invalid credentials path: {}", creds), Some(1))
    };
    let contents = match read(path).await {
        Ok(contents) => {
            match String::from_utf8(contents) {
                Ok(str) => str,
                Err(_) => exit_with_error_msg("Failed to interpret creds file as text", Some(1))
            }
        }
        Err(_) => exit_with_error_msg("Failed to read from credentials file", Some(1))
    };
    let creds: TwitchCredentials = match serde_json::from_str(&contents) {
        Ok(creds) => creds,
        Err(err) => exit_with_error_msg(&format!("json file has invalid formatting: {err}"), Some(1))
    };
    creds
}

async fn handle_clip_subcommand(args: ClipCommandArgs) {
    let re = Regex::new(r"(?:https?://(?:www\.)?twitch\.tv/[^/]+/clip/|https?://clips\.twitch\.tv/)?([A-Za-z0-9_-]+)")
        .expect("Failed to parse regex string");

    let path = match PathBuf::from_str(&args.output) {
        Ok(path) => path,
        Err(_) => exit_with_error_msg("Invalid output path", Some(1))
    };

    let slug = if let Some(caps) = re.captures(&args.clip) {
        if let Some(m) = caps.get(1) {
            m.as_str().to_string()
        } else {
            exit_with_error_msg("No clip slug found in URL", Some(1))
        }
    } else {
        exit_with_error_msg("Invalid Clip URL format", Some(1))
    };

    let files = match get_video_source_files(&slug).await {
        Ok(files) => files,
        Err(_) => exit_with_error_msg(&format!("Failed to get clips for slug {slug}"), Some(1))
    };

    let best = match files.iter().max() {
        Some(best) => best,
        None => exit_with_error_msg("No Source files found", Some(1))
    };

    if args.link {
        println!("{}", best.url.clone().as_str());
    } else {
        if args.metadata {
            let creds = match args.credentials {
                Some(creds) => load_credentials(creds).await,
                None => exit_with_error_msg("metadata requires twitch credentials to be provided", Some(1))
            };
            let token = match twdl::twitch_utils::get_token(&creds.client_id, &creds.client_secret).await {
                Ok(token) => token,
                Err(_) => {
                    exit_with_error_msg("Failed to fetch token from twitch", Some(1));
                }
            };
            let clip = twdl::twitch_utils::get_clip(&slug, &token).await;
            if let Ok(Some(clip)) = clip {
                twdl::save_metadata(&clip, &path).await;
            }
        }
        let clip_path = &path.join(PathBuf::from_str(&format!("{}.mp4", &slug)).unwrap());
        twdl::download_file(best.url.clone(), &clip_path).await;
    }
    

}

async fn handle_channel_subcommand(args: ChannelCommandArgs, multi: Arc<MultiProgress>) -> () {
    let creds = load_credentials(args.credentials).await;
    let token = match twdl::twitch_utils::get_token(&creds.client_id, &creds.client_secret).await {
        Ok(token) => token,
        Err(err) => exit_with_error_msg(&format!("Failed to fetch application token: {err}"), Some(1))
    };
    let id = login_or_id(&args.broadcaster_id, &args.broadcaster_login, &token).await;
    let (start, end) = interpret_datetimes(args.start_timestamp, args.end_timestamp);
    let output_path = match PathBuf::from_str(&args.output) {
        Ok(path) => path,
        Err(_) => exit_with_error_msg("Invalid path", Some(1))
    };
    let clips = match twdl::twitch_utils::get_clips(multi.clone(), &id, &token, start, end, Some(100)).await {
        Ok(clips) => clips,
        Err(err) => exit_with_error_msg(&format!("Failed to fetch clips: {err}"), Some(1))
    };
    info!("Fetched {} clips, starting download", clips.len());
    if args.link {
        let mut source_file_futures = Vec::new();
        for clip in &clips {
            source_file_futures.push(get_video_source_files(&clip.id));
        }
        let source_file_results = join_all(source_file_futures).await;
        for result in &source_file_results {
            let files = match result {
                Ok(files) => files,
                Err(_) => {
                    error!("Error fetching source URL");
                    continue;
                }
            };
            let url = match files.iter().max() {
                Some(best) => &best.url,
                None => {
                    error!("Could not find any source files for clip");
                    continue;
                }
            };
            println!("{}", url.as_str())
        }
    } else {
        download_clips(multi, 
            clips, 
            &output_path, 
            args.chunk_size.unwrap_or(10), 
            args.link,
            args.metadata
        ).await;
    }
}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    let multi = Arc::new(MultiProgress::new());

    {
        // for outputting links, limit logs to errors
        let link = match &args.command {
            Commands::Clip(args) => args.link,
            Commands::Channel(args) => args.link
        };
        let log_level = match link {
            true => log::LevelFilter::Error,
            false => log::LevelFilter::Info
        };

        let multi_for_logs = multi.clone();
        env_logger::Builder::new()
            .format(move |buf, record| {
                let ts = buf.timestamp();
                let msg = format!("{} [{}] {}", ts, record.level(), record.args());
                multi_for_logs.println(msg).unwrap();
                Ok(())
            })
            .filter_level(log_level)
            .init();
    }

    match args.command {
        Commands::Clip(args) => {
            handle_clip_subcommand(args).await
        }
        Commands::Channel(args) => {
            handle_channel_subcommand(args, multi).await
        }
    }

}