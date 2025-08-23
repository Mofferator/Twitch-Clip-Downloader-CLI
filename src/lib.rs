pub mod twitch_utils;

use log::{error, debug};

use std::{fmt::Display, path::{Path, PathBuf}, str::FromStr, sync::Arc};
use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar};
mod video_source_response;
use futures_util::{future::join_all, StreamExt};
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use reqwest::Url;
use tokio::{fs::File, io::AsyncWriteExt};
use twitch_api::helix::clips::Clip;
use video_source_response::VideoSourceResponse;

#[derive(PartialEq, Eq, Debug)]
pub struct SourceFile {
    pub quality: u32,
    pub frame_rate: u32,
    pub url: Url
}

impl PartialOrd for SourceFile {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.quality.partial_cmp(&other.quality)
    }
}

impl Ord for SourceFile {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.quality.cmp(&other.quality)
    }
}

impl Display for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url.as_str())
    }
}

fn format_source_urls(response: &VideoSourceResponse) -> Result<Vec<SourceFile>> {
    let sig = &response.data.clip.playback_access_token.signature;
    let token = &response.data.clip.playback_access_token.value;
    let encoded_token = percent_encode(token.as_bytes(), NON_ALPHANUMERIC);
    let mut output: Vec<SourceFile> = Vec::new();
    for quality in &response.data.clip.video_qualities {
        let url = quality.source_url.clone();
        let source_file = SourceFile{
            quality: quality.quality.parse::<u32>()?,
            frame_rate: quality.frame_rate.round() as u32,
            url: Url::from_str(&format!("{url}?sig={sig}&token={encoded_token}"))?,
        };
        output.push(source_file);
    }
    
    Ok(output)
}

async fn request_video_source_info(clip_slug: &String) -> Result<String> {
    let client = reqwest::Client::builder()
    .build()?;

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Client-ID", "kimne78kx3ncx6brgo4mv6wki5h1ko".parse()?);
    headers.insert("Content-Type", "application/json".parse()?);

    let data = format!(
    r#"{{
    "operationName": "VideoAccessToken_Clip",
    "variables": {{
        "slug": "{clip_slug}"
    }},
    "query": "query VideoAccessToken_Clip($slug: ID!) {{ clip(slug: $slug) {{ playbackAccessToken(params: {{platform: \"web\", playerBackend: \"mediaplayer\", playerType: \"site\"}}) {{ signature value }} videoQualities {{ quality frameRate sourceURL }} }} }}"
    }}"#
    );

    let json: serde_json::Value = serde_json::from_str(&data)?;

    let request = client.request(reqwest::Method::POST, "https://gql.twitch.tv/gql")
        .headers(headers)
        .json(&json);

    let response = request.send().await?;
    let body = response.text().await?;
    Ok(body)
}

pub async fn get_video_source_files(clip_slug: &String) -> Result<Vec<SourceFile>> {
    let body = request_video_source_info(clip_slug).await?;

    let video_source_response: VideoSourceResponse = serde_json::from_str(&body)?;

    format_source_urls(&video_source_response)
}

pub async fn download_clips(multi: Arc<MultiProgress>, clips: Vec<Clip>, directory: &Path, chunk_size: usize, silent: bool) {
    let bar = match silent {
        false => Some(multi.add(ProgressBar::new(clips.len().try_into().unwrap()))),
        true => None
    };
    for chunk in clips.chunks(chunk_size) {
        let futures: Vec<_> = chunk.iter().map(|clip| download_clip(clip, directory)).collect();
        let _ = join_all(futures).await;
        if let Some(ref bar) = bar {
            bar.inc(chunk.len().try_into().unwrap());
        }
    }
}

pub async fn download_file(url: Url, file: &PathBuf) {
    let client = reqwest::Client::new();
    let response = match client.get(url).send().await {
        Ok(resp) => resp,
        Err(e) => {
            error!("Failed to send request: {e}");
            return;
        }
    };

    let mut stream = response.bytes_stream();

    let mut output = match File::create(&file).await {
        Ok(f) => f,
        Err(e) => {
            error!("Failed to create file {}: {}", file.display(), e);
            return;
        }
    };

    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                if let Err(e) = output.write_all(&bytes).await {
                    error!("Failed to write to file {}: {}", file.display(), e);
                    return;
                }
            }
            Err(e) => {
                error!("Error while downloading: {e}");
                return;
            }
        }
    }

    debug!("Downloaded file to {}", file.display());
}

pub async fn download_clip(clip: &Clip, directory: &Path) {
    let source_files = match get_video_source_files(&clip.id).await {
        Ok(files) => files,
        Err(err) => {
            error!("Failed to download clip: {} ({err})", clip.id);
            return;
        }
    };
    let best = match source_files.iter().max() {
        Some(best) => best,
        None => {
            error!("Could not find source file for clip: {}", clip.id);
            return;
        }
    };
    let url = best.url.clone();
    let path = match PathBuf::from_str(&format!("{}.mp4", &clip.id)) {
        Ok(path) => path,
        Err(err) => {
            error!("Failed to specify path: {err}");
            return;
        }
    };
    let path = directory.join(path);
    download_file(url.clone(), &path).await;
}