// SPDX-License-Identifier: GPL-3.0-or-later
//! Only an explicit paste reaches this module; HTML never executes or loads a page.
use crate::{DesktopStorage, PlatformError};
use scraper::{Html, Selector};
use std::{
    io::Read,
    time::{Duration, Instant},
};
use url::Url;

const MAX_MARKUP: usize = 1024 * 1024;
const MAX_URL: usize = 8192;

fn web_url(value: &str, base: Option<&Url>) -> Result<Url, PlatformError> {
    let value = value.trim();
    if value.is_empty() || value.len() > MAX_URL || value.chars().any(char::is_control) {
        return Err(PlatformError::Download("invalid or oversized URL"));
    }
    let mut url = base
        .map_or_else(|| Url::parse(value), |base| base.join(value))
        .map_err(|_| PlatformError::Download("expected an absolute HTTP(S) image URL"))?;
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(PlatformError::Download(
            "only HTTP(S) URLs without credentials are supported",
        ));
    }
    url.set_fragment(None);
    if url.as_str().len() > MAX_URL {
        return Err(PlatformError::Download("normalized URL exceeds 8 KiB"));
    }
    Ok(url)
}

pub(super) fn source(html: Option<&str>, text: Option<&str>) -> Result<Url, PlatformError> {
    if html.is_some_and(|html| html.len() > MAX_MARKUP)
        || text.is_some_and(|text| text.len() > MAX_MARKUP)
    {
        return Err(PlatformError::Budget("Clipboard text/HTML exceeds 1 MiB"));
    }
    if let Some(html) = html {
        let document = Html::parse_document(html);
        let images = Selector::parse("img[src]").expect("static image selector is valid");
        // shortcut: paste one HTML image; extend when multi-image paste is requested.
        if let Some(image) = document.select(&images).next() {
            let source_url = html
                .lines()
                .take_while(|line| !line.contains('<'))
                .find_map(|line| line.strip_prefix("SourceURL:"))
                .and_then(|value| web_url(value, None).ok());
            let bases = Selector::parse("base[href]").expect("static base selector is valid");
            let base = document
                .select(&bases)
                .next()
                .and_then(|base| base.value().attr("href"))
                .and_then(|value| web_url(value, source_url.as_ref()).ok())
                .or(source_url);
            return web_url(image.value().attr("src").unwrap_or_default(), base.as_ref());
        }
    }
    match text.map(str::trim).filter(|text| !text.is_empty()) {
        Some(text) => web_url(text, None),
        None => Err(PlatformError::EmptyClipboard),
    }
}

pub(super) fn import(
    storage: &DesktopStorage,
    value: &str,
) -> Result<(moth_format::Handle, moth_io::Decoded), PlatformError> {
    let _span = tracing::info_span!("clipboard_download").entered();
    let mut url = web_url(value, None)?;
    let certificates = rustls_native_certs::load_native_certs().certs;
    let roots: ureq::tls::RootCerts = certificates
        .iter()
        .map(|cert| ureq::tls::Certificate::from_der(cert.as_ref()).to_owned())
        .into();
    let deadline = Instant::now() + Duration::from_secs(20);
    for _ in 0..=5 {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(PlatformError::Download("timed out"))?;
        let agent = ureq::Agent::config_builder()
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .root_certs(roots.clone())
                    .build(),
            )
            .timeout_global(Some(remaining))
            .max_redirects(0)
            .build()
            .new_agent();
        let mut response = agent
            .get(url.as_str())
            .header("Accept", "image/*")
            .call()
            .map_err(|_| PlatformError::Download("HTTP or TLS failure"))?;
        if response.status().is_redirection() {
            let target = response
                .headers()
                .get("location")
                .and_then(|value| value.to_str().ok())
                .ok_or(PlatformError::Download("redirect has no valid location"))?;
            let secure = url.scheme() == "https";
            url = web_url(target, Some(&url))?;
            if secure && url.scheme() != "https" {
                return Err(PlatformError::Download("HTTPS downgrade rejected"));
            }
            continue;
        }
        if response.status() != 200 {
            return Err(PlatformError::Download("expected HTTP 200"));
        }
        if response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > moth_io::MAX_INPUT_BYTES)
        {
            return Err(PlatformError::Budget("Downloaded image exceeds 64 MiB"));
        }
        let mut bytes = Vec::new();
        response
            .body_mut()
            .as_reader()
            .take(moth_io::MAX_INPUT_BYTES + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PlatformError::Download("incomplete or timed out response"))?;
        if bytes.len() as u64 > moth_io::MAX_INPUT_BYTES {
            return Err(PlatformError::Budget("Downloaded image exceeds 64 MiB"));
        }
        return storage.stage_encoded(&bytes);
    }
    Err(PlatformError::Download("too many redirects"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Write, net::TcpListener};

    #[test]
    fn extracts_images_without_executing_html() {
        assert_eq!(
            source(None, Some(" https://example.org/a.png#part "))
                .unwrap()
                .as_str(),
            "https://example.org/a.png"
        );
        assert_eq!(
            source(
                Some(
                    "<IMG SRC='https://example.org/a?x=1&amp;y=2'><img src='https://other.test/'>"
                ),
                Some("https://page.test")
            )
            .unwrap()
            .as_str(),
            "https://example.org/a?x=1&y=2"
        );
        assert_eq!(
            source(
                Some("<base href='https://example.org/dir/'><img src='../a.png'>"),
                None
            )
            .unwrap()
            .as_str(),
            "https://example.org/a.png"
        );
        assert_eq!(source(Some("Version:1.0\r\nSourceURL:https://example.org/dir/page\r\n<html><img src='a.png'>"), None).unwrap().as_str(), "https://example.org/dir/a.png");
        assert!(source(Some("<script>fetch('https://example.org')</script>"), None).is_err());
        for value in [
            "file:///tmp/a",
            "data:image/png;base64,AA",
            "javascript:alert(1)",
            "https://user:pass@example.org/a",
            "https://example.org/a\nb",
        ] {
            assert!(source(None, Some(value)).is_err(), "{value}");
        }
        assert!(source(Some("<img src='/relative.png'>"), None).is_err());
        assert!(source(Some(&"x".repeat(MAX_MARKUP + 1)), None).is_err());
        assert!(
            source(
                None,
                Some(&format!("https://example.org/{}", "x".repeat(MAX_URL)))
            )
            .is_err()
        );
    }

    fn server(responses: Vec<Vec<u8>>) -> (String, std::thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let thread = std::thread::spawn(move || {
            for response in responses {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = Vec::new();
                while !request.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    if socket.read(&mut byte).unwrap() == 0 {
                        break;
                    }
                    request.push(byte[0]);
                }
                let _ = socket.write_all(&response);
            }
        });
        (url, thread)
    }

    #[test]
    fn download_redirect_preserves_original_and_survives_reopen() {
        let mut png = Vec::new();
        moth_io::write_png(&mut png, 1, 1, &[20, 40, 60, 255]).unwrap();
        let mut response = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            png.len()
        )
        .into_bytes();
        response.extend_from_slice(&png);
        let (url, thread) = server(vec![b"HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec(), response]);
        let directory = std::env::temp_dir().join(format!("mothref-web-{}", uuid::Uuid::now_v7()));
        let storage = std::sync::Arc::new(DesktopStorage::in_directory(directory.clone()));
        let (handle, image) = import(&storage, &url).unwrap();
        thread.join().unwrap();
        assert_eq!(std::fs::read(storage.path(handle).unwrap()).unwrap(), png);
        assert_eq!(image.original, [1, 1]);
        assert_eq!(image.mips[0].rgba, [20, 40, 60, 255]);
        assert_eq!(image.hash, moth_io::decode(&png, 256).unwrap().hash);
        let worker =
            crate::boards::Worker::new(storage.clone(), std::sync::Arc::new(|| {})).unwrap();
        let item = moth_scene::Item::image(
            image.hash,
            moth_scene::Rect {
                x: 0.,
                y: 0.,
                width: 1.,
                height: 1.,
            },
            None,
        );
        worker
            .requests
            .send(crate::boards::Request::Apply(
                moth_scene::Command::Insert(item.clone()),
                Some(handle),
            ))
            .unwrap();
        loop {
            match worker.results.recv_timeout(Duration::from_secs(5)).unwrap() {
                crate::boards::Completion::Applied(_, _) => break,
                crate::boards::Completion::Failed(error) => panic!("{error}"),
                _ => {}
            }
        }
        worker.finish().unwrap();
        let mut journal =
            std::fs::File::open(storage.recovery_directory().unwrap().join("board.moth")).unwrap();
        let board = moth_format::board::read(&mut journal).unwrap();
        assert_eq!(board.scene.get(item.id), Some(&item));
        let blob = board.blobs.get(&image.hash).unwrap();
        std::io::Seek::seek(&mut journal, std::io::SeekFrom::Start(blob.offset)).unwrap();
        let mut original = vec![0; blob.length as usize];
        journal.read_exact(&mut original).unwrap();
        assert_eq!(original, png);
        drop(journal);
        storage.discard().unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn redirect_loop_is_bounded() {
        let response = b"HTTP/1.1 302 Found\r\nLocation: /again\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec();
        let (url, thread) = server(vec![response; 6]);
        let storage = DesktopStorage::default();
        assert!(matches!(
            import(&storage, &url),
            Err(PlatformError::Download("too many redirects"))
        ));
        thread.join().unwrap();
    }

    #[test]
    fn rejects_bad_responses_without_staging() {
        for response in [
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 67108865\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\nbroken",
            "HTTP/1.1 200 OK\r\nContent-Length: 4\r\n\r\nHTML",
            "HTTP/1.1 302 Found\r\nLocation: file:///tmp/secret\r\nContent-Length: 0\r\n\r\n",
        ] {
            let (url, thread) = server(vec![response.as_bytes().to_vec()]);
            let directory =
                std::env::temp_dir().join(format!("mothref-web-{}", uuid::Uuid::now_v7()));
            let storage = DesktopStorage::in_directory(directory.clone());
            assert!(import(&storage, &url).is_err());
            thread.join().unwrap();
            assert!(!directory.exists());
        }
    }
}
