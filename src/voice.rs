use crate::fm_core::FmCore;
// A voice should contain an oscillator, an envelope, and a filter
// The voice should handle note on and note off events. It needs a render function,
// an initialize function, and an reset function.
use crate::linear_eg::EGParameters;
use crate::linear_eg::EnvelopeGenerator;
use crate::linear_eg::LinearEG;
use nih_plug::prelude::*;

pub struct VoiceParameters {
    pub eg_params: EGParameters,
    pub sample_rate: f32,
}
impl Default for VoiceParameters {
    fn default() -> Self {
        Self {
            eg_params: EGParameters::default(),
            sample_rate: 0.0,
        }
    }
}

pub struct Voice {
    core: FmCore,
    eg: LinearEG,
    // TODO: Add a filter
    voice_id: Option<i32>,
    gain: Smoother<f32>,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            core: FmCore::new(),
            eg: LinearEG::new(),
            voice_id: None,
            gain: Smoother::new(SmoothingStyle::Linear(1.0)),
        }
    }
    pub fn render(
        &mut self,
        params: &VoiceParameters,
        num_samples_to_process: usize,
        channels: usize,
    ) -> Vec<Vec<f32>> {
        let eg_value = self.eg.render(&params.eg_params, num_samples_to_process);
        // make an array of zeros
        let mut output = vec![vec![0.0; num_samples_to_process]; channels];
        for sample_index in 0..num_samples_to_process {
            let gain = self.gain.next();
            let core_output = self.core.render();
            for channel in output.iter_mut() {
                channel[sample_index] = core_output * eg_value * gain;
            }
        }
        output
    }
    pub fn reset(&mut self, params: &VoiceParameters) {
        self.core.reset();
        self.eg.reset(&params.eg_params, params.sample_rate);
    }
    pub fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &VoiceParameters,
    ) {
        self.core
            .note_on(note, velocity, params.sample_rate, voice_id, channel);
        self.eg.note_on(&params.eg_params);
    }
    pub fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &VoiceParameters,
    ) {
        self.eg.note_off(&params.eg_params);
        self.core.note_off(note, voice_id, channel);
    }
}
