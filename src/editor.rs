use nih_plug::prelude::*;
use nih_plug_vizia::vizia::prelude::*;
use nih_plug_vizia::vizia::vg;
use nih_plug_vizia::widgets::{ParamSlider, ParamEvent};
use std::sync::Arc;
use crate::{Trajectory, ToposynthParams, TRAJECTORY_SIZE};

use nih_plug::context::gui::ParamSetter;

fn apply_preset(gui_cx: &Arc<dyn GuiContext>, params: &ToposynthParams, preset: &str) {
    let setter = ParamSetter::new(gui_cx.as_ref());

    let set_float = |param: &FloatParam, val: f32| {
        setter.begin_set_parameter(param);
        setter.set_parameter(param, val);
        setter.end_set_parameter(param);
    };

    match preset {
        "Init" => {
            set_float(&params.cutoff, 1000.0);
            set_float(&params.resonance, 0.1);
            set_float(&params.morph, 0.0);
            set_float(&params.chaos_to_cutoff, 0.0);
            set_float(&params.chaos_to_fm, 0.0);
            set_float(&params.macro_organic, 0.0);
            set_float(&params.macro_metal, 0.0);
            set_float(&params.macro_drift, 0.0);
            set_float(&params.macro_unstable, 0.0);
        }
        "Metal" => {
            set_float(&params.cutoff, 4000.0);
            set_float(&params.resonance, 0.8);
            set_float(&params.morph, 1.0);
            set_float(&params.chaos_to_cutoff, 0.8);
            set_float(&params.chaos_to_fm, 0.9);
            set_float(&params.macro_organic, 0.0);
            set_float(&params.macro_metal, 1.0);
            set_float(&params.macro_drift, 0.2);
            set_float(&params.macro_unstable, 0.3);
        }
        "Rhythmic" => {
            set_float(&params.cutoff, 500.0);
            set_float(&params.resonance, 0.9);
            set_float(&params.morph, 0.5);
            set_float(&params.chaos_to_cutoff, 0.9);
            set_float(&params.chaos_to_fm, 0.2);
            set_float(&params.macro_organic, 0.8);
            set_float(&params.macro_metal, 0.0);
            set_float(&params.macro_drift, 0.9);
            set_float(&params.macro_unstable, 0.8);
        }
        _ => {}
    }
}

#[derive(Lens)]
pub struct EditorData {
    pub params: Arc<ToposynthParams>,
}

impl Model for EditorData {}

pub struct TrajectoryView {
    trajectory: Arc<std::sync::RwLock<Trajectory>>,
}

impl TrajectoryView {
    pub fn new(cx: &mut Context, trajectory: Arc<std::sync::RwLock<Trajectory>>) -> Handle<Self> {
        Self { trajectory }.build(cx, |_| {})
    }
}

impl View for TrajectoryView {
    fn element(&self) -> Option<&'static str> {
        Some("trajectory-view")
    }

    fn draw(&self, cx: &mut DrawContext, canvas: &mut Canvas) {
        let bounds = cx.bounds();
        if bounds.w == 0.0 || bounds.h == 0.0 { return; }

        let traj = match self.trajectory.try_read() {
            Ok(t) => t,
            Err(_) => return,
        };

        let mut path = vg::Path::new();
        let center_x = bounds.x + bounds.w / 2.0;
        let center_y = bounds.y + bounds.h / 2.0;
        let scale = bounds.w.min(bounds.h) * 0.45;

        let mut first = true;
        for i in 0..TRAJECTORY_SIZE {
            let (x, y, _) = traj.points[(traj.write_pos + i) % TRAJECTORY_SIZE];
            let px = center_x + (x * scale / 25.0);
            let py = center_y - (y * scale / 25.0);
            if first { path.move_to(px, py); first = false; }
            else { path.line_to(px, py); }
        }

        let mut paint = vg::Paint::color(vg::Color::rgb(0, 255, 200));
        paint.set_line_width(1.5);
        canvas.stroke_path(&path, &paint);
    }
}

pub fn create_editor(
    params: Arc<ToposynthParams>,
    trajectory: Arc<std::sync::RwLock<Trajectory>>,
) -> Option<Box<dyn Editor>> {
    nih_plug_vizia::create_vizia_editor(params.editor_state.clone(), nih_plug_vizia::ViziaTheming::Custom, move |cx, gui_ctx| {
        EditorData { params: params.clone() }.build(cx);

        // Use include_str! for CSS — this is safe as it's embedded at compile time
        let _ = cx.add_stylesheet(include_str!("editor.css"));

        VStack::new(cx, |cx| {
            Label::new(cx, "TOPOSYNTH")
                .color(Color::rgb(0, 255, 200))
                .font_size(20.0);

            HStack::new(cx, |cx| {
                TrajectoryView::new(cx, trajectory.clone())
                    .width(Stretch(1.0))
                    .height(Stretch(1.0));

                VStack::new(cx, |cx| {
                    HStack::new(cx, |cx| {
                        let p = params.clone();
                        let gcx = gui_ctx.clone();
                        Button::new(cx, move |_| apply_preset(&gcx, &p, "Init"), |cx| Label::new(cx, "Init"));
                        let p = params.clone();
                        let gcx = gui_ctx.clone();
                        Button::new(cx, move |_| apply_preset(&gcx, &p, "Metal"), |cx| Label::new(cx, "Metal"));
                        let p = params.clone();
                        let gcx = gui_ctx.clone();
                        Button::new(cx, move |_| apply_preset(&gcx, &p, "Rhythmic"), |cx| Label::new(cx, "Rhythmic"));
                    }).height(Pixels(30.0)).col_between(Pixels(10.0));

                    Label::new(cx, "Gain").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.gain);
                    Label::new(cx, "Cutoff").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.cutoff);
                    Label::new(cx, "Resonance").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.resonance);
                    Label::new(cx, "Morph").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.morph);
                    Label::new(cx, "Chaos > Cutoff").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.chaos_to_cutoff);
                    Label::new(cx, "Chaos > FM").color(Color::rgb(180, 180, 180)).font_size(11.0);
                    ParamSlider::new(cx, EditorData::params, |p| &p.chaos_to_fm);
                }).width(Pixels(300.0));
            });
        });
    })
}
