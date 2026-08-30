use std::cell::Cell;
use std::f64::consts::TAU;
use std::rc::Rc;
use std::time::Duration;

use gtk::cairo::Context;
use gtk::prelude::*;
use roonscape_renderer::{PresentationActivityWaveform, PresentationBehavior};

pub(crate) fn activity_waveform(
    waveform: PresentationActivityWaveform,
    behavior: PresentationBehavior,
) -> gtk::DrawingArea {
    let drawing = gtk::DrawingArea::new();
    drawing.add_css_class("activity-waveform");
    drawing.set_hexpand(false);
    drawing.set_vexpand(false);

    let scales = Rc::new(Cell::new([1.0; 7]));
    let draw_scales = scales.clone();
    drawing.set_draw_func(move |drawing, context, width, height| {
        draw_waveform(
            drawing,
            context,
            width,
            height,
            waveform.reference_heights_percent,
            draw_scales.get(),
        );
    });

    drawing.add_tick_callback(move |drawing, frame_clock| {
        let system_animations_enabled =
            gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
        let animations_enabled = behavior.animations_enabled(system_animations_enabled);
        let elapsed =
            Duration::from_micros(frame_clock.frame_time().try_into().unwrap_or_default());
        let next_scales = waveform.bar_scales_at(elapsed, animations_enabled);
        if scales.get() != next_scales {
            scales.set(next_scales);
            drawing.queue_draw();
        }
        gtk::glib::ControlFlow::Continue
    });

    drawing
}

fn draw_waveform(
    drawing: &gtk::DrawingArea,
    context: &Context,
    width: i32,
    height: i32,
    reference_heights_percent: [u8; 7],
    scales: [f64; 7],
) {
    let color = drawing.style_context().color();
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()),
    );

    let width = f64::from(width);
    let height = f64::from(height);
    let bar_width = width * 0.075;
    let gap = width * 0.057;
    let waveform_width = bar_width * 7.0 + gap * 6.0;
    let mut x = (width - waveform_width) / 2.0;
    for (reference_height, scale) in reference_heights_percent.into_iter().zip(scales) {
        let bar_height = height * (f64::from(reference_height) / 100.0) * scale;
        let y = (height - bar_height) / 2.0;
        rounded_rectangle(
            context,
            x,
            y,
            bar_width,
            bar_height,
            bar_width.min(bar_height) / 2.0,
        );
        x += bar_width + gap;
    }
    let _ = context.fill();
}

fn rounded_rectangle(context: &Context, x: f64, y: f64, width: f64, height: f64, radius: f64) {
    context.new_sub_path();
    context.arc(x + width - radius, y + radius, radius, -TAU / 4.0, 0.0);
    context.arc(
        x + width - radius,
        y + height - radius,
        radius,
        0.0,
        TAU / 4.0,
    );
    context.arc(
        x + radius,
        y + height - radius,
        radius,
        TAU / 4.0,
        TAU / 2.0,
    );
    context.arc(x + radius, y + radius, radius, TAU / 2.0, TAU * 0.75);
    context.close_path();
}
