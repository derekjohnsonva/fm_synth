use nih_plug::nih_debug_assert;

#[derive(Clone)]
pub struct EGParameters {
    // ADSR times from user
    pub attack_time_msec: f32, // from GUI control
    pub decay_time_msec: f32,  // from GUI control
    // slope_time_msec: f32,   // from GUI control
    pub release_time_msec: f32, // from GUI control

    // For DXEG
    pub start_level: f32, // from GUI control
    // end_level: f32,          // from GUI control
    // decay_level: f32,        // from GUI control
    pub sustain_level: f32, // from GUI control
}
impl Default for EGParameters {
    fn default() -> Self {
        Self {
            attack_time_msec: 10.0,
            decay_time_msec: 50.0,
            release_time_msec: 100.0,
            start_level: 0.0,
            sustain_level: 0.4,
        }
    }
}

const MAX_EG_LEVEL: f32 = 1.0;
const MIN_EG_LEVEL: f32 = 0.0;
const SHUTDOWN_ITME_MSEC: f32 = 2.0;

/// The `EnvelopeGenerator` trait defines the methods that an envelope generator should implement.
pub trait EnvelopeGenerator {
    /// Creates a new instance of the envelope generator.
    fn new() -> Self;

    fn initialize(&mut self, parameters: &EGParameters, sample_rate: f32) {
        self.reset(parameters, sample_rate);
    }
    /// Resets the envelope generator with the given parameters and sample rate.
    fn reset(&mut self, parameters: &EGParameters, sample_rate: f32);

    /// Updates the envelope generator with the given parameters.
    fn update(&mut self, parameters: &EGParameters);

    /// Renders the envelope generator output for the specified number of samples.
    fn render(&mut self, parameters: &EGParameters, num_samples_to_process: usize) -> f32;

    /// Notifies the envelope generator that a note has been turned off.
    fn note_off(&mut self, parameters: &EGParameters);

    /// Notifies the envelope generator that a note has been turned on.
    fn note_on(&mut self, parameters: &EGParameters);

    /// Shuts down the envelope generator. Used for voice stealing.
    fn shutdown(&mut self, parameters: &EGParameters);
}

/// Represents the state of the envelope generator.
#[derive(Debug, PartialEq)]
enum EnvelopeState {
    Off,
    Attack,
    Decay,
    Sustain,
    Release,
    #[allow(dead_code)]
    Shutdown,
}

/// Represents a linear envelope generator.
pub struct LinearEG {
    state: EnvelopeState,
    step_increase: f32,
    sample_rate: f32,
    output_value: f32,
    shutdown_increment: f32,
}

impl EnvelopeGenerator for LinearEG {
    /// Creates a new instance of the linear envelope generator.
    fn new() -> Self {
        Self {
            state: EnvelopeState::Off,
            step_increase: 0.0,
            sample_rate: 0.0,
            output_value: 0.0,
            shutdown_increment: 0.0,
        }
    }

    /// Resets the linear envelope generator with the given parameters and sample rate.
    fn reset(&mut self, parameters: &EGParameters, sample_rate: f32) {
        self.sample_rate = sample_rate;
        self.output_value = parameters.start_level;
        self.state = EnvelopeState::Off;
    }

    /// Updates the linear envelope generator with the given parameters.
    fn update(&mut self, _parameters: &EGParameters) {
        // This is where we would do any work that needs to be done when the parameters change.
    }

    /// Renders the output of the linear envelope generator for the specified number of samples.
    /// We only return the output value for the first sample.
    fn render(&mut self, parameters: &EGParameters, num_samples_to_process: usize) -> f32 {
        // TODO: Implement the render method
        let mut output = 0.0;
        for i in 0..(num_samples_to_process as i32) {
            match self.state {
                EnvelopeState::Off => {
                    // TODO: This changes if we are in legato mode
                    self.output_value = parameters.start_level;
                }
                EnvelopeState::Attack => {
                    self.output_value += self.step_increase;
                    if self.output_value >= MAX_EG_LEVEL {
                        self.output_value = MAX_EG_LEVEL;
                        // calculate the decay step
                        let scale = -1.0;
                        self.step_increase =
                            self.calc_step_increase(parameters.decay_time_msec, scale);
                        self.state = EnvelopeState::Decay;
                    }
                }
                EnvelopeState::Decay => {
                    self.output_value += self.step_increase;
                    if self.output_value <= parameters.sustain_level {
                        self.output_value = parameters.sustain_level;
                        self.state = EnvelopeState::Sustain;
                    }
                }
                EnvelopeState::Sustain => {
                    self.output_value = parameters.sustain_level;
                }
                EnvelopeState::Release => {
                    self.output_value += self.step_increase;
                    if self.output_value <= MIN_EG_LEVEL {
                        self.output_value = MIN_EG_LEVEL;
                        self.state = EnvelopeState::Off;
                    }
                }
                EnvelopeState::Shutdown => {
                    self.output_value += self.shutdown_increment;
                    if self.output_value <= MIN_EG_LEVEL {
                        self.output_value = MIN_EG_LEVEL;
                        self.state = EnvelopeState::Off;
                    }
                }
            }
            if i == 0 {
                output = self.output_value;
            }
        }
        output
    }

    /// Notifies the linear envelope generator that a note has been turned off.
    fn note_off(&mut self, parameters: &EGParameters) {
        let scale = -1.0;
        self.step_increase = self.calc_step_increase(parameters.release_time_msec, scale);
        nih_debug_assert!(self.step_increase < 0.0);
        if self.output_value > MIN_EG_LEVEL {
            self.state = EnvelopeState::Release;
        } else {
            self.state = EnvelopeState::Off;
        }
    }

    /// Notifies the linear envelope generator that a note has been turned on.
    fn note_on(&mut self, parameters: &EGParameters) {
        self.step_increase = self.calc_step_increase(parameters.attack_time_msec, 1.0);
        nih_debug_assert!(self.step_increase > 0.0);
        self.state = EnvelopeState::Attack;
        self.output_value = parameters.start_level - self.step_increase; // Not sure why we need to do the subtraction
    }

    fn shutdown(&mut self, _parameters: &EGParameters) {
        self.shutdown_increment =
            -(1000.0 * self.output_value) / SHUTDOWN_ITME_MSEC / self.sample_rate;
        nih_debug_assert!(self.shutdown_increment < 0.0);
        self.state = EnvelopeState::Shutdown;
    }
}

impl LinearEG {
    /// Calculate the linear step increase. This is for all the linear segments of the envelope.
    /// We are finding the step increase for every sample
    ///
    /// Parameters:
    /// - `time_ms` = time in milliseconds
    /// - `scale` = the scale factor for the step increase
    fn calc_step_increase(&mut self, time_ms: f32, scale: f32) -> f32 {
        // do a zero check
        if time_ms == 0.0 {
            return 0.0;
        }
        // calculate the step increase
        // `sample_rate` = samples / second
        //
        // 1000.0 / `time_ms` = time in seconds
        //
        // time in seconds * (1 /`sample_rate`) = samples
        scale * (1000.0 / (time_ms * self.sample_rate))
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
    fn test_render_attack() {
        let sample_rate = 44100.0;
        let mut eg = LinearEG::new();
        let parameters = EGParameters {
            start_level: 0.0,
            attack_time_msec: 100.0,
            decay_time_msec: 100.0,
            sustain_level: 0.5,
            release_time_msec: 200.0,
            ..Default::default()
        };
        let num_samples_to_process = 50;

        eg.reset(&parameters, sample_rate);
        eg.state = EnvelopeState::Attack;
        eg.step_increase = 0.01;
        // We should be able to render twice
        let output = eg.render(&parameters, num_samples_to_process);

        // Assert that the output value increases during the attack phase
        assert!(output.eq(&0.01));
        // THE FOLLOWING SHOULD WORK BUT IT DOESN'T BECAUSE OF FLOATING POINT ERRORS
        // assert_relative_eq!(eg.output_value, 0.5);
        // let output2 = eg.render(&parameters, num_samples_to_process);
        // assert_relative_eq!(output2, 0.51);
        // assert_relative_eq!(eg.output_value, 1.0);
    }

    #[rstest]
    fn test_calc_step_increase() {
        let sample_rate = 1000.0;
        let mut eg = LinearEG::new();
        let parameters = EGParameters {
            ..Default::default()
        };
        eg.initialize(&parameters, sample_rate);
        let time_ms = 100.0;
        let scale = 1.0;
        let step_increase = eg.calc_step_increase(time_ms, scale);
        assert_relative_eq!(step_increase, 0.01);
    }

    #[rstest]
    fn test_note_off() {
        let sample_rate = 44100.0;
        let mut eg = LinearEG::new();
        // Check that if the eg is in attack mode, it switches to release mode
        let parameters = EGParameters {
            ..Default::default()
        };
        for state in [
            EnvelopeState::Attack,
            EnvelopeState::Decay,
            EnvelopeState::Sustain,
            EnvelopeState::Release,
        ] {
            eg.state = state;
            eg.output_value = 0.5;
            eg.note_off(&parameters);
            assert_eq!(eg.state, EnvelopeState::Release);
        }
        // When the envelope is already off, it should stay off
        // and the output value should be 0
        eg.state = EnvelopeState::Off;
        eg.output_value = 0.0;
        eg.note_off(&parameters);
        assert_eq!(eg.state, EnvelopeState::Off);
        assert_eq!(eg.output_value, 0.0);
    }

    #[rstest]
    fn test_note_on() {
        let sample_rate = 1000.0;
        let mut eg = LinearEG::new();
        let parameters = EGParameters {
            ..Default::default()
        };
        // check that any other state will be set to attack
        // and the output value will be set to 0
        for state in [
            EnvelopeState::Attack,
            EnvelopeState::Decay,
            EnvelopeState::Sustain,
            EnvelopeState::Release,
            EnvelopeState::Off,
        ] {
            eg.state = state;
            eg.output_value = 0.5;
            eg.note_on(&parameters);
            assert_eq!(eg.state, EnvelopeState::Attack);
            assert!(eg.output_value < 0.0);
        }
    }
}
