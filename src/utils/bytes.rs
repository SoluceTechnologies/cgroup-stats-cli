/// Format bytes with IEC units, always to one decimal place above `B` (`1.5K`,
/// `4.0G`). Deliberately fixed-width — unlike `numfmt --to=iec`, which uses
/// three significant digits and prints `340K` where this prints `340.0K` — so
/// columns do not jump around under `watch`.
pub fn iec(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i < UNITS.len() - 1 && (v * 10.0).round() / 10.0 >= 1024.0 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes}B")
    } else {
        format!("{v:.1}{}", UNITS[i])
    }
}

pub(crate) fn limit(v: Option<u64>) -> String {
    v.map_or("unlimited".to_string(), iec)
}
