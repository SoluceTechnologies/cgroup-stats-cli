mod common;

use cgroup_stats_cli::utils::cgfile::{device_name, normalize_path, require};
use cgroup_stats_cli::utils::parse::max_value_to_option;
use cgroups_rs::fs::MaxValue;
use common::tmpdir;
use std::fs;
use std::path::Path;

#[test]
fn require_passes_when_every_file_is_present() {
    let dir = tmpdir("require-ok");
    fs::write(dir.join("a"), "1").unwrap();
    fs::write(dir.join("b"), "2").unwrap();
    assert!(require(&dir, &["a", "b"]).is_ok());
}

#[test]
fn require_names_the_missing_file() {
    let dir = tmpdir("require-missing");
    fs::write(dir.join("a"), "1").unwrap();
    let err = require(&dir, &["a", "b"]).unwrap_err();
    assert!(
        err.contains("b"),
        "error should name the missing file, got: {err}"
    );
}

#[test]
fn max_value_max_and_negative_are_unlimited() {
    assert_eq!(max_value_to_option(Some(MaxValue::Max)), None);
    assert_eq!(max_value_to_option(Some(MaxValue::Value(-1))), None);
    assert_eq!(max_value_to_option(None), None);
    assert_eq!(max_value_to_option(Some(MaxValue::Value(4096))), Some(4096));
}

#[test]
fn device_name_falls_back_to_major_minor_when_unresolvable() {
    let dir = tmpdir("devname");
    assert_eq!(device_name(&dir, "8:0"), "8:0");
}

#[test]
fn absolute_and_relative_paths_normalize_the_same() {
    let root = Path::new("/sys/fs/cgroup");
    assert_eq!(normalize_path("/sys/fs/cgroup/a/b", root), "a/b");
    assert_eq!(normalize_path("a/b", root), "a/b");
}

#[test]
fn normalize_handles_trailing_and_leading_slashes() {
    let root = Path::new("/sys/fs/cgroup");
    assert_eq!(normalize_path("/sys/fs/cgroup/a/b/", root), "a/b");
    assert_eq!(normalize_path("/a/b", root), "a/b");
    assert_eq!(normalize_path("/sys/fs/cgroup", root), "");
    assert_eq!(normalize_path("/sys/fs/cgroup/", root), "");
}

#[test]
fn normalize_respects_a_non_standard_mount_point() {
    let root = Path::new("/mnt/cg2");
    assert_eq!(normalize_path("/mnt/cg2/svc.slice", root), "svc.slice");
}

#[test]
fn normalize_does_not_strip_a_sibling_that_merely_shares_a_prefix() {
    let root = Path::new("/sys/fs/cgroup");
    assert_eq!(
        normalize_path("/sys/fs/cgroup-old/svc.slice", root),
        "sys/fs/cgroup-old/svc.slice"
    );
    assert_eq!(
        normalize_path("/sys/fs/cgroup2/foo", root),
        "sys/fs/cgroup2/foo"
    );
}
