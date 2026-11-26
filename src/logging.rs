/// The rust modules that have `log::debug` enabled.
/// Be wary that ESP-IDF matches tags exactly, so `lector_humedad` does not cover its submodules.
/// Please add your module here if you want debug.
const DEBUG_TARGETS: &[&str] = &[
    "lector_humedad",
    "lector_humedad::display",
    "lector_humedad::logging",
    //"lector_humedad::moisture_sensor",
    "lector_humedad::restart",
    "lector_humedad::wifi",
];

/// Installs the logger and sets each target level.
/// Call it once at boot.
pub fn init() -> anyhow::Result<()> {
    let logger = esp_idf_svc::log::init_from_esp_idf();
    for target in DEBUG_TARGETS {
        logger
            .filter()
            .set_target_level(target, log::LevelFilter::Debug)?;
    }
    Ok(())
}
