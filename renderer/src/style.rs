use crate::{FullFieldLayout, GallerySplitLayout, PresentationPalette};

pub fn presentation_palette_styles(
    class_name: &str,
    palette: PresentationPalette,
    layout: &GallerySplitLayout,
    full_field_layout: &FullFieldLayout,
) -> String {
    let background = palette.background.to_hex();
    let artwork_field = palette.artwork_field.to_hex();
    let metadata_field = palette.metadata_field.to_hex();
    let primary_text = palette.primary_text.to_hex();
    let secondary_text = palette.secondary_text.to_hex();
    let muted_text = palette.muted_text.to_hex();
    let accent = palette.accent.to_hex();
    let progress_track = palette.progress_track.to_hex();
    let progress_fill = palette.progress_fill.to_hex();
    let diagnostics_field = palette.diagnostics_field.to_hex();
    let diagnostics_text = palette.diagnostics_text.to_hex();
    let diagnostics_border = palette.diagnostics_border.to_hex();
    let shadow_offset = layout.artwork_shadow_offset_px;
    let shadow_blur = layout.artwork_shadow_blur_px;
    let accent_width = full_field_layout.accent_width_px;
    format!(
        ".{class_name} {{ background-color: {background}; color: {primary_text}; }}\n\
         .{class_name}.gallery-split {{ background-image: linear-gradient(118deg, {artwork_field} 0%, {background} 62%, {metadata_field} 100%); }}\n\
         .{class_name} .artwork-frame {{ box-shadow: 0 {shadow_offset}px {shadow_blur}px alpha({background}, 0.72); }}\n\
         .{class_name} .artwork {{ border-color: alpha({primary_text}, 0.16); background-color: {artwork_field}; }}\n\
         .{class_name} .artwork-missing {{ border-color: alpha({muted_text}, 0.22); background-image: linear-gradient(142deg, alpha({muted_text}, 0.09), {artwork_field} 52%, {background}); box-shadow: inset 0 0 0 24px alpha({background}, 0.16); }}\n\
         .{class_name}.full-field .full-copy {{ border-left: {accent_width}px solid {accent}; }}\n\
         .{class_name} .playback-state {{ color: {accent}; }}\n\
         .{class_name} .state-dot {{ background-color: {accent}; box-shadow: 0 0 18px alpha({accent}, 0.72); }}\n\
         .{class_name} .title, .{class_name} .full-field-heading {{ color: {primary_text}; }}\n\
         .{class_name} .artist, .{class_name} .album, .{class_name} .time, .{class_name} .identity-name {{ color: {secondary_text}; }}\n\
         .{class_name} .full-field-explanation {{ color: {muted_text}; }}\n\
         .{class_name} .identity-label {{ color: {muted_text}; }}\n\
         .{class_name} progressbar trough {{ background-color: {progress_track}; }}\n\
         .{class_name} progressbar progress {{ background-color: {progress_fill}; }}\n\
         .{class_name} .diagnostics {{ color: {diagnostics_text}; background-color: {diagnostics_field}; border-color: {diagnostics_border}; }}\n"
    )
}
