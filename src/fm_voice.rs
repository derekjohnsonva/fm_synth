use crate::{
    fm_operator::Operator,
    linear_eg::{EnvelopeGenerator, LinearEG},
    voice_utils::{MidiEvent, Voice},
};

/// This is an FM Synth voice that implements the Voice trait.
/// It is modeled on section 16.8 in the book "Designing Software
/// Synthesizer Plugins in C++: 2nd Edition" by Will Pirkle.

pub struct FmVoice {
    operator_a: Operator,
    operator_b: Operator,
    operator_c: Operator,
    operator_d: Operator,
    eg: LinearEG,
    // TODO: Add a filter
    _id: Option<i32>,
    // TODO: decide if there should be some other way to handle the output
    is_stealing: bool,
    current_midi_event: Option<MidiEvent>,
    next_midi_event: Option<MidiEvent>,
    output_buffer: Vec<Vec<f32>>, // 2D output buffer for stereo
}

impl Voice for FmVoice {
    fn new() -> Self {
        Self {
            operator_a: Operator::new(),
            operator_b: Operator::new(),
            operator_c: Operator::new(),
            operator_d: Operator::new(),

            eg: LinearEG::new(),
            _id: None,
            is_stealing: false,
            current_midi_event: None,
            next_midi_event: None,
            output_buffer: vec![vec![0.0; 1]; 2],
        }
    }

    fn initialize(&mut self, num_channels: usize, max_samples_per_channel: usize) {
        for operator in [
            &mut self.operator_a,
            &mut self.operator_b,
            &mut self.operator_c,
            &mut self.operator_d,
        ] {
            operator.initialize(num_channels, max_samples_per_channel);
        }

        self.output_buffer = vec![vec![0.0; max_samples_per_channel]; num_channels];
    }

    fn render(
        &mut self,
        num_samples_to_process: usize,
        params: &crate::voice_utils::Parameters,
        sample_rate: f32,
    ) {
        // update the ratio of the core A oscillator
        self.update_core_ratios(params);
        // Core A phase modulates Core B. Core A is the modulator and Core B is the carrier.
        // The EG output is then multiplied by the output of Core B.

        // get the length of the audio buffer
        let eg_value = self
            .eg
            .render(&params.eg_params, num_samples_to_process, sample_rate);

        self.operator_a.render(
            num_samples_to_process,
            params,
            sample_rate,
            false,
            params.fm_params.op_a_index,
        );

        // copy the output of operator a into the pm input of operator b
        self.operator_b.add_pm_source(&self.operator_a);
        self.operator_b.render(
            num_samples_to_process,
            params,
            sample_rate,
            false,
            params.fm_params.op_b_index,
        );
        self.operator_c.add_pm_source(&self.operator_b);
        self.operator_c.render(
            num_samples_to_process,
            params,
            sample_rate,
            false,
            params.fm_params.op_c_index,
        );
        self.operator_d.add_pm_source(&self.operator_c);
        self.operator_d.render(
            num_samples_to_process,
            params,
            sample_rate,
            false,
            params.fm_params.op_d_index,
        );
        // multiply the output of operator b by the eg value
        for (channel, output) in self.output_buffer.iter_mut().enumerate() {
            for (sample_index, sample) in output.iter_mut().enumerate() {
                *sample += self.operator_a.output_buffer[channel][sample_index]
                    * params.fm_params.op_a_mix;
                *sample += self.operator_b.output_buffer[channel][sample_index]
                    * params.fm_params.op_b_mix;
                *sample += self.operator_c.output_buffer[channel][sample_index]
                    * params.fm_params.op_c_mix;
                *sample += self.operator_d.output_buffer[channel][sample_index]
                    * params.fm_params.op_d_mix;
                *sample *= eg_value;
            }
        }
        // Check the stealPending flag to see if the voice is being stolen, and if so:
        if self.is_stealing && !self.eg.is_playing() {
            self.finish_voice_steal(params, sample_rate);
        }
    }

    fn reset(&mut self, params: &crate::voice_utils::Parameters) {
        self.operator_a.reset(params);
        self.operator_b.reset(params);
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
            self.operator_a
                .note_on(note, velocity, voice_id, channel, params, sample_rate);
            self.operator_b
                .note_on(note, velocity, voice_id, channel, params, sample_rate);
            self.operator_c
                .note_on(note, velocity, voice_id, channel, params, sample_rate);
            self.operator_d
                .note_on(note, velocity, voice_id, channel, params, sample_rate);
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
                self.operator_a.note_off(params, sample_rate);
                self.operator_b.note_off(params, sample_rate);
                self.operator_c.note_off(params, sample_rate);
                self.operator_d.note_off(params, sample_rate);
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

impl FmVoice {
    fn update_core_ratios(&mut self, params: &crate::voice_utils::Parameters) {
        self.operator_a
            .update_core_ratio(params.fm_params.op_a_ratio);
        self.operator_b
            .update_core_ratio(params.fm_params.op_b_ratio);
        self.operator_c
            .update_core_ratio(params.fm_params.op_c_ratio);
        self.operator_d
            .update_core_ratio(params.fm_params.op_d_ratio);
    }
    /// This should be called after the voice has been stolen and the steal operation is complete
    fn finish_voice_steal(&mut self, params: &crate::voice_utils::Parameters, sample_rate: f32) {
        // --- What needs to be done ---
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
