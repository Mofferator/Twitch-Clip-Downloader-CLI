use clap::{command, Args, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "twdl", version, about = "Downloads twitch clips")]
pub struct Cli {
    #[clap(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    Clip(ClipCommandArgs),

    Channel(ChannelCommandArgs)
}

#[derive(Args, Debug)]
pub struct ClipCommandArgs {
     #[arg(short = 'o', long = "output", default_value_t = String::from("."), help = "Output dir to download clip to")]
    pub output: String,

    #[arg(short = 'L', long = "link", help = "Skip download and print the source file URL")]
    pub link: bool,

    #[arg(short = 'm', long = "metadata", help = "Download json metadata alongside the clip")]
    pub metadata: bool,

    #[arg(short = 'c', long = "credentials", help = "Path to a json file containing client_id and client_secret")]
    pub credentials: Option<String>,

    pub clip: String
}

#[derive(Args, Debug)]
pub struct ChannelCommandArgs {
    #[arg(short = 'o', long = "output", default_value_t = String::from("."), help = "Path to directory to store the clips")]
    pub output: String,

    #[arg(short = 'c', long = "credentials", help = "Path to a json file containing client_id and client_secret")]
    pub credentials: String,

    #[arg(short = 'i', long = "broadcaster-id", help = "Numeric broadcaster ID")]
    pub broadcaster_id: Option<u32>,

    #[arg(short = 'l', long = "broadcaster-login", help = "Broadcaster login")]
    pub broadcaster_login: Option<String>,

    #[arg(short = 's', long = "start", help = "Start of datetime range (If no end provided, defaults to 1 week)")]
    pub start_timestamp: Option<String>,

    #[arg(short = 'e', long = "end", help = "End of datetime range, requires a start time")]
    pub end_timestamp: Option<String>,

    #[arg(short = 'C', long = "chunk-size", help = "Number of clips fetched per page, default=20 max=100")]
    pub chunk_size: Option<usize>,

    #[arg(short = 'L', long = "link", help = "Skip downloads and print the source file URLs to stdout")]
    pub link: bool,

    #[arg(short = 'm', long = "metadata", help = "Download json metadata alongside the clip")]
    pub metadata: bool
}