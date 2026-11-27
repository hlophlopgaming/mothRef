// SPDX-License-Identifier: GPL-3.0-or-later
use super::{Installation, Kind, state_directory};
use ring::digest::{Context, SHA256};
use semver::Version;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Write},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const REPOSITORY: &str = "hlophlopgaming/mothRef";
const API_LIMIT: u64 = 4 * 1024 * 1024;
const NOTES_LIMIT: usize = 64 * 1024;
const DOWNLOAD_LIMIT: u64 = 512 * 1024 * 1024;
const DAY: u64 = 86_400;

#[derive(Clone, Deserialize, Serialize)]
pub struct Release {
    tag_name: String,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    assets: Vec<Asset>,
}

#[derive(Clone, Deserialize, Serialize)]
struct Asset {
    name: String,
    size: u64,
}

impl Release {
    /// The version offered to the user and accepted by the restart supervisor.
    pub fn tag(&self) -> &str {
        &self.tag_name
    }

    /// Release notes are displayed as inert text inside the application.
    pub fn notes(&self) -> &str {
        self.body.as_deref().unwrap_or_default()
    }

    fn version(&self) -> Option<Version> {
        Version::parse(self.tag().strip_prefix('v')?).ok()
    }

    fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }

    fn supports(&self, kind: Kind) -> bool {
        kind.asset().is_some_and(|name| self.asset(name).is_some())
            && self.asset("SHA256SUMS").is_some()
    }
}

#[derive(Default, Deserialize, Serialize)]
struct State {
    checked: u64,
    skipped: Option<String>,
    releases: Vec<Release>,
}

fn locked_state(directory: &Path) -> io::Result<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(directory.join("check.lock"))?;
    lock.lock()?;
    Ok(lock)
}

fn read_state(directory: &Path) -> State {
    File::open(directory.join("check.json"))
        .ok()
        .and_then(|file| serde_json::from_reader(file.take(API_LIMIT)).ok())
        .unwrap_or_default()
}

fn write_state(directory: &Path, state: &State) -> io::Result<()> {
    let mut file = tempfile::NamedTempFile::new_in(directory)?;
    serde_json::to_writer(file.as_file_mut(), state)?;
    file.as_file().sync_all()?;
    file.persist(directory.join("check.json"))?;
    Ok(())
}

fn select(releases: Vec<Release>, current: &Version, kind: Kind) -> Option<Release> {
    releases
        .into_iter()
        .filter_map(|release| {
            let version = release.version()?;
            if release.draft
                || version.cmp_precedence(current).is_le()
                || (current.pre.is_empty() && (release.prerelease || !version.pre.is_empty()))
                || !release.supports(kind)
            {
                return None;
            }
            Some((version, release))
        })
        .max_by(|(a, _), (b, _)| a.cmp_precedence(b))
        .map(|(_, release)| release)
}

/// Runs on a worker once startup preferences are known. Caches a compatible
/// offer and rate-limits requests across multiple windows, restarts and processes.
pub fn check() -> io::Result<Option<Release>> {
    let Some(installation) = Installation::detect()? else {
        return Ok(None);
    };
    let current = Version::parse(env!("CARGO_PKG_VERSION")).map_err(io::Error::other)?;
    let directory = state_directory()?;
    let _lock = locked_state(&directory)?;
    let mut state = read_state(&directory);
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_secs();
    if now.saturating_sub(state.checked) >= DAY || state.checked > now.saturating_add(DAY) {
        // Persist the attempt before networking so failures and repeated boots cannot hammer GitHub.
        state.checked = now;
        write_state(&directory, &state)?;
        match releases() {
            Ok(releases) => {
                state.releases = releases;
                write_state(&directory, &state)?;
            }
            Err(error) if !state.releases.is_empty() => {
                tracing::debug!(%error, "update check temporarily unavailable")
            }
            Err(error) => return Err(error),
        }
    }
    Ok(select(state.releases, &current, installation.kind)
        .filter(|release| state.skipped.as_deref() != Some(release.tag())))
}

/// Remembers the declined release across startup checks without disabling future versions.
pub fn skip(release: &Release) -> io::Result<()> {
    let directory = state_directory()?;
    let _lock = locked_state(&directory)?;
    let mut state = read_state(&directory);
    state.skipped = Some(release.tag().to_owned());
    write_state(&directory, &state)
}

fn agent(timeout: Duration) -> ureq::Agent {
    let certificates = rustls_native_certs::load_native_certs().certs;
    let roots = certificates
        .iter()
        .map(|cert| ureq::tls::Certificate::from_der(cert.as_ref()).to_owned())
        .into();
    ureq::Agent::config_builder()
        .https_only(true)
        .tls_config(ureq::tls::TlsConfig::builder().root_certs(roots).build())
        .max_redirects(4)
        .timeout_global(Some(timeout))
        .user_agent(concat!("MothRef/", env!("CARGO_PKG_VERSION")))
        .build()
        .new_agent()
}

/// Fetches the installed version's notes when explicitly requested.
pub fn current() -> io::Result<Release> {
    let tag = concat!("v", env!("CARGO_PKG_VERSION"));
    let mut release: Release = get_json(&format!("releases/tags/{tag}"))?;
    if release.tag() != tag || release.draft {
        return Err(io::Error::other("Unexpected release"));
    }
    if let Some(body) = &mut release.body {
        body.truncate(body.floor_char_boundary(NOTES_LIMIT));
    }
    Ok(release)
}

fn releases() -> io::Result<Vec<Release>> {
    let mut releases: Vec<Release> = get_json("releases?per_page=20")?;
    for release in &mut releases {
        if let Some(body) = &mut release.body {
            body.truncate(body.floor_char_boundary(NOTES_LIMIT));
        }
    }
    Ok(releases)
}

fn get_json<T: serde::de::DeserializeOwned>(path: &str) -> io::Result<T> {
    let agent = agent(Duration::from_secs(20));
    let mut response = agent
        .get(format!("https://api.github.com/repos/{REPOSITORY}/{path}"))
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(io::Error::other)?;
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(API_LIMIT + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > API_LIMIT {
        return Err(io::Error::other("Release metadata exceeds limit"));
    }
    serde_json::from_slice(&bytes).map_err(io::Error::other)
}

fn copy_hashed(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limit: u64,
) -> io::Result<(ring::digest::Digest, u64)> {
    let mut reader = reader.take(limit + 1);
    let mut hash = Context::new(&SHA256);
    let mut total = 0;
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        total += count as u64;
        if total > limit {
            return Err(io::Error::other("Update artifact exceeds limit"));
        }
        hash.update(&buffer[..count]);
        writer.write_all(&buffer[..count])?;
    }
    Ok((hash.finish(), total))
}

fn checksum(sums: &str, name: &str) -> Option<[u8; 32]> {
    let mut matches = sums.lines().filter_map(|line| {
        let (hash, file) = line.split_once(' ')?;
        (file.trim_start().trim_start_matches('*') == name).then_some(hash)
    });
    let hash = matches.next()?.as_bytes();
    if hash.len() != 64 || matches.next().is_some() {
        return None;
    }
    let mut bytes = [0; 32];
    for (pair, byte) in hash.as_chunks::<2>().0.iter().zip(&mut bytes) {
        *byte = u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()?;
    }
    Some(bytes)
}

pub(super) fn fingerprint(path: &Path) -> io::Result<[u8; 32]> {
    let (digest, _) = copy_hashed(&mut File::open(path)?, &mut io::sink(), DOWNLOAD_LIMIT)?;
    let mut result = [0; 32];
    result.copy_from_slice(digest.as_ref());
    Ok(result)
}

pub(super) fn download(
    release: &Release,
    kind: Kind,
    directory: &Path,
) -> io::Result<std::path::PathBuf> {
    let version = release
        .version()
        .ok_or_else(|| io::Error::other("Invalid release tag"))?;
    let name = kind
        .asset()
        .ok_or_else(|| io::Error::other("No matching update artifact"))?;
    let asset = release
        .asset(name)
        .filter(|asset| (1..=DOWNLOAD_LIMIT).contains(&asset.size))
        .ok_or_else(|| io::Error::other("Invalid update artifact size"))?;
    // Construct both URLs from a pinned repository, validated SemVer and fixed filenames.
    let base = format!("https://github.com/{REPOSITORY}/releases/download/v{version}");
    let agent = agent(Duration::from_secs(300));
    let mut sums = agent
        .get(format!("{base}/SHA256SUMS"))
        .call()
        .map_err(io::Error::other)?;
    let mut manifest = String::new();
    sums.body_mut()
        .as_reader()
        .take(65_537)
        .read_to_string(&mut manifest)?;
    let expected = (manifest.len() <= 65_536)
        .then(|| checksum(&manifest, name))
        .flatten()
        .ok_or_else(|| io::Error::other("Missing or invalid update checksum"))?;
    let mut response = agent
        .get(format!("{base}/{name}"))
        .call()
        .map_err(io::Error::other)?;
    let path = directory.join(name);
    let mut output = File::create_new(&path)?;
    let (actual, size) = copy_hashed(
        &mut response.body_mut().as_reader(),
        &mut output,
        asset.size,
    )?;
    if size != asset.size || actual.as_ref() != expected {
        return Err(io::Error::other(
            "Update artifact failed SHA-256 verification",
        ));
    }
    output.sync_all()?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release(tag: &str, asset: &str) -> Release {
        Release {
            tag_name: tag.into(),
            body: None,
            draft: false,
            prerelease: false,
            assets: vec![
                Asset {
                    name: "SHA256SUMS".into(),
                    size: 1024,
                },
                Asset {
                    name: asset.into(),
                    size: 1024,
                },
            ],
        }
    }

    #[test]
    fn selects_newest_compatible_release_without_putting_stable_users_on_alpha() {
        let Some(asset) = Kind::AppImage.asset() else {
            return;
        };
        let candidates = vec![
            release("v0.3.0-alpha.9", asset),
            release("v0.3.0-alpha.10", asset),
            release("v0.4.0", "other.zip"),
            release("v0.4.0-alpha.1", asset),
        ];
        let stable = Version::parse("0.3.0").unwrap();
        assert!(select(candidates.clone(), &stable, Kind::AppImage).is_none());
        let alpha = Version::parse("0.4.0-alpha.0").unwrap();
        assert_eq!(
            select(candidates, &alpha, Kind::AppImage).unwrap().tag(),
            "v0.4.0-alpha.1"
        );
        assert_eq!(
            select(
                vec![
                    release("v0.3.0-alpha.9", asset),
                    release("v0.3.0-alpha.10", asset)
                ],
                &Version::parse("0.3.0-alpha.8").unwrap(),
                Kind::AppImage
            )
            .unwrap()
            .tag(),
            "v0.3.0-alpha.10"
        );
    }

    #[test]
    fn sha256_checksum_must_match_a_unique_exact_asset() {
        let hash = "aB".repeat(32);
        let line = format!("{hash} *mothref.zip\n");
        assert_eq!(checksum(&line, "mothref.zip"), Some([0xab; 32]));
        assert!(checksum(&line, "ref.zip").is_none());
        assert!(checksum(&line.repeat(2), "mothref.zip").is_none());
        assert!(copy_hashed(&mut &b"1234"[..], &mut io::sink(), 3).is_err());
    }
}
