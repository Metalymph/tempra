#[derive(Debug, Clone)]
pub struct PackageManifest {
    pub name: String,
    pub entry: Option<String>,
}
