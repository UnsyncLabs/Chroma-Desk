//! Stable device identity, independent of OpenRGB's volatile indices and `/dev/hidraw` paths.
//!
//! Key = `name#serial` when the device reports a serial, otherwise `name@ordinal`, where
//! the ordinal counts devices with the same name in server order.

use std::collections::HashMap;

/// Computes the identity key for every `(name, serial)` pair, in order.
pub fn device_keys<'a>(devices: impl IntoIterator<Item = (&'a str, &'a str)>) -> Vec<String> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    devices
        .into_iter()
        .map(|(name, serial)| {
            let ordinal = seen.entry(name).or_insert(0);
            let n = *ordinal;
            *ordinal += 1;
            if serial.trim().is_empty() {
                format!("{name}@{n}")
            } else {
                format!("{name}#{}", serial.trim())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serial_wins_over_ordinal() {
        let k = device_keys([("G502", "ABC123"), ("G502", "XYZ")]);
        assert_eq!(k, vec!["G502#ABC123", "G502#XYZ"]);
    }

    #[test]
    fn ordinal_counts_same_names_only() {
        let k = device_keys([("ENE DRAM", ""), ("Mouse", ""), ("ENE DRAM", " "), ("ENE DRAM", "")]);
        assert_eq!(k, vec!["ENE DRAM@0", "Mouse@0", "ENE DRAM@1", "ENE DRAM@2"]);
    }

    #[test]
    fn keys_ignore_location_and_are_unique() {
        let k = device_keys([("A", ""), ("A", ""), ("B", "s")]);
        let set: std::collections::HashSet<_> = k.iter().collect();
        assert_eq!(set.len(), k.len());
    }
}
