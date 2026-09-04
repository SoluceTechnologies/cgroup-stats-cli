use std::path::{Path, PathBuf};

pub fn require(dir: &Path, files: &[&str]) -> Result<(), String> {
    for file in files {
        if !dir.join(file).exists() {
            return Err(format!("{file} not present"));
        }
    }
    Ok(())
}

pub(crate) fn read(dir: &Path, file: &str) -> Result<String, String> {
    std::fs::read_to_string(dir.join(file)).map_err(|err| format!("{file}: {err}"))
}

pub fn device_name(sys_dev_block: &Path, dev: &str) -> String {
    std::fs::read_link(sys_dev_block.join(dev))
        .ok()
        .and_then(|target| {
            target
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| dev.to_string())
}

pub fn sys_dev_block() -> PathBuf {
    PathBuf::from("/sys/dev/block")
}

pub fn normalize_path(path: &str, root: &Path) -> String {
    let full = Path::new(path);
    let relative = full.strip_prefix(root).unwrap_or(full);
    relative.to_string_lossy().trim_matches('/').to_string()
}
