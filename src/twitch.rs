use twitch_api::{helix::{clips::{get_clips, Clip}}, twitch_oauth2::AppAccessToken, HelixClient};
use anyhow::Result;

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
pub async fn get_clips(broadcaster_id: &String, token: &AppAccessToken) -> Result<Vec<Clip>> {
    let client: HelixClient<reqwest::Client> = HelixClient::default();
    let mut clips = Vec::new();
    let mut cursor = None;

    loop {
        let mut request = get_clips::GetClipsRequest::broadcaster_id(broadcaster_id);
        request.after = cursor.clone();

        let response = client.req_get(request, token).await?;
        clips.extend(response.data);

        if let Some(next_cursor) = response.pagination {
            cursor = Some(next_cursor.into());
        } else {
            break;
        }
        
    }
    Ok(clips)
}

