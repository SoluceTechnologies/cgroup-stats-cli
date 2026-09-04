#![allow(dead_code)]

use std::path::PathBuf;

pub const CPU_STAT: &str = "\
usage_usec 38354643000
user_usec 29454747000
system_usec 8899896000
nice_usec 47780000
core_sched.force_idle_usec 0
";

pub fn tmpdir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cgstats-test-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

pub fn v2() -> bool {
    cgroups_rs::fs::hierarchies::is_cgroup2_unified_mode()
}
