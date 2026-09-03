use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

fn bin() -> PathBuf {
    // target/debug/deps/<test binary> -> target/debug/cgroup-stats-cli
    let mut p = std::env::current_exe().unwrap();
    p.pop();
    if p.ends_with("deps") {
        p.pop();
    }
    p.join("cgroup-stats-cli")
}

fn is_v2() -> bool {
    PathBuf::from("/sys/fs/cgroup/cgroup.controllers").exists()
}

/// The root cgroup has no cpu.max, so a leaf must be found for a meaningful
/// end-to-end check. Returns None when nothing suitable exists.
fn leaf() -> Option<PathBuf> {
    let root = PathBuf::from("/sys/fs/cgroup");
    std::fs::read_dir(&root).ok()?.flatten().find_map(|e| {
        let p = e.path();
        (p.is_dir() && p.join("cpu.max").exists()).then_some(p)
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
    let s = String::from_utf8_lossy(&out.stdout);
    for want in ["RAM:", "CPU:", "PIDs:", "IO:"] {
        assert!(s.contains(want), "missing {want} in:\n{s}");
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
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(v["path"].is_string());
}

#[test]
fn memory_only_skips_the_sampling_sleep() {
    if !is_v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let Some(leaf) = leaf() else { return };
    let t = Instant::now();
    let out = Command::new(bin())
        .args(["--path", leaf.to_str().unwrap(), "--mem", "-i", "10"])
        .output()
        .unwrap();
    assert!(out.status.success());
    assert!(
        t.elapsed().as_secs() < 5,
        "a --mem run must not honour --interval, took {:?}",
        t.elapsed()
    );
}

#[test]
fn a_missing_cgroup_exits_one_with_a_message() {
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
