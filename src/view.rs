use eframe::egui::{Pos2, Rect, Vec2};

#[derive(Default)]
pub struct View {
    zoom: Option<f32>,
    offset: Vec2,
}

impl View {
    pub fn fit(&mut self) {
        *self = Self::default();
    }

    pub fn actual_size(&mut self) {
        self.zoom = Some(1.0);
        self.offset = Vec2::ZERO;
    }

    pub fn is_fit(&self) -> bool {
        self.zoom.is_none()
    }

    pub fn scale(&self, bounds: Rect, source: Vec2, pixels_per_point: f32) -> f32 {
        self.zoom
            .unwrap_or_else(|| fit_scale(bounds, source, pixels_per_point))
    }

    pub fn rect(&mut self, bounds: Rect, source: Vec2, pixels_per_point: f32) -> Rect {
        let size = source * (self.scale(bounds, source, pixels_per_point) / pixels_per_point);
        let limit = ((size - bounds.size()) * 0.5).max(Vec2::ZERO);
        self.offset = self.offset.clamp(-limit, limit);
        Rect::from_center_size(bounds.center() + self.offset, size)
    }

    pub fn zoom_at(
        &mut self,
        factor: f32,
        anchor: Pos2,
        bounds: Rect,
        source: Vec2,
        pixels_per_point: f32,
    ) {
        self.rect(bounds, source, pixels_per_point);
        let old = self.scale(bounds, source, pixels_per_point);
        let minimum = fit_scale(bounds, source, pixels_per_point).min(0.01);
        let next = (old * factor).clamp(minimum, 16.0);
        let anchor = anchor - bounds.center();
        self.offset = anchor + (self.offset - anchor) * (next / old);
        self.zoom = Some(next);
        self.rect(bounds, source, pixels_per_point);
    }

    pub fn pan(&mut self, delta: Vec2, bounds: Rect, source: Vec2, pixels_per_point: f32) {
        self.offset += delta;
        self.rect(bounds, source, pixels_per_point);
    }
}

fn fit_scale(bounds: Rect, source: Vec2, pixels_per_point: f32) -> f32 {
    (bounds.width().max(1.0) * pixels_per_point / source.x)
        .min(bounds.height().max(1.0) * pixels_per_point / source.y)
        .min(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bounds() -> Rect {
        Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0))
    }

    #[test]
    fn fits_without_upscaling_and_reacts_to_resize() {
        let mut view = View::default();
        assert_eq!(
            view.rect(bounds(), Vec2::new(1600.0, 800.0), 1.0).size(),
            Vec2::new(800.0, 400.0)
        );
        assert_eq!(
            view.rect(bounds(), Vec2::new(100.0, 50.0), 1.0).size(),
            Vec2::new(100.0, 50.0)
        );
        let smaller = Rect::from_min_size(Pos2::ZERO, Vec2::new(400.0, 300.0));
        assert_eq!(
            view.rect(smaller, Vec2::new(1600.0, 800.0), 1.0).size(),
            Vec2::new(400.0, 200.0)
        );
    }

    #[test]
    fn actual_size_maps_source_pixels_to_physical_screen_pixels() {
        let mut view = View::default();
        view.actual_size();
        assert_eq!(
            view.rect(bounds(), Vec2::new(800.0, 600.0), 2.0).size(),
            Vec2::new(400.0, 300.0)
        );
    }

    #[test]
    fn keeps_the_image_point_under_the_zoom_anchor() {
        let mut view = View::default();
        let source = Vec2::new(1600.0, 1200.0);
        let anchor = Pos2::new(500.0, 350.0);
        let before = view.rect(bounds(), source, 1.0);
        let point = (anchor - before.min) / before.size();
        view.zoom_at(2.0, anchor, bounds(), source, 1.0);
        let after = view.rect(bounds(), source, 1.0);
        assert!(((anchor - after.min) / after.size() - point).length() < 0.0001);
    }

    #[test]
    fn clamps_pan_and_recenters_when_the_image_fits() {
        let mut view = View::default();
        let source = Vec2::new(1600.0, 1200.0);
        view.actual_size();
        view.pan(Vec2::splat(10000.0), bounds(), source, 1.0);
        let rect = view.rect(bounds(), source, 1.0);
        assert_eq!(rect.min, Pos2::ZERO);
        view.fit();
        assert_eq!(view.rect(bounds(), source, 1.0).center(), bounds().center());
    }

    #[test]
    fn bounds_zoom_and_clears_pan_on_reset() {
        let mut view = View::default();
        let source = Vec2::new(1600.0, 1200.0);
        view.zoom_at(1e9, bounds().center(), bounds(), source, 1.0);
        assert_eq!(view.scale(bounds(), source, 1.0), 16.0);
        view.zoom_at(1e-12, bounds().center(), bounds(), source, 1.0);
        assert_eq!(view.scale(bounds(), source, 1.0), 0.01);
        view.fit();
        assert!(view.is_fit());
        assert_eq!(view.rect(bounds(), source, 1.0).center(), bounds().center());
    }
}
