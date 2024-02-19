use nih_plug::prelude::*;

use std::sync::Arc;

mod clock;
mod fm_core;
mod linear_eg;
mod sin_osc;
mod voice;

/// The maximum size of an audio block. We'll split up the audio in blocks and render smoothed
/// values to buffers since these values may need to be reused for multiple voices.
const MAX_BLOCK_SIZE: usize = 64;

pub struct FmSynth {
    params: Arc<FmSynthParams>,
    // used to store the state of one fm operator
    voice: voice::Voice,
    sample_rate: f32,
    voice_params: voice::VoiceParameters,
}

#[derive(Params)]
struct FmSynthParams {
    /// The parameter's ID is used to identify the parameter in the wrapper plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined. In this case, this
    /// gain parameter is stored as linear gain while the values are displayed in decibels.
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "modulation_index"]
    pub modulation_index: FloatParam,
    #[id = "attack_time"]
    pub attack_time: FloatParam,
    // TODO: Make it so that if decay time is less that a certain value, sustain level is not used.
    #[id = "decay_time"]
    pub decay_time: FloatParam,
    #[id = "sustain_level"]
    pub sustain_level: FloatParam,
    #[id = "release_time"]
    pub release_time: FloatParam,
}

impl Default for FmSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(FmSynthParams::default()),
            voice: voice::Voice::new(),
            sample_rate: 0.0,
            voice_params: voice::VoiceParameters::default(),
        }
    }
}

impl FmSynth {
    // make a function that will update the VoiceParameters from the gui
    fn update_voice_params(&mut self, num_samples_to_process: usize, sample_rate: f32) {
        let eg_params = linear_eg::EGParameters {
            attack_time_msec: self
                .params
                .attack_time
                .smoothed
                .next_step(num_samples_to_process as u32),
            decay_time_msec: self
                .params
                .decay_time
                .smoothed
                .next_step(num_samples_to_process as u32),
            release_time_msec: self
                .params
                .release_time
                .smoothed
                .next_step(num_samples_to_process as u32),
            start_level: 0.0,
            sustain_level: self
                .params
                .sustain_level
                .smoothed
                .next_step(num_samples_to_process as u32),
        };
        self.voice_params.eg_params = eg_params;
        self.voice_params.sample_rate = sample_rate;
    }
}

impl Default for FmSynthParams {
    fn default() -> Self {
        Self {
            // This gain is stored as linear gain. NIH-plug comes with useful conversion functions
            // to treat these kinds of parameters as if we were dealing with decibels. Storing this
            // as decibels is easier to work with, but requires a conversion for every sample.
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    // This makes the range appear as if it was linear when displaying the values as
                    // decibels
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            // Because the gain parameter is stored as linear gain instead of storing the value as
            // decibels, we need logarithmic smoothing
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            // There are many predefined formatters we can use here. If the gain was stored as
            // decibels instead of as a linear gain value, we could have also used the
            // `.with_step_size(0.1)` function to get internal rounding.
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            modulation_index: FloatParam::new(
                "Modulation Index",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            attack_time: FloatParam::new(
                "Attack Time",
                10.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 1000.0,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" ms"),
            decay_time: FloatParam::new(
                "Decay Time",
                100.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 1000.0,
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" ms"),
            sustain_level: FloatParam::new(
                "Sustain Level",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(50.0)),
            release_time: FloatParam::new(
                "Release Time",
                100.0,
                FloatRange::Linear {
                    min: 1.0,
                    max: 1000.0,
                },
            ),
        }
    }
}

impl Plugin for FmSynth {
    const NAME: &'static str = "Fm Synth";
    const VENDOR: &'static str = "Derek Johnson";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "derekjohnsonva@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            // This is also the default and can be omitted here
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        // AudioIOLayout {
        //     main_input_channels: NonZeroU32::new(1),
        //     main_output_channels: NonZeroU32::new(1),
        //     ..AudioIOLayout::const_default()
        // },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    // const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = buffer_config.sample_rate;
        self.voice.reset(&self.voice_params);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.voice.reset(&self.voice_params);
    }
    #[allow(clippy::cast_possible_truncation)]
    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // NIH-plug has a block-splitting adapter for `Buffer`. While this works great for effect
        // plugins, for polyphonic synths the block size should be `min(MAX_BLOCK_SIZE,
        // num_remaining_samples, next_event_idx - block_start_idx)`. Because blocks also need to be
        // split on note events, it's easier to work with raw audio here and to do the splitting by
        // hand.
        let num_samples = buffer.samples();
        let sample_rate = context.transport().sample_rate;
        let output = buffer.as_slice();

        let mut next_event = context.next_event();
        let mut block_start: usize = 0;
        let mut block_end: usize = MAX_BLOCK_SIZE.min(num_samples);

        while block_start < num_samples {
            // We have three things that can happen in a block:
            // 1. The block ends before the next event
            // 2. The next event happens before the block ends. In this case, we have to split the
            // block and process the event. We then continue with the next block.
            // 3. The block ends at the same time as the next event. In this case, we process the
            // event and continue with the next block.
            // To handle these cases, we will only process events at the start of a block. If an event
            // happens in the middle of a block, we will process it at the start of the next block.
            match next_event {
                Some(event) if (event.timing() as usize) <= block_start => {
                    match event {
                        NoteEvent::NoteOn {
                            note,
                            velocity,
                            voice_id,
                            channel,
                            ..
                        } => {
                            self.voice.note_on(
                                note,
                                velocity,
                                voice_id,
                                channel,
                                &self.voice_params,
                            );
                        }
                        NoteEvent::NoteOff {
                            note,
                            voice_id,
                            channel,
                            ..
                        } => {
                            self.voice
                                .note_off(voice_id, channel, note, &self.voice_params);
                        }
                        _ => {}
                    }

                    next_event = context.next_event();
                }
                Some(event) if (event.timing() as usize) < block_end => {
                    block_end = event.timing() as usize;
                }
                _ => (),
            }

            // fill the buffer from the start of the block to the end of the block with zeros
            // This will make it easier when we turn this into a poly synth
            for channel in output.iter_mut() {
                channel[block_start..block_end].fill(0.0);
            }

            // Get the Envelope generator value
            let num_samples_to_process = block_end.checked_sub(block_start);
            let num_samples_to_process = match num_samples_to_process {
                Some(num_samples_to_process) => num_samples_to_process,
                None => {
                    nih_error!("Error with block size");
                    break;
                }
            };

            self.update_voice_params(num_samples_to_process, sample_rate);
            let rendered_values =
                self.voice
                    .render(&self.voice_params, num_samples_to_process, output.len());
            nih_dbg!("Pased Rendered values");
            for (channel, rendered_values) in output.iter_mut().zip(rendered_values) {
                for (sample_idx, rendered_value) in rendered_values.iter().enumerate() {
                    channel[block_start + sample_idx] = *rendered_value;
                }
            }

            // let eg_value = self.eg.render(&self.eg_params, num_samples_to_process);

            // // let block_len = block_end - block_start;
            // for sample_idx in (block_start..block_end).step_by(1) {
            //     let gain = self.params.gain.smoothed.next();
            //     let sine = self.fm_core.render();
            //     for channel in output.iter_mut() {
            //         channel[sample_idx] = sine * gain * eg_value;
            //     }
            // }
            // And then just keep processing blocks until we've run out of buffer to fill
            block_start = block_end;
            block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);
        }

        ProcessStatus::KeepAlive
    }
}

impl ClapPlugin for FmSynth {
    const CLAP_ID: &'static str = "com.derekjohnson.fm-synth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Simple FM synth");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;

    // Don't forget to change these features
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::Instrument,
        ClapFeature::Synthesizer,
        ClapFeature::Stereo,
        // ClapFeature::Mono,
    ];
}

impl Vst3Plugin for FmSynth {
    const VST3_CLASS_ID: [u8; 16] = *b"DerekJohnson.fms";

    // And also don't forget to change these categories
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Synth,
        Vst3SubCategory::Stereo,
    ];
}

nih_export_clap!(FmSynth);
// nih_export_vst3!(FmSynth);
