use crate::pdf_engine::PageInfo;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect};
use std::sync::Arc;
use std::time::Instant;

impl super::AeroPdfApp {
    /// Copy the currently selected text to both the OS clipboard (via arboard)
    /// and the egui context clipboard.
    pub(crate) fn copy_selection(&mut self, ctx: &egui::Context) -> bool {
        if let (Some(doc), Some(page_idx), Some(start), Some(end)) = (
            &self.doc,
            self.selected_page,
            self.selection_start_char,
            self.selection_end_char,
        ) {
            if !self.page_text_cache.contains_key(&page_idx) {
                let info = doc.get_page_text(page_idx);
                self.page_text_cache.insert(page_idx, Arc::new(info));
            }

            if let Some(page_info) = self.page_text_cache.get(&page_idx) {
                let min_idx = start.min(end);
                let max_idx = start.max(end);

                if min_idx < page_info.chars.len() {
                    let max_idx = max_idx.min(page_info.chars.len() - 1);
                    let mut text = String::new();
                    for i in min_idx..=max_idx {
                        text.push(page_info.chars[i].c);
                    }

                    if !text.is_empty() {
                        // Direct Win32 OS clipboard via arboard
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            let _ = clipboard.set_text(text.clone());
                        }

                        // Also inform egui context
                        ctx.copy_text(text);

                        self.toast_message =
                            Some(("Texte copié !".to_string(), Instant::now()));
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Find the character index closest to the mouse position on a given page.
    /// Vertical distance is weighted 2.2× to favour horizontal proximity.
    pub(crate) fn find_closest_char(
        page_info: &PageInfo,
        mouse_pos: Pos2,
        page_rect: Rect,
    ) -> Option<usize> {
        if page_info.chars.is_empty() {
            return None;
        }

        let scale = page_rect.width() / page_info.width_pt;
        let mut best_idx = None;
        let mut min_dist_sq = f32::MAX;

        for (idx, char_info) in page_info.chars.iter().enumerate() {
            let w = char_info.rect[2] - char_info.rect[0];
            let h = char_info.rect[3] - char_info.rect[1];
            if w <= 0.0 || h <= 0.0 {
                continue;
            }

            let char_center_x =
                page_rect.min.x + (char_info.rect[0] + char_info.rect[2]) * 0.5 * scale;
            let char_center_y = page_rect.min.y
                + (page_info.height_pt - (char_info.rect[1] + char_info.rect[3]) * 0.5) * scale;

            let dx = mouse_pos.x - char_center_x;
            let dy = (mouse_pos.y - char_center_y) * 2.2;
            let dist_sq = dx * dx + dy * dy;

            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                best_idx = Some(idx);
            }
        }

        best_idx.or_else(|| {
            if !page_info.chars.is_empty() {
                Some(0)
            } else {
                None
            }
        })
    }

    /// Draw smooth, continuous highlight ribbons over the currently selected text.
    pub(crate) fn draw_selection_highlights(
        &self,
        page_painter: &egui::Painter,
        page_info: &PageInfo,
        page_idx: usize,
        page_rect: Rect,
    ) {
        if self.selected_page != Some(page_idx) {
            return;
        }
        if let (Some(start), Some(end)) = (self.selection_start_char, self.selection_end_char) {
            if page_info.chars.is_empty() {
                return;
            }
            let min_idx = start.min(end);
            let max_idx = start.max(end).min(page_info.chars.len() - 1);

            let text_scale = page_rect.width() / page_info.width_pt;
            let highlight_color = if self.dark_mode {
                Color32::from_rgba_unmultiplied(65, 135, 245, 105)
            } else {
                Color32::from_rgba_unmultiplied(35, 115, 245, 80)
            };

            let mut merged_rects: Vec<Rect> = Vec::new();

            for i in min_idx..=max_idx {
                let char_info = &page_info.chars[i];
                if char_info.rect[2] <= char_info.rect[0]
                    || char_info.rect[3] <= char_info.rect[1]
                {
                    continue;
                }

                let x1 = page_rect.min.x + char_info.rect[0] * text_scale;
                let x2 = page_rect.min.x + char_info.rect[2] * text_scale;
                let y1 = page_rect.min.y
                    + (page_info.height_pt - char_info.rect[3]) * text_scale;
                let y2 = page_rect.min.y
                    + (page_info.height_pt - char_info.rect[1]) * text_scale;

                let char_rect = Rect::from_min_max(Pos2::new(x1, y1), Pos2::new(x2, y2));

                if let Some(last) = merged_rects.last_mut() {
                    let last_h = last.height().max(1.0);
                    let char_h = char_rect.height().max(1.0);
                    let avg_h = (last_h + char_h) * 0.5;

                    let center_y_diff = (char_rect.center().y - last.center().y).abs();
                    let v_overlap = (last.max.y.min(char_rect.max.y) - last.min.y.max(char_rect.min.y)).max(0.0);
                    let h_gap = char_rect.min.x - last.max.x;

                    // Merge if characters are on the same line and adjacent
                    let same_line = (center_y_diff < avg_h * 0.55 || v_overlap > avg_h * 0.35)
                        && h_gap > -avg_h * 0.5
                        && h_gap < avg_h * 3.5;

                    if same_line {
                        last.min.x = last.min.x.min(char_rect.min.x);
                        last.max.x = last.max.x.max(char_rect.max.x);
                        last.min.y = last.min.y.min(char_rect.min.y);
                        last.max.y = last.max.y.max(char_rect.max.y);
                        continue;
                    }
                }

                merged_rects.push(char_rect);
            }

            for rect in merged_rects {
                // Add a subtle padding and rounded corners for a modern, smooth highlight
                let padded = rect.expand2(egui::vec2(0.5, 1.0));
                page_painter.rect_filled(padded, CornerRadius::same(2), highlight_color);
            }
        }
    }

    /// Handle mouse interactions for text selection: click-drag, double-click
    /// for word selection, and right-click to copy.
    pub(crate) fn handle_selection_interaction(
        &mut self,
        response: &egui::Response,
        page_info: &PageInfo,
        page_idx: usize,
        page_rect: Rect,
        window_rect: Rect,
        should_copy: &mut bool,
    ) {
        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let in_resize_margin = pos.x <= window_rect.min.x + 12.0
                    || pos.y >= window_rect.max.y - 12.0
                    || pos.y <= window_rect.min.y + 12.0;

                if !in_resize_margin {
                    response.request_focus();
                    if let Some(char_idx) =
                        Self::find_closest_char(page_info, pos, page_rect)
                    {
                        self.selected_page = Some(page_idx);
                        self.selection_start_char = Some(char_idx);
                        self.selection_end_char = Some(char_idx);
                    }
                }
            }
        } else if response.dragged() {
            if self.selected_page == Some(page_idx) {
                if let Some(pos) = response.interact_pointer_pos() {
                    if let Some(char_idx) =
                        Self::find_closest_char(page_info, pos, page_rect)
                    {
                        self.selection_end_char = Some(char_idx);
                    }
                }
            }
        } else if response.double_clicked() {
            response.request_focus();
            if let Some(pos) = response.interact_pointer_pos() {
                if let Some(char_idx) =
                    Self::find_closest_char(page_info, pos, page_rect)
                {
                    let mut start = char_idx;
                    while start > 0
                        && !page_info.chars[start - 1].c.is_whitespace()
                    {
                        start -= 1;
                    }
                    let mut end = char_idx;
                    while end + 1 < page_info.chars.len()
                        && !page_info.chars[end + 1].c.is_whitespace()
                    {
                        end += 1;
                    }
                    self.selected_page = Some(page_idx);
                    self.selection_start_char = Some(start);
                    self.selection_end_char = Some(end);
                }
            }
        } else if response.secondary_clicked() {
            if self.selected_page == Some(page_idx)
                && self.selection_start_char != self.selection_end_char
            {
                *should_copy = true;
            }
        } else if response.clicked() && !response.drag_stopped() {
            self.selected_page = None;
            self.selection_start_char = None;
            self.selection_end_char = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::AeroPdfApp;
    use crate::pdf_engine::{CharInfo, PageInfo};
    use eframe::egui::{Pos2, Rect};

    fn make_page_info(chars: Vec<(char, [f32; 4])>) -> PageInfo {
        PageInfo {
            index: 0,
            width_pt: 595.0,
            height_pt: 842.0,
            chars: chars
                .into_iter()
                .map(|(c, rect)| CharInfo { c, rect })
                .collect(),
        }
    }

    #[test]
    fn test_find_closest_char_empty() {
        let page_info = make_page_info(vec![]);
        let page_rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(595.0, 842.0));
        let result =
            AeroPdfApp::find_closest_char(&page_info, Pos2::new(100.0, 100.0), page_rect);
        assert_eq!(result, None);
    }

    #[test]
    fn test_find_closest_char_single() {
        let page_info = make_page_info(vec![('A', [100.0, 700.0, 120.0, 720.0])]);
        let page_rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(595.0, 842.0));
        // Click near the character
        let result =
            AeroPdfApp::find_closest_char(&page_info, Pos2::new(110.0, 130.0), page_rect);
        assert_eq!(result, Some(0));
    }

    #[test]
    fn test_find_closest_char_picks_nearest() {
        let page_info = make_page_info(vec![
            ('A', [50.0, 800.0, 70.0, 820.0]),
            ('B', [200.0, 800.0, 220.0, 820.0]),
        ]);
        let page_rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(595.0, 842.0));
        // Click close to 'B'
        let result =
            AeroPdfApp::find_closest_char(&page_info, Pos2::new(210.0, 30.0), page_rect);
        assert_eq!(result, Some(1));
    }
}

