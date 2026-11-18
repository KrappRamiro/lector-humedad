use esp_idf_svc::hal::delay::FreeRtos;

/// On a fail, logs why, waits 10s, and restarts the board
pub fn fail_and_restart(reason: &anyhow::Error) -> ! {
    log::error!("restarting: {reason:?}");
    //TODO: Buzzer sounds go here
    FreeRtos::delay_ms(10_000);
    esp_idf_svc::hal::reset::restart()
}
