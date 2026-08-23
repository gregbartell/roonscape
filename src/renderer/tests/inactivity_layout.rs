#[path = "support/representative_viewports.rs"]
mod representative_viewports;

use roonscape_renderer::{
    FullFieldLayout, INACTIVE_HORIZONTAL_BOUND, INACTIVE_VERTICAL_BOUND, InactivityLayout,
    InactivityTransform, LayoutOffset, NowPlayingLayout, Viewport,
};

#[test]
fn reserves_the_complete_oled_movement_envelope_at_representative_landscape_viewports() {
    for display_viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        for offset in [
            LayoutOffset {
                x: -INACTIVE_HORIZONTAL_BOUND,
                y: -INACTIVE_VERTICAL_BOUND,
            },
            LayoutOffset {
                x: INACTIVE_HORIZONTAL_BOUND,
                y: INACTIVE_VERTICAL_BOUND,
            },
        ] {
            let inactivity = InactivityTransform {
                opacity: 0.3,
                offset,
            };
            let safe = InactivityLayout::for_viewport(display_viewport, inactivity);

            assert_eq!(
                safe.content_viewport.width_px + safe.margin_start_px + safe.margin_end_px,
                display_viewport.width_px
            );
            assert_eq!(
                safe.content_viewport.height_px + safe.margin_top_px + safe.margin_bottom_px,
                display_viewport.height_px
            );
            assert_eq!(
                safe.content_viewport.width_px,
                display_viewport.width_px - 2 * INACTIVE_HORIZONTAL_BOUND as u32
            );
            assert_eq!(
                safe.content_viewport.height_px,
                display_viewport.height_px - 2 * INACTIVE_VERTICAL_BOUND as u32
            );

            let now_playing = NowPlayingLayout::for_viewport(safe.content_viewport);
            assert_eq!(
                now_playing.outer_gutter_px * 2
                    + now_playing.artwork_column_width_px
                    + now_playing.column_gap_px
                    + now_playing.information.utility_width_px,
                safe.content_viewport.width_px,
                "Now Playing layout should fit the viewport left inside the OLED envelope"
            );
            assert_eq!(
                now_playing.information.left_viewport_x_px
                    + now_playing.information.utility_width_px
                    + now_playing.outer_gutter_px,
                safe.content_viewport.width_px,
                "the complete information rail should stay inside the OLED envelope",
            );
            assert!(
                now_playing.outer_gutter_px
                    + now_playing.artwork_field_width_px
                    + now_playing.artwork_print_plate.offset_x_px
                    < now_playing.information.left_viewport_x_px,
                "the print plate should remain clear of the information rail",
            );
            let artwork_bottom_clearance =
                (safe.content_viewport.height_px - now_playing.artwork_field_height_px) / 2;
            let shadow_bottom_extent = now_playing.artwork_shadow_offset_px
                + now_playing.artwork_shadow_blur_px.div_ceil(2);
            assert!(
                shadow_bottom_extent <= artwork_bottom_clearance,
                "the artwork shadow should remain inside the OLED-safe Now Playing layout"
            );
            assert!(
                now_playing.artwork_print_plate.offset_y_px <= artwork_bottom_clearance,
                "the print plate should remain inside the OLED-safe Now Playing layout",
            );

            let full_field = FullFieldLayout::for_viewport(safe.content_viewport);
            assert!(
                full_field.composition_width_px <= safe.content_viewport.width_px,
                "full-field copy should remain inside the OLED envelope"
            );
            assert!(
                full_field.identity_width_px
                    + full_field.identity_right_inset_px
                    + full_field.outer_gutter_px
                    <= safe.content_viewport.width_px,
                "the Output and Zone footer should remain inside the OLED envelope"
            );
        }
    }
}

#[test]
fn leaves_the_active_now_playing_viewport_uninset() {
    let active =
        InactivityLayout::for_viewport(Viewport::WINDOWED_FIXTURE, InactivityTransform::default());

    assert_eq!(active.content_viewport, Viewport::WINDOWED_FIXTURE);
    assert_eq!(
        (
            active.margin_start_px,
            active.margin_end_px,
            active.margin_top_px,
            active.margin_bottom_px,
        ),
        (0, 0, 0, 0)
    );
}
