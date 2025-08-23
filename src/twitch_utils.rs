use std::{borrow::Cow, sync::Arc};

use twitch_api::{helix::clips::{get_clips, Clip}, twitch_oauth2::AppAccessToken, types::{TimestampRef, UserId}, HelixClient};
use anyhow::Result;
use indicatif::{MultiProgress, ProgressBar};

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
pub async fn get_clips(multi: Arc<MultiProgress>,
                        broadcaster_id: &UserId,
                        token: &AppAccessToken,
                        started_at: Option<Cow<'_, TimestampRef>>,
                        ended_at: Option<Cow<'_, TimestampRef>>,
                        first: Option<usize>) -> Result<Vec<Clip>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let mut clips = Vec::new();
    let mut cursor = None;

    let mut request = get_clips::GetClipsRequest::builder()
        .broadcaster_id(broadcaster_id.as_cow())
        .started_at(started_at)
        .ended_at(ended_at)
        .first(first)
        .build();

    let spinner = multi.add(ProgressBar::new_spinner());
    spinner.set_message("Fetching clips from twitch");
    loop {
        request.after = cursor.clone();

        let response = client.req_get(request.clone(), token).await?;
        clips.extend(response.data);

        if let Some(next_cursor) = response.pagination {
            cursor = Some(next_cursor.into());
        } else {
            break;
        }
        spinner.tick();
    }
    Ok(clips)
}

pub async fn get_broadcaster_id(login: &String, token: &AppAccessToken) -> Result<Option<UserId>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let user_option = client.get_user_from_login(login, token).await?;

    Ok(user_option.map(|user| user.id))
}

