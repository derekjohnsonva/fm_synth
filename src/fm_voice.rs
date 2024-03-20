use crate::{
    fm_core::FmCore,
    linear_eg::{EnvelopeGenerator, LinearEG},
    voice_utils::{MidiEvent, Voice},
};

/// This is an FM Synth voice that implements the Voice trait.
/// It is modeled on section 16.8 in the book "Designing Software
/// Synthesizer Plugins in C++: 2nd Edition" by Will Pirkle.

pub struct FmVoice {
    core_a: FmCore,
    core_b: FmCore,
    eg: LinearEG,
    // TODO: Add a filter
    _id: Option<i32>,
    // TODO: decide if there should be some other way to handle the output
    is_stealing: bool,
    current_midi_event: Option<MidiEvent>,
    next_midi_event: Option<MidiEvent>,
    output_buffer: Vec<Vec<f32>>, // 2D output buffer for stereo
    pm_buffer: Vec<f32>,
}

impl Voice for FmVoice {
    fn new() -> Self {
        Self {
            core_a: FmCore::new(),
            core_b: FmCore::new(),
            eg: LinearEG::new(),
            _id: None,
            is_stealing: false,
            current_midi_event: None,
            next_midi_event: None,
            output_buffer: vec![vec![0.0; 1]; 2],
            pm_buffer: vec![0.0; 1],
        }
    }

    fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize) {
        self.output_buffer = vec![vec![0.0; max_samples_per_channel]; num_channels];
        self.pm_buffer = vec![0.0; max_samples_per_channel];
    }

    fn render(
        &mut self,
        num_samples_to_process: usize,
        params: &crate::voice_utils::Parameters,
        sample_rate: f32,
    ) {
        // update the ratio of the core A oscillator
        self.core_a.ratio = params.fm_params.ratio;
        // Core A phase modulates Core B. Core A is the modulator and Core B is the carrier.
        // The EG output is then multiplied by the output of Core B.

        // get the length of the audio buffer
        let eg_value = self
            .eg
            .render(&params.eg_params, num_samples_to_process, sample_rate);

        // add the output of core A to the phase modulation buffer
        for sample_index in 0..num_samples_to_process {
            let core_output = self.core_a.render(sample_rate);
            self.pm_buffer[sample_index] = core_output;
        }
        // render core B
        for sample_index in 0..num_samples_to_process {
            // add the phase modulation to the phase of core B
            self.core_b
                .clock
                .add_phase_offset(self.pm_buffer[sample_index] * params.fm_params.index, true);
            let core_output = self.core_b.render(sample_rate);
            self.core_b.clock.remove_phase_offset();
            // Add the core B output to the output buffer
            for chanel in &mut self.output_buffer {
                chanel[sample_index] = core_output * eg_value;
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

    fn reset(&mut self, params: &crate::voice_utils::Parameters) {
        self.core_a.reset();
        self.core_b.reset();
        self.eg.reset(&params.eg_params);
    }

    fn note_on(
        &mut self,
        note: u8,
        velocity: f32,
        voice_id: Option<i32>,
        channel: u8,
        params: &crate::voice_utils::Parameters,
        sample_rate: f32,
    ) {
        // Check to see if the voice is already playing a note. If so, we need to steal the voice.
        if self.eg.is_playing() {
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
            // Core A is the modulator and Core B is the carrier. Thus, Apply the fm ratio to the core a note
            self.core_a
                .note_on(note, velocity, sample_rate, voice_id, channel);
            self.core_b
                .note_on(note, velocity, sample_rate, voice_id, channel);
            self.eg.note_on(&params.eg_params, sample_rate);
        }
    }

    fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &crate::voice_utils::Parameters,
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
                self.eg.note_off(&params.eg_params, sample_rate);
                self.core_a.note_off();
                self.core_b.note_off();
                self.current_midi_event = None;
            }
        }
    }

    fn is_playing(&self) -> bool {
        self.eg.is_playing()
    }

    fn accumulate_output(
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
