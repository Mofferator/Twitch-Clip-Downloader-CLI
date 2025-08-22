use clap::{arg, command, Parser, Subcommand};
use twdl::get_video_source_files;
use std::{path::{PathBuf}, process, str::FromStr};
use regex::Regex;
use reqwest::Url;
use tokio::io::AsyncWriteExt;
use tokio::fs::File;
use futures_util::StreamExt;

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
    }
}

async fn download_file(url: Url, file: PathBuf) {
    let client = reqwest::Client::new();
    let response = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            eprintln!("Failed to send request: {}", e);
            return;
        }
    };

    let mut stream = response.bytes_stream();

    let mut output = match File::create(&file).await {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Failed to create file {}: {}", file.display(), e);
            return;
        }
    };

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = output.write_all(&bytes).await {
                    eprintln!("Failed to write to file {}: {}", file.display(), e);
                    return;
                }
            }
            Err(e) => {
                eprintln!("Error while downloading: {}", e);
                return;
            }
        }
    }

    println!("Downloaded file to {}", file.display());
}


fn output_file_or_cwd(output_file: &Option<String>, slug: &String) -> Result<PathBuf, std::convert::Infallible> {
    match output_file {
        Some(path) => PathBuf::from_str(&path),
        None => PathBuf::from_str(&format!("./{slug}.mp4"))
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

    download_file(best.url.clone(), path).await;

}

#[tokio::main]
async fn main() {
    let args = Cli::parse();
    match args.command {
        Commands::Clip { output, clip } => {
            handle_clip_subcommand(output, clip).await
        }
    }

}