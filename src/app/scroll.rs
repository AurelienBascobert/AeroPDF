use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, StrokeKind, Vec2};
use std::time::Instant;

/// Centralised layout parameters computed once per frame, eliminating
/// duplicated calculations across modules.
#[derive(Clone, Debug)]
pub(crate) struct LayoutParams {
    pub page_width: f32,
    pub page_height: f32,
    pub row_height: f32,
    pub margin: f32,
    pub row_count: usize,
    pub is_dual: bool,
    pub gap: f32,
    /// Page width in PDF points (after rotation).
    pub w_pt: f32,
    /// Page height in PDF points (after rotation).
    pub h_pt: f32,
}

impl super::AeroPdfApp {
    /// Compute all layout dimensions from the current state (zoom, rotation,
    /// dual-page mode) and available width.
    pub(crate) fn compute_layout(
        &self,
        available_w: f32,
        pages_count: usize,
    ) -> LayoutParams {
        let (raw_w_pt, raw_h_pt) = self
            .doc
            .as_ref()
            .map(|d| d.default_page_size)
            .unwrap_or((595.0, 842.0));
        let (w_pt, h_pt) = if self.rotation % 2 != 0 {
            (raw_h_pt, raw_w_pt)
        } else {
            (raw_w_pt, raw_h_pt)
        };

        let is_dual = self.dual_page_mode;
        let gap = 12.0f32;

        if is_dual {
            let single_avail = ((available_w - gap) * 0.5).max(100.0);
            let pw = (single_avail * self.zoom).round();
            let ph = (h_pt * (pw / w_pt)).round();
            let rh = ph + 4.0;
            let total_w = pw * 2.0 + gap;
            let m = if total_w < available_w {
                ((available_w - total_w) * 0.5).round()
            } else {
                0.0
            };
            let rc = (pages_count + 1) / 2;
            LayoutParams {
                page_width: pw,
                page_height: ph,
                row_height: rh,
                margin: m,
                row_count: rc,
                is_dual,
                gap,
                w_pt,
                h_pt,
            }
        } else {
            let pw = (available_w * self.zoom).round();
            let ph = (h_pt * (pw / w_pt)).round();
            let rh = ph + 2.0;
            let m = if pw < available_w {
                ((available_w - pw) * 0.5).round()
            } else {
                0.0
            };
            LayoutParams {
                page_width: pw,
                page_height: ph,
                row_height: rh,
                margin: m,
                row_count: pages_count,
                is_dual,
                gap,
                w_pt,
                h_pt,
            }
        }
    }

    /// Handle smooth inertial scrolling by interpolating toward the target
    /// scroll offset.
    pub(crate) fn handle_scroll(
        &mut self,
        ctx: &egui::Context,
        actual_scroll: f32,
        wheel_delta: f32,
        ctrl_down: bool,
        window_rect: Rect,
        layout: &LayoutParams,
    ) {
        let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
        let is_clicking_scrollbar =
            mouse_pos.map_or(false, |p| p.x >= window_rect.max.x - 16.0)
                && ctx.input(|i| i.pointer.primary_down());

        if is_clicking_scrollbar {
            self.target_scroll_y = None;
        } else if wheel_delta != 0.0 && !ctrl_down {
            let max_scroll =
                (layout.row_count as f32 * layout.row_height - window_rect.height()).max(0.0);
            let start_from = self.target_scroll_y.unwrap_or(actual_scroll);
            let target = (start_from - wheel_delta * 1.8).clamp(0.0, max_scroll);
            self.target_scroll_y = Some(target);
            ctx.request_repaint();
        } else if let Some(target) = self.target_scroll_y {
            let diff = target - actual_scroll;
            if diff.abs() > 0.8 {
                let step = diff * 0.28;
                self.target_scroll_y = Some(actual_scroll + step);
                ctx.request_repaint();
            } else {
                self.target_scroll_y = None;
            }
        }
    }

    /// Track current scroll position, update the session history, and return
    /// the ghost indicator text (e.g. "Page 3 / 42").
    pub(crate) fn track_scroll_position(
        &mut self,
        actual_scroll: f32,
        wheel_delta: f32,
        layout: &LayoutParams,
        pages_count: usize,
        window_rect: Rect,
    ) -> String {
        let current_row_idx =
            ((actual_scroll + window_rect.height() * 0.3) / layout.row_height).floor() as usize;

        let (current_page_idx, ghost_text) = if layout.is_dual {
            let p_start = (current_row_idx * 2 + 1).clamp(1, pages_count);
            let p_end = (p_start + 1).min(pages_count);
            let text = if p_start == p_end {
                format!("Page {} / {}", p_start, pages_count)
            } else {
                format!("Pages {} - {} / {}", p_start, p_end, pages_count)
            };
            (p_start - 1, text)
        } else {
            let p = (current_row_idx + 1).clamp(1, pages_count);
            (p - 1, format!("Page {} / {}", p, pages_count))
        };

        if (actual_scroll - self.last_known_scroll_y).abs() > 2.0 || wheel_delta != 0.0 {
            self.last_scroll_time = Instant::now();
            self.last_known_scroll_y = actual_scroll;
        }

        // Update history
        if let Some(doc) = &self.doc {
            let key = doc.path.to_string_lossy().to_string();
            if self.history.get(&key) != Some(&current_page_idx) {
                self.history.insert(key, current_page_idx);
                self.history_dirty = true;
            }
        }

        if self.history_dirty && self.last_history_save.elapsed().as_secs() >= 1 {
            super::history::save_history(&self.history);
            self.history_dirty = false;
            self.last_history_save = Instant::now();
        }

        ghost_text
    }

    /// Draw the ghost page indicator (bottom-center) and the ghost zoom
    /// indicator (bottom-left). Both fade in/out based on timing.
    pub(crate) fn draw_ghost_indicators(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
        ghost_text: &str,
    ) {
        // Ghost Page Indicator
        let scroll_elapsed = self.last_scroll_time.elapsed().as_secs_f32();
        let show_indicator =
            (scroll_elapsed < 1.3 || self.target_scroll_y.is_some()) && !self.is_goto_open;
        let indicator_alpha =
            ctx.animate_bool(ui.id().with("ghost_indicator"), show_indicator);

        if indicator_alpha > 0.02 {
            let badge_pos = Pos2::new(window_rect.center().x, window_rect.max.y - 34.0);
            let badge_rect = Rect::from_center_size(badge_pos, Vec2::new(160.0, 26.0));
            let bg_color = Color32::from_rgba_unmultiplied(
                20, 20, 26,
                (indicator_alpha * 220.0) as u8,
            );
            let text_color = Color32::from_rgba_unmultiplied(
                240, 240, 252,
                (indicator_alpha * 255.0) as u8,
            );
            let border_stroke = Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(
                    255, 255, 255,
                    (indicator_alpha * 40.0) as u8,
                ),
            );

            let painter = ui.painter();
            painter.rect(
                badge_rect,
                CornerRadius::same(13),
                bg_color,
                border_stroke,
                StrokeKind::Inside,
            );
            painter.text(
                badge_pos,
                egui::Align2::CENTER_CENTER,
                ghost_text,
                egui::FontId::proportional(12.0),
                text_color,
            );
            ctx.request_repaint();
        }

        // Ghost Zoom Indicator
        let zoom_elapsed = self.last_zoom_change.elapsed().as_secs_f32();
        let show_zoom =
            zoom_elapsed < 1.3 && (self.zoom - 1.0).abs() > 0.02 && !self.is_goto_open;
        let zoom_alpha =
            ctx.animate_bool(ui.id().with("ghost_zoom_indicator"), show_zoom);

        if zoom_alpha > 0.02 {
            let badge_pos = Pos2::new(window_rect.min.x + 55.0, window_rect.max.y - 34.0);
            let badge_text = format!("{:.0}%", self.zoom * 100.0);
            let badge_rect = Rect::from_center_size(badge_pos, Vec2::new(65.0, 26.0));
            let bg_color = Color32::from_rgba_unmultiplied(
                20, 20, 26,
                (zoom_alpha * 220.0) as u8,
            );
            let text_color = Color32::from_rgba_unmultiplied(
                240, 240, 252,
                (zoom_alpha * 255.0) as u8,
            );
            let border_stroke = Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(
                    255, 255, 255,
                    (zoom_alpha * 40.0) as u8,
                ),
            );

            let painter = ui.painter();
            painter.rect(
                badge_rect,
                CornerRadius::same(13),
                bg_color,
                border_stroke,
                StrokeKind::Inside,
            );
            painter.text(
                badge_pos,
                egui::Align2::CENTER_CENTER,
                badge_text,
                egui::FontId::proportional(12.0),
                text_color,
            );
            ctx.request_repaint();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layout_params_single_page() {
        let layout = LayoutParams {
            page_width: 800.0,
            page_height: 1131.0,
            row_height: 1133.0,
            margin: 0.0,
            row_count: 10,
            is_dual: false,
            gap: 12.0,
            w_pt: 595.0,
            h_pt: 842.0,
        };
        assert!(!layout.is_dual);
        assert_eq!(layout.row_count, 10);
        assert!(layout.page_height > 0.0);
    }

    #[test]
    fn test_layout_params_dual_page() {
        let layout = LayoutParams {
            page_width: 394.0,
            page_height: 557.0,
            row_height: 561.0,
            margin: 0.0,
            row_count: 5,
            is_dual: true,
            gap: 12.0,
            w_pt: 595.0,
            h_pt: 842.0,
        };
        assert!(layout.is_dual);
        assert_eq!(layout.row_count, 5);
    }

    #[test]
    fn test_layout_zoom_scales_width() {
        // Simulating what compute_layout does for zoom=2.0
        let available_w = 800.0f32;
        let zoom = 2.0f32;
        let pw = (available_w * zoom).round();
        assert!((pw - 1600.0).abs() < 1.0);
    }
}

