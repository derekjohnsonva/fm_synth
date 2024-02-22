use crate::fm_core::FmCore;
// A voice should contain an oscillator, an envelope, and a filter
// The voice should handle note on and note off events. It needs a render function,
// an initialize function, and an reset function.
use crate::linear_eg::EGParameters;
use crate::linear_eg::EnvelopeGenerator;
use crate::linear_eg::LinearEG;

#[derive(Default)]
pub struct Parameters {
    pub eg_params: EGParameters,
}

pub struct Voice {
    core: FmCore,
    eg: LinearEG,
    // TODO: Add a filter
    id: Option<i32>,
    // TODO: Add gain
    // gain: Smoother<f32>,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            core: FmCore::new(),
            eg: LinearEG::new(),
            id: None,
            // gain: Smoother::new(SmoothingStyle::Linear(1.0)),
        }
    }
    pub fn render(
        &mut self,
        params: &Parameters,
        num_samples_to_process: usize,
        channels: usize,
        sample_rate: f32,
    ) -> Vec<Vec<f32>> {
        let eg_value = self
            .eg
            .render(&params.eg_params, num_samples_to_process, sample_rate);
        // make an array of zeros
        let mut output = vec![vec![0.0; num_samples_to_process]; channels];
        for sample_index in 0..num_samples_to_process {
            let core_output = self.core.render();
            for channel in &mut output {
                channel[sample_index] = core_output * eg_value;
            }
        }
        output
    }
    pub fn reset(&mut self, params: &Parameters) {
        self.core.reset();
        self.eg.reset(&params.eg_params);
    }
    pub fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        self.core
            .note_on(note, velocity, sample_rate, voice_id, channel);
        self.eg.note_on(&params.eg_params, sample_rate);
    }
    pub fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        self.eg.note_off(&params.eg_params, sample_rate);
        self.core.note_off(note, voice_id, channel);
    }
}
