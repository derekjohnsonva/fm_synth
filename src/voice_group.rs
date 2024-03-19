use nih_plug::nih_log;

use crate::consts::MAX_VOICES;
/// A container for multiple voices. Used to achieve polyphony.
use crate::voice_utils::{Parameters, Voice};

pub struct VoiceGroup<T: Voice> {
    // TODO: Identify if using an active an inactive vec is the best approach
    active_voices: Vec<Box<T>>,
    inactive_voices: Vec<Box<T>>,
    voice_timings: Vec<i32>,
}

impl<T: Voice> VoiceGroup<T> {
    pub fn new() -> Self {
        let active_voices = Vec::with_capacity(MAX_VOICES);
        let inactive_voices = Vec::with_capacity(MAX_VOICES);
        let voice_timings = Vec::with_capacity(MAX_VOICES);

        VoiceGroup {
            active_voices,
            inactive_voices,
            voice_timings,
        }
    }

    pub fn initialize(
        &mut self,
        num_voices: usize,
        num_channels: usize,
        max_samples_per_channel: usize,
    ) {
        self.active_voices.clear();
        self.voice_timings.clear();
        assert!(num_voices <= MAX_VOICES, "num_voices must be <= MAX_VOICES");
        for _ in 0..MAX_VOICES {
            let mut voice = T::new();
            voice.initialize(num_channels, max_samples_per_channel);
            self.inactive_voices.push(Box::new(voice));
        }
        for _ in 0..num_voices {
            self.active_voices.push(self.inactive_voices.pop().unwrap());
            self.voice_timings.push(0);
        }
    }

    pub fn render(
        &mut self,
        audio_buffer: &mut [&mut [f32]],
        params: &Parameters,
        sample_rate: f32,
        block_start: usize,
        block_end: usize,
    ) {
        // Accumulate the outputs from all voices
        let block_size = block_end - block_start;

        for voice in &mut self.active_voices {
            // Render the voice into the temporary buffer
            voice.render(block_size, params, sample_rate);
        }
        // Accumulate the outputs from all voices
        for voice in self.active_voices.iter_mut() {
            voice.accumulate_output(audio_buffer, block_start, block_end)
        }
    }
    pub fn reset(&mut self, params: &Parameters) {
        self.active_voices
            .iter_mut()
            .for_each(|voice| voice.reset(params));
        self.voice_timings.iter_mut().for_each(|timing| *timing = 0);
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
        // get a voice index. If get_free_voice return None, get the index of the oldest voice
        let voice_index = self.get_free_voice().unwrap_or_else(|| {
            nih_log!("No free voice, using the oldest one");
            self.get_oldest_voice()
                .unwrap_or_else(|| Self::compute_fallback_voice_id(note, channel))
        });
        nih_log!("voice_index chosen: {}", voice_index);
        self.active_voices[voice_index].note_on(
            note,
            velocity,
            voice_id,
            channel,
            params,
            sample_rate,
        );
        // for all voice that are currently playing, increment the timing
        self.voice_timings
            .iter_mut()
            .enumerate()
            .for_each(|(index, timing)| {
                if self.active_voices[index].is_playing() {
                    *timing += 1;
                }
            });
    }
    pub fn note_off(
        &mut self,
        voice_id: Option<i32>,
        channel: u8,
        note: u8,
        params: &Parameters,
        sample_rate: f32,
    ) {
        for voice in self.active_voices.iter_mut() {
            voice.note_off(voice_id, channel, note, params, sample_rate);
        }
    }

    pub fn update_num_voices(&mut self, new_num_voices: usize) {
        assert!(
            new_num_voices <= MAX_VOICES,
            "new_num_voices must be <= MAX_VOICES"
        );
        assert!(new_num_voices > 0, "new_num_voices must be > 0");
        if new_num_voices > self.active_voices.len() {
            for _ in 0..new_num_voices - self.active_voices.len() {
                self.active_voices.push(self.inactive_voices.pop().unwrap());
                self.voice_timings.push(0);
            }
        } else {
            while new_num_voices < self.active_voices.len() {
                // get the oldest voice
                if let Some(oldest_voice) = self.get_oldest_voice() {
                    self.inactive_voices
                        .push(self.active_voices.remove(oldest_voice));
                    self.voice_timings.remove(oldest_voice);
                }
            }
        }
    }

    fn get_oldest_voice(&self) -> Option<usize> {
        self.voice_timings
            .iter()
            .enumerate()
            .max_by_key(|&(_, timing)| timing)
            .map(|(index, _)| index)
    }

    fn get_free_voice(&self) -> Option<usize> {
        self.active_voices
            .iter()
            .position(|voice| !voice.is_playing())
    }
    /// Compute a voice ID in case the host doesn't provide them. Polyphonic modulation will not work in
    /// this case, but playing notes will.
    const fn compute_fallback_voice_id(note: u8, channel: u8) -> usize {
        note as usize | ((channel as usize) << 16)
    }
}

// setup tests
#[cfg(test)]
mod tests {
    use crate::sin_voice::SinVoice;

    use super::*;

    #[test]
    fn test_get_oldest_voice() {
        let mut voice_group: VoiceGroup<SinVoice> = VoiceGroup::new();
        voice_group.initialize(4, 2, 1024);
        voice_group.voice_timings = vec![0, 1, 2, 3];
        assert_eq!(voice_group.get_oldest_voice(), Some(3));

        // Test case 2: all voices have the same timing
        voice_group.voice_timings = vec![0, 0, 0, 0];
        assert_eq!(voice_group.get_oldest_voice(), Some(3));

        // Test case 3: voice_timings is empty
        voice_group.voice_timings = vec![];
        assert_eq!(voice_group.get_oldest_voice(), None);
    }

    #[test]
    fn test_update_num_voices() {
        let mut voice_group: VoiceGroup<SinVoice> = VoiceGroup::new();
        voice_group.initialize(4, 2, 1024);
        voice_group.update_num_voices(6);
        assert_eq!(voice_group.active_voices.len(), 6);
        assert_eq!(voice_group.voice_timings.len(), 6);
        voice_group.voice_timings = vec![0, 5, 2, 4, 1, 3];
        voice_group.update_num_voices(2);
        assert_eq!(voice_group.active_voices.len(), 2);
        assert_eq!(voice_group.voice_timings.len(), 2);
        assert_eq!(voice_group.voice_timings, vec![0, 1]);
    }

    #[test]
    fn test_render() {
        let mut voice_group: VoiceGroup<SinVoice> = VoiceGroup::new();
        voice_group.initialize(4, 2, 1024);
        let mut audio_buffer = vec![vec![0.0; 1024], vec![0.0; 1024]];
        let audio_buffer_slices: &mut [&mut [f32]] = &mut audio_buffer
            .iter_mut()
            .map(|v| v.as_mut_slice())
            .collect::<Vec<_>>();
        let params = Parameters::default();
        voice_group.render(audio_buffer_slices, &params, 44100.0, 0, 1024);
        assert_eq!(audio_buffer_slices[0][0], 0.0);
        assert_eq!(audio_buffer_slices[1][0], 0.0);
    }
}
