use std::convert::{TryFrom, TryInto};

use futures::future::TryFutureExt;
use reqwest::Client;
use snafu::ResultExt;
use unhtml::{self, FromHtml};
use unhtml_derive::*;

#[derive(Debug, Clone)]
pub struct Album {
    pub url: String,
    pub title: String,
    pub artist: String,
    pub year: u16,
    pub version: Option<String>,
}

#[derive(FromHtml, Debug, Clone)]
struct AlbumRaw {
    #[html(selector = "div.album__title a.deco-link", attr = "href")]
    url: String,
    #[html(selector = "div.album__title", attr = "inner")]
    title: String,
    #[html(selector = "div.album__artist", attr = "inner")]
    artist: String,
    #[html(selector = "div.album__year", attr = "inner")]
    year_with_version: String,
    #[html(selector = "div.album__year span.album__version", attr = "inner")]
    version: Option<String>,
}

impl TryFrom<AlbumRaw> for Album {
    type Error = ();

    fn try_from(raw: AlbumRaw) -> std::result::Result<Self, Self::Error> {
        Ok(Self {
            url: raw.url,
            title: raw.title,
            artist: raw.artist,
            year: raw
                .year_with_version
                .replace(raw.version.as_deref().unwrap_or(""), "")
                .parse()
                .map_err(|_| ())?,
            version: raw.version,
        })
    }
}

impl Album {
    #[allow(unused)]
    fn id(&self) -> u32 {
        self.url.split('/').nth(1).unwrap().parse().unwrap()
    }
}

#[derive(Debug)]
pub struct Albums {
    pub albums: Vec<Album>,
}

#[derive(FromHtml)]
#[html(selector = "div.serp-snippet__albums")]
struct AlbumsRaw {
    #[html(selector = "div.album_selectable")]
    albums: Vec<AlbumRaw>,
}

impl From<AlbumsRaw> for Albums {
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

#[derive(FromHtml, Debug, Clone)]
pub struct Track {
    #[html(selector = "div.d-track__name a.d-track__title", attr = "href")]
    pub url: String,
    #[html(selector = "div.d-track__name a.d-track__title", attr = "inner")]
    pub name: String,
}

impl Track {
    fn id(&self) -> (u32, u32) {
        // `/album/4766/track/57703`
        let mut iter = self.url.split('/');
        iter.next();
        iter.next();
        let album_id = iter.next().unwrap().parse().unwrap();
        iter.next();
        let track_id = iter.next().unwrap().parse().unwrap();
        (album_id, track_id)
    }
}

#[derive(FromHtml, Debug)]
#[html(selector = "div.lightlist")]
pub struct Tracks {
    #[html(selector = "div.d-track__name")]
    pub tracks: Vec<Track>,
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
    #[snafu(display("JsonError({})", source))]
    JsonError {
        body: String,
        source: serde_json::Error,
    },
    #[snafu(display("XmlError({})", source))]
    XmlError {
        body: String,
        source: serde_xml_rs::Error,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

/// Yandex Music info/media provider
pub struct Provider {
    client: Client,
}

impl Provider {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub async fn text_search(&self, text: &str) -> Result<Albums> {
        const SEARCH_TYPE: &str = "albums"; // FIXME: paramterize
        let url = format!("{}/search?text={}&type={}", BASE_URL, text, SEARCH_TYPE);

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

    pub async fn album_tracks(&self, album: &Album) -> Result<Tracks> {
        let url = format!("{}{}", BASE_URL, album.url);

        self.client
            .get(&url)
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|body| Tracks::from_html(&body).context(HtmlError {}))
    }

    pub async fn get_track_url(&self, track: &Track) -> Result<String> {
        let (album_id, track_id) = track.id();
        let url = format!("https://music.yandex.ru/api/v2.1/handlers/track/{}:{}/web-album-track-track-saved/download/m", track_id, album_id);

        let url = self
            .client
            .get(&url)
            .header(
                "X-Retpath-Y",
                format!("https%3A%2F%2Fmusic.yandex.ru%2Falbum%2F{}", album_id),
            )
            .send()
            .and_then(|r| r.text())
            .await
            .context(HttpError { url })
            .and_then(|balancer| {
                serde_json::from_str::<BalancerResponse>(&balancer)
                    .map(|r| r.src)
                    .context(JsonError { body: balancer })
            })?;

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
            info.host, info.ts, info.path, track_id
        ))
    }
}
