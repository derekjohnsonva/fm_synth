use crate::linear_eg::EGParameters;

#[derive(Default)]
pub struct Parameters {
    pub eg_params: EGParameters,
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
