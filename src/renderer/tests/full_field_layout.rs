#[path = "support/representative_viewports.rs"]
mod representative_viewports;

use roonscape_renderer::{
    FullFieldLayout, IdentityLineLayout, IdentityPlacement, NowPlayingLayout, TextOverflow,
};

#[test]
fn bounds_full_field_states_and_identities_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let now_playing = NowPlayingLayout::for_viewport(viewport);
        let maximum_copy_height = layout.presentation_status.symbol_size_px
            + layout.status_spacing_px
            + layout.heading_px * 3
            + layout.explanation_spacing_px
            + layout.explanation_px * 3;

        assert!(
            layout.copy_width_px + layout.outer_gutter_px * 2 <= viewport.width_px,
            "full-field copy should remain horizontally bounded at {viewport:?}"
        );
        assert!(
            maximum_copy_height + layout.outer_gutter_px * 2 <= viewport.height_px,
            "full-field copy should remain vertically bounded at {viewport:?}"
        );
        assert!(layout.accent_padding_px < layout.copy_width_px);
        assert_eq!(layout.identity_placement, IdentityPlacement::BottomRight);
        assert_eq!(
            layout.identity_line,
            IdentityLineLayout {
                maximum_lines: 1,
                overflow: TextOverflow::EllipsizeEnd,
            }
        );
        assert_eq!(
            layout.identity_width_px,
            now_playing.metadata_column_width_px - now_playing.metadata_right_inset_px,
            "full-field states should share the stable Output and Zone row at {viewport:?}"
        );
        assert!(
            layout.identity_width_px + layout.identity_right_inset_px + layout.outer_gutter_px
                <= viewport.width_px,
            "the identity row should remain inside the field at {viewport:?}"
        );
    }
}

#[test]
fn reserves_a_complete_long_heading_word_at_representative_landscape_viewports() {
    for viewport in representative_viewports::REPRESENTATIVE_VIEWPORTS {
        let layout = FullFieldLayout::for_viewport(viewport);
        let heading_width = layout
            .copy_width_px
            .saturating_sub(layout.accent_padding_px);

        assert!(
            heading_width * 5 >= layout.heading_px * 27,
            "full-field copy should reserve 5.4 em for a complete heading word at {viewport:?}"
        );
    }
}
