use std::fs;
use std::path::PathBuf;

pub struct Config {
    #[allow(dead_code)]
    pub base_dir: PathBuf,
    pub versions_dir: PathBuf,
    pub bin_dir: PathBuf,
}

impl Config {
    pub fn init() -> Self {
        let home_dir = dirs::home_dir().expect("Could not find home directory");
        let base_dir = home_dir.join(".nacvm");
        let versions_dir = base_dir.join("versions");
        let bin_dir = base_dir.join("bin");

        fs::create_dir_all(&versions_dir).expect("Failed to create versions directory");
        fs::create_dir_all(&bin_dir).expect("Failed to create bin directory");

        crate::utils::path_setup::ensure_in_path(&bin_dir);

        Self {
            base_dir,
            versions_dir,
            bin_dir,
        }
    }
}
