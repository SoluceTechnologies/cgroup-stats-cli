use cgroup_stats_cli::utils::bytes::iec;

#[test]
fn iec_boundaries() {
    assert_eq!(iec(0), "0B");
    assert_eq!(iec(1023), "1023B");
    assert_eq!(iec(1024), "1.0K");
    assert_eq!(iec(1025), "1.0K");
    assert_eq!(iec(1536), "1.5K");
    assert_eq!(iec(1024 * 1024), "1.0M");
    assert_eq!(iec(4 * 1024 * 1024 * 1024), "4.0G");
    assert_eq!(iec(2 * 1024_u64.pow(4)), "2.0T");
    // A value one byte below a unit boundary must not round up into a
    // "1024.0" mantissa of the smaller unit.
    assert_eq!(iec(1_048_575), "1.0M");
    assert_eq!(iec(1_073_741_823), "1.0G");
    assert_eq!(iec(1_099_511_627_775), "1.0T");
    // The ladder stops at the last unit rather than indexing past it.
    assert_eq!(iec(u64::MAX), "16384.0P");
}
