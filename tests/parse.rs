mod common;

use cgroup_stats_cli::utils::parse::{parse_cpu_max, parse_flat_key, parse_io_stat, rate};
use common::CPU_STAT;

#[test]
fn reads_a_key_that_is_first() {
    assert_eq!(parse_flat_key(CPU_STAT, "usage_usec"), Some(38354643000));
}

#[test]
fn reads_a_key_that_is_not_first() {
    assert_eq!(parse_flat_key(CPU_STAT, "system_usec"), Some(8899896000));
    assert_eq!(parse_flat_key(CPU_STAT, "nice_usec"), Some(47780000));
}

#[test]
fn missing_key_is_none_not_a_panic() {
    assert_eq!(parse_flat_key(CPU_STAT, "nope"), None);
    assert_eq!(parse_flat_key("", "usage_usec"), None);
}

#[test]
fn key_match_is_exact_not_a_prefix() {
    assert_eq!(parse_flat_key(CPU_STAT, "usage"), None);
    assert_eq!(parse_flat_key(CPU_STAT, "usec"), None);
}

#[test]
fn cpu_max_with_a_quota_gives_quota_and_period() {
    assert_eq!(parse_cpu_max("200000 100000\n"), Some((200000, 100000)));
}

#[test]
fn cpu_max_unlimited_is_none() {
    assert_eq!(parse_cpu_max("max 100000\n"), None);
}

#[test]
fn cpu_max_malformed_is_none() {
    assert_eq!(parse_cpu_max(""), None);
    assert_eq!(parse_cpu_max("200000"), None);
    assert_eq!(parse_cpu_max("abc def"), None);
}

#[test]
fn cpu_max_with_a_zero_period_is_none() {
    assert_eq!(parse_cpu_max("200000 0\n"), None);
}

#[test]
fn io_stat_parses_multiple_devices() {
    let text = "\
7:55 rbytes=14336 wbytes=0 rios=11 wios=0 dbytes=0 dios=0
259:0 rbytes=966656 wbytes=512 rios=17 wios=3 dbytes=0 dios=0
";
    let devices = parse_io_stat(text);
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].device, "7:55");
    assert_eq!(devices[0].rbytes, 14336);
    assert_eq!(devices[1].device, "259:0");
    assert_eq!(devices[1].wbytes, 512);
}

#[test]
fn io_stat_keeps_lines_carrying_extra_iocost_keys() {
    let text = "8:0 rbytes=180224 wbytes=0 rios=3 wios=0 dbytes=0 dios=0 \
cost.usage=123 cost.wait=0 cost.indebt=0 cost.indelay=0\n";
    let devices = parse_io_stat(text);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].rbytes, 180224);
}

#[test]
fn io_stat_handles_a_large_minor() {
    let text = "253:1048575 rbytes=1 wbytes=2 rios=0 wios=0 dbytes=0 dios=0\n";
    let devices = parse_io_stat(text);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].device, "253:1048575");
}

#[test]
fn io_stat_empty_is_an_empty_list_not_an_error() {
    assert!(parse_io_stat("").is_empty());
    assert!(parse_io_stat("\n\n").is_empty());
}

#[test]
fn io_stat_skips_junk_lines_without_panicking() {
    let text = "garbage\n8:0 rbytes=5 wbytes=6\nalso junk\n";
    let devices = parse_io_stat(text);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].rbytes, 5);
}

#[test]
fn rate_divides_the_delta_by_elapsed_time() {
    assert_eq!(rate(0, 1000, 2.0), 500.0);
    assert_eq!(rate(1_000_000, 3_000_000, 1.0), 2_000_000.0);
}

#[test]
fn rate_saturates_when_the_counter_resets() {
    assert_eq!(rate(5000, 10, 1.0), 0.0);
}

#[test]
fn rate_of_zero_elapsed_is_zero_not_infinity() {
    assert_eq!(rate(0, 100, 0.0), 0.0);
}

#[test]
fn rate_of_non_finite_elapsed_is_zero() {
    assert_eq!(rate(0, 100, f64::NAN), 0.0);
    assert_eq!(rate(0, 100, -1.0), 0.0);
}
