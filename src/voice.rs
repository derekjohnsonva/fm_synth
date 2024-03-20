use nih_plug::nih_log;

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
/// This stores Midi information.
#[derive(Debug, PartialEq, Clone)]
pub struct MidiEvent {
    timing: u32,
    /// A unique identifier for this note, if available. Using this to refer to a note is
    /// required when allowing overlapping voices for CLAP plugins.
    voice_id: Option<i32>,
    /// The note's channel, in `0..16`.
    channel: u8,
    /// The note's MIDI key number, in `0..128`.
    note: u8,
    /// The note's velocity, in `[0, 1]`. Some plugin APIs may allow higher precision than the
    /// 128 levels available in MIDI.
    velocity: f32,
}

#[derive(PartialEq, Clone, Debug)]
pub struct Voice {
    core: FmCore,
    eg: LinearEG,
    // TODO: Add a filter
    _id: Option<i32>,
    // TODO: decide if there should be some other way to handle the output
    is_stealing: bool,
    current_midi_event: Option<MidiEvent>,
    next_midi_event: Option<MidiEvent>,
    output_buffer: Vec<Vec<f32>>, // 2D output buffer for stereo
                                  // TODO: Add gain
                                  // gain: Smoother<f32>,
}

impl Voice {
    pub fn new() -> Self {
        Self {
            core: FmCore::new(),
            eg: LinearEG::new(),
            _id: None,
            is_stealing: false,
            current_midi_event: None,
            next_midi_event: None,
            output_buffer: vec![vec![0.0; 1]; 2],
            // gain: Smoother::new(SmoothingStyle::Linear(1.0)),
        }
    }

    pub fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize) {
        self.output_buffer = vec![vec![0.0; max_samples_per_channel]; num_channels];
    }

    pub fn render(&mut self, num_samples_to_process: usize, params: &Parameters, sample_rate: f32) {
        // get the length of the audio buffer
        let eg_value = self
            .eg
            .render(&params.eg_params, num_samples_to_process, sample_rate);

        // add the core output to the audio_buffer
        for sample_index in 0..num_samples_to_process {
            let core_output = self.core.render();
            // add the core output to the different channels
            for channel in &mut self.output_buffer {
                channel[sample_index] = core_output * eg_value;
            }
        }
        // Check the stealPending flag to see if the voice is being stolen, and if so:
        if self.is_stealing && !self.eg.is_playing() {
            // Call the voice’s note-off handler – this was never called because the event was stolen
            // Copy the voiceStealMIDIEvent structure into the voiceMIDIEvent structure
            // Call the note-on handler with the new MIDI event information, switching the pitch and velocity to the stolen note – the steal operation is complete”
            self.is_stealing = false;
            if let Some(midi_event) = &self.current_midi_event {
                self.note_off(
                    midi_event.voice_id,
                    midi_event.channel,
                    midi_event.note,
                    params,
                    sample_rate,
                );
            }
            // Note: it is possible we received the `note_off` event for the `next_midi_event` during the steal
            // operation and so there is nothing to be done here.
            self.current_midi_event = self.next_midi_event.take();

            if let Some(midi_event) = &self.current_midi_event {
                self.note_on(
                    midi_event.note,
                    midi_event.velocity,
                    midi_event.voice_id,
                    midi_event.channel,
                    params,
                    sample_rate,
                );
            }
        }
    }
    pub fn reset(&mut self, params: &Parameters) {
        self.core.reset();
        self.eg.reset(&params.eg_params);
    }
    /// This function is called when a note on event is received. There should never be two calls to ``note_on`` without a
    /// call to render in between.
    pub fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        // print "note on" plus the note number
        nih_log!("Note on; note = {}", note);
        // Check to see if the voice is already playing a note. If so, we need to steal the voice.
        if self.eg.is_playing() {
            nih_log!("Stealing");
            self.is_stealing = true;
            self.next_midi_event = Some(MidiEvent {
                timing: 0,
                voice_id,
                channel,
                note,
                velocity,
            });
            self.eg.shutdown(&params.eg_params, sample_rate);
        } else {
            self.current_midi_event = Some(MidiEvent {
                timing: 0,
                voice_id,
                channel,
                note,
                velocity,
            });
            self.core
                .note_on(note, velocity, sample_rate, voice_id, channel);
            self.eg.note_on(&params.eg_params, sample_rate);
        }
    }
    /// This function is called when a note off event is received.
    /// There are a few cases to consider:
    /// 1) The note off event is unrelated to the `current_midi_event`. We should ignore it.
    /// 2) The note off event is related to the `current_midi_event`, but the voice is in the stealing state. We should ignore it.
    /// 3) The note off event is related to the `next_midi_event`, and the voice is in the stealing state. We should
    ///    finish the note steal but we should not process the note off event as the `note_on` event has not been processed yet.
    pub fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        if self.is_stealing {
            if let Some(midi_event) = &self.next_midi_event {
                if midi_event.voice_id == voice_id
                    || (midi_event.channel == channel && midi_event.note == note)
                {
                    // we are in the 3rd case
                    self.next_midi_event = None;
                }
            }
        } else if let Some(midi_event) = &self.current_midi_event {
            if midi_event.voice_id == voice_id
                || (midi_event.channel == channel && midi_event.note == note)
            {
                nih_log!("Note off matches current midi event");
                self.eg.note_off(&params.eg_params, sample_rate);
                self.core.note_off();
                self.current_midi_event = None;
            }
        }
    }
    pub fn is_playing(&self) -> bool {
        self.eg.is_playing()
    }

    pub fn accumulate_output(
        &mut self,
        audio_buffer: &mut [&mut [f32]],
        block_start: usize,
        block_end: usize,
    ) {
        for (channel, output) in audio_buffer.iter_mut().enumerate() {
            for (sample_index, sample) in output[block_start..block_end].iter_mut().enumerate() {
                *sample += self.output_buffer[channel][sample_index];
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
mod tests {
    use super::*;
    use crate::consts::SHUTDOWN_TIME_MSEC;
    use approx::assert_relative_eq;
    use rstest::{fixture, rstest};

    #[fixture]
    fn voice() -> Voice {
        Voice::new()
    }
    #[fixture]
    fn params() -> Parameters {
        Parameters {
            eg_params: EGParameters {
                attack_time_msec: 10.0,
                decay_time_msec: 10.0,
                release_time_msec: 10.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        }
    }

    #[rstest]
    fn test_voice_stealing(mut voice: Voice, params: Parameters) {
        // Do a note on
        const SAMPLES_RATE: f32 = 44100.0;
        const NUM_SAMPLES_TO_PROCESS_2: usize =
            (SAMPLES_RATE * SHUTDOWN_TIME_MSEC / 1000.0) as usize;
        let note_1 = 60;
        voice.initialize(2, 1024);
        voice.note_on(note_1, 0.5, Some(1), 0, &params, SAMPLES_RATE);
        // Assert that we are not in the stealing state
        assert!(!voice.is_stealing);
        // render a couple of samples so that the envelope is in the attack phase and there is a non-zero output
        let audio_buffer = &mut [0.0; 10];
        voice.render(audio_buffer.len(), &params, SAMPLES_RATE);
        // Do another note on
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &params, SAMPLES_RATE);
        // Assert that we are in the stealing state
        assert!(voice.is_stealing);
        // Find how many samples it takes to escape the stealing state
        // render one less sample than it takes to escape the stealing state
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS_2];
        voice.render(audio_buffer.len(), &params, SAMPLES_RATE);
        // Assert that we are in the stealing state
        assert!(voice.is_stealing);
        // render enough samples to escape the stealing state
        let audio_buffer = &mut [0.0; 1];
        voice.render(audio_buffer.len(), &params, SAMPLES_RATE);
        // Assert that we are not in the stealing state
        assert!(!voice.is_stealing);
        // Assert that the current midi event is the second note on
        assert_eq!(
            voice
                .current_midi_event
                .expect("Expected current midi event")
                .note,
            note_2
        );
        // Assert that the next midi event is None
        assert_eq!(voice.next_midi_event, None);
    }

    /// Test what happens when a new note on event is received while the voice is in the stealing state
    #[rstest]
    fn test_note_on_during_steal(mut voice: Voice, params: Parameters) {
        // Do a note on
        const SAMPLE_RATE: f32 = 44100.0;
        const NUM_SAMPLES_TO_PROCESS: usize = (SAMPLE_RATE * SHUTDOWN_TIME_MSEC / 1000.0) as usize;
        let note_1 = 60;
        voice.initialize(2, 1024);
        voice.note_on(note_1, 0.5, Some(1), 0, &params, SAMPLE_RATE);
        // Assert that we are not in the stealing state
        assert!(!voice.is_stealing);
        // render a couple of samples so that the envelope is in the attack phase and there is a non-zero output
        let audio_buffer = &mut [0.0; 10];
        voice.render(audio_buffer.len(), &params, SAMPLE_RATE);
        // Do another note on
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &params, SAMPLE_RATE);
        // Assert that we are in the stealing state
        assert!(voice.is_stealing);
        // Do another note on
        let note_3 = 62;
        voice.note_on(note_3, 0.5, Some(1), 0, &params, SAMPLE_RATE);
        // Assert that we are in the stealing state
        assert!(voice.is_stealing);
        // Find how many samples it takes to escape the stealing state
        // render one less sample than it takes to escape the stealing state
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS + 1];
        voice.render(audio_buffer.len(), &params, SAMPLE_RATE);
        // Assert that we are not in the stealing state
        assert!(!voice.is_stealing);
        // Assert that the current midi event is the third note on
        assert_eq!(
            voice
                .current_midi_event
                .expect("Expected current midi event")
                .note,
            note_3
        );
        // Assert that the next midi
    }

    #[rstest]
    fn test_note_on_and_off() {
        const PARAMS: Parameters = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 10.0,
                decay_time_msec: 10.0,
                release_time_msec: 10.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        const SAMPLE_RATE: f32 = 44100.0;
        const NUM_SAMPLES_TO_PROCESS: usize =
            (SAMPLE_RATE * PARAMS.eg_params.release_time_msec / 1000.0) as usize;
        let mut voice = Voice::new();
        let note = 60;
        voice.initialize(2, NUM_SAMPLES_TO_PROCESS);
        voice.note_on(note, 0.5, Some(1), 0, &PARAMS, SAMPLE_RATE);
        let audio_buffer = &mut [0.0; 10];
        voice.render(audio_buffer.len(), &PARAMS, SAMPLE_RATE);
        voice.note_off(Some(1), 0, note, &PARAMS, SAMPLE_RATE);
        assert!(voice.eg.is_playing());
        // find the number of samples it takes to reach the off phase
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS];

        voice.render(audio_buffer.len(), &PARAMS, SAMPLE_RATE);
        assert!(!voice.eg.is_playing());
        assert_relative_eq!(voice.eg.render(&PARAMS.eg_params, 1, SAMPLE_RATE), 0.0);
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS];
        voice.render(audio_buffer.len(), &PARAMS, SAMPLE_RATE);
        // assert that the audio buffer is zero
        for sample in audio_buffer {
            assert_relative_eq!(*sample, 0.0);
        }
    }

    // Write a test to assert that when we play a note on immediately after a note off, we enter the steal state
    #[rstest]
    fn test_note_on_after_note_off(mut voice: Voice) {
        const PARAMS: Parameters = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 10.0,
                decay_time_msec: 10.0,
                release_time_msec: 10.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        const SAMPLES_RATE: f32 = 1000.0;
        const NUM_SAMPLES_TO_PROCESS: usize =
            (SAMPLES_RATE * PARAMS.eg_params.attack_time_msec / 1000.0) as usize;
        let note = 60;
        voice.initialize(2, NUM_SAMPLES_TO_PROCESS);
        voice.note_on(note, 0.5, Some(1), 0, &PARAMS, SAMPLES_RATE);
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS];
        voice.render(audio_buffer.len(), &PARAMS, SAMPLES_RATE);
        voice.note_off(Some(1), 0, note, &PARAMS, SAMPLES_RATE);
        // We are at the end of the attack phase
        let audio_buffer = &mut [0.0; 1];
        voice.render(audio_buffer.len(), &PARAMS, SAMPLES_RATE);
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &PARAMS, SAMPLES_RATE);
        assert!(voice.is_stealing);
    }

    #[rstest]
    fn test_note_off_guard(mut voice: Voice, params: Parameters) {
        const SAMPLE_RATE: f32 = 1000.0;
        const ATTACK_TIME_MS: f32 = 10.0;
        const NUM_SAMPLES_TO_PROCESS: usize = (SAMPLE_RATE * ATTACK_TIME_MS / 1000.0) as usize;
        let note = 60;
        voice.initialize(2, NUM_SAMPLES_TO_PROCESS);
        voice.note_on(note, 0.5, Some(1), 0, &params, SAMPLE_RATE);
        let audio_buffer = &mut [0.0; NUM_SAMPLES_TO_PROCESS];
        voice.render(audio_buffer.len(), &params, SAMPLE_RATE);
        // different note and voice_id - should have no effect
        assert!(voice.current_midi_event.is_some());
        voice.note_off(Some(2), 0, note + 1, &params, SAMPLE_RATE);
        assert!(voice.current_midi_event.is_some());
        // different channel and voice_id - should have no effect
        voice.note_off(Some(2), 1, note, &params, SAMPLE_RATE);
        assert!(voice.current_midi_event.is_some());
        // Enter the steal state
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(2), 0, &params, SAMPLE_RATE);
        assert!(voice.is_stealing);
        // process the note off event for the first note - should have no effect
        voice.note_off(Some(1), 0, note, &params, SAMPLE_RATE);
        assert!(voice.is_stealing);
        // process the note off event for the second note - should have no effect except for removing the next midi event
        assert!(voice.next_midi_event.is_some());
        voice.note_off(Some(2), 0, note_2, &params, SAMPLE_RATE);
        assert!(voice.is_stealing);
        assert!(voice.next_midi_event.is_none());
    }
}
