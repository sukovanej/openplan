use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use semver::Version;
use serde::Deserialize;

use crate::digest::verify_sha256;

const API_BASE: &str = "https://api.github.com";
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

pub struct Github {
    api_base: String,
    repo: String,
    http: reqwest::blocking::Client,
}

pub struct Release {
    pub version: Version,
    pub tag: String,
    assets: Vec<Asset>,
}

#[derive(Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

#[derive(Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    assets: Vec<Asset>,
}

impl Default for Github {
    fn default() -> Self {
        Self::at(API_BASE, repo_slug(env!("CARGO_PKG_REPOSITORY")))
    }
}

impl Github {
    pub fn at(api_base: &str, repo: &str) -> Self {
        let http = reqwest::blocking::Client::builder()
            .user_agent(format!("openplan/{}", env!("CARGO_PKG_VERSION")))
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("a reqwest client with a user agent and a timeout builds on every platform");
        Self {
            api_base: api_base.trim_end_matches('/').to_owned(),
            repo: repo.to_owned(),
            http,
        }
    }

    pub fn latest_release(&self) -> Result<Release> {
        let url = format!("{}/repos/{}/releases/latest", self.api_base, self.repo);
        let response: ReleaseResponse = self
            .http
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .with_context(|| format!("reading {url}"))?
            .json()
            .with_context(|| format!("parsing the release at {url}"))?;
        let version = Version::parse(response.tag_name.trim_start_matches('v'))
            .with_context(|| format!("the release tag {:?} is not a version", response.tag_name))?;
        Ok(Release {
            version,
            tag: response.tag_name,
            assets: response.assets,
        })
    }

    pub fn download_verified(&self, release: &Release, name: &str) -> Result<Vec<u8>> {
        let asset = release.asset(name)?;
        let digest = release.asset(&format!("{name}.sha256"))?;
        let published = String::from_utf8(self.download(&digest.url)?)
            .with_context(|| format!("{} is not text", digest.name))?;
        let bytes = self.download(&asset.url)?;
        verify_sha256(&bytes, &published).with_context(|| format!("verifying {name}"))?;
        Ok(bytes)
    }

    fn download(&self, url: &str) -> Result<Vec<u8>> {
        let bytes = self
            .http
            .get(url)
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .with_context(|| format!("downloading {url}"))?
            .bytes()
            .with_context(|| format!("reading {url}"))?;
        Ok(bytes.to_vec())
    }
}

impl Release {
    pub fn asset(&self, name: &str) -> Result<&Asset> {
        match self.assets.iter().find(|asset| asset.name == name) {
            Some(asset) => Ok(asset),
            None => bail!("release {} publishes no asset named {name}", self.tag),
        }
    }
}

fn repo_slug(repository_url: &str) -> &str {
    let trimmed = repository_url
        .trim_end_matches('/')
        .trim_end_matches(".git");
    trimmed
        .split_once("github.com/")
        .map_or(trimmed, |(_, slug)| slug)
}
