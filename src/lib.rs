use nih_plug::prelude::*;

use std::sync::Arc;

mod clock;
mod fm_core;
mod sin_osc;

use fm_core::FmCore;

/// The maximum size of an audio block. We'll split up the audio in blocks and render smoothed
/// values to buffers since these values may need to be reused for multiple voices.
const MAX_BLOCK_SIZE: usize = 64;

struct FmSynth {
    params: Arc<FmSynthParams>,
    // used to store the state of one fm operator
    fm_core: FmCore,
    sample_rate: f32,
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
}

impl Default for FmSynth {
    fn default() -> Self {
        Self {
            params: Arc::new(FmSynthParams::default()),
            fm_core: FmCore::new(),
            sample_rate: 0.0,
        }
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
        self.fm_core.note_on(60, 1.0, self.sample_rate);
        true
    }

    fn reset(&mut self) {
        // Reset buffers and envelopes here. This can be called from the audio thread and may not
        // allocate. You can remove this function if you do not need it.
        self.fm_core.reset();
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
                        NoteEvent::NoteOn { note, velocity, .. } => {
                            self.fm_core.note_on(note, velocity, sample_rate);
                        }
                        NoteEvent::NoteOff { .. } => {
                            self.fm_core.note_off();
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

            // let block_len = block_end - block_start;
            for sample_idx in (block_start..block_end).step_by(1) {
                let gain = self.params.gain.smoothed.next();
                let sine = self.fm_core.render();
                for channel in output.iter_mut() {
                    channel[sample_idx] = sine * gain;
                }
            }
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
