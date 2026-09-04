use cgroup_stats_cli::cli::Args;
use clap::{CommandFactory, Parser};

#[test]
fn clap_definition_is_valid() {
    Args::command().debug_assert();
}

#[test]
fn no_metric_flags_selects_everything() {
    let a = Args::parse_from(["x", "--path", "/sys/fs/cgroup"]);
    let s = a.selection();
    assert!(s.cpu && s.mem && s.pids && s.io);
}

#[test]
fn explicit_flags_select_only_those() {
    let a = Args::parse_from(["x", "--path", "p", "--cpu", "--mem"]);
    let s = a.selection();
    assert!(s.cpu && s.mem);
    assert!(!s.pids && !s.io);
}

#[test]
fn sampling_needed_only_for_delta_metrics() {
    let mem_only = Args::parse_from(["x", "--path", "p", "--mem"]).selection();
    assert!(!mem_only.needs_sampling());

    let with_cpu = Args::parse_from(["x", "--path", "p", "--cpu"]).selection();
    assert!(with_cpu.needs_sampling());

    let with_io = Args::parse_from(["x", "--path", "p", "--io"]).selection();
    assert!(with_io.needs_sampling());

    let pids_only = Args::parse_from(["x", "--path", "p", "--pids"]).selection();
    assert!(!pids_only.needs_sampling());
}

#[test]
fn interval_must_be_positive() {
    assert!(Args::try_parse_from(["x", "--path", "p", "-i", "0"]).is_err());
    assert!(Args::try_parse_from(["x", "--path", "p", "-i", "-1"]).is_err());
    assert!(Args::try_parse_from(["x", "--path", "p", "-i", "0.5"]).is_ok());
}

#[test]
fn interval_rejects_non_finite_values() {
    for bad in ["nan", "NaN", "inf", "infinity", "-inf"] {
        assert!(
            Args::try_parse_from(["x", "--path", "p", "-i", bad]).is_err(),
            "{bad} should be rejected"
        );
    }
}

#[test]
fn interval_rejects_a_finite_but_unrepresentable_value() {
    // Duration::from_secs_f64 panics on overflow as well as on NaN and
    // infinity; 1e20 seconds is finite and positive but not representable.
    assert!(Args::try_parse_from(["x", "--path", "p", "-i", "1e20"]).is_err());
    assert!(Args::try_parse_from(["x", "--path", "p", "-i", "99999999999999999999"]).is_err());
}
