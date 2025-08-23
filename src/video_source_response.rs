use serde::{Deserialize, Serialize};

// Collection of nested structs capturing the format of twitch's GraphQL response

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoSourceResponse {
    pub data: Data,

    pub extensions: Extensions
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Data {
    pub clip: Clip
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub playback_access_token: PlaybackAccessToken,

    pub video_qualities: Vec<VideoQuality>
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoQuality {
    pub quality: String,

    pub frame_rate: f32,

    #[serde(rename = "sourceURL")]
    pub source_url: String
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackAccessToken {
    pub signature: String,

    pub value: String
}

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Extensions {
    pub duration_milliseconds: i32,

    pub operation_name: String,

    #[serde(rename = "requestID")]
    pub request_id: String
}