use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use roonscape_renderer::{
    NowPlayingGradient, NowPlayingGradientCacheKey, PresentationPalette, Viewport,
};

use crate::bounded_lru_cache::BoundedLruCache;

#[derive(Clone, Copy, PartialEq, Eq)]
struct NowPlayingGradientRasterKey {
    palette: PresentationPalette,
    viewport: NowPlayingGradientCacheKey,
}

pub(crate) struct PreparedNowPlayingGradient {
    key: NowPlayingGradientRasterKey,
    rgba8: Arc<[u8]>,
}

pub(crate) struct RenderedNowPlayingGradient {
    pub(crate) logical_viewport: Viewport,
    pub(crate) physical_viewport: Viewport,
    pub(crate) stride_bytes: usize,
    pub(crate) rgba8: Arc<[u8]>,
}

pub(crate) struct CachedNowPlayingGradient {
    palette: PresentationPalette,
    cache: Rc<NowPlayingGradientCache>,
    rendered_viewport: Cell<Option<NowPlayingGradientCacheKey>>,
}

pub(crate) struct NowPlayingGradientCache {
    rasters: BoundedLruCache<NowPlayingGradientRasterKey, Arc<[u8]>>,
}

impl NowPlayingGradientCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            rasters: BoundedLruCache::new(capacity),
        }
    }

    pub(crate) fn raster(
        &self,
        palette: PresentationPalette,
        viewport: NowPlayingGradientCacheKey,
    ) -> Arc<[u8]> {
        let key = NowPlayingGradientRasterKey { palette, viewport };
        if let Some(raster) = self.cached_raster(&key) {
            return raster;
        }

        let raster = Self::generate_raster(palette, viewport);
        self.rasters.insert(key, Arc::clone(&raster));
        raster
    }

    pub(crate) fn prepare_while<T>(
        &self,
        palette: PresentationPalette,
        logical_viewport: Viewport,
        scale_factor: u32,
        independent_work: impl FnOnce() -> T,
    ) -> (T, PreparedNowPlayingGradient) {
        let viewport = NowPlayingGradientCacheKey::new(logical_viewport, scale_factor);
        let key = NowPlayingGradientRasterKey { palette, viewport };
        if let Some(raster) = self.cached_raster(&key) {
            return (
                independent_work(),
                PreparedNowPlayingGradient { key, rgba8: raster },
            );
        }

        std::thread::scope(|scope| {
            let generation = scope.spawn(move || Self::generate_raster(palette, viewport));
            let result = independent_work();
            let raster = generation
                .join()
                .expect("now-playing gradient generation should not panic");
            self.rasters.insert(key, Arc::clone(&raster));
            (result, PreparedNowPlayingGradient { key, rgba8: raster })
        })
    }

    pub(crate) fn gradient(
        self: &Rc<Self>,
        palette: PresentationPalette,
    ) -> CachedNowPlayingGradient {
        CachedNowPlayingGradient {
            palette,
            cache: Rc::clone(self),
            rendered_viewport: Cell::new(None),
        }
    }

    fn cached_raster(&self, key: &NowPlayingGradientRasterKey) -> Option<Arc<[u8]>> {
        self.rasters
            .access_and_promote(key, |raster| Arc::clone(raster))
    }

    fn generate_raster(
        palette: PresentationPalette,
        viewport: NowPlayingGradientCacheKey,
    ) -> Arc<[u8]> {
        Arc::from(NowPlayingGradient::new(palette, viewport.physical_viewport()).into_rgba8())
    }
}

impl CachedNowPlayingGradient {
    pub(crate) fn render(
        &self,
        logical_viewport: Viewport,
        scale_factor: u32,
    ) -> Option<RenderedNowPlayingGradient> {
        let viewport = NowPlayingGradientCacheKey::new(logical_viewport, scale_factor);
        let rgba8 = self.cache.raster(self.palette, viewport);
        self.render_for(viewport, rgba8)
    }

    pub(crate) fn refresh(&self, scale_factor: u32) -> Option<RenderedNowPlayingGradient> {
        let logical_viewport = self.rendered_viewport.get()?.logical_viewport();
        self.render(logical_viewport, scale_factor)
    }

    pub(crate) fn render_prepared(
        &self,
        prepared: PreparedNowPlayingGradient,
    ) -> Option<RenderedNowPlayingGradient> {
        debug_assert_eq!(prepared.key.palette, self.palette);
        self.render_for(prepared.key.viewport, prepared.rgba8)
    }

    fn render_for(
        &self,
        viewport: NowPlayingGradientCacheKey,
        rgba8: Arc<[u8]>,
    ) -> Option<RenderedNowPlayingGradient> {
        if self.rendered_viewport.get() == Some(viewport) {
            return None;
        }

        let logical_viewport = viewport.logical_viewport();
        let physical_viewport = viewport.physical_viewport();
        self.rendered_viewport.set(Some(viewport));
        Some(RenderedNowPlayingGradient {
            logical_viewport,
            physical_viewport,
            stride_bytes: physical_viewport.width_px as usize * 4,
            rgba8,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::rc::Rc;
    use std::sync::Arc;

    use super::NowPlayingGradientCache;
    use roonscape_renderer::{NowPlayingGradientCacheKey, PresentationPalette, Rgb, Viewport};

    #[test]
    fn reuses_a_gradient_raster_for_the_same_palette_and_viewport() {
        let cache = NowPlayingGradientCache::new(2);
        let palette = PresentationPalette::fallback();
        let key = NowPlayingGradientCacheKey::new(Viewport::new(16, 9), 1);

        let first = cache.raster(palette, key);
        let second = cache.raster(palette, key);

        assert!(Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn prepares_and_reuses_a_gradient_raster() {
        let cache = Rc::new(NowPlayingGradientCache::new(2));
        let palette = PresentationPalette::fallback();
        let viewport = Viewport::new(16, 9);

        let (_, prepared) = cache.prepare_while(palette, viewport, 1, || {});
        let gradient = cache.gradient(palette);
        let rendered = gradient
            .render_prepared(prepared)
            .expect("the prepared raster should render once");
        let repeated = gradient.render(viewport, 1);

        assert_eq!(rendered.logical_viewport, viewport);
        assert!(repeated.is_none());
    }

    #[test]
    fn renders_physical_gradient_metadata_and_suppresses_cache_hits() {
        let cache = Rc::new(NowPlayingGradientCache::new(2));
        let gradient = cache.gradient(PresentationPalette::fallback());
        let viewport = Viewport::new(16, 9);

        let rendered = gradient
            .render(viewport, 2)
            .expect("the first viewport should render");
        assert_eq!(rendered.logical_viewport, viewport);
        assert_eq!(rendered.physical_viewport, Viewport::new(32, 18));
        assert_eq!(rendered.stride_bytes, 32 * 4);
        assert_eq!(rendered.rgba8.len(), 32 * 18 * 4);
        assert!(gradient.render(viewport, 2).is_none());

        let rescaled = gradient
            .refresh(3)
            .expect("a display-scale change should rerender");
        assert_eq!(rescaled.physical_viewport, Viewport::new(48, 27));
    }

    #[test]
    fn keeps_gradient_rasters_separate_across_palettes() {
        let cache = NowPlayingGradientCache::new(2);
        let key = NowPlayingGradientCacheKey::new(Viewport::new(16, 9), 1);
        let first_palette = PresentationPalette::fallback();
        let mut second_palette = first_palette;
        second_palette.background = Rgb {
            red: 1,
            green: 2,
            blue: 3,
        };

        let first = cache.raster(first_palette, key);
        let second = cache.raster(second_palette, key);

        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn keeps_gradient_rasters_separate_across_display_scales() {
        let cache = NowPlayingGradientCache::new(2);
        let palette = PresentationPalette::fallback();
        let viewport = Viewport::new(16, 9);

        let first = cache.raster(palette, NowPlayingGradientCacheKey::new(viewport, 1));
        let second = cache.raster(palette, NowPlayingGradientCacheKey::new(viewport, 2));

        assert!(!Arc::ptr_eq(&first, &second));
    }

    #[test]
    fn evicts_the_least_recently_used_raster_beyond_capacity() {
        let cache = NowPlayingGradientCache::new(2);
        let palette = PresentationPalette::fallback();
        let key = |width, height| NowPlayingGradientCacheKey::new(Viewport::new(width, height), 1);

        let first = cache.raster(palette, key(16, 9));
        let second = cache.raster(palette, key(20, 12));
        let reused_first = cache.raster(palette, key(16, 9));
        assert!(Arc::ptr_eq(&first, &reused_first));

        let _third = cache.raster(palette, key(24, 14));
        let regenerated_second = cache.raster(palette, key(20, 12));
        assert!(!Arc::ptr_eq(&second, &regenerated_second));
    }
}
