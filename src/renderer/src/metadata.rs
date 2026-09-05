use crate::presentation::NowPlayingPresentation;
use crate::{MetadataFitting, MetadataFontSizes, NowPlayingLayout, Viewport};

const TITLE_MAXIMUM_LINES: u32 = 5;
const ARTIST_MAXIMUM_LINES: u32 = 3;
const ALBUM_MAXIMUM_LINES: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataTypography {
    EditorialSerif,
    ArtistSans,
    AlbumSans,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLineLayout {
    pub text: String,
    pub typography: MetadataTypography,
    pub font_sizes: MetadataFontSizes,
    pub maximum_lines: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataLinePlan {
    pub lines: Vec<String>,
    pub font_size_px: u32,
    pub line_height_percent: u32,
    pub height_px: u32,
    pub ellipsized: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetadataDensity {
    Normal,
    CompactCredits,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetadataGroupPlan {
    pub title: Option<MetadataLinePlan>,
    pub artist: Option<MetadataLinePlan>,
    pub album: Option<MetadataLinePlan>,
    pub density: MetadataDensity,
    pub title_to_credit_gap_px: u32,
    pub album_gap_px: u32,
    pub height_px: u32,
}

impl MetadataLineLayout {
    pub fn fitting_font_size(&self, fits: impl FnMut(u32) -> bool) -> u32 {
        self.font_sizes.fitting_font_size(fits)
    }

    fn plan_at_size(
        &self,
        available_width_px: u32,
        font_size_px: u32,
        maximum_lines: usize,
        ellipsize_when_needed: bool,
        measure_text_px: &mut impl FnMut(MetadataTypography, &str, u32) -> (u32, u32),
    ) -> Option<MetadataLinePlan> {
        let words = self.text.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return None;
        }
        let mut plan = {
            let mut measure_line_width_px =
                |text: &str, size_px: u32| measure_text_px(self.typography, text, size_px).0;
            if measure_line_width_px(&self.text, font_size_px) <= available_width_px {
                Some(line_plan(
                    vec![self.text.clone()],
                    font_size_px,
                    self.line_height_percent(font_size_px),
                    false,
                ))
            } else if let Some(lines) = balanced_word_lines(
                &words,
                maximum_lines,
                available_width_px,
                font_size_px,
                &mut measure_line_width_px,
            ) {
                Some(line_plan(
                    lines,
                    font_size_px,
                    self.line_height_percent(font_size_px),
                    false,
                ))
            } else if ellipsize_when_needed {
                Some(line_plan(
                    truncated_word_lines(
                        &words,
                        maximum_lines,
                        available_width_px,
                        font_size_px,
                        &mut measure_line_width_px,
                    ),
                    font_size_px,
                    self.line_height_percent(font_size_px),
                    true,
                ))
            } else {
                None
            }
        }?;
        let natural_line_height_px = measure_text_px(self.typography, "Ag", font_size_px).1;
        plan.height_px = rendered_line_plan_height_px(&plan, natural_line_height_px);
        Some(plan)
    }

    pub fn single_line_font_size_px(&self) -> u32 {
        self.font_sizes.preferred_px
    }

    pub fn fitting_line_plan(
        &self,
        available_width_px: u32,
        mut measure_width_px: impl FnMut(&str, u32) -> u32,
    ) -> MetadataLinePlan {
        assert!(available_width_px > 0, "metadata width must be positive");
        let words = self.text.split_whitespace().collect::<Vec<_>>();
        if words.is_empty() {
            return line_plan(
                vec![String::new()],
                self.font_sizes.preferred_px,
                self.line_height_percent(self.font_sizes.preferred_px),
                false,
            );
        }

        let single_line_font_size_px = self.single_line_font_size_px();
        if measure_width_px(&self.text, single_line_font_size_px) <= available_width_px {
            return line_plan(
                vec![self.text.clone()],
                single_line_font_size_px,
                self.line_height_percent(single_line_font_size_px),
                false,
            );
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
                return line_plan(
                    lines,
                    font_size_px,
                    self.line_height_percent(font_size_px),
                    false,
                );
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
            self.line_height_percent(self.font_sizes.minimum_px),
            true,
        )
    }

    fn line_height_percent(&self, font_size_px: u32) -> u32 {
        match self.typography {
            MetadataTypography::EditorialSerif if font_size_px == self.font_sizes.preferred_px => {
                94
            }
            MetadataTypography::EditorialSerif if font_size_px == self.font_sizes.reduced_px => 96,
            MetadataTypography::EditorialSerif => 98,
            MetadataTypography::ArtistSans | MetadataTypography::AlbumSans
                if font_size_px == self.font_sizes.preferred_px =>
            {
                125
            }
            MetadataTypography::ArtistSans | MetadataTypography::AlbumSans => 118,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct MetadataLayout {
    pub title: Option<MetadataLineLayout>,
    pub artist: Option<MetadataLineLayout>,
    pub album: Option<MetadataLineLayout>,
}

impl MetadataLayout {
    pub fn fitting_group_plan(
        &self,
        available_width_px: u32,
        available_height_px: u32,
        fitting: MetadataFitting,
        mut measure_text_px: impl FnMut(MetadataTypography, &str, u32) -> (u32, u32),
    ) -> MetadataGroupPlan {
        assert!(available_width_px > 0, "metadata width must be positive");
        assert!(available_height_px > 0, "metadata height must be positive");

        let title_sizes =
            self.title
                .as_ref()
                .map(|title| title.font_sizes)
                .unwrap_or(MetadataFontSizes {
                    preferred_px: 0,
                    reduced_px: 0,
                    minimum_px: 0,
                });
        for title_size_px in [
            title_sizes.preferred_px,
            title_sizes.reduced_px,
            title_sizes.minimum_px,
        ] {
            let plan = self.group_plan(
                available_width_px,
                MetadataDensity::Normal,
                title_size_px,
                self.title
                    .as_ref()
                    .map_or(0, |line| line.maximum_lines as usize),
                self.artist
                    .as_ref()
                    .map_or(0, |line| line.maximum_lines as usize),
                self.album
                    .as_ref()
                    .map_or(0, |line| line.maximum_lines as usize),
                fitting,
                title_size_px == title_sizes.minimum_px,
                &mut measure_text_px,
            );
            if let Some(plan) = plan.filter(|plan| plan.height_px <= available_height_px) {
                return plan;
            }
        }

        let title_range = line_count_range(self.title.as_ref());
        let artist_range = line_count_range(self.artist.as_ref());
        let album_range = line_count_range(self.album.as_ref());
        let mut smallest_plan = None;
        for title_lines in title_range.rev() {
            for artist_lines in artist_range.clone().rev() {
                for album_lines in album_range.clone().rev() {
                    let plan = self
                        .group_plan(
                            available_width_px,
                            MetadataDensity::CompactCredits,
                            title_sizes.minimum_px,
                            title_lines,
                            artist_lines,
                            album_lines,
                            fitting,
                            true,
                            &mut measure_text_px,
                        )
                        .expect("compact fitting always ellipsizes bounded metadata");
                    if plan.height_px <= available_height_px {
                        return plan;
                    }
                    smallest_plan = Some(plan);
                }
            }
        }
        smallest_plan.expect("a presentation has at least one metadata line")
    }

    #[allow(clippy::too_many_arguments)]
    fn group_plan(
        &self,
        available_width_px: u32,
        density: MetadataDensity,
        title_size_px: u32,
        title_lines: usize,
        artist_lines: usize,
        album_lines: usize,
        fitting: MetadataFitting,
        ellipsize_title: bool,
        measure_text_px: &mut impl FnMut(MetadataTypography, &str, u32) -> (u32, u32),
    ) -> Option<MetadataGroupPlan> {
        let compact = density == MetadataDensity::CompactCredits;
        let title = match self.title.as_ref() {
            Some(line) => Some(line.plan_at_size(
                available_width_px,
                title_size_px,
                title_lines,
                ellipsize_title,
                measure_text_px,
            )?),
            None => None,
        };
        let artist = self.artist.as_ref().map(|line| {
            let size = if compact {
                line.font_sizes.minimum_px
            } else {
                line.font_sizes.preferred_px
            };
            line.plan_at_size(
                available_width_px,
                size,
                artist_lines,
                true,
                measure_text_px,
            )
            .expect("present Artist produces a plan")
        });
        let album = self.album.as_ref().map(|line| {
            let size = if compact {
                line.font_sizes.minimum_px
            } else {
                line.font_sizes.preferred_px
            };
            line.plan_at_size(available_width_px, size, album_lines, true, measure_text_px)
                .expect("present Album produces a plan")
        });
        let title_to_credit_gap_px = if compact {
            fitting.compact_title_to_credit_gap_px
        } else {
            fitting.normal_title_to_credit_gap_px
        };
        let album_gap_px = if compact {
            fitting.compact_album_gap_px
        } else {
            fitting.normal_album_gap_px
        };
        let has_credit = artist.is_some() || album.is_some();
        let height_px = [&title, &artist, &album]
            .into_iter()
            .flatten()
            .map(|plan| plan.height_px)
            .sum::<u32>()
            + u32::from(title.is_some() && has_credit) * title_to_credit_gap_px
            + u32::from(artist.is_some() && album.is_some()) * album_gap_px;
        Some(MetadataGroupPlan {
            title,
            artist,
            album,
            density,
            title_to_credit_gap_px,
            album_gap_px,
            height_px,
        })
    }
}

fn line_count_range(line: Option<&MetadataLineLayout>) -> std::ops::RangeInclusive<usize> {
    line.map_or(0..=0, |line| 1..=line.maximum_lines as usize)
}

fn rendered_line_plan_height_px(plan: &MetadataLinePlan, natural_line_height_px: u32) -> u32 {
    rendered_text_height_px(
        plan.lines.len(),
        natural_line_height_px,
        plan.line_height_percent,
    )
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
                MetadataTypography::ArtistSans,
                typography.artist,
                ARTIST_MAXIMUM_LINES,
            )
        }),
        album: presentation.album.as_deref().map(|text| {
            line_layout(
                text,
                MetadataTypography::AlbumSans,
                typography.album,
                ALBUM_MAXIMUM_LINES,
            )
        }),
    }
}

fn line_plan(
    lines: Vec<String>,
    font_size_px: u32,
    line_height_percent: u32,
    ellipsized: bool,
) -> MetadataLinePlan {
    let height_px = rendered_text_height_px(lines.len(), font_size_px, line_height_percent);
    MetadataLinePlan {
        lines,
        font_size_px,
        line_height_percent,
        height_px,
        ellipsized,
    }
}

fn rendered_text_height_px(line_count: usize, font_size_px: u32, line_height_percent: u32) -> u32 {
    let additional_lines = line_count.saturating_sub(1) as u64;
    let additional_height_px =
        u64::from(font_size_px) * u64::from(line_height_percent) * additional_lines;
    font_size_px + additional_height_px.div_ceil(100) as u32
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
    }
}
