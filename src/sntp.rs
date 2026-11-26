//! Sets the system clock from an SNTP server.

use embassy_time::Instant;
use esp_idf_svc::{
    hal::delay::FreeRtos,
    sntp::{EspSntp, SntpConf, SyncStatus},
};

/// Starts the SNTP client and waits until the clock has synced.
///
/// The returned client keeps the clock in sync. DO NOT drop it, it would stop the client.
///
/// `server` must be a bare hostname or IP address.
/// OK:
/// - `0.pool.ntp.org`
/// - `192.168.1.10`
///
/// WRONG:
/// - `ntp://0.pool.ntp.org`
/// - `0.pool.ntp.org:123`
/// - `https://etc.....`
///
/// # Errors
///
/// Returns an error if the client can't start, or if the clock hasn't synced within `timeout`.
pub fn sync(server: &str, timeout: embassy_time::Duration) -> anyhow::Result<EspSntp<'static>> {
    // `servers` has one slot per CONFIG_LWIP_SNTP_MAX_SERVERS. Filling only the first
    // keeps this compiling if that option grows, and the other slots keep
    // esp-idf-svc's default pool servers as fallbacks.
    let mut conf = SntpConf::default();
    conf.servers[0] = server;

    let sntp = EspSntp::new(&conf)?;
    log::debug!("waiting for the clock to sync with {server}");

    let started = Instant::now();
    let deadline = started + timeout;
    loop {
        if sntp.get_sync_status() == SyncStatus::Completed {
            log::debug!("clock synced after {} ms", started.elapsed().as_millis());
            return Ok(sntp);
        }
        if Instant::now() > deadline {
            anyhow::bail!(
                "the clock didn't sync within the deadline of {} ms",
                timeout.as_millis()
            )
        }
        FreeRtos::delay_ms(100);
    }
}
