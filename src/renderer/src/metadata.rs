use crate::presentation::NowPlayingPresentation;
use crate::{MetadataFontSizes, NowPlayingLayout, TextOverflow, Viewport};

const TITLE_MAXIMUM_LINES: u32 = 5;
const ARTIST_MAXIMUM_LINES: u32 = 3;
const ALBUM_MAXIMUM_LINES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataTypography {
    EditorialSerif,
    UtilitySans,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLineLayout {
    pub text: String,
    pub typography: MetadataTypography,
    pub font_sizes: MetadataFontSizes,
    pub maximum_lines: u32,
    pub overflow: TextOverflow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLinePlan {
    pub lines: Vec<String>,
    pub font_size_px: u32,
    pub line_height_percent: u32,
    pub top_padding_px: u32,
    pub ellipsized: bool,
}

impl MetadataLineLayout {
    pub fn fitting_font_size(&self, fits: impl FnMut(u32) -> bool) -> u32 {
        self.font_sizes.fitting_font_size(fits)
    }

    pub fn single_line_font_size_px(&self) -> u32 {
        if self.typography == MetadataTypography::EditorialSerif {
            rounded_fraction(self.font_sizes.preferred_px, 112, 100)
        } else {
            self.font_sizes.preferred_px
        }
    }

    pub fn fitting_line_plan(
        &self,
        available_width_px: u32,
        mut measure_width_px: impl FnMut(&str, u32) -> u32,
    ) -> MetadataLinePlan {
        assert!(available_width_px > 0, "metadata width must be positive");
        let words = self.text.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return line_plan(vec![String::new()], self.font_sizes.preferred_px, false);
        }

        let single_line_font_size_px = self.single_line_font_size_px();
        if measure_width_px(&self.text, single_line_font_size_px) <= available_width_px {
            return line_plan(vec![self.text.clone()], single_line_font_size_px, false);
        }

        let mut previous_font_size_px = None;
        for font_size_px in [
            self.font_sizes.preferred_px,
            self.font_sizes.reduced_px,
            self.font_sizes.minimum_px,
        ] {
            if previous_font_size_px == Some(font_size_px) {
                continue;
            }
            previous_font_size_px = Some(font_size_px);
            if let Some(lines) = balanced_word_lines(
                &words,
                self.maximum_lines as usize,
                available_width_px,
                font_size_px,
                &mut measure_width_px,
            ) {
                return line_plan(lines, font_size_px, false);
            }
        }

        line_plan(
            truncated_word_lines(
                &words,
                self.maximum_lines as usize,
                available_width_px,
                self.font_sizes.minimum_px,
                &mut measure_width_px,
            ),
            self.font_sizes.minimum_px,
            true,
        )
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MetadataLayout {
    pub title: Option<MetadataLineLayout>,
    pub artist: Option<MetadataLineLayout>,
    pub album: Option<MetadataLineLayout>,
}

pub fn metadata_layout(
    presentation: &NowPlayingPresentation,
    viewport: Viewport,
) -> MetadataLayout {
    let typography = NowPlayingLayout::for_viewport(viewport).typography;
    MetadataLayout {
        title: presentation.title.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::EditorialSerif,
                typography.title,
                TITLE_MAXIMUM_LINES,
            )
        }),
        artist: presentation.artist.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::UtilitySans,
                typography.artist,
                ARTIST_MAXIMUM_LINES,
            )
        }),
        album: presentation.album.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::UtilitySans,
                typography.album,
                ALBUM_MAXIMUM_LINES,
            )
        }),
    }
}

fn rounded_fraction(value: u32, numerator: u32, denominator: u32) -> u32 {
    let scaled = u64::from(value) * u64::from(numerator);
    ((scaled + u64::from(denominator) / 2) / u64::from(denominator)) as u32
}

fn line_plan(lines: Vec<String>, font_size_px: u32, ellipsized: bool) -> MetadataLinePlan {
    let line_height_percent = match lines.len() {
        0 | 1 => 100,
        2 => 94,
        _ => 98,
    };
    let top_padding_px = match lines.len() {
        3 => font_size_px * 2,
        4.. => font_size_px * 3,
        _ => 0,
    };
    MetadataLinePlan {
        lines,
        font_size_px,
        line_height_percent,
        top_padding_px,
        ellipsized,
    }
}

fn balanced_word_lines(
    words: &[&str],
    maximum_lines: usize,
    available_width_px: u32,
    font_size_px: u32,
    measure_width_px: &mut impl FnMut(&str, u32) -> u32,
) -> Option<Vec<String>> {
    let (line_widths, unbroken_width_px) = line_widths(words, font_size_px, measure_width_px);
    for line_count in 1..=maximum_lines.min(words.len()) {
        if let Some(breaks) = balanced_breaks(
            &line_widths,
            line_count,
            available_width_px,
            unbroken_width_px,
        ) {
            return Some(lines_from_breaks(words, &breaks));
        }
    }
    None
}

fn line_widths(
    words: &[&str],
    font_size_px: u32,
    measure_width_px: &mut impl FnMut(&str, u32) -> u32,
) -> (Vec<Vec<u32>>, u32) {
    let space_width_px = measure_width_px(" ", font_size_px);
    let word_widths = words
        .iter()
        .map(|word| measure_width_px(word, font_size_px))
        .collect::<Vec<_>>();
    let mut prefix_widths = Vec::with_capacity(words.len() + 1);
    prefix_widths.push(0_u64);
    for width_px in word_widths {
        prefix_widths.push(prefix_widths.last().copied().unwrap_or(0) + u64::from(width_px));
    }
    let line_widths = (0..words.len())
        .map(|start| {
            (0..=words.len())
                .map(|end| {
                    if end <= start {
                        0
                    } else {
                        let words_width_px = prefix_widths[end] - prefix_widths[start];
                        let spaces_width_px = u64::from(space_width_px) * (end - start - 1) as u64;
                        (words_width_px + spaces_width_px) as u32
                    }
                })
                .collect()
        })
        .collect();
    let unbroken_width_px = prefix_widths.last().copied().unwrap_or(0)
        + u64::from(space_width_px) * words.len().saturating_sub(1) as u64;
    (line_widths, unbroken_width_px as u32)
}

fn balanced_breaks(
    line_widths: &[Vec<u32>],
    line_count: usize,
    available_width_px: u32,
    unbroken_width_px: u32,
) -> Option<Vec<usize>> {
    let word_count = line_widths.len();
    let target_width_px = u64::from(unbroken_width_px).div_ceil(line_count as u64);
    let mut plans = vec![vec![None::<(u128, Vec<usize>)>; word_count + 1]; line_count + 1];
    plans[0][0] = Some((0, Vec::new()));

    for used_lines in 0..line_count {
        for start in used_lines..word_count {
            let Some((score, breaks)) = plans[used_lines][start].clone() else {
                continue;
            };
            let remaining_lines = line_count - used_lines - 1;
            let last_end = word_count - remaining_lines;
            for end in (start + 1)..=last_end {
                let width_px = line_widths[start][end];
                if width_px > available_width_px {
                    break;
                }
                let shortfall_px = target_width_px.abs_diff(u64::from(width_px));
                let candidate_score = score + u128::from(shortfall_px).pow(2);
                let entry = &mut plans[used_lines + 1][end];
                if entry
                    .as_ref()
                    .is_none_or(|(best_score, _)| candidate_score < *best_score)
                {
                    let mut candidate_breaks = breaks.clone();
                    candidate_breaks.push(end);
                    *entry = Some((candidate_score, candidate_breaks));
                }
            }
        }
    }

    plans[line_count][word_count]
        .take()
        .map(|(_, breaks)| breaks)
}

fn lines_from_breaks(words: &[&str], breaks: &[usize]) -> Vec<String> {
    let mut start = 0;
    breaks
        .iter()
        .map(|end| {
            let line = words[start..*end].join(" ");
            start = *end;
            line
        })
        .collect()
}

fn truncated_word_lines(
    words: &[&str],
    maximum_lines: usize,
    available_width_px: u32,
    font_size_px: u32,
    measure_width_px: &mut impl FnMut(&str, u32) -> u32,
) -> Vec<String> {
    let maximum_lines = maximum_lines.max(1).min(words.len());
    let mut lines = Vec::with_capacity(maximum_lines);
    let mut start = 0;
    for line_index in 0..maximum_lines {
        if line_index + 1 == maximum_lines {
            lines.push(ellipsized_line(
                &words[start..],
                available_width_px,
                font_size_px,
                measure_width_px,
            ));
            break;
        }
        let remaining_lines = maximum_lines - line_index - 1;
        let last_end = words.len() - remaining_lines;
        let mut end = start + 1;
        while end < last_end
            && measure_width_px(&words[start..=end].join(" "), font_size_px) <= available_width_px
        {
            end += 1;
        }
        lines.push(words[start..end].join(" "));
        start = end;
    }
    lines
}

fn ellipsized_line(
    words: &[&str],
    available_width_px: u32,
    font_size_px: u32,
    measure_width_px: &mut impl FnMut(&str, u32) -> u32,
) -> String {
    let mut fitted = "…".to_owned();
    for end in 1..=words.len() {
        let candidate = format!("{}…", words[..end].join(" "));
        if measure_width_px(&candidate, font_size_px) > available_width_px {
            break;
        }
        fitted = candidate;
    }
    fitted
}

fn line_layout(
    text: &str,
    typography: MetadataTypography,
    sizes: MetadataFontSizes,
    maximum_lines: u32,
) -> MetadataLineLayout {
    MetadataLineLayout {
        text: text.to_owned(),
        typography,
        font_sizes: sizes,
        maximum_lines,
        overflow: TextOverflow::EllipsizeEnd,
    }
}
