use std::sync::Arc;

use nih_plug::editor::Editor;
use nih_plug_vizia::{
    create_vizia_editor,
    vizia::prelude::*,
    widgets::{ParamSlider, ResizeHandle},
    ViziaState,
};

use crate::FmSynthParams;
#[allow(clippy::expl_impl_clone_on_copy)]
#[derive(Lens)]
struct Data {
    params: Arc<FmSynthParams>,
}

impl Model for Data {}

pub fn default_state() -> Arc<ViziaState> {
    ViziaState::new(|| (200, 150))
}

pub fn create(
    params: Arc<FmSynthParams>,
    editor_state: Arc<ViziaState>,
) -> Option<Box<dyn Editor>> {
    create_vizia_editor(
        editor_state,
        nih_plug_vizia::ViziaTheming::Custom,
        move |cx, _| {
            nih_plug_vizia::assets::register_noto_sans_light(cx);
            nih_plug_vizia::assets::register_noto_sans_thin(cx);

            Data {
                params: params.clone(),
            }
            .build(cx);

            VStack::new(cx, |cx| {
                Label::new(cx, "FM Synth")
                    .font_family(vec![FamilyOwned::Name(String::from(
                        nih_plug_vizia::assets::NOTO_SANS,
                    ))])
                    .font_weight(FontWeightKeyword::Thin)
                    .font_size(30.0)
                    .height(Pixels(50.0))
                    .child_top(Stretch(1.0))
                    .child_bottom(Pixels(0.0));
                Label::new(cx, "Gain");
                ParamSlider::new(cx, Data::params, |params| &params.gain);
            })
            .row_between(Pixels(0.0))
            .child_left(Stretch(1.0))
            .child_right(Stretch(1.0));

            ResizeHandle::new(cx);
        },
    )
}
