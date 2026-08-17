use std::cell::Cell;
use std::f64::consts::TAU;
use std::rc::Rc;

use gtk::cairo::{Context, LineCap, LineJoin};
use gtk::prelude::*;
use roonscape_renderer::{PresentationStatus, PresentationStatusMotion, PresentationStatusSymbol};

const GLYPH_GRID: f64 = 32.0;
const GLYPH_SCALE: f64 = 0.44;

pub(crate) fn presentation_status_symbol(status: &PresentationStatus) -> gtk::Box {
    let container = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    container.add_css_class("status-symbol-container");
    container.set_halign(gtk::Align::Center);
    container.set_valign(gtk::Align::Center);

    let drawing = gtk::DrawingArea::new();
    drawing.set_hexpand(true);
    drawing.set_vexpand(true);
    let rotation = Rc::new(Cell::new(0.0));
    let draw_rotation = rotation.clone();
    let symbol = status.symbol;
    drawing.set_draw_func(move |drawing, context, width, height| {
        draw_symbol(drawing, context, width, height, symbol, draw_rotation.get());
    });

    if let PresentationStatusMotion::ContinuousRotation { period } = status.motion {
        drawing.add_tick_callback(move |drawing, frame_clock| {
            let animations_enabled =
                gtk::Settings::default().is_none_or(|settings| settings.is_gtk_enable_animations());
            let next_rotation = if animations_enabled {
                let period_micros = period.as_micros() as f64;
                (frame_clock.frame_time() as f64).rem_euclid(period_micros) / period_micros * TAU
            } else {
                0.0
            };
            if (rotation.get() - next_rotation).abs() > f64::EPSILON {
                rotation.set(next_rotation);
                drawing.queue_draw();
            }
            gtk::glib::ControlFlow::Continue
        });
    }

    container.append(&drawing);
    container
}

fn draw_symbol(
    drawing: &gtk::DrawingArea,
    context: &Context,
    width: i32,
    height: i32,
    symbol: PresentationStatusSymbol,
    rotation: f64,
) {
    let color = drawing.style_context().color();
    context.set_source_rgba(
        f64::from(color.red()),
        f64::from(color.green()),
        f64::from(color.blue()),
        f64::from(color.alpha()),
    );
    let extent = f64::from(width.min(height)) * GLYPH_SCALE;
    context.translate(
        (f64::from(width) - extent) / 2.0,
        (f64::from(height) - extent) / 2.0,
    );
    context.scale(extent / GLYPH_GRID, extent / GLYPH_GRID);
    context.set_line_cap(symbol_line_cap(symbol));
    context.set_line_join(LineJoin::Round);

    match symbol {
        PresentationStatusSymbol::Playing => draw_playing(context),
        PresentationStatusSymbol::Paused => draw_paused(context),
        PresentationStatusSymbol::Starting => draw_starting(context, rotation),
        PresentationStatusSymbol::Idle => draw_idle(context),
        PresentationStatusSymbol::PairingRequired => draw_pairing_required(context),
        PresentationStatusSymbol::Disconnected => draw_disconnected(context),
        PresentationStatusSymbol::OutputUnavailable => draw_output_unavailable(context),
    }
}

fn symbol_line_cap(symbol: PresentationStatusSymbol) -> LineCap {
    match symbol {
        PresentationStatusSymbol::PairingRequired
        | PresentationStatusSymbol::Disconnected
        | PresentationStatusSymbol::OutputUnavailable => LineCap::Round,
        PresentationStatusSymbol::Playing
        | PresentationStatusSymbol::Paused
        | PresentationStatusSymbol::Starting
        | PresentationStatusSymbol::Idle => LineCap::Butt,
    }
}

fn draw_playing(context: &Context) {
    context.move_to(10.0, 6.5);
    context.line_to(25.0, 16.0);
    context.line_to(10.0, 25.5);
    context.close_path();
    let _ = context.fill();
}

fn draw_paused(context: &Context) {
    rounded_rectangle(context, 8.0, 6.0, 6.0, 20.0, 1.0);
    rounded_rectangle(context, 18.0, 6.0, 6.0, 20.0, 1.0);
    let _ = context.fill();
}

fn draw_starting(context: &Context, rotation: f64) {
    let _ = context.save();
    context.translate(16.0, 16.0);
    context.rotate(rotation);
    context.translate(-16.0, -16.0);
    context.set_line_width(4.0);
    context.set_dash(&[17.0, 8.0], 0.0);
    context.arc(16.0, 16.0, 10.0, 0.0, TAU);
    let _ = context.stroke();
    context.set_dash(&[], 0.0);
    context.arc(16.0, 16.0, 3.0, 0.0, TAU);
    let _ = context.fill();
    let _ = context.restore();
}

fn draw_idle(context: &Context) {
    rounded_rectangle(context, 9.0, 9.0, 14.0, 14.0, 2.0);
    let _ = context.fill();
}

fn draw_pairing_required(context: &Context) {
    context.set_line_width(3.0);

    context.move_to(13.5, 20.5);
    context.line_to(10.0, 24.0);
    context.curve_to(8.1, 25.9, 4.9, 25.9, 3.0, 24.0);
    context.curve_to(1.1, 22.1, 1.1, 18.9, 3.0, 17.0);
    context.line_to(8.0, 12.0);
    context.curve_to(9.9, 10.1, 13.1, 10.1, 15.0, 12.0);
    let _ = context.stroke();

    context.move_to(18.5, 11.5);
    context.line_to(22.0, 8.0);
    context.curve_to(23.9, 6.1, 27.1, 6.1, 29.0, 8.0);
    context.curve_to(30.9, 9.9, 30.9, 13.1, 29.0, 15.0);
    context.line_to(24.0, 20.0);
    context.curve_to(22.1, 21.9, 18.9, 21.9, 17.0, 20.0);
    let _ = context.stroke();

    context.move_to(11.0, 21.0);
    context.line_to(21.0, 11.0);
    let _ = context.stroke();
}

fn draw_disconnected(context: &Context) {
    context.set_line_width(3.0);
    context.move_to(5.0, 12.0);
    context.curve_to(11.5, 6.5, 20.5, 6.5, 27.0, 12.0);
    context.move_to(9.0, 17.0);
    context.curve_to(13.2, 13.5, 18.8, 13.5, 23.0, 17.0);
    context.move_to(13.0, 22.0);
    context.curve_to(14.8, 20.5, 17.2, 20.5, 19.0, 22.0);
    let _ = context.stroke();

    context.set_line_width(3.5);
    context.move_to(5.0, 5.0);
    context.line_to(27.0, 27.0);
    let _ = context.stroke();
}

fn draw_output_unavailable(context: &Context) {
    context.move_to(5.0, 13.0);
    context.line_to(11.0, 13.0);
    context.line_to(18.0, 7.0);
    context.line_to(18.0, 25.0);
    context.line_to(11.0, 19.0);
    context.line_to(5.0, 19.0);
    context.close_path();
    let _ = context.fill();

    context.set_line_width(3.0);
    context.move_to(23.0, 12.0);
    context.line_to(29.0, 20.0);
    context.move_to(29.0, 12.0);
    context.line_to(23.0, 20.0);
    let _ = context.stroke();
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_butt_caps_for_starting_and_round_caps_only_for_unavailable_symbols() {
        assert!(matches!(
            symbol_line_cap(PresentationStatusSymbol::Starting),
            LineCap::Butt
        ));

        for symbol in [
            PresentationStatusSymbol::Playing,
            PresentationStatusSymbol::Paused,
            PresentationStatusSymbol::Idle,
        ] {
            assert!(matches!(symbol_line_cap(symbol), LineCap::Butt));
        }

        for symbol in [
            PresentationStatusSymbol::PairingRequired,
            PresentationStatusSymbol::Disconnected,
            PresentationStatusSymbol::OutputUnavailable,
        ] {
            assert!(matches!(symbol_line_cap(symbol), LineCap::Round));
        }
    }
}
