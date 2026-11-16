use embassy_time::{Duration, Timer};
use esp_idf_svc::hal::gpio::{GpioError, Output, PinDriver};
use fak_core::{
    gauge::Gauge,
    readout::{FOUR_DIGITS, NumericScreen, Readout, readout_for},
};
use tm1637_embedded_hal::{TM1637, tokens::Async};

/// Type alias for the TM1637 display. Dicho en español, el tipo asi crudo es un bardo.
/// Build one and pass it to `TM1637Screen::init`, and draw through `TM1637Screen`.
pub type TM1637Display =
    TM1637<4, Async, PinDriver<'static, Output>, PinDriver<'static, Output>, embassy_time::Delay>;

/// A screen the firmware can draw a `Readout` on
pub trait Draw: NumericScreen {
    type Error: std::fmt::Debug;

    /// Displays a `Readout` onto the screen
    async fn show(&mut self, readout: Readout<Self::Number>) -> Result<(), Self::Error>;
}

/// The TM1637 module as a `NumericScreen` and a `Draw`
/// its newtype to get around orphan rule.
pub struct TM1637Screen(TM1637Display);

impl Draw for TM1637Screen {
    type Error = tm1637_embedded_hal::Error<GpioError>;
    async fn show(&mut self, readout: Readout<Self::Number>) -> Result<(), Self::Error> {
        let word = match readout {
            Readout::Number(n) => return self.0.options().r_i16_4(n).display().await,
            Readout::TooHigh => "HIGH",
            Readout::TooLow => "  LO",
            Readout::On => "  On",
            Readout::Off => " OFF",
            Readout::NoReading => "----",
        };
        self.0.options().str(word).display().await
    }
}

impl TM1637Screen {
    /// Runs the driver `init()`
    pub async fn init(
        mut display: TM1637Display,
    ) -> Result<Self, tm1637_embedded_hal::Error<GpioError>> {
        display.init().await?;
        Ok(Self(display))
    }
}

impl NumericScreen for TM1637Screen {
    type Number = i16;
    const DRAWABLE_RANGE: std::ops::RangeInclusive<Self::Number> = FOUR_DIGITS;
}

/// How often the display loop checks the gauge for a new readout
const REFRESH_INTERVAL: Duration = Duration::from_millis(100);

/// Draws the gauge latest reading on `screen`. Forever.
/// Writes to the screen only when the readout changes.
pub async fn display_loop<S: Draw>(gauge: &dyn Gauge, screen: &mut S) {
    let mut last_drawn: Option<Readout<S::Number>> = None;
    let mut failing = false;
    loop {
        let readout = readout_for::<S>(gauge.latest());
        if last_drawn == Some(readout) {
            Timer::after(REFRESH_INTERVAL).await;
            continue;
        }
        match screen.show(readout).await {
            Ok(()) => {
                if failing {
                    log::info!("Display recovered");
                    failing = false;
                }
                last_drawn = Some(readout);
            }
            Err(e) => {
                if !failing {
                    log::warn!("Display write failed: {e:?}");
                    failing = true;
                }
                last_drawn = None;
            }
        }
        Timer::after(REFRESH_INTERVAL).await;
    }
}
