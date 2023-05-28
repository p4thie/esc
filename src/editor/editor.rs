use atomic_float::AtomicF32;
use nih_plug::prelude::{util, Editor};
use nih_plug_vizia::vizia::image::Pixel;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg::Paint;
use nih_plug_vizia::vizia_assets;
use nih_plug_vizia::widgets::*;
use nih_plug_vizia::{assets, create_vizia_editor, ViziaState, ViziaTheming};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;

use crate::editor::visualizer::VisualizerViewAlt;
use crate::visualizer::Visualizer;
use crate::EscParams;

use super::visualizer::VisualizerView;

#[derive(Lens)]
struct Data {
    params: Arc<EscParams>,
    peak_meter: Arc<AtomicF32>,
    visualizer_main: Arc<Visualizer>,
    visualizer_sc: Arc<Visualizer>,
}

impl Model for Data {}

// Makes sense to also define this here, makes it a bit easier to keep track of
pub(crate) fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (200, 200))
}

pub(crate) fn create(
    params: Arc<EscParams>,
    peak_meter: Arc<AtomicF32>,
    editor_state: Arc<ViziaState>,
    visualizer_main: Arc<Visualizer>,
    visualizer_sc: Arc<Visualizer>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(editor_state, ViziaTheming::Custom, move |cx, _| {
        cx.add_theme(include_str!("theme.css"));
        cx.add_theme(include_str!("widgets.css"));

        const FRAGMENTPATH: &[u8] = include_bytes!("../../../fonts/FragmentMono-Regular.ttf");

        cx.add_fonts_mem(&[FRAGMENTPATH]);

        const FRAGMENT: &str = "Fragment Mono";

        Data {
            params: params.clone(),
            peak_meter: peak_meter.clone(),
            visualizer_main: visualizer_main.clone(),
            visualizer_sc: visualizer_sc.clone(),
        }
        .build(cx);

        VStack::new(cx, |cx| {
            HStack::new(cx, |cx| {
                Label::new(cx, "esc")
                    .font_family(vec![FamilyOwned::Name(String::from(FRAGMENT))])
                    .height(Pixels(25.0))
                    .id("plugin-label");
            })
            .height(Pixels(12.0))
            .id("plugin-desc");

            ZStack::new(cx, |cx| {
                VisualizerViewAlt::new(cx, Data::visualizer_sc)
                    .id("visualizer_sc")
                    .height(Percentage(50.0));
                VisualizerView::new(cx, Data::visualizer_main)
                    .id("visualizer_main")
                    .top(Percentage(25.0))
                    .height(Percentage(75.0));
            })
            .id("visualizer");
            VStack::new(cx, |cx| {
                GenericUi::new(cx, Data::params)
                    .top(Pixels(0.0))
                    .font_size(10.0)
                    .font_family(vec![FamilyOwned::Name(String::from(FRAGMENT))]);
            })
            .id("param-sliders")
            .height(Stretch(1.0))
            .child_right(Stretch(1.0))
            .child_bottom(Stretch(1.0));
        });

        ResizeHandle::new(cx);
        Label::new(cx, "0.1.0")
            .font_family(vec![FamilyOwned::Name(String::from(FRAGMENT))])
            .id("plugin-version");
    })
}
