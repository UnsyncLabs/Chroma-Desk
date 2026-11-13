//! Prints the system monitor readings for a few seconds (compare with nvidia-smi / sensors / top).
use chroma_desk_lib::sysmon::{self, ALL};

fn main() {
    println!("{}", serde_json::to_string_pretty(&sysmon::status()).unwrap());
    for _ in 0..6 {
        for m in ALL {
            sysmon::reading(m);
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
        let line: Vec<String> = ALL
            .iter()
            .map(|m| match sysmon::reading(*m) {
                Some(r) => format!("{}={:.1}{}{}", m.id(), r.value, m.unit(), r.used.map(|u| format!(" ({:.0}/{:.0} MiB)", u / 1048576.0, r.total.unwrap_or(0.0) / 1048576.0)).unwrap_or_default()),
                None => format!("{}=n/a", m.id()),
            })
            .collect();
        println!("{}", line.join("  "));
    }
}
