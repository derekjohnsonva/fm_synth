// synth_clock.rs

fn wrap_max(value: f32, max: f32) -> f32 {
    (max + value % max) % max
}

/// Wraps a value around a max value so that it falls within the max bounds.
/// Will wrap from min to max.
/// Honestly, I do not really understand this section. I copied it from `SynthLab`
///
/// Arguments
///
/// * `value` - The value to wrap
/// * `min` - The minimum value
/// * `max` - The maximum value
///
/// # Returns
///
/// The wrapped value
fn wrap_min_max(value: f32, min: f32, max: f32) -> f32 {
    min + wrap_max(value - min, max - min)
}
#[derive(Debug)]
pub struct Clock {
    // Public fields
    pub mcounter: f32,     // modulo counter [0.0, +1.0], this is the value you use
    pub phase_inc: f32,    // phase inc = fo/fs
    pub phase_offset: f32, // PM
    pub freq_offset: f32,  // FM
    pub frequency_hz: f32, // clock frequency
}

// Methods for SynthClock
/// Implementation of a synthesizer clock.
///
/// The `SynthClock` struct represents a clock used in a synthesizer. It keeps track of the current phase
/// and provides methods for advancing the clock and wrapping it around when necessary.
impl Clock {
    /// Creates a new `SynthClock` instance.
    ///
    /// Returns:
    /// - `SynthClock`: The newly created `SynthClock` instance.
    pub const fn new() -> Self {
        Self {
            mcounter: 0.0,
            phase_inc: 0.0,
            phase_offset: 0.0,
            freq_offset: 0.0,
            frequency_hz: 0.0,
        }
    }
    /// Resets the clock to its initial state.
    pub fn reset(&mut self) {
        self.mcounter = 0.0;
        self.phase_offset = 0.0;
        self.freq_offset = 0.0;
    }

    /// Advances the clock by a given render interval.
    ///
    /// Parameters:
    /// - `render_interval`: The render interval in seconds.
    pub fn advance_clock(&mut self, render_interval: f32) {
        self.mcounter += render_interval * self.phase_inc;
    }

    /// Wraps the clock around if necessary.
    ///
    /// If the modulo counter is greater than 1 or less than 0, it is wrapped around to keep it within the range of 0 to 1.
    pub fn wrap_clock(&mut self) {
        if self.mcounter > 1.0 && self.mcounter < 2.0 {
            self.mcounter -= 1.0;
        } else if self.mcounter < 0.0 && self.mcounter > -1.0 {
            self.mcounter += 1.0;
        } else {
            self.mcounter = wrap_min_max(self.mcounter, 0.0, 1.0);
        }
    }

    /// Advances the clock by a given render interval and wraps it around if necessary.
    ///
    /// Parameters:
    /// - `render_interval`: The render interval in seconds.
    pub fn advance_wrap_clock(&mut self, render_interval: f32) {
        self.advance_clock(render_interval);
        self.wrap_clock();
    }

    /// Sets the frequency and sample rate of the clock.
    ///
    /// This method is used for saving the state of the clock.
    ///
    /// Parameters:
    /// - `frequency_hz`: The frequency in Hz.
    /// - `sample_rate`: The sample rate in Hz.
    pub fn set_freq(&mut self, frequency_hz: f32, sample_rate: f32) {
        self.frequency_hz = frequency_hz;
        self.phase_inc = frequency_hz / sample_rate;
    }

    /// For phase modulation. Adds a phase offset to the clock.
    ///
    /// Parameters:
    /// - `phase_offset`: The phase offset to add.
    /// - `wrap`: Whether to wrap the clock around after adding the phase offset.
    pub fn add_phase_offset(&mut self, phase_offset: f32, wrap: bool) {
        self.phase_offset = phase_offset;
        if self.phase_inc > 0.0 {
            self.mcounter += phase_offset;
        } else {
            self.mcounter -= phase_offset;
        }
        if wrap {
            self.wrap_clock();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;
    use rstest::*;

    #[fixture]
    fn setup() {
        #[allow(clippy::unwrap_used)]
        color_eyre::install().unwrap();
    }

    #[rstest]
    fn test_wrap_min_max() {
        assert_relative_eq!(wrap_min_max(0.5, 0.0, 1.0), 0.5);
        assert_relative_eq!(wrap_min_max(1.5, 0.0, 1.0), 0.5);
        assert_relative_eq!(wrap_min_max(-0.5, 0.0, 1.0), 0.5);
        assert_relative_eq!(wrap_min_max(-1.5, 0.0, 1.0), 0.5);
        assert_relative_eq!(wrap_min_max(2.5, 0.0, 1.0), 0.5);
    }

    #[rstest]
    fn test_advance_clock() {
        let mut clock = Clock::new();
        clock.advance_clock(0.1);
        assert_relative_eq!(clock.mcounter, 0.0);

        clock.phase_inc = 1.0;
        clock.advance_clock(0.1);
        assert_relative_eq!(clock.mcounter, 0.1);

        clock.phase_inc = 2.0;
        clock.advance_clock(0.3);
        assert_relative_eq!(clock.mcounter, 0.7);
    }

    #[rstest]
    fn test_wrap_clock() {
        let mut clock = Clock::new();
        clock.mcounter = 1.5;
        clock.wrap_clock();
        assert_relative_eq!(clock.mcounter, 0.5);

        clock.mcounter = -0.5;
        clock.wrap_clock();
        assert_relative_eq!(clock.mcounter, 0.5);

        clock.mcounter = 0.8;
        clock.wrap_clock();
        assert_relative_eq!(clock.mcounter, 0.8);
    }

    #[rstest]
    fn test_advance_wrap_clock() {
        let mut clock = Clock::new();
        clock.phase_inc = 1.0;
        clock.advance_wrap_clock(0.1);
        assert_relative_eq!(clock.mcounter, 0.1);

        clock.phase_inc = 2.0;
        clock.advance_wrap_clock(0.3);
        assert_relative_eq!(clock.mcounter, 0.7);

        clock.phase_inc = -1.0;
        clock.advance_wrap_clock(0.1);
        assert_relative_eq!(clock.mcounter, 0.6);
    }

    #[rstest]
    fn test_set_freq() {
        let mut clock = Clock::new();
        clock.set_freq(440.0, 44100.0);
        assert_relative_eq!(clock.frequency_hz, 440.0);
        assert_relative_eq!(clock.phase_inc, 440.0 / 44100.0);
    }

    #[rstest]
    fn test_add_phase_offset() {
        let mut clock = Clock::new();
        clock.phase_inc = 1.0;
        clock.add_phase_offset(0.2, true);
        assert_relative_eq!(clock.mcounter, 0.2);

        clock.phase_inc = -1.0;
        clock.add_phase_offset(0.3, false);
        assert_relative_eq!(clock.mcounter, -0.1);
    }
}
