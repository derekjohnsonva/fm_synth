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
#[derive(PartialEq, Clone)]
pub struct Voice {
    core: FmCore,
    eg: LinearEG,
    // TODO: Add a filter
    _id: Option<i32>,
    // TODO: decide if there should be some other way to handle the output
    pub voice_output: Vec<Vec<f32>>,
    is_stealing: bool,
    current_midi_event: Option<MidiEvent>,
    next_midi_event: Option<MidiEvent>,
    // TODO: Add gain
    // gain: Smoother<f32>,
}
// Write a debug implementation for Voice
impl std::fmt::Debug for Voice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Voice")
            .field("eg", &self.eg)
            .field("_id", &self._id)
            .field("is_stealing", &self.is_stealing)
            .field("current_midi_event", &self.current_midi_event)
            .field("next_midi_event", &self.next_midi_event)
            .finish()
    }
}

impl Voice {
    pub fn new() -> Self {
        Self {
            core: FmCore::new(),
            eg: LinearEG::new(),
            _id: None,
            voice_output: vec![vec![0.0; 1]; 1],
            is_stealing: false,
            current_midi_event: None,
            next_midi_event: None,
            // gain: Smoother::new(SmoothingStyle::Linear(1.0)),
        }
    }

    pub fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize) {
        self.voice_output = vec![vec![0.0; max_samples_per_channel]; num_channels];
    }

    pub fn render(&mut self, params: &Parameters, num_samples_to_process: usize, sample_rate: f32) {
        let eg_value = self
            .eg
            .render(&params.eg_params, num_samples_to_process, sample_rate);

        for sample_index in 0..num_samples_to_process {
            let core_output = self.core.render();
            for channel in &mut self.voice_output {
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
    /// This function is called when a note on event is received. There should never be two calls to note_on without a
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
    pub fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        // print "note off" plus the note number
        nih_log!("Note off; note = {}", note);
        if let Some(midi_event) = &self.current_midi_event {
            if midi_event.voice_id == voice_id
                || (midi_event.channel == channel && midi_event.note == note)
            {
                self.eg.note_off(&params.eg_params, sample_rate);
                self.core.note_off();
                self.current_midi_event = None;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::SHUTDOWN_TIME_MSEC;
    use approx::assert_relative_eq;
    use rstest::*;

    #[fixture]
    fn setup() {
        #[allow(clippy::unwrap_used)]
        color_eyre::install().unwrap();
    }

    #[rstest]
    fn test_voice_new() {
        let voice = Voice::new();
        assert_eq!(voice.voice_output, vec![vec![0.0; 1]; 1]);
    }

    #[rstest]
    fn test_voice_stealing() {
        let mut voice = Voice::new();
        let params = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 100.0,
                decay_time_msec: 100.0,
                release_time_msec: 100.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        voice.initialize(2, 100);
        // Do a note on
        let sample_rate = 44100.0;
        let note_1 = 60;
        voice.note_on(note_1, 0.5, Some(1), 0, &params, sample_rate);
        // Assert that we are not in the stealing state
        assert_eq!(voice.is_stealing, false);
        // render a couple of samples so that the envelope is in the attack phase and there is a non-zero output
        voice.render(&params, 10, sample_rate);
        // Do another note on
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &params, sample_rate);
        // Assert that we are in the stealing state
        assert_eq!(voice.is_stealing, true);
        // Find how many samples it takes to escape the stealing state
        let num_samples_to_process = sample_rate * SHUTDOWN_TIME_MSEC / 1000.0;
        // render one less sample than it takes to escape the stealing state
        voice.render(&params, num_samples_to_process as usize, sample_rate);
        // Assert that we are in the stealing state
        assert_eq!(voice.is_stealing, true);
        // render enough samples to escape the stealing state
        voice.render(&params, 1, sample_rate);
        // Assert that we are not in the stealing state
        assert_eq!(voice.is_stealing, false);
        // Assert that the current midi event is the second note on
        assert_eq!(voice.current_midi_event.unwrap().note, note_2);
        // Assert that the next midi event is None
        assert_eq!(voice.next_midi_event, None);
    }

    /// Test what happens when a new note on event is received while the voice is in the stealing state
    #[rstest]
    fn test_note_on_during_steal() {
        let mut voice = Voice::new();
        let params = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 100.0,
                decay_time_msec: 100.0,
                release_time_msec: 100.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        voice.initialize(2, 100);
        // Do a note on
        let sample_rate = 44100.0;
        let note_1 = 60;
        voice.note_on(note_1, 0.5, Some(1), 0, &params, sample_rate);
        // Assert that we are not in the stealing state
        assert_eq!(voice.is_stealing, false);
        // render a couple of samples so that the envelope is in the attack phase and there is a non-zero output
        voice.render(&params, 10, sample_rate);
        // Do another note on
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &params, sample_rate);
        // Assert that we are in the stealing state
        assert_eq!(voice.is_stealing, true);
        // Do another note on
        let note_3 = 62;
        voice.note_on(note_3, 0.5, Some(1), 0, &params, sample_rate);
        // Assert that we are in the stealing state
        assert_eq!(voice.is_stealing, true);
        // Find how many samples it takes to escape the stealing state
        let num_samples_to_process = sample_rate * SHUTDOWN_TIME_MSEC / 1000.0;
        // render one less sample than it takes to escape the stealing state
        voice.render(&params, num_samples_to_process as usize + 1, sample_rate);
        // Assert that we are not in the stealing state
        assert_eq!(voice.is_stealing, false);
        // Assert that the current midi event is the third note on
        assert_eq!(voice.current_midi_event.unwrap().note, note_3);
        // Assert that the next midi
    }

    #[rstest]
    fn test_note_on_and_off() {
        let mut voice = Voice::new();
        let params = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 100.0,
                decay_time_msec: 100.0,
                release_time_msec: 10.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        let sample_rate = 44100.0;
        let num_samples_to_process = sample_rate * params.eg_params.release_time_msec / 1000.0;
        voice.initialize(2, num_samples_to_process as usize);
        let note = 60;
        voice.note_on(note, 0.5, Some(1), 0, &params, sample_rate);
        voice.render(&params, 10, sample_rate);
        voice.note_off(Some(1), 0, note, &params, sample_rate);
        assert_eq!(voice.eg.is_playing(), true);
        // find the number of samples it takes to reach the off phase
        voice.render(&params, num_samples_to_process as usize, sample_rate);
        assert!(voice.eg.is_playing() == false);
        assert_relative_eq!(voice.eg.render(&params.eg_params, 1, sample_rate), 0.0);
        voice.render(&params, num_samples_to_process as usize, sample_rate);
        for channel in voice.voice_output {
            for sample in channel {
                assert_relative_eq!(sample, 0.0);
            }
        }
    }

    // Write a test to assert that when we play a note on immediately after a note off, we enter the steal state
    #[rstest]
    fn test_note_on_after_note_off() {
        let mut voice = Voice::new();
        let params = Parameters {
            eg_params: EGParameters {
                attack_time_msec: 10.0,
                decay_time_msec: 10.0,
                release_time_msec: 10.0,
                start_level: 0.0,
                sustain_level: 0.1,
            },
        };
        let sample_rate = 1000.0;
        let num_samples_to_process = sample_rate * params.eg_params.attack_time_msec / 1000.0;
        voice.initialize(2, num_samples_to_process as usize);
        let note = 60;
        voice.note_on(note, 0.5, Some(1), 0, &params, sample_rate);
        voice.render(&params, num_samples_to_process as usize, sample_rate);
        voice.note_off(Some(1), 0, note, &params, sample_rate);
        // We are at the end of the attack phase
        voice.render(&params, 1, sample_rate);
        let note_2 = 61;
        voice.note_on(note_2, 0.5, Some(1), 0, &params, sample_rate);
        assert!(voice.is_stealing);
    }
}
