//! Shared view utilities

/// Format a byte count as a human-readable string.
pub fn format_bytes(bytes: u64) -> String {
    const GB: u64 = 1_073_741_824;
    const MB: u64 = 1_048_576;
    const KB: u64 = 1_024;
    if bytes == 0 {
        "0 B".to_string()
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes_zero() {
        assert_eq!(format_bytes(0), "0 B");
    }

    #[test]
    fn test_format_bytes_bytes() {
        assert_eq!(format_bytes(512), "512 B");
    }

    #[test]
    fn test_format_bytes_exact_kb() {
        assert_eq!(format_bytes(1_024), "1 KB");
    }

    #[test]
    fn test_format_bytes_mb() {
        assert_eq!(format_bytes(5 * 1_048_576), "5.0 MB");
    }

    #[test]
    fn test_format_bytes_gb() {
        let s = format_bytes(2 * 1_073_741_824);
        assert_eq!(s, "2.00 GB");
    }

    #[test]
    fn test_format_bytes_half_gb() {
        let half_gb = 1_073_741_824 / 2;
        let s = format_bytes(half_gb);
        assert_eq!(s, "512.0 MB");
    }
}
