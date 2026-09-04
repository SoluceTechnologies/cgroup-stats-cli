use cgroup_stats_cli::task::Stats;
use cgroup_stats_cli::task::format::{human, json, table};
use cgroup_stats_cli::task::metrics::{Cpu, Io, IoDevice, Memory, Pids};

fn stats() -> Stats {
    Stats {
        path: "system.slice/foo.service".into(),
        cpu: Some(Ok(Cpu {
            used_cores: 0.53,
            max_cores: Some(2.0),
        })),
        memory: Some(Ok(Memory {
            high: None,
            current: 1_288_490_188,
            max: Some(4_294_967_296),
        })),
        pids: Some(Ok(Pids {
            current: 42,
            max: Some(512),
        })),
        io: Some(Ok(Io {
            devices: vec![IoDevice {
                device: "sda".into(),
                read_bytes_per_sec: 1_258_291.0,
                write_bytes_per_sec: 348_160.0,
            }],
        })),
    }
}

#[test]
fn human_renders_every_metric() {
    let out = human(&stats());
    assert!(out.contains("RAM:  1.2G / 4.0G"), "{out}");
    assert!(out.contains("CPU:  0.53 / 2.00 cores"), "{out}");
    assert!(out.contains("PIDs: 42 / 512"), "{out}");
    assert!(out.contains("sda  r 1.2M/s  w 340.0K/s"), "{out}");
}

#[test]
fn unlimited_renders_as_the_word_unlimited() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        high: None,
        current: 8192,
        max: None,
    }));
    stats.cpu = Some(Ok(Cpu {
        used_cores: 0.1,
        max_cores: None,
    }));
    let out = human(&stats);
    assert!(out.contains("8.0K / unlimited"), "{out}");
    assert!(out.contains("0.10 / unlimited cores"), "{out}");
}

#[test]
fn unavailable_metrics_say_why() {
    let mut stats = stats();
    stats.memory = Some(Err("memory.current not present".into()));
    let out = human(&stats);
    assert!(out.contains("n/a"), "{out}");
    assert!(out.contains("memory.current not present"), "{out}");
}

#[test]
fn unrequested_metrics_are_absent_entirely() {
    let mut stats = stats();
    stats.pids = None;
    stats.io = None;
    let out = human(&stats);
    assert!(!out.contains("PIDs"), "{out}");
    assert!(!out.contains("IO"), "{out}");
}

#[test]
fn io_with_no_devices_says_so_rather_than_printing_nothing() {
    let mut stats = stats();
    stats.io = Some(Ok(Io { devices: vec![] }));
    let out = human(&stats);
    assert!(out.contains("IO:"), "{out}");
    assert!(out.contains("no activity"), "{out}");
}

#[test]
fn json_uses_raw_values_and_null_for_unlimited() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        high: None,
        current: 8192,
        max: None,
    }));
    let parsed: serde_json::Value = serde_json::from_str(&json(&stats)).unwrap();
    assert_eq!(parsed["memory"]["current"], 8192);
    assert!(parsed["memory"]["max"].is_null());
    assert_eq!(parsed["pids"]["current"], 42);
}

#[test]
fn json_omits_unrequested_metrics() {
    let mut stats = stats();
    stats.io = None;
    let parsed: serde_json::Value = serde_json::from_str(&json(&stats)).unwrap();
    assert!(
        parsed.get("io").is_none(),
        "unrequested metrics must be absent: {parsed}"
    );
    assert!(parsed.get("memory").is_some());
}

#[test]
fn json_is_valid_when_a_metric_is_unavailable() {
    let mut stats = stats();
    stats.memory = Some(Err("memory.current not present".into()));
    let parsed: serde_json::Value = serde_json::from_str(&json(&stats)).unwrap();
    assert!(
        parsed.get("memory").is_none(),
        "unavailable metrics are omitted: {parsed}"
    );
}

#[test]
fn table_contains_the_values() {
    let out = table(&stats());
    assert!(out.contains("RAM"), "{out}");
    assert!(out.contains("1.2G"), "{out}");
    assert!(out.contains("sda"), "{out}");
}

#[test]
fn table_marks_an_unavailable_metric_and_says_why() {
    let mut stats = stats();
    stats.memory = Some(Err("memory.current not present".into()));
    let out = table(&stats);
    assert!(out.contains("n/a"), "{out}");
    assert!(out.contains("memory.current not present"), "{out}");
}

#[test]
fn table_omits_unrequested_metrics_entirely() {
    let mut stats = stats();
    stats.pids = None;
    stats.io = None;
    let out = table(&stats);
    assert!(!out.contains("PIDs"), "{out}");
    assert!(!out.contains("IO"), "{out}");
}

#[test]
fn both_renderers_hide_idle_devices_but_json_keeps_them() {
    let mut stats = stats();
    stats.io = Some(Ok(Io {
        devices: vec![
            IoDevice {
                device: "loop0".into(),
                read_bytes_per_sec: 0.0,
                write_bytes_per_sec: 0.0,
            },
            IoDevice {
                device: "nvme0n1".into(),
                read_bytes_per_sec: 2048.0,
                write_bytes_per_sec: 0.0,
            },
        ],
    }));
    let human_out = human(&stats);
    assert!(human_out.contains("nvme0n1"), "{human_out}");
    assert!(
        !human_out.contains("loop0"),
        "idle device must not appear in human output: {human_out}"
    );

    let table_out = table(&stats);
    assert!(table_out.contains("nvme0n1"), "{table_out}");
    assert!(
        !table_out.contains("loop0"),
        "idle device must not appear in table output: {table_out}"
    );

    let parsed: serde_json::Value = serde_json::from_str(&json(&stats)).unwrap();
    let devs = parsed["io"]["devices"].as_array().unwrap();
    assert_eq!(devs.len(), 2, "json must keep idle devices: {parsed}");
}

#[test]
fn all_devices_idle_renders_as_no_activity() {
    let mut stats = stats();
    stats.io = Some(Ok(Io {
        devices: vec![IoDevice {
            device: "loop0".into(),
            read_bytes_per_sec: 0.0,
            write_bytes_per_sec: 0.0,
        }],
    }));
    assert!(human(&stats).contains("no activity"), "{}", human(&stats));
    assert!(table(&stats).contains("no activity"), "{}", table(&stats));
}

#[test]
fn memory_high_is_labelled_and_shown_beside_max() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        current: 1_288_490_188,
        high: Some(2_147_483_648),
        max: Some(4_294_967_296),
    }));
    assert!(
        human(&stats).contains("RAM:  1.2G / 2.0G high / 4.0G max"),
        "{}",
        human(&stats)
    );
    assert!(
        table(&stats).contains("2.0G high / 4.0G max"),
        "{}",
        table(&stats)
    );
}

#[test]
fn memory_high_alone_does_not_render_as_unlimited() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        current: 1_932_735_283,
        high: Some(2_147_483_648),
        max: None,
    }));
    let human_out = human(&stats);
    assert!(human_out.contains("RAM:  1.8G / 2.0G high"), "{human_out}");
    assert!(
        !human_out.contains("unlimited"),
        "a cgroup capped by memory.high is not unlimited: {human_out}"
    );
}

#[test]
fn output_is_unchanged_when_memory_high_is_unset() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        current: 1_288_490_188,
        high: None,
        max: Some(4_294_967_296),
    }));
    assert!(
        human(&stats).contains("RAM:  1.2G / 4.0G"),
        "{}",
        human(&stats)
    );

    stats.memory = Some(Ok(Memory {
        current: 8192,
        high: None,
        max: None,
    }));
    assert!(
        human(&stats).contains("RAM:  8.0K / unlimited"),
        "{}",
        human(&stats)
    );
}

#[test]
fn json_reports_high_alongside_max() {
    let mut stats = stats();
    stats.memory = Some(Ok(Memory {
        current: 100,
        high: Some(2048),
        max: None,
    }));
    let parsed: serde_json::Value = serde_json::from_str(&json(&stats)).unwrap();
    assert_eq!(parsed["memory"]["high"], 2048);
    assert!(parsed["memory"]["max"].is_null());
}
