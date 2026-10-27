pub fn external(value: &str) -> Option<url::Url> {
    if value.len() > 4096 || value.chars().any(char::is_control) {
        return None;
    }
    let url = url::Url::parse(value.trim()).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then_some(url)
}

pub fn local_navigation(url: &url::Url, dev: Option<&url::Url>) -> bool {
    if !url.username().is_empty() || url.password().is_some() {
        return false;
    }
    let bundled = matches!(
        (url.scheme(), url.host_str(), url.port()),
        ("tauri", Some("localhost"), None) | ("http" | "https", Some("tauri.localhost"), None)
    );
    bundled || dev.is_some_and(|origin| url.origin() == origin.origin())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn external_links_are_web_only_without_credentials_or_control_characters() {
        for value in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "https://user:secret@example.org",
            "https://example.org/\nsecret",
        ] {
            assert!(external(value).is_none(), "{value}");
        }
        assert!(external("https://example.org/license#terms").is_some());
    }
    #[test]
    fn webview_only_navigates_to_its_bundle_or_exact_dev_origin() {
        let dev = url::Url::parse("http://127.0.0.1:1420").unwrap();
        for value in [
            "https://example.org",
            "file:///etc/passwd",
            "https://tauri.localhost.evil.org",
            "http://localhost:1420",
            "tauri://user@localhost",
        ] {
            assert!(
                !local_navigation(&url::Url::parse(value).unwrap(), Some(&dev)),
                "{value}"
            );
        }
        assert!(local_navigation(
            &url::Url::parse("tauri://localhost").unwrap(),
            None
        ));
        assert!(local_navigation(
            &url::Url::parse("http://tauri.localhost").unwrap(),
            None
        ));
        assert!(!local_navigation(&dev, None));
        assert!(local_navigation(&dev, Some(&dev)));
    }
}
