#![feature(portable_simd)]


// use filter::iir::{IirFilter, IirFilterType};
use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use nih_plug_vizia::ViziaState;
use std::{sync::Arc, f32::INFINITY};
use std::sync::atomic::{AtomicBool, Ordering};
use std::simd::f32x2;

mod editor;
mod filter;
mod delay;

/// The time it takes for the peak meter to decay by 12 dB after switching to complete silence.
const PEAK_METER_DECAY_MS: f64 = 150.0;

/// This is mostly identical to the gain example, minus some fluff, and with a GUI.
pub struct Esc {
    params: Arc<EscParams>,

    /// Needed to normalize the peak meter's response based on the sample rate.
    peak_meter_decay_weight: f32,
    /// The current data for the peak meter. This is stored as an [`Arc`] so we can share it between
    /// the GUI and the audio processing parts. If you have more state to share, then it's a good
    /// idea to put all of that in a struct behind a single `Arc`.
    ///
    /// This is stored as voltage gain.
    peak_meter: Arc<AtomicF32>,

    /// Needed for computing the filter coefficients. Also used to update `bypass_smoother`, hence
    /// why this needs to be an `Arc<AtomicF32>`.
    sample_rate: Arc<AtomicF32>,

    /// All of the high-pass filters, with vectorized coefficients so they can be calculated for
    /// multiple channels at once. [`DiopserParams::num_stages`] controls how many filters are
    /// actually active.
    filter: filter::Biquad<f32>,

    delay: delay::RingBuffer,
}

#[derive(Params)]
struct EscParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<ViziaState>,

    #[id = "gain"]
    pub gain: FloatParam,

    #[id = "lookahead"]
    pub lookahead: FloatParam,
}

impl Default for Esc {
    fn default() -> Self {
        let sample_rate = Arc::new(AtomicF32::new(1.0));
    
        Self {
            params: Arc::new(
                EscParams::default(),
            ),

            sample_rate,
            filter: filter::Biquad::default(),
            delay: delay::RingBuffer::default(),
            peak_meter_decay_weight: 1.0,
            peak_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),


        }
    }
}

impl Default for EscParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),

            gain: FloatParam::new(
                "gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-120.0),
                    max: util::db_to_gain(24.0),
                    factor: FloatRange::gain_skew_factor(-120.0, 24.0),
                },
            )
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),

            lookahead: FloatParam::new(
                "lookahead",
                0.0,
                FloatRange::Linear {
                    min: 0.0,
                    max: 15.0,
                },
            )
            .with_unit(" ms")
            .with_step_size(0.1)
        }
    }
}

impl Plugin for Esc {
    const NAME: &'static str = "esc";
    const VENDOR: &'static str = "p4thie";
    const URL: &'static str = "";
    const EMAIL: &'static str = "pathiestic@gmail.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),

            aux_input_ports: &[new_nonzero_u32(2)],
            aux_output_ports: &[new_nonzero_u32(2)],

            // Individual ports and the layout as a whole can be named here. By default these names
            // are generated as needed. This layout will be called 'Stereo', while the other one is
            // given the name 'Mono' based no the number of input and output channels.
            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),

            aux_input_ports: &[new_nonzero_u32(1)],
            aux_output_ports: &[new_nonzero_u32(1)],

            ..AudioIOLayout::const_default()
        },
    ];

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.peak_meter.clone(),
            self.params.editor_state.clone(),
        )
    }

    fn initialize(
        &mut self,
        audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate
        .store(buffer_config.sample_rate, Ordering::Relaxed);
        let sample_rate = self.sample_rate.load(Ordering::Relaxed);


        // After `PEAK_METER_DECAY_MS` milliseconds of pure silence, the peak meter's value should
        // have dropped by 12 dB
        self.peak_meter_decay_weight = 0.25f64
            .powf((buffer_config.sample_rate as f64 * PEAK_METER_DECAY_MS / 1000.0).recip())
            as f32;
        
        self.filter.coefficients = filter::BiquadCoefficients::lowpass(sample_rate, 5.0, 0.72);

        self.delay.initialize(audio_io_layout.aux_input_ports[0].get() as usize, sample_rate);

        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {

        let delay_time = self.params.lookahead.smoothed.next();
        let latency_samples = (delay_time / 1000.0 * self.sample_rate.load(Ordering::Relaxed)) as u32;
        context.set_latency_samples(latency_samples);


        for (main_channel_samples, sc_channel_samples) in buffer.iter_samples().zip(&mut aux.inputs[0].iter_samples()) {
            let mut amplitude = 0.0;
            let num_samples = main_channel_samples.len();
    
            let gain = self.params.gain.smoothed.next();                
            for (channel_idx, (sample, sc_sample)) in main_channel_samples.into_iter().zip(&mut sc_channel_samples.into_iter()).enumerate()  {
                    
                *sc_sample = self.filter.process(sc_sample.abs());
                *sc_sample = self.softclip(*sc_sample * gain);

                *sample = self.delay.process(channel_idx, *sample, delay_time);

                *sample -= *sample * *sc_sample;
                //*sample += *sc_sample;
            }
    
            // To save resources, a plugin can (and probably should!) only perform expensive
            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                amplitude = (amplitude / num_samples as f32).abs();
                let current_peak_meter = self.peak_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_peak_meter = if amplitude > current_peak_meter {
                    amplitude
                } else {
                    current_peak_meter * self.peak_meter_decay_weight
                    + amplitude * (1.0 - self.peak_meter_decay_weight)
                };
    
                self.peak_meter
                    .store(new_peak_meter, std::sync::atomic::Ordering::Relaxed)
                }
        }
        ProcessStatus::Normal
    }
}

impl Esc {
    // simple cubic softclipper
    fn softclip(&self, x: f32) -> f32 {
        if x < -1.0 {
            return -1.0;
        } else if x > 1.0 {
            return 1.0;
        } else {
            return (x - x * x * x / 3.0) * (3.0 / 2.0);
        }
    }
}

impl ClapPlugin for Esc {
    const CLAP_ID: &'static str = "com.perfect4th.esc";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("An Easy Sidechain plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for Esc {
    const VST3_CLASS_ID: [u8; 16] = *b"EasySideChain!!!";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Dynamics];
}

nih_export_clap!(Esc);
nih_export_vst3!(Esc);
