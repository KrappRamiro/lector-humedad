//! Checks for host names given in `cfg.toml`, such as `sntp_server`.

use std::net::IpAddr;

/// Checks if a value is a bare IP address or hostname: no scheme, port, path, or
/// trailing dot.
///
/// A hostname follows RFC 1123: at most 253 bytes in total, labels of 1 to 63
/// letters, digits, or `-`, and no label starts or ends with `-`. The last label
/// can't be all digits (RFC 3696, section 2), so a broken IP such as
/// `192.168.1.300` is rejected instead of being looked up as a name.
///
/// A trailing dot (`pool.ntp.org.`) is rejected: lwIP's DNS client encodes it as
/// an empty label and sends a malformed query.
///
/// ```
/// use fak_core::host::is_bare_host;
///
/// assert!(is_bare_host("0.pool.ntp.org"));
/// assert!(is_bare_host("192.168.1.10"));
/// assert!(!is_bare_host("ntp://0.pool.ntp.org"));
/// assert!(!is_bare_host("192.168.1.300"));
/// ```
#[must_use]
pub fn is_bare_host(value: &str) -> bool {
    if value.parse::<IpAddr>().is_ok() {
        return true;
    }
    let last_label_is_numeric = value
        .rsplit('.')
        .next()
        .is_some_and(|label| label.bytes().all(|b| b.is_ascii_digit()));
    value.len() <= 253 && !last_label_is_numeric && value.split('.').all(is_label)
}

/// Checks one dot-separated part of a hostname against RFC 1123.
fn is_label(label: &str) -> bool {
    (1..=63).contains(&label.len())
        && !label.starts_with('-')
        && !label.ends_with('-')
        && label
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts() {
        for host in [
            // Hostnames
            "0.pool.ntp.org",
            "localhost",
            "a",
            "POOL.NTP.ORG",
            "time-a.nist.gov",
            "a--b.example",
            // RFC 1123 allows a label to start with a digit, and only the last
            // label must have a letter.
            "1e100.net",
            "192.168.1.example",
            "ntp.1a",
            // IPv4
            "192.168.1.10",
            "0.0.0.0",
            "255.255.255.255",
            // IPv6
            "::1",
            "2001:db8::123",
            "::ffff:192.168.1.10",
        ] {
            assert!(is_bare_host(host), "{host:?} should be accepted");
        }
    }

    #[test]
    fn rejects() {
        for host in [
            "",
            // Scheme, port, path, or user info
            "ntp://0.pool.ntp.org",
            "https://pool.ntp.org",
            "0.pool.ntp.org:123",
            "192.168.1.10:123",
            "[::1]:123",
            "[::1]",
            "pool.ntp.org/",
            "pool.ntp.org/time",
            "user@pool.ntp.org",
            // Whitespace
            " pool.ntp.org",
            "pool.ntp.org ",
            "pool .ntp.org",
            "pool.ntp.org\t",
            // Broken IPs: an all-digit last label must be a valid IP.
            // lwIP reads a leading 0 as octal, so 010 would mean 8.
            "192.168.1.300",
            "256.0.0.0",
            "10.0.0",
            "10",
            "1.2.3.4.5",
            "192.168.001.010",
            "2001:db8:::1",
            ":::",
            "fe80::1%eth0",
            // lwIP's DNS client turns a trailing dot into a malformed query.
            "pool.ntp.org.",
            // Empty labels
            ".pool.ntp.org",
            "pool..ntp.org",
            ".",
            // A label can't start or end with a hyphen.
            "-pool.ntp.org",
            "pool-.ntp.org",
            "pool.ntp.org-",
            "-",
            // Only ASCII letters, digits, and hyphens. `_` is allowed in some DNS
            // records, but not in a hostname.
            "my_ntp.local",
            "ñandú.example",
            "pool.ntp.org\u{200b}",
        ] {
            assert!(!is_bare_host(host), "{host:?} should be rejected");
        }
    }

    #[test]
    fn length_limits() {
        let label = |len| "a".repeat(len);
        let name = |last| format!("{0}.{0}.{0}.{1}", label(63), label(last));

        assert!(is_bare_host(&label(63)), "63-byte label");
        assert!(!is_bare_host(&label(64)), "64-byte label");
        assert!(is_bare_host(&name(61)), "253-byte name");
        assert!(!is_bare_host(&name(62)), "254-byte name");
        // 'é' is 2 bytes: the limit counts bytes, not chars.
        assert!(!is_bare_host(&"é".repeat(32)), "non-ASCII name");
    }
}
