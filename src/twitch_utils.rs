use std::{str::FromStr};

use chrono::{DateTime, Duration, Utc};
use futures_util::future::join_all;
use twitch_api::{helix::{clips::{get_clips, Clip}, users::{GetUsersRequest, User}}, twitch_oauth2::AppAccessToken, types::UserId, HelixClient};
use anyhow::Result;
use twitch_types::Timestamp;
use log::error;

pub fn convert_dt(input: &DateTime<Utc>) -> Timestamp {
    match Timestamp::from_str(&input.to_rfc3339()) {
        Ok(date) => date,
        Err(_) => panic!(),
    }
}

pub fn convert_ts(input: &Timestamp) -> DateTime<Utc> {
    input.as_str().parse::<DateTime<Utc>>().expect("Invalid timestamp format")
}

pub enum DateChunkingType {
    ByDuration(Duration),
    ByNumber(u16)
}

fn split_date_range(start: DateTime<Utc>, end: DateTime<Utc>, chunking_type: DateChunkingType) -> Vec<(Timestamp, Timestamp)> {
    let step = match chunking_type {
        DateChunkingType::ByDuration(step) => step,
        DateChunkingType::ByNumber(num) => (end - start) / num.into()
    };

    let mut chunks = Vec::new();

    let mut current = start;
    while current < end {
        let next = (current + step).min(end);
        chunks.push((convert_dt(&current), convert_dt(&next)));
        current = next;
    }

    chunks
}

pub async fn get_token(client_id: &str, client_secret: &str) -> Result<AppAccessToken> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    Ok(AppAccessToken::get_app_access_token(
        &client,
        client_id.into(),
        client_secret.into(),
        vec![/* scopes */],
    )
    .await?)
}

pub async fn get_clips_chunked(broadcaster_id: &UserId,
                        token: &AppAccessToken,
                        start: DateTime<Utc>,
                        end: DateTime<Utc>,
                        chunking_type: DateChunkingType,
                        first: Option<usize>) -> Vec<Clip> {
    let date_ranges = split_date_range(start, end, chunking_type);
    let futures = date_ranges
        .iter()
        .map(|chunk| get_clips(broadcaster_id, token, chunk.0.clone(), chunk.1.clone(), first));

    let mut clips = Vec::new();

    let results = join_all(futures).await;

    for result in results {
        match result {
            Ok(clip_sublist) => clips.extend(clip_sublist),
            Err(err) => {
                error!("Failed to get clips for sublist {err}")
            }
        }
    }

    clips
}

async fn get_clips(broadcaster_id: &UserId,
                    token: &AppAccessToken,
                    started_at: Timestamp,
                    ended_at: Timestamp,
                    first: Option<usize>) -> Result<Vec<Clip>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let mut clips = Vec::new();
    let mut cursor = None;

    let mut request = get_clips::GetClipsRequest::builder()
        .broadcaster_id(broadcaster_id.as_cow())
        .started_at(Some(started_at.as_cow()))
        .ended_at(Some(ended_at.as_cow()))
        .first(first)
        .build();


    loop {
        request.after = cursor.clone();

        let response = client.req_get(request.clone(), token).await?;
        clips.extend(response.data);

        if let Some(next_cursor) = response.pagination {
            cursor = Some(next_cursor.into());
        } else {
            break;
        }
    }
    Ok(clips)
}

pub async fn get_broadcaster_id(login: &String, token: &AppAccessToken) -> Result<Option<UserId>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let user_option = client.get_user_from_login(login, token).await?;

    Ok(user_option.map(|user| user.id))
}

pub async fn get_clip(clip_id: &String, token: &AppAccessToken) -> Result<Option<Clip>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let get_clip_request = get_clips::GetClipsRequest::builder()
        .id(vec![clip_id].into())
        .build();
    let response = client.req_get(get_clip_request, token).await?;
    let clip = response.data.first();
    Ok(clip.cloned())
}

pub async fn get_user(user_id: &UserId, token: &AppAccessToken) -> Result<Option<User>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let request = GetUsersRequest::builder()
        .id(user_id)
        .build();

    let response = client.req_get(request, token).await?;
    Ok(response.data.first().cloned())
}

