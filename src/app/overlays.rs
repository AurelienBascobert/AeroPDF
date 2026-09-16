use crate::pdf_engine::LoadedDoc;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Instant;

use super::scroll::LayoutParams;

// ── Asynchronous Search Worker ──────────────────────────────────────────

/// A request to search a document for a query string.
pub(crate) struct SearchRequest {
    pub query: String,
    pub doc: Arc<LoadedDoc>,
}

/// An event emitted by the search worker thread.
pub(crate) enum SearchEvent {
    /// One match found on `page_idx` with the given highlight rectangles.
    Match {
        page_idx: usize,
        rects: Vec<[f32; 4]>,
    },
    /// All pages have been scanned.
    Done,
}

/// Spawn a background thread that processes [`SearchRequest`]s and sends
/// [`SearchEvent`]s back. When `cancel` is set to `true`, the current
/// scan is aborted. Newer requests automatically cancel older ones.
pub(crate) fn spawn_search_worker(
    request_rx: mpsc::Receiver<SearchRequest>,
    result_tx: mpsc::Sender<SearchEvent>,
    cancel: Arc<AtomicBool>,
) {
    std::thread::Builder::new()
        .name("aeropdf-search".into())
        .spawn(move || {
            while let Ok(mut request) = request_rx.recv() {
                // Drain to keep only the latest request
                while let Ok(newer) = request_rx.try_recv() {
                    request = newer;
                }

                cancel.store(false, Ordering::Relaxed);

                if request.query.trim().is_empty() {
                    let _ = result_tx.send(SearchEvent::Done);
                    continue;
                }

                for page_idx in 0..request.doc.page_count {
                    if cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let rects_list = request.doc.search_page(page_idx, &request.query);
                    for rects in rects_list {
                        let _ = result_tx.send(SearchEvent::Match { page_idx, rects });
                    }
                }

                if !cancel.load(Ordering::Relaxed) {
                    let _ = result_tx.send(SearchEvent::Done);
                }
            }
        })
        .expect("Failed to spawn search thread");
}

// ── Overlay Drawing Methods ─────────────────────────────────────────────

impl super::AeroPdfApp {
    /// Drain results from the background search thread and update state.
    pub(crate) fn tick_search(&mut self, ctx: &egui::Context, layout: &LayoutParams) {
        while let Ok(event) = self.search_result_rx.try_recv() {
            match event {
                SearchEvent::Match { page_idx, rects } => {
                    let is_first = self.search_matches.is_empty();
                    self.search_matches.push(super::SearchMatch {
                        page_idx,
                        rects,
                    });
                    if is_first {
                        let target_row = if layout.is_dual {
                            page_idx / 2
                        } else {
                            page_idx
                        };
                        self.target_scroll_y =
                            Some(target_row as f32 * layout.row_height);
                        self.last_scroll_time = Instant::now();
                    }
                }
                SearchEvent::Done => {
                    self.is_searching = false;
                }
            }
            ctx.request_repaint();
        }
    }

    // ── Ctrl+G: Go-to Page Modal ────────────────────────────────────

    pub(crate) fn draw_goto_modal(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
        pages_count: usize,
        layout: &LayoutParams,
    ) {
        if !self.is_goto_open {
            return;
        }

        // Backdrop
        ui.painter().rect_filled(
            window_rect,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0, 0, 0, 120),
        );

        let modal_center = window_rect.center();
        let modal_rect = Rect::from_center_size(modal_center, Vec2::new(260.0, 110.0));

        ui.painter().rect(
            modal_rect,
            CornerRadius::same(12),
            Color32::from_rgb(28, 28, 34),
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 45),
            ),
            StrokeKind::Inside,
        );

        egui::Area::new(egui::Id::new("goto_modal_input"))
            .fixed_pos(modal_rect.min + Vec2::splat(14.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(modal_rect.width() - 28.0);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "Aller à la page (1 - {})",
                            pages_count
                        ))
                        .size(13.0)
                        .color(Color32::from_rgb(210, 215, 225)),
                    );
                    ui.add_space(8.0);

                    let text_edit =
                        egui::TextEdit::singleline(&mut self.goto_page_input)
                            .font(egui::FontId::proportional(15.0))
                            .text_color(Color32::WHITE)
                            .margin(egui::Margin::symmetric(10, 6));

                    let resp = ui.add_sized(Vec2::new(120.0, 30.0), text_edit);
                    resp.request_focus();

                    if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(num) =
                            self.goto_page_input.trim().parse::<usize>()
                        {
                            if num >= 1 && num <= pages_count {
                                let target_row = if layout.is_dual {
                                    (num - 1) / 2
                                } else {
                                    num - 1
                                };
                                let target_y =
                                    target_row as f32 * layout.row_height;
                                self.target_scroll_y = Some(target_y);
                                self.last_scroll_time = Instant::now();
                                self.is_goto_open = false;
                                ctx.request_repaint();
                            }
                        }
                    }
                });
            });
    }

    // ── Ctrl+T / Ctrl+B: Table of Contents Modal ────────────────────

    pub(crate) fn draw_toc_modal(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
        layout: &LayoutParams,
    ) {
        if !self.is_toc_open {
            return;
        }

        // Backdrop
        ui.painter().rect_filled(
            window_rect,
            CornerRadius::ZERO,
            Color32::from_rgba_unmultiplied(0, 0, 0, 135),
        );

        let modal_center = window_rect.center();
        let modal_max_w = (window_rect.width() - 40.0).max(260.0);
        let modal_max_h = (window_rect.height() - 60.0).max(200.0);
        let modal_w = 520.0f32.min(modal_max_w);
        let modal_h = 440.0f32.min(modal_max_h);
        let modal_rect =
            Rect::from_center_size(modal_center, Vec2::new(modal_w, modal_h));

        ui.painter().rect(
            modal_rect,
            CornerRadius::same(12),
            Color32::from_rgb(26, 26, 32),
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 45),
            ),
            StrokeKind::Inside,
        );

        egui::Area::new(egui::Id::new("toc_modal_dialog"))
            .fixed_pos(modal_rect.min + Vec2::splat(16.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.set_width(modal_rect.width() - 32.0);
                ui.set_max_height(modal_rect.height() - 32.0);

                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Sommaire du document")
                            .size(15.5)
                            .strong()
                            .color(Color32::from_rgb(230, 235, 245)),
                    );
                    ui.with_layout(
                        egui::Layout::right_to_left(egui::Align::Center),
                        |ui| {
                            if ui.add(egui::Button::new("X").small()).clicked() {
                                self.is_toc_open = false;
                            }
                        },
                    );
                });
                ui.add_space(8.0);

                let filter_resp = ui.add(
                    egui::TextEdit::singleline(&mut self.toc_filter)
                        .hint_text("Filtrer les chapitres...")
                        .font(egui::FontId::proportional(14.0))
                        .margin(egui::Margin::symmetric(8, 6)),
                );
                if self.toc_filter.is_empty() && !filter_resp.has_focus() {
                    filter_resp.request_focus();
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if let Some(items) = &self.toc_items {
                    if items.is_empty() {
                        ui.label(
                            egui::RichText::new(
                                "Ce document ne contient aucun signet.",
                            )
                            .size(13.0)
                            .color(Color32::from_rgb(160, 165, 175)),
                        );
                    } else {
                        let filter_lower = self.toc_filter.trim().to_lowercase();
                        let matching_indices: Vec<usize> =
                            if filter_lower.is_empty() {
                                (0..items.len()).collect()
                            } else {
                                items
                                    .iter()
                                    .enumerate()
                                    .filter(|(_, it)| {
                                        it.title
                                            .to_lowercase()
                                            .contains(&filter_lower)
                                    })
                                    .map(|(i, _)| i)
                                    .collect()
                            };

                        if matching_indices.is_empty() {
                            ui.label(
                                egui::RichText::new(
                                    "Aucun chapitre correspondant.",
                                )
                                .size(13.0)
                                .color(Color32::GRAY),
                            );
                        } else {
                            let row_h = 24.0;
                            egui::ScrollArea::vertical()
                                .max_height(modal_rect.height() - 110.0)
                                .auto_shrink([false, false])
                                .show_rows(
                                    ui,
                                    row_h,
                                    matching_indices.len(),
                                    |ui, row_range| {
                                        for &idx in
                                            &matching_indices[row_range]
                                        {
                                            let item = &items[idx];
                                            let indent = (item.level as f32
                                                * 14.0)
                                                .min(70.0);
                                            ui.horizontal(|ui| {
                                                if indent > 0.0 {
                                                    ui.add_space(indent);
                                                }
                                                let label =
                                                    egui::RichText::new(
                                                        &item.title,
                                                    )
                                                    .size(13.0)
                                                    .color(Color32::from_rgb(
                                                        220, 225, 235,
                                                    ));
                                                let btn = ui.add(
                                                    egui::Button::new(label)
                                                        .frame(false),
                                                );
                                                if btn.clicked() {
                                                    let p = item.page_index;
                                                    let target_row =
                                                        if layout.is_dual {
                                                            p / 2
                                                        } else {
                                                            p
                                                        };
                                                    self.target_scroll_y =
                                                        Some(
                                                            target_row as f32
                                                                * layout
                                                                    .row_height,
                                                        );
                                                    self.last_scroll_time =
                                                        Instant::now();
                                                    self.is_toc_open = false;
                                                    ctx.request_repaint();
                                                }
                                                ui.with_layout(
                                                    egui::Layout::right_to_left(egui::Align::Center),
                                                    |ui| {
                                                        ui.label(
                                                            egui::RichText::new(format!("p. {}", item.page_index + 1))
                                                                .size(11.5)
                                                                .color(Color32::from_rgb(140, 145, 160)),
                                                        );
                                                    },
                                                );
                                            });
                                            ui.add_space(2.0);
                                        }
                                    },
                                );
                        }
                    }
                } else {
                    ui.label("Chargement des signets...");
                }
            });
    }

    // ── Ctrl+F: Floating Search Bar ─────────────────────────────────

    pub(crate) fn draw_search_bar(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
        layout: &LayoutParams,
    ) {
        if !self.is_search_open {
            return;
        }

        let bar_w = 340.0;
        let bar_h = 42.0;
        let bar_pos =
            Pos2::new(window_rect.max.x - bar_w - 55.0, window_rect.min.y + 16.0);
        let bar_rect = Rect::from_min_size(bar_pos, Vec2::new(bar_w, bar_h));

        ui.painter().rect(
            bar_rect,
            CornerRadius::same(10),
            Color32::from_rgb(26, 26, 32),
            Stroke::new(
                1.0,
                Color32::from_rgba_unmultiplied(255, 255, 255, 45),
            ),
            StrokeKind::Inside,
        );

        egui::Area::new(egui::Id::new("floating_search_bar"))
            .fixed_pos(bar_pos + Vec2::new(8.0, 7.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Crisp vector magnifying glass icon
                    let (icon_rect, _) =
                        ui.allocate_exact_size(Vec2::new(16.0, 16.0), Sense::hover());
                    let p = ui.painter();
                    let center =
                        Pos2::new(icon_rect.min.x + 6.0, icon_rect.min.y + 6.0);
                    let col = Color32::from_rgb(160, 165, 180);
                    p.circle_stroke(center, 4.0, Stroke::new(1.4, col));
                    p.line_segment(
                        [
                            Pos2::new(center.x + 2.8, center.y + 2.8),
                            Pos2::new(icon_rect.max.x - 2.0, icon_rect.max.y - 2.0),
                        ],
                        Stroke::new(1.6, col),
                    );

                    let edit_resp = ui.add_sized(
                        Vec2::new(135.0, 26.0),
                        egui::TextEdit::singleline(&mut self.search_query)
                            .hint_text("Rechercher...")
                            .font(egui::FontId::proportional(13.5))
                            .margin(egui::Margin::symmetric(6, 4)),
                    );

                    // Trigger async search when query changes
                    if self.search_query != self.last_searched_query {
                        self.last_searched_query = self.search_query.clone();
                        self.search_matches.clear();
                        self.current_match_idx = 0;
                        self.is_searching = !self.search_query.trim().is_empty();

                        // Cancel previous search and start new one
                        self.search_cancel.store(true, Ordering::Relaxed);
                        if !self.search_query.trim().is_empty() {
                            if let Some(doc) = &self.doc {
                                let _ =
                                    self.search_request_tx.send(SearchRequest {
                                        query: self.search_query.clone(),
                                        doc: Arc::clone(doc),
                                    });
                            }
                        }
                    }

                    let count_text = if self.search_query.trim().is_empty() {
                        "".to_string()
                    } else if self.is_searching {
                        format!("{}/... ", self.search_matches.len())
                    } else if self.search_matches.is_empty() {
                        "0 / 0".to_string()
                    } else {
                        format!(
                            "{} / {}",
                            self.current_match_idx + 1,
                            self.search_matches.len()
                        )
                    };

                    ui.label(
                        egui::RichText::new(count_text)
                            .size(11.5)
                            .color(Color32::from_rgb(170, 175, 190)),
                    );

                    let prev_clicked =
                        ui.add(egui::Button::new("<").small()).clicked();
                    let next_clicked =
                        ui.add(egui::Button::new(">").small()).clicked();
                    let enter_pressed = edit_resp.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter));
                    let shift_down = ui.input(|i| i.modifiers.shift);

                    if prev_clicked || (enter_pressed && shift_down) {
                        if !self.search_matches.is_empty() {
                            if self.current_match_idx == 0 {
                                self.current_match_idx =
                                    self.search_matches.len() - 1;
                            } else {
                                self.current_match_idx -= 1;
                            }
                            let pg =
                                self.search_matches[self.current_match_idx]
                                    .page_idx;
                            let target_row =
                                if layout.is_dual { pg / 2 } else { pg };
                            self.target_scroll_y =
                                Some(target_row as f32 * layout.row_height);
                            self.last_scroll_time = Instant::now();
                            ctx.request_repaint();
                        }
                    } else if next_clicked || (enter_pressed && !shift_down) {
                        if !self.search_matches.is_empty() {
                            self.current_match_idx = (self.current_match_idx + 1)
                                % self.search_matches.len();
                            let pg =
                                self.search_matches[self.current_match_idx]
                                    .page_idx;
                            let target_row =
                                if layout.is_dual { pg / 2 } else { pg };
                            self.target_scroll_y =
                                Some(target_row as f32 * layout.row_height);
                            self.last_scroll_time = Instant::now();
                            ctx.request_repaint();
                        }
                    }

                    if ui.add(egui::Button::new("X").small()).clicked() {
                        self.is_search_open = false;
                        self.search_matches.clear();
                        self.search_cancel.store(true, Ordering::Relaxed);
                        ctx.request_repaint();
                    }
                });
            });
    }

    // ── Drop Zone (no document loaded) ──────────────────────────────

    pub(crate) fn draw_dropzone(
        &mut self,
        ui: &mut egui::Ui,
        _ctx: &egui::Context,
        window_rect: Rect,
    ) {
        let bg_color = Color32::from_rgb(22, 22, 26);
        let border_stroke = Stroke::new(
            1.0,
            Color32::from_rgba_unmultiplied(255, 255, 255, 35),
        );
        let corner_radius = CornerRadius::same(10);

        ui.painter().rect(
            window_rect,
            corner_radius,
            bg_color,
            border_stroke,
            StrokeKind::Inside,
        );

        ui.vertical_centered(|ui| {
            ui.add_space(window_rect.height() * 0.28);

            ui.label(
                egui::RichText::new("AeroPDF")
                    .size(36.0)
                    .strong()
                    .color(Color32::from_rgb(230, 235, 245)),
            );
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Lecteur PDF ultra-léger et instantané")
                    .size(15.0)
                    .color(Color32::from_rgb(160, 165, 175)),
            );

            ui.add_space(36.0);

            let btn_response = ui.add_sized(
                Vec2::new(220.0, 48.0),
                egui::Button::new(
                    egui::RichText::new("Ouvrir un document PDF")
                        .size(15.0)
                        .color(Color32::WHITE),
                )
                .fill(Color32::from_rgb(45, 95, 210))
                .corner_radius(CornerRadius::same(8)),
            );

            if btn_response.clicked() {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("PDF Files", &["pdf"])
                    .pick_file()
                {
                    self.load_file(&path);
                }
            }

            ui.add_space(14.0);
            ui.label(
                egui::RichText::new(
                    "ou glissez-déposez votre fichier directement ici",
                )
                .size(13.0)
                .color(Color32::from_rgb(130, 135, 145)),
            );

            ui.add_space(40.0);
            ui.label(
                egui::RichText::new("Raccourcis : Clic-droit/Alt = Déplacer  |  Molette = Défiler  |  Ctrl+C = Copier  |  Echap = Quitter")
                    .size(12.0)
                    .color(Color32::from_rgb(100, 105, 115)),
            );
        });
    }

    // ── Toast Notification ──────────────────────────────────────────

    pub(crate) fn draw_toast(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        window_rect: Rect,
    ) {
        if let Some((msg, created_at)) = &self.toast_message {
            let elapsed = created_at.elapsed().as_secs_f32();
            if elapsed < 2.2 {
                let alpha = if elapsed < 0.15 {
                    elapsed / 0.15
                } else if elapsed > 1.8 {
                    (2.2 - elapsed) / 0.4
                } else {
                    1.0
                };

                let toast_bg = Color32::from_rgba_unmultiplied(
                    25, 25, 30,
                    (alpha * 240.0) as u8,
                );
                let toast_text_color = Color32::from_rgba_unmultiplied(
                    245, 245, 255,
                    (alpha * 255.0) as u8,
                );

                let toast_pos =
                    Pos2::new(window_rect.center().x, window_rect.max.y - 36.0);
                let toast_rect =
                    Rect::from_center_size(toast_pos, Vec2::new(220.0, 32.0));

                let painter = ui.painter();
                painter.rect(
                    toast_rect,
                    CornerRadius::same(16),
                    toast_bg,
                    Stroke::new(
                        1.0,
                        Color32::from_rgba_unmultiplied(
                            255, 255, 255,
                            (alpha * 40.0) as u8,
                        ),
                    ),
                    StrokeKind::Inside,
                );

                painter.text(
                    toast_pos,
                    egui::Align2::CENTER_CENTER,
                    msg,
                    egui::FontId::proportional(13.0),
                    toast_text_color,
                );

                ctx.request_repaint();
            } else {
                self.toast_message = None;
            }
        }
    }
}
