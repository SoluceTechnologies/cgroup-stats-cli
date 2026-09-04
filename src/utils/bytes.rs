/// Format bytes with IEC units, always to one decimal place above `B` (`1.5K`,
/// `4.0G`). Deliberately fixed-width — unlike `numfmt --to=iec`, which uses
/// three significant digits and prints `340K` where this prints `340.0K` — so
/// columns do not jump around under `watch`.
pub fn iec(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit < UNITS.len() - 1 && (value * 10.0).round() / 10.0 >= 1024.0 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes}B")
    } else {
        format!("{value:.1}{}", UNITS[unit])
    }
}

pub(crate) fn limit(bytes: Option<u64>) -> String {
    bytes.map_or("unlimited".to_string(), iec)
}
