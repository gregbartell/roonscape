use std::path::{Path, PathBuf};

use roonscape_renderer::ArtworkDimensions;

use crate::bounded_lru_cache::BoundedLruCache;

struct ArtworkCacheEntry {
    source: gdk_pixbuf::Pixbuf,
    scaled: Option<ScaledArtwork>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ArtworkCacheKey {
    path: PathBuf,
    revision: Option<u64>,
}

impl ArtworkCacheKey {
    pub(crate) fn new(path: PathBuf, revision: Option<u64>) -> Self {
        Self { path, revision }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

struct ScaledArtwork {
    dimensions: ArtworkDimensions,
    pixbuf: gdk_pixbuf::Pixbuf,
}

pub(crate) struct ArtworkCache {
    entries: BoundedLruCache<ArtworkCacheKey, ArtworkCacheEntry>,
}

impl ArtworkCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: BoundedLruCache::new(capacity),
        }
    }

    pub(crate) fn source(&self, key: &ArtworkCacheKey) -> Option<gdk_pixbuf::Pixbuf> {
        if let Some(source) = self
            .entries
            .access_and_promote(key, |entry| entry.source.clone())
        {
            return Some(source);
        }

        let source = gdk_pixbuf::Pixbuf::from_file(key.path()).ok()?;
        self.entries.insert(
            key.clone(),
            ArtworkCacheEntry {
                source: source.clone(),
                scaled: None,
            },
        );
        Some(source)
    }

    pub(crate) fn scaled(
        &self,
        key: &ArtworkCacheKey,
        dimensions: ArtworkDimensions,
    ) -> Option<gdk_pixbuf::Pixbuf> {
        let source = self.source(key)?;
        if let Some(scaled) = self
            .entries
            .access_and_promote(key, |entry| {
                entry
                    .scaled
                    .as_ref()
                    .filter(|scaled| scaled.dimensions == dimensions)
                    .map(|scaled| scaled.pixbuf.clone())
            })
            .flatten()
        {
            return Some(scaled);
        }

        let pixbuf = source.scale_simple(
            dimension(dimensions.width_px),
            dimension(dimensions.height_px),
            gdk_pixbuf::InterpType::Bilinear,
        )?;
        self.entries
            .access_and_promote(key, |entry| {
                entry.scaled = Some(ScaledArtwork {
                    dimensions,
                    pixbuf: pixbuf.clone(),
                });
            })
            .expect("loading artwork should leave it cached");
        Some(pixbuf)
    }
}

fn dimension(value: u32) -> i32 {
    i32::try_from(value).expect("supported artwork dimensions fit GTK's signed sizes")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use gtk::glib::object::ObjectType;

    use super::{ArtworkCache, ArtworkCacheKey};
    use roonscape_renderer::ArtworkDimensions;

    fn artwork_key(name: &str, revision: Option<u64>) -> ArtworkCacheKey {
        ArtworkCacheKey::new(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../shared/fixtures/artwork")
                .join(name),
            revision,
        )
    }

    #[test]
    fn reuses_decoded_artwork_across_rendered_presentations() {
        let artwork_cache = ArtworkCache::new(2);
        let key = artwork_key("playing.svg", None);

        let first = artwork_cache
            .source(&key)
            .expect("the Playing artwork should decode");
        let second = artwork_cache
            .source(&key)
            .expect("the cached Playing artwork should decode");

        assert_eq!(first.as_ptr(), second.as_ptr());
    }

    #[test]
    fn reuses_scaled_artwork_at_the_same_dimensions() {
        let artwork_cache = ArtworkCache::new(2);
        let key = artwork_key("playing.svg", None);
        let dimensions = ArtworkDimensions::new(120, 120);

        let first = artwork_cache
            .scaled(&key, dimensions)
            .expect("the Playing artwork should scale");
        let second = artwork_cache
            .scaled(&key, dimensions)
            .expect("the cached Playing artwork should scale");
        let resized = artwork_cache
            .scaled(&key, ArtworkDimensions::new(100, 100))
            .expect("the Playing artwork should rescale at new dimensions");

        assert_eq!(first.as_ptr(), second.as_ptr());
        assert_ne!(second.as_ptr(), resized.as_ptr());
    }

    #[test]
    fn invalidates_cached_artwork_when_its_revision_changes() {
        let artwork_cache = ArtworkCache::new(2);
        let first_key = artwork_key("playing.svg", Some(1));
        let second_key = artwork_key("playing.svg", Some(2));

        let first = artwork_cache
            .source(&first_key)
            .expect("the first artwork revision should decode");
        let second = artwork_cache
            .source(&second_key)
            .expect("the second artwork revision should decode");

        assert_ne!(first.as_ptr(), second.as_ptr());
    }

    #[test]
    fn evicts_the_least_recently_used_artwork_beyond_capacity() {
        let artwork_cache = ArtworkCache::new(2);
        let playing = artwork_key("playing.svg", None);
        let light = artwork_key("light.svg", None);
        let non_square = artwork_key("non-square.svg", None);

        let first_playing = artwork_cache
            .source(&playing)
            .expect("Playing should decode");
        let first_light = artwork_cache.source(&light).expect("light should decode");
        let reused_playing = artwork_cache
            .source(&playing)
            .expect("cached Playing should decode");
        assert_eq!(first_playing.as_ptr(), reused_playing.as_ptr());

        artwork_cache
            .source(&non_square)
            .expect("non-square artwork should decode");
        let regenerated_light = artwork_cache
            .source(&light)
            .expect("evicted light artwork should decode again");
        assert_ne!(first_light.as_ptr(), regenerated_light.as_ptr());
    }

    #[test]
    fn returns_no_source_for_undecodable_artwork() {
        let artwork_cache = ArtworkCache::new(2);
        let missing = artwork_key("does-not-exist.jpg", None);

        assert!(artwork_cache.source(&missing).is_none());
    }
}
