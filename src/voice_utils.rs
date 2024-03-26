use crate::linear_eg::EGParameters;
#[derive(Default)]
/// Ratio is the ratio of the carrier frequency to the modulator frequency.
/// Index is the value that we multiply the output of the modulator by.
#[allow(clippy::struct_field_names)]
pub struct FmParams {
    pub op_a_ratio: f32,
    pub op_b_ratio: f32,
    pub op_c_ratio: f32,
    pub op_d_ratio: f32,

    pub op_a_index: f32,
    pub op_b_index: f32,
    pub op_c_index: f32,
    pub op_d_index: f32,
    /// How much of the output of each operator is mixed into the output of the voice.
    pub op_a_mix: f32,
    pub op_b_mix: f32,
    pub op_c_mix: f32,
    pub op_d_mix: f32,
}

#[derive(Default)]
pub struct Parameters {
    pub eg_params: EGParameters,
    pub fm_params: FmParams,
}
/// This stores Midi information.
#[derive(Debug, PartialEq, Clone)]
pub struct MidiEvent {
    pub timing: u32,
    /// A unique identifier for this note, if available. Using this to refer to a note is
    /// required when allowing overlapping voices for CLAP plugins.
    pub voice_id: Option<i32>,
    /// The note's channel, in `0..16`.
    pub channel: u8,
    /// The note's MIDI key number, in `0..128`.
    pub note: u8,
    /// The note's velocity, in `[0, 1]`. Some plugin APIs may allow higher precision than the
    /// 128 levels available in MIDI.
    pub velocity: f32,
}

pub trait Voice {
    fn new() -> Self;
    fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize);
    fn render(&mut self, num_samples_to_process: usize, params: &Parameters, sample_rate: f32);
    fn reset(&mut self, params: &Parameters);
    fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &Parameters,
        sample_rate: f32,
    );
    fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &Parameters,
        sample_rate: f32,
    );
    fn is_playing(&self) -> bool;
    fn accumulate_output(
        &mut self,
        audio_buffer: &mut [&mut [f32]],
        block_start: usize,
        block_end: usize,
    );
}
