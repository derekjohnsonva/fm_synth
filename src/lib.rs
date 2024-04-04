use nih_plug::prelude::*;

use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::Arc;

mod clock;
mod consts;
mod fm_core;
mod fm_operator;
mod fm_voice;
mod linear_eg;
mod sin_osc;
mod sin_voice;
mod voice_group;
mod voice_utils;

/// The maximum size of an audio block. We'll split up the audio in blocks and render smoothed
/// values to buffers since these values may need to be reused for multiple voices.
const MAX_BLOCK_SIZE: usize = 64;

pub struct FmSynth {
    params: Arc<FmSynthParams>,
    // used to store the state of one fm operator
    voices: voice_group::VoiceGroup<fm_voice::FmVoice>,
    voice_params: voice_utils::Parameters,
    sample_rate: f32,
}

#[derive(Params)]
struct FmSynthParams {
    /// The parameter's ID is used to identify the parameter in the wrapper plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined. In this case, this
    /// gain parameter is stored as linear gain while the values are displayed in decibels.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,
    #[id = "gain"]
    pub gain: FloatParam,
    #[id = "attack_time"]
    pub attack_time: FloatParam,
    // TODO: Make it so that if decay time is less that a certain value, sustain level is not used.
    #[id = "decay_time"]
    pub decay_time: FloatParam,
    #[id = "sustain_level"]
    pub sustain_level: FloatParam,
    #[id = "release_time"]
    pub release_time: FloatParam,
    #[id = "num_voices"]
    pub num_voices: IntParam,
    // idex
    #[id = "operator_a_index"]
    pub operator_a_index: FloatParam,
    #[id = "operator_b_index"]
    pub operator_b_index: FloatParam,
    #[id = "operator_c_index"]
    pub operator_c_index: FloatParam,
    #[id = "operator_d_index"]
    pub operator_d_index: FloatParam,
    // ratio
    #[id = "operator_a_ratio"]
    pub operator_a_ratio: FloatParam,
    #[id = "operator_b_ratio"]
    pub operator_b_ratio: FloatParam,
    #[id = "operator_c_ratio"]
    pub operator_c_ratio: FloatParam,
    #[id = "operator_d_ratio"]
    pub operator_d_ratio: FloatParam,
    // mix
    #[id = "operator_a_mix"]
    pub operator_a_mix: FloatParam,
    #[id = "operator_b_mix"]
    pub operator_b_mix: FloatParam,
    #[id = "operator_c_mix"]
    pub operator_c_mix: FloatParam,
    #[id = "operator_d_mix"]
    pub operator_d_mix: FloatParam,
}

impl Default for FmSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(FmSynthParams::default()),
            voices: voice_group::VoiceGroup::new(),
            voice_params: voice_utils::Parameters::default(),
            sample_rate: 0.0,
        }
    }
}

#[allow(clippy::too_many_lines)]
impl Default for FmSynthParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(300, 180),
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

            operator_a_index: FloatParam::new(
                "Operator A Index",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_b_index: FloatParam::new(
                "Operator B Index",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_c_index: FloatParam::new(
                "Operator C Index",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_d_index: FloatParam::new(
                "Operator D Index",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_a_ratio: FloatParam::new(
                "Operator A ratio",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_b_ratio: FloatParam::new(
                "Operator B ratio",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_c_ratio: FloatParam::new(
                "Operator C ratio",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),
            operator_d_ratio: FloatParam::new(
                "Operator D ratio",
                1.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 10.0,
                },
            ),

            operator_a_mix: FloatParam::new(
                "Operator A Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            operator_b_mix: FloatParam::new(
                "Operator B Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            operator_c_mix: FloatParam::new(
                "Operator C Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            ),
            operator_d_mix: FloatParam::new(
                "Operator D Mix",
                1.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
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
            num_voices: IntParam::new(
                "Number of Voices",
                4,
                IntRange::Linear {
                    min: 1,
                    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)]
                    max: consts::MAX_VOICES as i32,
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
    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        create_egui_editor(
            self.params.editor_state.clone(),
            (),
            |_, _| {},
            move |egui_ctx, setter, _state| {
                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    ui.label("Gain");
                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
                    ui.label("Attack Time");
                    ui.add(widgets::ParamSlider::for_param(&params.attack_time, setter));
                    ui.label("Decay Time");
                    ui.add(widgets::ParamSlider::for_param(&params.decay_time, setter));
                    ui.label("Sustain Level");
                    ui.add(widgets::ParamSlider::for_param(
                        &params.sustain_level,
                        setter,
                    ));
                    ui.label("Release Time");
                    ui.add(widgets::ParamSlider::for_param(
                        &params.release_time,
                        setter,
                    ));
                    ui.label("Number of Voices");
                    ui.add(widgets::ParamSlider::for_param(&params.num_voices, setter));
                    ui.label("Operator A Index");
                    ui.add(widgets::ParamSlider::for_param(
                        &params.operator_a_index,
                        setter,
                    ));
                });
            },
        )
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        // Resize buffers and perform other potentially expensive initialization operations here.
        // The `reset()` function is always called right after this function. You can remove this
        // function if you do not need it.
        self.sample_rate = buffer_config.sample_rate;
        // get the number of output channels
        let num_channels = audio_io_layout
            .main_output_channels
            .map_or(2, NonZeroU32::get);
        self.voices.initialize(
            4,
            num_channels as usize,
            buffer_config.max_buffer_size as usize,
        );
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.voices.reset(&self.voice_params);
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
        #[allow(clippy::cast_sign_loss)]
        self.voices.update_num_voices(
            self.params
                .num_voices
                .smoothed
                .next_step(num_samples as u32) as usize,
        );
        self.sample_rate = context.transport().sample_rate;
        let output = buffer.as_slice();

        let mut next_event = context.next_event();
        let mut block_start: usize = 0;
        let mut block_end: usize = MAX_BLOCK_SIZE.min(num_samples);

        while block_start < num_samples {
            // In each block of samples, we need to check for note events. We will split audio rendering
            // into sub-blocks based on the timing of the note events. If we have a note event at the start of the
            // block, we will update the voice parameters.

            'events: loop {
                match next_event {
                    Some(event) if (event.timing() as usize) <= block_start => {
                        nih_dbg!(event);
                        match event {
                            NoteEvent::NoteOn {
                                note,
                                velocity,
                                voice_id,
                                channel,
                                ..
                            } => {
                                self.voices.note_on(
                                    note,
                                    velocity,
                                    voice_id,
                                    channel,
                                    &self.voice_params,
                                    self.sample_rate,
                                );
                            }
                            NoteEvent::NoteOff {
                                note,
                                voice_id,
                                channel,
                                ..
                            } => self.voices.note_off(
                                voice_id,
                                channel,
                                note,
                                &self.voice_params,
                                self.sample_rate,
                            ),
                            _ => {}
                        };

                        next_event = context.next_event();
                    }
                    Some(event) if (event.timing() as usize) < block_end => {
                        block_end = event.timing() as usize;
                        break 'events;
                    }
                    _ => break 'events,
                }
            }

            let num_samples_to_process = block_end.checked_sub(block_start);
            let num_samples_to_process_u32 = num_samples_to_process.unwrap_or(0) as u32;
            self.set_parameters(num_samples_to_process_u32);
            self.voices.render(
                output,
                &self.voice_params,
                self.sample_rate,
                block_start,
                block_end,
            );
            // And then just keep processing blocks until we've run out of buffer to fill
            block_start = block_end;
            block_end = (block_start + MAX_BLOCK_SIZE).min(num_samples);
        }

        ProcessStatus::KeepAlive
    }
}

impl FmSynth {
    fn set_parameters(&mut self, num_samples_to_process_u32: u32) {
        self.voice_params.eg_params = linear_eg::EGParameters {
            attack_time_msec: self
                .params
                .attack_time
                .smoothed
                .next_step(num_samples_to_process_u32),
            decay_time_msec: self
                .params
                .decay_time
                .smoothed
                .next_step(num_samples_to_process_u32),
            release_time_msec: self
                .params
                .release_time
                .smoothed
                .next_step(num_samples_to_process_u32),
            start_level: 0.0,
            sustain_level: self
                .params
                .sustain_level
                .smoothed
                .next_step(num_samples_to_process_u32),
        };
        self.voice_params.fm_params = voice_utils::FmParams {
            op_a_ratio: self
                .params
                .operator_a_ratio
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_b_ratio: self
                .params
                .operator_b_ratio
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_c_ratio: self
                .params
                .operator_c_ratio
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_d_ratio: self
                .params
                .operator_d_ratio
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_a_index: self
                .params
                .operator_a_index
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_b_index: self
                .params
                .operator_b_index
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_c_index: self
                .params
                .operator_c_index
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_d_index: self
                .params
                .operator_d_index
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_a_mix: self
                .params
                .operator_a_mix
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_b_mix: self
                .params
                .operator_b_mix
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_c_mix: self
                .params
                .operator_c_mix
                .smoothed
                .next_step(num_samples_to_process_u32),
            op_d_mix: self
                .params
                .operator_d_mix
                .smoothed
                .next_step(num_samples_to_process_u32),
        };
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
