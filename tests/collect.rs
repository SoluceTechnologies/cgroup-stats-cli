mod common;

use cgroup_stats_cli::cli::Selection;
use cgroup_stats_cli::task::collect;
use common::v2;
use std::time::Instant;

const ALL: Selection = Selection {
    cpu: true,
    mem: true,
    pids: true,
    io: true,
};
const MEM: Selection = Selection {
    cpu: false,
    mem: true,
    pids: false,
    io: false,
};

#[test]
fn memory_only_does_not_sleep() {
    if !v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let t = Instant::now();
    collect("", MEM, 5.0).unwrap();
    assert!(
        t.elapsed().as_secs_f64() < 1.0,
        "a memory-only run must skip the sampling sleep, took {:?}",
        t.elapsed()
    );
}

#[test]
fn root_cgroup_reports_memory_unavailable_not_zero() {
    // Regression guard. The root cgroup has no memory.current; without the
    // existence precheck cgroups-rs reports a confident 0 / unlimited.
    if !v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let s = collect("", ALL, 0.05).unwrap();
    assert!(
        matches!(s.memory, Some(Err(_))),
        "expected memory unavailable at the root, got {:?}",
        s.memory
    );
}

#[test]
fn unselected_metrics_are_none() {
    if !v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let s = collect("", MEM, 0.05).unwrap();
    assert!(s.cpu.is_none() && s.pids.is_none() && s.io.is_none());
    assert!(s.memory.is_some());
}

#[test]
fn a_missing_cgroup_is_a_fatal_error() {
    if !v2() {
        eprintln!("skipped: host is not cgroup v2");
        return;
    }
    let e = collect("definitely/not/a/real/cgroup", ALL, 0.05).unwrap_err();
    assert!(e.to_string().contains("cgroup not found"), "got: {e}");
}

#[test]
fn a_non_finite_or_non_positive_interval_is_an_error_not_a_panic() {
    // No v2() guard: the interval check runs before any host inspection,
    // so this test is meaningful on every machine.
    for bad in [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        -1.0,
        0.0,
        1e20,
        f64::MAX,
    ] {
        assert!(
            collect("", ALL, bad).is_err(),
            "interval {bad} should be rejected, not panic"
        );
    }
}
