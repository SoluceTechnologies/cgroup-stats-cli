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
    // Pin the full line, not loose substrings: checking "1.2G" and "4.0G"
    // independently would pass even if current and max were swapped.
    assert!(out.contains("RAM:  1.2G / 4.0G"), "{out}");
    assert!(out.contains("CPU:  0.53 / 2.00 cores"), "{out}");
    assert!(out.contains("PIDs: 42 / 512"), "{out}");
    assert!(out.contains("sda  r 1.2M/s  w 340.0K/s"), "{out}");
}

#[test]
fn unlimited_renders_as_the_word_unlimited() {
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        high: None,
        current: 8192,
        max: None,
    }));
    s.cpu = Some(Ok(Cpu {
        used_cores: 0.1,
        max_cores: None,
    }));
    let out = human(&s);
    assert!(out.contains("8.0K / unlimited"), "{out}");
    assert!(out.contains("0.10 / unlimited cores"), "{out}");
}

#[test]
fn unavailable_metrics_say_why() {
    let mut s = stats();
    s.memory = Some(Err("memory.current not present".into()));
    let out = human(&s);
    assert!(out.contains("n/a"), "{out}");
    assert!(out.contains("memory.current not present"), "{out}");
}

#[test]
fn unrequested_metrics_are_absent_entirely() {
    let mut s = stats();
    s.pids = None;
    s.io = None;
    let out = human(&s);
    assert!(!out.contains("PIDs"), "{out}");
    assert!(!out.contains("IO"), "{out}");
}

#[test]
fn io_with_no_devices_says_so_rather_than_printing_nothing() {
    let mut s = stats();
    s.io = Some(Ok(Io { devices: vec![] }));
    let out = human(&s);
    assert!(out.contains("IO:"), "{out}");
    assert!(out.contains("no activity"), "{out}");
}

#[test]
fn json_uses_raw_values_and_null_for_unlimited() {
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        high: None,
        current: 8192,
        max: None,
    }));
    let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
    assert_eq!(v["memory"]["current"], 8192);
    assert!(v["memory"]["max"].is_null());
    assert_eq!(v["pids"]["current"], 42);
}

#[test]
fn json_omits_unrequested_metrics() {
    let mut s = stats();
    s.io = None;
    let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
    assert!(
        v.get("io").is_none(),
        "unrequested metrics must be absent: {v}"
    );
    assert!(v.get("memory").is_some());
}

#[test]
fn json_is_valid_when_a_metric_is_unavailable() {
    let mut s = stats();
    s.memory = Some(Err("memory.current not present".into()));
    let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
    assert!(
        v.get("memory").is_none(),
        "unavailable metrics are omitted: {v}"
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
    let mut s = stats();
    s.memory = Some(Err("memory.current not present".into()));
    let out = table(&s);
    assert!(out.contains("n/a"), "{out}");
    assert!(out.contains("memory.current not present"), "{out}");
}

#[test]
fn table_omits_unrequested_metrics_entirely() {
    let mut s = stats();
    s.pids = None;
    s.io = None;
    let out = table(&s);
    assert!(!out.contains("PIDs"), "{out}");
    assert!(!out.contains("IO"), "{out}");
}

#[test]
fn both_renderers_hide_idle_devices_but_json_keeps_them() {
    let mut s = stats();
    s.io = Some(Ok(Io {
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
    let h = human(&s);
    assert!(h.contains("nvme0n1"), "{h}");
    assert!(
        !h.contains("loop0"),
        "idle device must not appear in human output: {h}"
    );

    let t = table(&s);
    assert!(t.contains("nvme0n1"), "{t}");
    assert!(
        !t.contains("loop0"),
        "idle device must not appear in table output: {t}"
    );

    // JSON is the fidelity layer and keeps every device.
    let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
    let devs = v["io"]["devices"].as_array().unwrap();
    assert_eq!(devs.len(), 2, "json must keep idle devices: {v}");
}

#[test]
fn all_devices_idle_renders_as_no_activity() {
    let mut s = stats();
    s.io = Some(Ok(Io {
        devices: vec![IoDevice {
            device: "loop0".into(),
            read_bytes_per_sec: 0.0,
            write_bytes_per_sec: 0.0,
        }],
    }));
    assert!(human(&s).contains("no activity"), "{}", human(&s));
    assert!(table(&s).contains("no activity"), "{}", table(&s));
}

#[test]
fn memory_high_is_labelled_and_shown_beside_max() {
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        current: 1_288_490_188,
        high: Some(2_147_483_648),
        max: Some(4_294_967_296),
    }));
    assert!(
        human(&s).contains("RAM:  1.2G / 2.0G high / 4.0G max"),
        "{}",
        human(&s)
    );
    assert!(table(&s).contains("2.0G high / 4.0G max"), "{}", table(&s));
}

#[test]
fn memory_high_alone_does_not_render_as_unlimited() {
    // A systemd unit with MemoryHigh= and no MemoryMax= is throttled in
    // practice, so reporting "unlimited" here would be actively misleading.
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        current: 1_932_735_283,
        high: Some(2_147_483_648),
        max: None,
    }));
    let h = human(&s);
    assert!(h.contains("RAM:  1.8G / 2.0G high"), "{h}");
    assert!(
        !h.contains("unlimited"),
        "a cgroup capped by memory.high is not unlimited: {h}"
    );
}

#[test]
fn output_is_unchanged_when_memory_high_is_unset() {
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        current: 1_288_490_188,
        high: None,
        max: Some(4_294_967_296),
    }));
    assert!(human(&s).contains("RAM:  1.2G / 4.0G"), "{}", human(&s));

    s.memory = Some(Ok(Memory {
        current: 8192,
        high: None,
        max: None,
    }));
    assert!(
        human(&s).contains("RAM:  8.0K / unlimited"),
        "{}",
        human(&s)
    );
}

#[test]
fn json_reports_high_alongside_max() {
    let mut s = stats();
    s.memory = Some(Ok(Memory {
        current: 100,
        high: Some(2048),
        max: None,
    }));
    let v: serde_json::Value = serde_json::from_str(&json(&s)).unwrap();
    assert_eq!(v["memory"]["high"], 2048);
    assert!(v["memory"]["max"].is_null());
}
