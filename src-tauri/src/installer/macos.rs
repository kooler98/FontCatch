use super::*;
pub struct MacInstaller {
    root: PathBuf,
}
impl MacInstaller {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            root: dirs::home_dir()
                .ok_or("home_unavailable")?
                .join("Library/Fonts"),
        })
    }
}
impl Platform for MacInstaller {
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
        Ok(())
    }
    fn search_roots(&self) -> Vec<PathBuf> {
        vec![
            self.root.clone(),
            "/Library/Fonts".into(),
            "/System/Library/Fonts".into(),
        ]
    }
}
