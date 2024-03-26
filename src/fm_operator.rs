use nih_plug::nih_error;

use crate::fm_core::FmCore;
use crate::linear_eg::EnvelopeGenerator;
use crate::linear_eg::{self};

/// An operator is one of several oscillators in an FM voice.
pub struct Operator {
    // TODO: Should probably refactor to make the fields private
    pub core: FmCore,
    pub eg: linear_eg::LinearEG,
    last_output: f32,                 // used for self modulation (feedback)
    pub output_buffer: Vec<Vec<f32>>, // 2D output buffer for stereo
    pm_input: Vec<f32>,
}

impl Operator {
    pub fn new() -> Self {
        Self {
            core: FmCore::new(),
            eg: linear_eg::LinearEG::new(),
            last_output: 0.0,
            output_buffer: vec![vec![0.0; 1]; 2],
            pm_input: vec![0.0; 1],
        }
    }
    pub fn reset(&mut self, params: &crate::voice_utils::Parameters) {
        self.core.reset();
        self.eg.reset(&params.eg_params);
    }
    pub fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize) {
        self.output_buffer = vec![vec![0.0; max_samples_per_channel]; num_channels];
        self.pm_input = vec![0.0; max_samples_per_channel];
    }

    pub fn update_core_ratio(&mut self, new_ratio: f32) {
        self.core.ratio = new_ratio;
    }

    pub fn render(
        &mut self,
        num_samples_to_process: usize,
        _params: &crate::voice_utils::Parameters,
        sample_rate: f32,
        self_modulation: bool,
        index: f32,
    ) {
        // add the output of core to the phase modulation buffer
        for sample_index in 0..num_samples_to_process {
            // We will not batch process the eg value for this
            // let eg_value = self.eg.render(&params.eg_params, 1, sample_rate);
            if self_modulation {
                // add the output of the core to the phase modulation buffer
                self.pm_input[sample_index] = self.last_output; // TODO: We may need some sort of feedback value here to make things not explode
            }
            // modulate the phase by the pm_input
            self.core
                .clock
                .add_phase_offset(self.pm_input[sample_index] * index, true);
            let core_output = self.core.render(sample_rate);
            self.core.clock.remove_phase_offset();
            self.last_output = core_output;
            for chanel in &mut self.output_buffer {
                chanel[sample_index] = self.last_output;
            }
        }
        // set the pm_input buffer to zero
        for sample in &mut self.pm_input {
            *sample = 0.0;
        }
    }

    #[allow(clippy::cast_precision_loss)]
    pub fn add_pm_source(&mut self, other_operator: &Self) {
        // ensure that the pm_input buffer is the same size as the other operator's output buffer
        if self.pm_input.len() != other_operator.output_buffer[0].len() {
            nih_error!(
                "The pm_input buffer is not the same size as the other operator's output buffer"
            );
        }
        // get the number of channels in the other operator
        let num_channels = other_operator.output_buffer.len();
        let channel_weight = 1.0 / num_channels as f32;
        for channel in &other_operator.output_buffer {
            for (sample_index, sample) in channel.iter().enumerate() {
                self.pm_input[sample_index] += sample * channel_weight;
            }
        }
    }

    pub fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &crate::voice_utils::Parameters,
        sample_rate: f32,
    ) {
        self.core
            .note_on(note, velocity, sample_rate, voice_id, channel);
        self.eg.note_on(&params.eg_params, sample_rate);
    }

    pub fn note_off(&mut self, params: &crate::voice_utils::Parameters, sample_rate: f32) {
        self.core.note_off();
        self.eg.note_off(&params.eg_params, sample_rate);
    }
}
