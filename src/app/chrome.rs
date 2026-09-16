use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};

impl super::AeroPdfApp {
    /// Draw all window chrome elements: drag handle, close button, resize
    /// zones, grip indicator, and border.
    pub(crate) fn draw_chrome(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
    ) {
        self.draw_top_drag_handle(ui, ctx, window_rect);
        self.draw_close_button(ui, ctx, window_rect);

        let is_near_close = self.is_near_close_button(ctx, window_rect);
        Self::handle_window_resize(ctx, window_rect, is_near_close);

        self.draw_grip(ui, window_rect);
        self.draw_border(ui, window_rect);
    }

    fn is_near_close_button(&self, ctx: &egui::Context, window_rect: Rect) -> bool {
        let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
        let close_btn_pos = Pos2::new(window_rect.max.x - 22.0, window_rect.min.y + 22.0);
        let close_btn_radius = 15.0;
        let close_btn_rect =
            Rect::from_center_size(close_btn_pos, Vec2::splat(close_btn_radius * 2.0));
        mouse_pos.map_or(false, |pos| close_btn_rect.expand(20.0).contains(pos))
    }

    /// Invisible drag handle in the top 20px strip to move the borderless window.
    fn draw_top_drag_handle(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
    ) {
        let top_drag_rect = Rect::from_min_max(
            Pos2::new(window_rect.min.x + 35.0, window_rect.min.y),
            Pos2::new(window_rect.max.x - 50.0, window_rect.min.y + 20.0),
        );
        let top_drag = ui.allocate_rect(top_drag_rect, Sense::drag());
        if top_drag.dragged() {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }
    }

    /// Floating close button [✕] that fades in on hover near the top-right corner.
    fn draw_close_button(
        &self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
    ) {
        let mouse_pos = ctx.input(|i| i.pointer.hover_pos());
        let close_btn_pos = Pos2::new(window_rect.max.x - 22.0, window_rect.min.y + 22.0);
        let close_btn_radius = 15.0;
        let close_btn_rect =
            Rect::from_center_size(close_btn_pos, Vec2::splat(close_btn_radius * 2.0));

        let is_near_close = mouse_pos.map_or(false, |pos| close_btn_rect.expand(20.0).contains(pos));
        let hover_alpha = ctx.animate_bool(ui.id().with("close_btn_anim"), is_near_close);

        if hover_alpha > 0.02 {
            let is_btn_hovered = mouse_pos.map_or(false, |p| close_btn_rect.contains(p));

            if is_btn_hovered {
                ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            let bg_alpha = (hover_alpha * if is_btn_hovered { 235.0 } else { 160.0 }) as u8;
            let btn_bg_color = if is_btn_hovered {
                Color32::from_rgba_unmultiplied(225, 45, 55, bg_alpha)
            } else {
                Color32::from_rgba_unmultiplied(35, 35, 40, bg_alpha)
            };

            let painter = ui.painter();
            painter.circle_filled(close_btn_pos, close_btn_radius, btn_bg_color);

            // Subtle border
            painter.circle_stroke(
                close_btn_pos,
                close_btn_radius,
                Stroke::new(
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        255,
                        255,
                        255,
                        (hover_alpha * 50.0) as u8,
                    ),
                ),
            );

            // White "✕"
            let cross_color = Color32::from_rgba_unmultiplied(
                255,
                255,
                255,
                (hover_alpha * 255.0) as u8,
            );
            let cross_size = 5.0;
            let stroke = Stroke::new(1.8, cross_color);
            painter.line_segment(
                [
                    Pos2::new(
                        close_btn_pos.x - cross_size,
                        close_btn_pos.y - cross_size,
                    ),
                    Pos2::new(
                        close_btn_pos.x + cross_size,
                        close_btn_pos.y + cross_size,
                    ),
                ],
                stroke,
            );
            painter.line_segment(
                [
                    Pos2::new(
                        close_btn_pos.x + cross_size,
                        close_btn_pos.y - cross_size,
                    ),
                    Pos2::new(
                        close_btn_pos.x - cross_size,
                        close_btn_pos.y + cross_size,
                    ),
                ],
                stroke,
            );

            // Handle click
            if is_btn_hovered
                && ctx.input(|i| i.pointer.button_clicked(egui::PointerButton::Primary))
            {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        }
    }

    /// Custom resize zones on edges (12px) and corners (28px) for the
    /// borderless window. East edge omitted to avoid scrollbar interference.
    fn handle_window_resize(ctx: &egui::Context, window_rect: Rect, is_near_close: bool) {
        let mouse_pos = match ctx.input(|i| i.pointer.hover_pos()) {
            Some(pos) => pos,
            None => return,
        };

        if is_near_close {
            return;
        }

        let thickness = 12.0;
        let corner = 28.0;

        let x = mouse_pos.x;
        let y = mouse_pos.y;
        let min_x = window_rect.min.x;
        let max_x = window_rect.max.x;
        let min_y = window_rect.min.y;
        let max_y = window_rect.max.y;

        if x < min_x - 4.0 || x > max_x + 4.0 || y < min_y - 4.0 || y > max_y + 4.0 {
            return;
        }

        let resize_info = if x >= max_x - corner && y >= max_y - corner {
            Some((
                egui::ResizeDirection::SouthEast,
                egui::CursorIcon::ResizeNwSe,
            ))
        } else if x <= min_x + corner && y >= max_y - corner {
            Some((
                egui::ResizeDirection::SouthWest,
                egui::CursorIcon::ResizeNeSw,
            ))
        } else if x <= min_x + corner && y <= min_y + corner {
            Some((
                egui::ResizeDirection::NorthWest,
                egui::CursorIcon::ResizeNwSe,
            ))
        } else if x >= max_x - corner && y <= min_y + 20.0 {
            Some((
                egui::ResizeDirection::NorthEast,
                egui::CursorIcon::ResizeNeSw,
            ))
        } else if y >= max_y - thickness {
            Some((
                egui::ResizeDirection::South,
                egui::CursorIcon::ResizeVertical,
            ))
        } else if x <= min_x + thickness {
            Some((
                egui::ResizeDirection::West,
                egui::CursorIcon::ResizeHorizontal,
            ))
        } else if y <= min_y + thickness && x < max_x - 60.0 {
            Some((
                egui::ResizeDirection::North,
                egui::CursorIcon::ResizeVertical,
            ))
        } else {
            None
        };

        if let Some((dir, cursor)) = resize_info {
            ctx.set_cursor_icon(cursor);
            if ctx.input(|i| {
                i.pointer.primary_down()
                    || i.pointer.button_pressed(egui::PointerButton::Primary)
            }) {
                ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
            }
        }
    }

    /// Subtle diagonal lines in the bottom-right corner as a resize grip hint.
    fn draw_grip(&self, ui: &mut egui::Ui, window_rect: Rect) {
        let painter = ui.painter();
        let p = Pos2::new(window_rect.max.x - 3.0, window_rect.max.y - 3.0);
        let stroke = Stroke::new(
            1.2,
            Color32::from_rgba_unmultiplied(100, 100, 110, 60),
        );
        painter.line_segment(
            [Pos2::new(p.x - 3.0, p.y), Pos2::new(p.x, p.y - 3.0)],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(p.x - 7.0, p.y), Pos2::new(p.x, p.y - 7.0)],
            stroke,
        );
        painter.line_segment(
            [Pos2::new(p.x - 11.0, p.y), Pos2::new(p.x, p.y - 11.0)],
            stroke,
        );
    }

    /// Subtle outer border for the borderless window.
    fn draw_border(&self, ui: &mut egui::Ui, window_rect: Rect) {
        let border_stroke = if self.dark_mode {
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 32),
            )
        } else {
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(0, 0, 0, 38),
            )
        };
        ui.painter().rect_stroke(
            window_rect,
            CornerRadius::same(6),
            border_stroke,
            StrokeKind::Inside,
        );
    }
}
