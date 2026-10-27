use std::path::Path;
pub fn clean(value: &str) -> Option<String> {
    if value.len() > 16384 || value.chars().any(char::is_control) {
        return None;
    }
    let mut u = url::Url::parse(value.trim()).ok()?;
    if !matches!(u.scheme(), "http" | "https") || u.host_str().is_none() {
        return None;
    }
    // Download URLs often carry expiring credentials. Keep the source, never the tokens.
    let _ = u.set_username("");
    let _ = u.set_password(None);
    u.set_query(None);
    u.set_fragment(None);
    Some(u.to_string())
}
pub fn origin(path: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        let bytes = xattr::get(path, "com.apple.metadata:kMDItemWhereFroms").ok()??;
        let value = plist::Value::from_reader(std::io::Cursor::new(bytes)).ok()?;
        value
            .as_array()?
            .iter()
            .filter_map(|v| v.as_string())
            .find_map(clean)
    }
    #[cfg(target_os = "windows")]
    {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(format!("{}:Zone.Identifier", path.display()))
            .ok()?
            .take(64 * 1024)
            .read_to_end(&mut bytes)
            .ok()?;
        parse_zone(&String::from_utf8_lossy(&bytes))
    }
    #[cfg(target_os = "linux")]
    {
        let bytes = xattr::get(path, "user.xdg.origin.url").ok()??;
        clean(std::str::from_utf8(&bytes).ok()?)
    }
}
#[cfg(any(test, target_os = "windows"))]
fn parse_zone(text: &str) -> Option<String> {
    ["HostUrl=", "ReferrerUrl="].iter().find_map(|key| {
        text.lines()
            .find_map(|line| line.trim().strip_prefix(key).and_then(clean))
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_os = "macos")]
    #[test]
    fn reads_real_macos_origin_attribute() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let value = plist::Value::Array(vec![plist::Value::String(
            "https://example.org/font.zip".into(),
        )]);
        let mut bytes = Vec::new();
        value.to_writer_binary(&mut bytes).unwrap();
        xattr::set(file.path(), "com.apple.metadata:kMDItemWhereFroms", &bytes).unwrap();
        assert_eq!(
            origin(file.path()).as_deref(),
            Some("https://example.org/font.zip")
        );
    }
    #[test]
    fn host_url_precedes_referrer() {
        assert_eq!(parse_zone("[ZoneTransfer]\nReferrerUrl=https://example.org\nHostUrl=https://fonts.example.org/font.zip\r\n").unwrap(),"https://fonts.example.org/font.zip");
    }
    #[test]
    fn download_tokens_and_credentials_are_not_stored() {
        assert_eq!(
            clean("https://user:password@fonts.example.org/font.zip?token=secret#key").as_deref(),
            Some("https://fonts.example.org/font.zip")
        );
    }
    #[test]
    fn rejects_non_web_urls() {
        assert!(clean("javascript:alert(1)").is_none());
        assert!(clean("file:///secret").is_none());
    }
}
