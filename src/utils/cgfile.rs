use std::path::{Path, PathBuf};

/// Checked before every metric, because cgroups-rs cannot be asked whether a
/// read succeeded: `memory_stat()` returns `usage_in_bytes == 0` for a missing
/// `memory.current`,
/// and `get_mem()` maps a failed read of `memory.max` onto `MaxValue::Max`,
/// which is identical to a genuine "unlimited". Without this precheck the root
/// cgroup reports `0 / unlimited` instead of being reported unavailable.
pub fn require(dir: &Path, files: &[&str]) -> Result<(), String> {
    for f in files {
        if !dir.join(f).exists() {
            return Err(format!("{f} not present"));
        }
    }
    Ok(())
}

pub(crate) fn read(dir: &Path, file: &str) -> Result<String, String> {
    std::fs::read_to_string(dir.join(file)).map_err(|e| format!("{file}: {e}"))
}

/// Resolve `major:minor` to a kernel device name via `/sys/dev/block`, falling
/// back to the raw `major:minor`. Naming never fails the metric.
pub fn device_name(sys_dev_block: &Path, dev: &str) -> String {
    std::fs::read_link(sys_dev_block.join(dev))
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .unwrap_or_else(|| dev.to_string())
}

/// Injected rather than hardcoded at the call site so tests can point at a
/// directory holding no block device symlinks.
pub fn sys_dev_block() -> PathBuf {
    PathBuf::from("/sys/dev/block")
}

/// Strip the hierarchy root so both `/sys/fs/cgroup/a/b` and `a/b` resolve to
/// the `a/b` that `Cgroup::load` expects. The strip is component-aware, so a
/// sibling that merely shares the root's leading characters
/// (`/sys/fs/cgroup2/foo`) is not silently rewritten into a child of the root.
pub fn normalize_path(path: &str, root: &Path) -> String {
    let p = Path::new(path);
    let rel = p.strip_prefix(root).unwrap_or(p);
    rel.to_string_lossy().trim_matches('/').to_string()
}
