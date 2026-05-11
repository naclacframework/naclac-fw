use crate::config::Config;
use std::fs;
use std::io;

#[cfg(target_family = "unix")]
pub fn create_router(config: &Config, version: &str) -> io::Result<()> {
    let router_path = config.bin_dir.join("naclac");
    if router_path.exists() {
        fs::remove_file(&router_path)?;
    }

    let target_path = config.versions_dir.join(version).join("bin").join("naclac");
    std::os::unix::fs::symlink(target_path, router_path)
}

#[cfg(target_family = "windows")]
pub fn create_router(config: &Config, version: &str) -> io::Result<()> {
    let cmd_path = config.bin_dir.join("naclac.cmd");
    let ps1_path = config.bin_dir.join("naclac.ps1");

    if cmd_path.exists() {
        fs::remove_file(&cmd_path)?;
    }
    if ps1_path.exists() {
        fs::remove_file(&ps1_path)?;
    }

    let target_path = config.versions_dir.join(version).join("bin").join("naclac.exe");
    let target_str = target_path.to_string_lossy();

    let cmd_contents = format!("@\"{}\" %*\r\n", target_str);
    fs::write(&cmd_path, cmd_contents)?;

    let ps1_contents = format!("& \"{}\" $args\r\n", target_str);
    fs::write(&ps1_path, ps1_contents)?;

    Ok(())
}
