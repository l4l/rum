use std::convert::{TryFrom, TryInto};
use std::result::Result as StdResult;

use futures::future::TryFutureExt;
use reqwest::Client;
use snafu::ResultExt;
use strum_macros::Display;
use unhtml::FromHtml;

use crate::meta;

#[derive(FromHtml)]
struct ArtistRaw {
    #[html(attr = "href")]
    url: Option<String>,
    #[html(attr = "inner")]
    name: Option<String>,
}

impl TryFrom<ArtistRaw> for meta::Artist {
    type Error = ();

    fn try_from(raw: ArtistRaw) -> StdResult<Self, Self::Error> {
        Ok(Self {
            url: raw.url.ok_or(())?,
            name: raw.name.ok_or(())?,
        })
    }
}

#[derive(FromHtml)]
#[html(selector = "div.serp-snippet__artists")]
struct ArtistsRaw {
    #[html(selector = "div.artist__content div.artist__name a.d-link")]
    artists: Vec<ArtistRaw>,
}

impl From<ArtistsRaw> for meta::Artists {
    fn from(raw: ArtistsRaw) -> Self {
        Self {
            artists: raw
                .artists
                .into_iter()
                .filter_map(|artist| artist.try_into().ok())
                .collect(),
        }
    }
}

#[derive(FromHtml)]
struct AlbumRaw {
    #[html(selector = "div.album__title a.deco-link", attr = "href")]
    url: String,
    #[html(selector = "div.album__title", attr = "inner")]
    title: String,
    #[html(selector = "div.album__artist a.d-link")]
    artists: Vec<ArtistRaw>,
    #[html(selector = "div.album__year", attr = "inner")]
    year_with_version: String,
    #[html(selector = "div.album__year span.album__version", attr = "inner")]
    version: Option<String>,
}

impl TryFrom<AlbumRaw> for meta::Album {
    type Error = ();

    fn try_from(raw: AlbumRaw) -> StdResult<Self, Self::Error> {
        Ok(Self {
            url: raw.url,
            title: raw.title,
            artists: raw
                .artists
                .into_iter()
                .filter_map(|raw| raw.try_into().ok())
                .collect(),
            year: raw
                .year_with_version
                .replace(raw.version.as_deref().unwrap_or(""), "")
                .parse()
                .map_err(|_| ())?,
            version: raw.version,
        })
    }
}

#[derive(FromHtml)]
#[html(selector = "div.centerblock")]
struct AlbumsRaw {
    #[html(selector = "div.album_selectable")]
    albums: Vec<AlbumRaw>,
}

impl From<AlbumsRaw> for meta::Albums {
    fn from(raws: AlbumsRaw) -> Self {
        Self {
            albums: raws
                .albums
                .into_iter()
                .filter_map(|raws| raws.try_into().ok())
                .collect(),
        }
    }
}

#[derive(FromHtml)]
struct TrackRaw {
    #[html(selector = "div.d-track__name a.d-track__title", attr = "href")]
    url: Option<String>,
    #[html(selector = "div.d-track__name a.d-track__title", attr = "inner")]
    name: Option<String>,
    #[html(selector = "div.d-track__meta span.d-track__artists a")]
    artists: Vec<ArtistRaw>,
}

impl TryFrom<TrackRaw> for meta::Track {
    type Error = ();

    fn try_from(raw: TrackRaw) -> StdResult<Self, Self::Error> {
        // `/album/4766/track/57703`
        let url = raw.url.ok_or(())?;
        let name = raw.name.ok_or(())?;
        let mut iter = url.split('/');
        iter.next();
        let mut parse_int = move || {
            iter.next();
            iter.next().and_then(|val| val.parse().ok()).ok_or(())
        };
        let album_id = parse_int()?;
        let track_id = parse_int()?;

        let artists = raw
            .artists
            .into_iter()
            .filter_map(|raw| raw.try_into().ok())
            .collect();

        Ok(Self {
            album_id,
            track_id,
            name,
            artists: std::sync::Arc::new(artists),
        })
    }
}

#[derive(FromHtml)]
struct TracksRaw {
    #[html(selector = "div.d-track")]
    tracks: Vec<TrackRaw>,
}

impl From<TracksRaw> for meta::Tracks {
    fn from(raws: TracksRaw) -> Self {
        Self {
            tracks: raws
                .tracks
                .into_iter()
                .filter_map(|track| track.try_into().ok())
                .collect(),
        }
    }
}

const BASE_URL: &str = "https://music.yandex.ru";

/*
{"codec":"mp3"
 "bitrate":192,
 "src":"https://storage.mds.yandex.net/file-download-info/53090_49160231.49166739.1.57703/2?sign=1172df07524abd16c528c85adacf6e3716cb13aec818822d7fcf32c48d1a5fd3&ts=5db42aa5",
 "gain":false,
 "preview":false}
*/
#[derive(serde::Deserialize, Debug)]
struct BalancerResponse {
    codec: String,
    bitrate: u32,
    src: String,
    //..
}

/*
<?xml version="1.0" encoding="utf-8"?>
<download-info>
    <host>s96vla.storage.yandex.net</host>
    <path>/rmusic/U2FsdGVkX18HQugf-LCm69vdpBvuPSCgSPq64xSmb0Ld-WB0mwjeDmmEuxE9cVjT_LlO25BG46S_igZvDqh_AuafEynGp4qFyVMGb5iI5ZE/be2821fda525ebd020996360f6f394dee09af26c3623aabd1d62ac2dff7ec2e6</path>
    <ts>000595cf7fdf9b99</ts>
    <region>-1</region>
    <s>be2821fda525ebd020996360f6f394dee09af26c3623aabd1d62ac2dff7ec2e6</s>
</download-info>
*/
#[derive(serde::Deserialize, Debug)]
struct DownloadInfo {
    host: String,
    path: String,
    ts: String,
    s: String,
}

#[derive(Debug, snafu::Snafu)]
pub enum Error {
    #[snafu(display("http error, url: {}, err: {}", url, source))]
    HttpError { url: String, source: reqwest::Error },
    #[snafu(display("html error: {}", source))]
    HtmlError { source: unhtml::Error },
    #[snafu(display("XmlError({})", source))]
    XmlError {
        body: String,
        source: serde_xml_rs::Error,
    },
}

pub type Result<T> = StdResult<T, Error>;

/// Yandex Music info/media provider
pub struct Provider {
    client: Client,
}

#[derive(Display, Clone, Copy)]
#[strum(serialize_all = "snake_case")]
enum SearchType {
    Albums,
    Tracks,
    Artists,
}

impl SearchType {
    fn search_url(self, search_text: &str) -> String {
        format!(
            "{}/search?text={}&type={}",
            BASE_URL,
            search_text, // TODO: url encode
            self.to_string()
        )
    }
}

impl Provider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn artists_search(&self, text: &str) -> Result<meta::Artists> {
        let url = SearchType::Artists.search_url(text);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                ArtistsRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn artist_albums(&self, artist: &meta::Artist) -> Result<meta::Albums> {
        let url = format!("{}{}/albums", BASE_URL, artist.url);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                AlbumsRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn artist_tracks(&self, artist: &meta::Artist) -> Result<meta::Tracks> {
        let url = format!("{}{}/tracks", BASE_URL, artist.url);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                TracksRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn album_search(&self, text: &str) -> Result<meta::Albums> {
        let url = SearchType::Albums.search_url(text);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                AlbumsRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn track_search(&self, text: &str) -> Result<meta::Tracks> {
        let url = SearchType::Tracks.search_url(text);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                TracksRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn album_tracks(&self, album: &meta::Album) -> Result<meta::Tracks> {
        let url = format!("{}{}", BASE_URL, album.url);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| {
                TracksRaw::from_html(&body)
                    .map(Into::into)
                    .context(HtmlError {})
            })
    }

    pub async fn get_track_url(&self, track: &meta::Track) -> Result<String> {
        let url = format!("https://music.yandex.ru/api/v2.1/handlers/track/{}:{}/web-album-track-track-saved/download/m", track.track_id, track.album_id);

        let url = self
            .client
            .get(&url)
            .header(
                "X-Retpath-Y",
                format!("https%3A%2F%2Fmusic.yandex.ru%2Falbum%2F{}", track.album_id),
            )
            .send()
            .and_then(|r| r.json::<BalancerResponse>())
            .await
            .context(HttpError { url })?
            .src;

        let info = self
            .client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|response| {
                serde_xml_rs::from_str::<DownloadInfo>(&response)
                    .context(XmlError { body: response })
            })?;

        Ok(format!(
            "https://{}/get-mp3/11111111111111111111111111111111/{}{}?track-id={}&play=false",
            info.host, info.ts, info.path, track.track_id
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_search_type_string() {
        assert_eq!(SearchType::Albums.to_string(), "albums");
        assert_eq!(SearchType::Tracks.to_string(), "tracks");
        assert_eq!(SearchType::Artists.to_string(), "artists");
    }
}
