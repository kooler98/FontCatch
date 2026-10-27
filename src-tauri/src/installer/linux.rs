use super::*;
pub struct LinuxInstaller {
    root: PathBuf,
}
impl LinuxInstaller {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            root: dirs::home_dir()
                .ok_or("home_unavailable")?
                .join(".local/share/fonts"),
        })
    }
}
impl Platform for LinuxInstaller {
    fn root(&self) -> &Path {
        &self.root
    }
    fn register(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
        Ok(())
    }
    fn unregister(&self, _: &Path, _: &[FontFace]) -> Result<(), String> {
        Ok(())
    }
    fn refresh(&self) -> Result<(), String> {
        let status = std::process::Command::new("fc-cache")
            .arg("-f")
            .arg(&self.root)
            .status()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err("font_cache_failed".into())
        }
    }
    fn search_roots(&self) -> Vec<PathBuf> {
        let mut roots = vec![
            self.root.clone(),
            "/usr/share/fonts".into(),
            "/usr/local/share/fonts".into(),
        ];
        if let Some(home) = dirs::home_dir() {
            roots.push(home.join(".fonts"));
        }
        roots
    }
}
