mod common;

use cgroup_stats_cli::task::metrics::{collect_cpu, collect_io, read_cpu_usage};
use cgroup_stats_cli::utils::parse::parse_io_stat;
use common::{CPU_STAT, tmpdir};
use std::fs;

#[test]
fn cpu_usage_comes_from_the_stat_file() {
    let dir = tmpdir("cpu-usage");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    assert_eq!(read_cpu_usage(&dir).unwrap(), 38354643000);
}

#[test]
fn cpu_usage_missing_file_is_an_error() {
    let dir = tmpdir("cpu-usage-missing");
    assert!(read_cpu_usage(&dir).is_err());
}

#[test]
fn cpu_usage_present_but_keyless_is_an_error() {
    let dir = tmpdir("cpu-usage-keyless");
    fs::write(dir.join("cpu.stat"), "user_usec 5\n").unwrap();
    assert!(read_cpu_usage(&dir).is_err());
}

#[test]
fn collect_cpu_computes_cores_from_the_delta() {
    let dir = tmpdir("cpu-cores");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    fs::write(dir.join("cpu.max"), "200000 100000\n").unwrap();
    let cpu = collect_cpu(&dir, 1_000_000, 1_500_000, 1.0).unwrap();
    assert!(
        (cpu.used_cores - 0.5).abs() < 1e-9,
        "got {}",
        cpu.used_cores
    );
    assert_eq!(cpu.max_cores, Some(2.0));
}

#[test]
fn collect_cpu_reports_unlimited_when_cpu_max_is_max() {
    let dir = tmpdir("cpu-unlimited");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    fs::write(dir.join("cpu.max"), "max 100000\n").unwrap();
    assert_eq!(collect_cpu(&dir, 0, 0, 1.0).unwrap().max_cores, None);
}

#[test]
fn collect_cpu_errors_on_an_unparseable_cpu_max_rather_than_claiming_unlimited() {
    let dir = tmpdir("cpu-max-junk");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    fs::write(dir.join("cpu.max"), "garbage\n").unwrap();
    let err = collect_cpu(&dir, 0, 0, 1.0).unwrap_err();
    assert!(
        err.contains("cpu.max"),
        "error should name the file, got: {err}"
    );
}

#[test]
fn collect_cpu_treats_an_absent_cpu_max_as_unlimited() {
    // The root cgroup's shape: cpu.stat present, cpu.max absent.
    let dir = tmpdir("cpu-max-absent");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    assert_eq!(collect_cpu(&dir, 0, 0, 1.0).unwrap().max_cores, None);
}

#[test]
fn collect_cpu_without_cpu_max_still_reports_usage() {
    let dir = tmpdir("cpu-no-max");
    fs::write(dir.join("cpu.stat"), CPU_STAT).unwrap();
    let cpu = collect_cpu(&dir, 0, 1_000_000, 1.0).unwrap();
    assert!((cpu.used_cores - 1.0).abs() < 1e-9);
    assert_eq!(cpu.max_cores, None);
}

#[test]
fn collect_io_pairs_devices_across_samples() {
    let dir = tmpdir("io-pair");
    let before = parse_io_stat("8:0 rbytes=1000 wbytes=2000 rios=0 wios=0 dbytes=0 dios=0\n");
    let after = parse_io_stat("8:0 rbytes=3000 wbytes=2500 rios=0 wios=0 dbytes=0 dios=0\n");
    let io = collect_io(&dir, &before, &after, 2.0);
    assert_eq!(io.devices.len(), 1);
    assert_eq!(io.devices[0].read_bytes_per_sec, 1000.0);
    assert_eq!(io.devices[0].write_bytes_per_sec, 250.0);
}

#[test]
fn collect_io_treats_a_device_new_in_the_second_sample_as_starting_at_zero() {
    let dir = tmpdir("io-new-dev");
    let before = parse_io_stat("");
    let after = parse_io_stat("8:0 rbytes=500 wbytes=0 rios=0 wios=0 dbytes=0 dios=0\n");
    let io = collect_io(&dir, &before, &after, 1.0);
    assert_eq!(io.devices.len(), 1);
    assert_eq!(io.devices[0].read_bytes_per_sec, 500.0);
}
