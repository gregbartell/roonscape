use crate::{FullFieldLayout, NowPlayingLayout, PresentationPalette, Rgb, TypographyPair};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PresentationStyleLayer {
    Current,
    Outgoing,
}

impl PresentationStyleLayer {
    pub const fn class_name(self) -> &'static str {
        match self {
            Self::Current => "presentation-current",
            Self::Outgoing => "presentation-outgoing",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DiagnosticsStyle {
    pub layer: PresentationStyleLayer,
    pub field: Rgb,
    pub text: Rgb,
    pub border: Rgb,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PresentationTransitionStyles {
    current: PresentationPalette,
    outgoing: Option<PresentationPalette>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TypographyStyles {
    typography: TypographyPair,
}

impl TypographyStyles {
    pub const fn new(typography: TypographyPair) -> Self {
        Self { typography }
    }

    pub fn to_css(self) -> String {
        format!(
            ".editorial-text, .full-field-heading {{ font-family: \"{}\", serif; }}\n\
             .utility-text, .status-label, .identity-label, .identity-name, .time, .full-field-explanation, .diagnostics {{ font-family: \"{}\", sans-serif; }}\n",
            self.typography.editorial_family(),
            self.typography.utility_family(),
        )
    }
}

impl PresentationTransitionStyles {
    pub const fn new(current: PresentationPalette, outgoing: Option<PresentationPalette>) -> Self {
        Self { current, outgoing }
    }

    pub fn diagnostics(self) -> Vec<DiagnosticsStyle> {
        self.layers()
            .map(|(layer, palette)| diagnostics_style(layer, palette))
            .collect()
    }

    pub fn to_css(self, layout: &NowPlayingLayout, full_field_layout: &FullFieldLayout) -> String {
        let mut styles = String::new();
        for (layer, palette) in self.layers() {
            styles.push_str(&presentation_palette_styles(
                layer,
                palette,
                layout,
                full_field_layout,
            ));
        }
        for diagnostics in self.diagnostics() {
            styles.push_str(&diagnostics_palette_styles(diagnostics));
        }
        styles
    }

    fn layers(self) -> impl Iterator<Item = (PresentationStyleLayer, PresentationPalette)> {
        std::iter::once((PresentationStyleLayer::Current, self.current)).chain(
            self.outgoing
                .map(|palette| (PresentationStyleLayer::Outgoing, palette)),
        )
    }
}

fn diagnostics_style(
    layer: PresentationStyleLayer,
    palette: PresentationPalette,
) -> DiagnosticsStyle {
    DiagnosticsStyle {
        layer,
        field: palette.diagnostics_field,
        text: palette.diagnostics_text,
        border: palette.diagnostics_border,
    }
}

fn presentation_palette_styles(
    layer: PresentationStyleLayer,
    palette: PresentationPalette,
    layout: &NowPlayingLayout,
    full_field_layout: &FullFieldLayout,
) -> String {
    let class_name = layer.class_name();
    let background = palette.background.to_hex();
    let artwork_field = palette.artwork_field.to_hex();
    let metadata_field = palette.metadata_field.to_hex();
    let primary_text = palette.primary_text.to_hex();
    let secondary_text = palette.secondary_text.to_hex();
    let muted_text = palette.muted_text.to_hex();
    let accent = palette.accent.to_hex();
    let muted_accent = palette.status_muted_accent.to_hex();
    let progress_track = palette.progress_track.to_hex();
    let progress_fill = palette.progress_fill.to_hex();
    let shadow_offset = layout.artwork_shadow_offset_px;
    let shadow_blur = layout.artwork_shadow_blur_px;
    let accent_width = full_field_layout.accent_width_px;
    format!(
        ".{class_name} {{ background-color: {background}; color: {primary_text}; }}\n\
         .{class_name}.now-playing {{ background-image: linear-gradient(118deg, {artwork_field} 0%, {background} 62%, {metadata_field} 100%); }}\n\
         .{class_name} .artwork-frame {{ box-shadow: 0 {shadow_offset}px {shadow_blur}px alpha({background}, 0.72); }}\n\
         .{class_name} .artwork {{ border-color: alpha({primary_text}, 0.16); background-color: {artwork_field}; }}\n\
         .{class_name} .artwork-missing {{ border-color: alpha({muted_text}, 0.22); background-image: linear-gradient(142deg, alpha({muted_text}, 0.09), {artwork_field} 52%, {background}); box-shadow: inset 0 0 0 24px alpha({background}, 0.16); }}\n\
         .{class_name}.full-field .full-copy {{ border-left: {accent_width}px solid {accent}; }}\n\
         .{class_name} .status-full, .{class_name} .status-glow {{ color: {accent}; }}\n\
         .{class_name} .status-muted {{ color: {muted_accent}; }}\n\
         .{class_name} .status-glow .status-symbol-container {{ box-shadow: 0 0 34px alpha({accent}, 0.72); }}\n\
         .{class_name} .title, .{class_name} .full-field-heading {{ color: {primary_text}; }}\n\
         .{class_name} .artist, .{class_name} .album, .{class_name} .time, .{class_name} .identity-name {{ color: {secondary_text}; }}\n\
         .{class_name} .full-field-explanation {{ color: {muted_text}; }}\n\
         .{class_name} .identity-label {{ color: {muted_text}; }}\n\
         .{class_name} progressbar trough {{ background-color: {progress_track}; }}\n\
         .{class_name} progressbar progress {{ background-color: {progress_fill}; }}\n"
    )
}

fn diagnostics_palette_styles(style: DiagnosticsStyle) -> String {
    let class_name = style.layer.class_name();
    let field = style.field.to_hex();
    let text = style.text.to_hex();
    let border = style.border.to_hex();
    format!(
        ".{class_name} .diagnostics {{ color: {text}; background-color: {field}; border-color: {border}; }}\n"
    )
}
