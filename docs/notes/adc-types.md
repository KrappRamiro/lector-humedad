# ADC types in `MoistureSensor`

Notes moved out of `src/moisture_sensor.rs`.

## The five names

- `Pin`: the physical pin on the board.
- `AdcUnit`: the ADC hardware block inside the chip that does the
  analog-to-digital conversion (ADC1 here).
- `AdcChannel`: a pin together with the ADC unit it is wired to.
- `AdcDriver`: a handle that claims a whole ADC unit. It knows nothing about
  any specific pin.
- `AdcChannelDriver`: the same idea for one channel (pin plus unit).

## Why `MoistureSensor` has a generic `C`

Creating a sensor needs a pin and an ADC unit. The thing that combines both is
a channel, so the struct is generic over `C: AdcChannel`.

## Reading `AdcChannelDriver` on docs.rs

Source: https://esp-rs.github.io/esp-idf-svc/esp_idf_svc/hal/adc/oneshot/struct.AdcChannelDriver.html

```rust
pub struct AdcChannelDriver<'d, C, M>
where
    C: AdcChannel,
    M: Borrow<AdcDriver<'d, <C as AdcChannel>::AdcUnit>>,
```

`C` is the channel. `M` looks opaque, but `new()` on the same page shows
what it is:

```rust
pub fn new(
    adc: M,
    pin: impl ADCPin<AdcChannel = C> + 'd,
    config: &config::AdcChannelConfig,
) -> Result<Self, EspError>
```

`adc` is the ADC driver. The `Borrow` bound means it can be an owned
`AdcDriver` or a `&AdcDriver`. Read the three parameters as:

```text
AdcChannelDriver<
    lifetime,
    pin type (the channel C),
    something that borrows as an ADC driver,
>
```

## Why the sensor holds `&AdcDriver`

If the sensor owned the `AdcDriver`, it would own the whole ADC unit. Other
pins on the same unit could not get a channel. A reference lets several
sensors share one unit.

## The `impl` header

```rust
impl<'d, C: AdcChannel> MoistureSensor<'d, C> { ... }
```

Reads as: for any lifetime `'d` and any channel type `C` that implements
`AdcChannel`, here are the methods of `MoistureSensor<'d, C>`.

## The `pin` parameter

Its type is copied from `AdcChannelDriver::new()`: any type implementing
`ADCPin`, as long as the channel it is wired to equals `C`.
