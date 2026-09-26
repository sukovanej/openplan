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

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Channel {
    Stable,
    Canary,
}

impl Channel {
    pub fn of(installed: &Version) -> Self {
        if installed.pre.as_str().starts_with("canary.") {
            Channel::Canary
        } else {
            Channel::Stable
        }
    }
}

pub struct Release {
    pub version: Version,
    pub tag: String,
    pub channel: Channel,
    assets: Vec<Asset>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    UpToDate,
    Install,
    BackToStable,
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
    name: Option<String>,
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

    pub fn release(&self, channel: Channel) -> Result<Release> {
        let path = match channel {
            Channel::Stable => "releases/latest",
            Channel::Canary => "releases/tags/canary",
        };
        let url = format!("{}/repos/{}/{path}", self.api_base, self.repo);
        let response: ReleaseResponse = self
            .http
            .get(&url)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .and_then(reqwest::blocking::Response::error_for_status)
            .with_context(|| format!("reading {url}"))?
            .json()
            .with_context(|| format!("parsing the release at {url}"))?;
        // The tag `canary` moves with each build, so only the release title holds its version.
        let version = match channel {
            Channel::Stable => Version::parse(response.tag_name.trim_start_matches('v'))
                .with_context(|| {
                    format!("the release tag {:?} is not a version", response.tag_name)
                })?,
            Channel::Canary => {
                let title = response.name.unwrap_or_default();
                Version::parse(&title).with_context(|| {
                    format!("the canary release title {title:?} is not a version")
                })?
            }
        };
        Ok(Release {
            version,
            tag: response.tag_name,
            channel,
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
    pub fn step_from(&self, installed: &Version) -> Step {
        match self.channel {
            Channel::Canary if self.version == *installed => Step::UpToDate,
            Channel::Canary => Step::Install,
            Channel::Stable if self.version > *installed => Step::Install,
            Channel::Stable if self.version < *installed && !installed.pre.is_empty() => {
                Step::BackToStable
            }
            Channel::Stable => Step::UpToDate,
        }
    }

    pub fn publishes(&self, name: &str) -> bool {
        self.assets.iter().any(|asset| asset.name == name)
    }

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
