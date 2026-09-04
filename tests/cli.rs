use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn bin() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    if path.ends_with("deps") {
        path.pop();
    }
    path.join("cgroup-stats-cli")
}

fn is_v2() -> bool {
    PathBuf::from("/sys/fs/cgroup/cgroup.controllers").exists()
}

fn leaf() -> Option<PathBuf> {
    let root = PathBuf::from("/sys/fs/cgroup");
    std::fs::read_dir(&root).ok()?.flatten().find_map(|entry| {
        let path = entry.path();
        (path.is_dir() && path.join("cpu.max").exists()).then_some(path)
    })
}

#[test]
fn reports_every_metric_for_a_real_leaf_cgroup() {
    if !is_v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let Some(leaf) = leaf() else {
        eprintln!("skipped: no leaf cgroup with cpu.max found");
        return;
    };
    let out = Command::new(bin())
        .args(["--path", leaf.to_str().unwrap(), "-i", "0.1"])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    for want in ["RAM:", "CPU:", "PIDs:", "IO:"] {
        assert!(stdout.contains(want), "missing {want} in:\n{stdout}");
    }
}

#[test]
fn json_output_parses() {
    if !is_v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let Some(leaf) = leaf() else { return };
    let out = Command::new(bin())
        .args(["--path", leaf.to_str().unwrap(), "-i", "0.1", "-f", "json"])
        .output()
        .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(parsed["path"].is_string());
}

#[test]
fn memory_only_skips_the_sampling_sleep() {
    if !is_v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let Some(leaf) = leaf() else { return };
    let started = Instant::now();
    let out = Command::new(bin())
        .args(["--path", leaf.to_str().unwrap(), "--mem", "-i", "10"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        started.elapsed().as_secs() < 5,
        "a --mem run must not honour --interval, took {:?}",
        started.elapsed()
    );
}

#[test]
fn a_missing_cgroup_exits_one_with_a_message() {
    if !is_v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let out = Command::new(bin())
        .args(["--path", "/sys/fs/cgroup/definitely-not-real"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&out.stderr).contains("cgroup not found"));
}

#[test]
fn a_bad_interval_is_an_argument_error_exiting_two() {
    let out = Command::new(bin())
        .args(["--path", "/sys/fs/cgroup", "-i", "0"])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(2));
}
