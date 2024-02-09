use crate::clock::Clock;
use crate::sin_osc::SinOsc;
use nih_plug::util;

#[derive(Debug)]
pub struct FmCore {
    sample_rate: f32,
    midi_note: u8,
    pub output_amplitude: f32, // amplitude in dB
    output_value: f32,         // The last output value. This is used for self Feedback

    // -- table source
    sin_osc: SinOsc,
    // -- Timebase
    pub clock: Clock,
}
impl FmCore {
    pub fn new() -> Self {
        Self {
            sample_rate: 0.0,
            midi_note: 0,
            output_amplitude: 0.0,
            output_value: 0.0,
            sin_osc: SinOsc::new(),
            clock: Clock::new(),
        }
    }
    pub fn reset(&mut self) {
        self.output_amplitude = 0.0;
        self.output_value = 0.0;
        self.clock.reset();
    }

    pub fn render(&mut self) -> f32 {
        self.output_value = self.sin_osc.read_osc(self.clock.mcounter);
        self.output_value *= self.output_amplitude;
        self.clock.advance_wrap_clock(1.0);
        self.output_value
    }

    pub fn note_on(&mut self, note: u8, velocity: f32, sample_rate: f32) {
        self.sample_rate = sample_rate;
        // convert the midi note to a frequency
        let frequency = util::midi_note_to_freq(note);
        // set the frequency of the oscillator
        self.clock.set_freq(frequency, sample_rate);
        self.output_amplitude = velocity;
        self.midi_note = note;
        self.clock.reset();
    }

    pub fn note_off(&mut self) {
        // empty for now
        self.output_amplitude = 0.0;
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
    fn test_render() {
        let frequency = 440.0;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let midi_note = nih_plug::util::freq_to_midi_note(frequency).round() as u8;
        let sample_rate = 1760.0; // 4 times the frequency
                                  // Before we can make a sound, we need to send a note_on message to the synth
        let mut fm_core = FmCore::new();
        fm_core.note_on(midi_note, 1.0, sample_rate);
        // We will set the output amplitude to 1.0, so we can compare the output to the sine wave
        fm_core.output_amplitude = 1.0;
        // Now we can render the sound
        let output = fm_core.render();
        assert_relative_eq!(output, 0.0);
        let output_2 = fm_core.render();
        assert_relative_eq!(output_2, 1.0);
        let output_3 = fm_core.render();
        assert_relative_eq!(output_3, 0.0);
        let output_4 = fm_core.render();
        assert_relative_eq!(output_4, -1.0);
        let output_5 = fm_core.render();
        assert_relative_eq!(output_5, 0.0);
    }
}
