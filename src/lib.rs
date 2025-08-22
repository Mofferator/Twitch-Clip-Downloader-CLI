#![allow(dead_code)]
use std::{fmt::Display, str::FromStr};

use anyhow::Result;
mod video_source_response;
mod twitch;
use percent_encoding::{percent_encode, NON_ALPHANUMERIC};
use reqwest::Url;
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