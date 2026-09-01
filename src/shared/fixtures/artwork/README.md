# Fixture Scenario artwork

The SVG files are the canonical editable artwork sources. Fixture Scenario JSON
references the committed JPEG derivatives so Fixture Mode exercises the same
format and 1600-by-1600 fit requested from Roon in Live Mode.

The derivatives use a white JPEG background, an embedded sRGB ICC profile, and
a fixed libjpeg encoder configuration. Regenerate one from the repository root
with `SRGB_ICC` set to a standard sRGB profile:

```sh
source=src/shared/fixtures/artwork/playing.svg
rsvg-convert --format=png --width=1600 --height=1600 \
  --keep-aspect-ratio --background-color=white "$source" |
  cjpeg -quality 90 -baseline -optimize -dct int \
    -sample 2x2,1x1,1x1 -icc "$SRGB_ICC" \
    -outfile "${source%.svg}.jpg"
```
