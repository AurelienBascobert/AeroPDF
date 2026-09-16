mod chrome;
mod config;
mod history;
mod input;
mod overlays;
mod render;
mod scroll;
mod selection;

use crate::pdf_engine::*;
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Instant;

// ── Shared Types ────────────────────────────────────────────────────────

/// A single search match: one occurrence on a specific page.
#[derive(Clone, Debug)]
pub struct SearchMatch {
    pub page_idx: usize,
    pub rects: Vec<[f32; 4]>,
}

// ── Application State ───────────────────────────────────────────────────

pub struct AeroPdfApp {
    // Document
    pub(crate) doc: Option<Arc<LoadedDoc>>,

    // Rendering
    pub(crate) page_textures: HashMap<usize, (egui::TextureHandle, u32)>,
    pub(crate) page_text_cache: HashMap<usize, Arc<PageInfo>>,
    pub(crate) pending_renders: HashSet<usize>,
    pub(crate) render_queue: Arc<(Mutex<VecDeque<render::RenderTask>>, Condvar)>,
    pub(crate) render_rx: mpsc::Receiver<render::RenderResult>,
    pub(crate) render_shutdown: Arc<AtomicBool>,

    // Scroll & Zoom
    pub(crate) target_scroll_y: Option<f32>,
    pub(crate) zoom: f32,
    pub(crate) last_zoom_change: Instant,
    pub(crate) last_scroll_time: Instant,
    pub(crate) last_known_scroll_y: f32,

    // Selection
    pub(crate) selected_page: Option<usize>,
    pub(crate) selection_start_char: Option<usize>,
    pub(crate) selection_end_char: Option<usize>,

    // UI State
    pub(crate) toast_message: Option<(String, Instant)>,
    pub(crate) pending_resize: Option<Vec2>,
    pub(crate) dark_mode: bool,
    pub(crate) is_fullscreen: bool,
    pub(crate) rotation: u8,
    pub(crate) dual_page_mode: bool,

    // Go-to Page
    pub(crate) is_goto_open: bool,
    pub(crate) goto_page_input: String,

    // History
    pub(crate) history: HashMap<String, usize>,
    pub(crate) history_dirty: bool,
    pub(crate) last_history_save: Instant,

    // Search
    pub(crate) is_search_open: bool,
    pub(crate) search_query: String,
    pub(crate) last_searched_query: String,
    pub(crate) search_matches: Vec<SearchMatch>,
    pub(crate) current_match_idx: usize,
    pub(crate) search_request_tx: mpsc::Sender<overlays::SearchRequest>,
    pub(crate) search_result_rx: mpsc::Receiver<overlays::SearchEvent>,
    pub(crate) search_cancel: Arc<AtomicBool>,
    pub(crate) is_searching: bool,

    // Table of Contents
    pub(crate) is_toc_open: bool,
    pub(crate) toc_filter: String,
    pub(crate) toc_items: Option<Vec<BookmarkNode>>,
}

// ── Construction & Core Methods ─────────────────────────────────────────

impl AeroPdfApp {
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        // Render worker
        let render_queue = Arc::new((
            Mutex::new(VecDeque::<render::RenderTask>::new()),
            Condvar::new(),
        ));
        let (render_tx, render_rx) = mpsc::channel();
        let render_shutdown = Arc::new(AtomicBool::new(false));
        let render_worker_count = std::thread::available_parallelism()
            .map(|n| n.get().saturating_sub(1).max(1))
            .unwrap_or(2);
        render::spawn_render_pool(
            Arc::clone(&render_queue),
            render_tx,
            Arc::clone(&render_shutdown),
            render_worker_count,
        );

        // Search worker
        let (search_request_tx, search_request_rx) = mpsc::channel();
        let (search_result_tx, search_result_rx) = mpsc::channel();
        let search_cancel = Arc::new(AtomicBool::new(false));
        overlays::spawn_search_worker(
            search_request_rx,
            search_result_tx,
            Arc::clone(&search_cancel),
        );

        let history = history::load_history();
        let user_config = config::load_config();

        let mut app = Self {
            doc: None,
            page_textures: HashMap::new(),
            page_text_cache: HashMap::new(),
            pending_renders: HashSet::new(),
            render_queue,
            render_rx,
            render_shutdown,
            target_scroll_y: None,
            zoom: user_config.zoom,
            last_zoom_change: Instant::now(),
            last_scroll_time: Instant::now(),
            last_known_scroll_y: 0.0,
            selected_page: None,
            selection_start_char: None,
            selection_end_char: None,
            toast_message: None,
            pending_resize: None,
            dark_mode: user_config.dark_mode,
            is_fullscreen: false,
            rotation: 0,
            dual_page_mode: user_config.dual_page_mode,
            is_goto_open: false,
            goto_page_input: String::new(),
            history,
            history_dirty: false,
            last_history_save: Instant::now(),
            is_search_open: false,
            search_query: String::new(),
            last_searched_query: String::new(),
            search_matches: Vec::new(),
            current_match_idx: 0,
            search_request_tx,
            search_result_rx,
            search_cancel,
            is_searching: false,
            is_toc_open: false,
            toc_filter: String::new(),
            toc_items: None,
        };

        if let Some(path) = initial_file {
            app.load_file(&path);
        }

        app
    }

    pub fn load_file(&mut self, path: &Path) {
        match LoadedDoc::open(path) {
            Ok(doc) => {
                let (w, h) = doc.default_page_size;
                if self.doc.is_none() && w > 0.0 && h > 0.0 {
                    let aspect = h / w;
                    let target_w = 800.0f32;
                    let target_h = (target_w * aspect).clamp(400.0, 720.0);
                    self.pending_resize = Some(Vec2::new(target_w, target_h));
                }

                // Clear previous state
                self.page_textures.clear();
                self.page_text_cache.clear();
                self.pending_renders.clear();
                self.target_scroll_y = None;

                {
                    let (lock, _) = &*self.render_queue;
                    lock.lock().unwrap().clear();
                }
                while self.render_rx.try_recv().is_ok() {}

                // Restore reading position from history
                let canonical_key = path.to_string_lossy().to_string();
                if let Some(&saved_page) = self.history.get(&canonical_key) {
                    if saved_page > 0 && saved_page < doc.page_count && w > 0.0 {
                        let row_h = h * (800.0 / w) + 1.0;
                        self.target_scroll_y = Some(saved_page as f32 * row_h);
                        self.toast_message = Some((
                            format!(
                                "Reprise à la page {} / {}",
                                saved_page + 1,
                                doc.page_count
                            ),
                            Instant::now(),
                        ));
                    }
                }

                self.doc = Some(Arc::new(doc));
                self.selected_page = None;
                self.selection_start_char = None;
                self.selection_end_char = None;
                self.zoom = 1.0;
                self.rotation = 0;
                self.dual_page_mode = false;
                self.is_search_open = false;
                self.search_matches.clear();
                self.search_query.clear();
                self.last_searched_query.clear();
                self.search_cancel.store(true, Ordering::Relaxed);
                self.is_searching = false;
                self.is_toc_open = false;
                self.toc_filter.clear();
                self.toc_items = None;
                self.last_scroll_time = Instant::now();
            }
            Err(err) => {
                self.toast_message = Some((err, Instant::now()));
            }
        }
    }

    pub fn show_toast(&mut self, msg: String) {
        self.toast_message = Some((msg, Instant::now()));
    }

    pub fn cycle_rotation(&mut self, ctx: &egui::Context) {
        self.rotation = (self.rotation + 1) % 4;
        self.page_textures.clear();
        self.pending_renders.clear();
        if let Ok(mut q) = self.render_queue.0.lock() {
            q.clear();
        }
        self.show_toast(format!("Rotation : {}°", self.rotation as u32 * 90));
        ctx.request_repaint();
    }
}

// ── Clean Shutdown ──────────────────────────────────────────────────────

impl Drop for AeroPdfApp {
    fn drop(&mut self) {
        // Signal the render workers to exit
        self.render_shutdown.store(true, Ordering::Relaxed);
        self.render_queue.1.notify_all();

        // Cancel any running search
        self.search_cancel.store(true, Ordering::Relaxed);

        // Save history on exit
        if self.history_dirty {
            history::save_history(&self.history);
        }

        // Persist user preferences
        config::save_config(&config::UserConfig {
            dark_mode: self.dark_mode,
            zoom: self.zoom,
            dual_page_mode: self.dual_page_mode,
        });
    }
}

// ── eframe::App Implementation ──────────────────────────────────────────

impl eframe::App for AeroPdfApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        let window_rect = ui.max_rect();

        // Fill window background
        let frame_bg = if self.dark_mode {
            Color32::from_rgb(18, 18, 22)
        } else {
            Color32::from_rgb(240, 242, 246)
        };
        ui.painter()
            .rect_filled(window_rect, CornerRadius::same(6), frame_bg);

        // 0. Drain render results from background thread
        self.drain_render_results(&ctx);

        // Apply pending window auto-resize to match PDF aspect ratio
        if let Some(size) = self.pending_resize.take() {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        }

        // 1. Handle drag & drop
        self.handle_drag_and_drop(&ctx);

        // 2. Handle keyboard shortcuts
        let input = self.handle_keyboard_shortcuts(&ctx, ui, window_rect);
        if input.should_return {
            return;
        }
        let mut should_copy = input.should_copy;

        // 3. Main content
        let doc_opt = self.doc.clone();
        if let Some(doc) = doc_opt {
            ui.spacing_mut().item_spacing = Vec2::ZERO;
            ui.spacing_mut().window_margin = egui::Margin::ZERO;

            let pages_count = doc.page_count;
            let available_w = ui.available_width();
            let layout = self.compute_layout(available_w, pages_count);

            // Tick async search
            self.tick_search(&ctx, &layout);

            let mut scroll = egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .scroll_bar_visibility(
                    egui::scroll_area::ScrollBarVisibility::VisibleWhenNeeded,
                );

            if let Some(target) = self.target_scroll_y {
                scroll = scroll.vertical_scroll_offset(target);
            }

            let scroll_output = scroll.show_rows(
                ui,
                layout.row_height,
                layout.row_count,
                |ui, row_range| {
                    let pixels_per_point = ctx.pixels_per_point();
                    let target_render_width =
                        (layout.page_width * pixels_per_point * 1.5)
                            .round()
                            .max(1000.0) as u32;

                    // Priority rendering for visible pages
                    let (start_p, end_p) = if layout.is_dual {
                        (
                            row_range.start * 2,
                            (row_range.end * 2).min(pages_count),
                        )
                    } else {
                        (row_range.start, row_range.end.min(pages_count))
                    };

                    for p in start_p..end_p {
                        self.request_render(p, target_render_width, true);
                    }

                    // Pre-cache adjacent pages
                    let precache_count = if layout.is_dual { 4 } else { 2 };
                    for p in end_p..(end_p + precache_count).min(pages_count) {
                        self.request_render(p, target_render_width, false);
                    }
                    if start_p > 0 {
                        let prev_start = start_p.saturating_sub(precache_count);
                        for p in prev_start..start_p {
                            self.request_render(p, target_render_width, false);
                        }
                    }

                    // LRU eviction to keep memory bounded
                    if self.page_textures.len() > 30 {
                        let mid_page = (start_p + end_p) as f32 * 0.5;
                        let mut entries: Vec<(usize, f32)> = self
                            .page_textures
                            .keys()
                            .map(|&idx| (idx, (idx as f32 - mid_page).abs()))
                            .collect();
                        entries.sort_by(|a, b| {
                            b.1.partial_cmp(&a.1)
                                .unwrap_or(std::cmp::Ordering::Equal)
                        });
                        for (farthest_idx, _) in entries
                            .iter()
                            .take(self.page_textures.len().saturating_sub(22))
                        {
                            self.page_textures.remove(farthest_idx);
                        }
                    }

                    // Render rows
                    for row_idx in row_range {
                        let pages_in_this_row: Vec<usize> = if layout.is_dual {
                            let p1 = row_idx * 2;
                            let p2 = row_idx * 2 + 1;
                            if p2 < pages_count {
                                vec![p1, p2]
                            } else {
                                vec![p1]
                            }
                        } else {
                            vec![row_idx]
                        };

                        ui.horizontal(|ui| {
                            if layout.margin > 0.0 {
                                ui.add_space(layout.margin);
                            }

                            for (col_idx, &page_idx) in
                                pages_in_this_row.iter().enumerate()
                            {
                                if col_idx > 0 {
                                    ui.add_space(layout.gap);
                                }

                                let (page_rect, response) =
                                    ui.allocate_exact_size(
                                        Vec2::new(
                                            layout.page_width,
                                            layout.page_height,
                                        ),
                                        Sense::click_and_drag(),
                                    );

                                if response.hovered() {
                                    ui.output_mut(|o| {
                                        o.cursor_icon = egui::CursorIcon::Text
                                    });
                                }

                                // Page background
                                let page_bg = if self.dark_mode {
                                    Color32::from_rgb(20, 20, 24)
                                } else {
                                    Color32::WHITE
                                };
                                let page_painter = ui.painter();
                                page_painter.rect_filled(
                                    page_rect,
                                    CornerRadius::ZERO,
                                    page_bg,
                                );

                                // Page texture
                                if let Some((texture, _)) =
                                    self.page_textures.get(&page_idx)
                                {
                                    page_painter.image(
                                        texture.id(),
                                        page_rect,
                                        Rect::from_min_max(
                                            Pos2::new(0.0, 0.0),
                                            Pos2::new(1.0, 1.0),
                                        ),
                                        Color32::WHITE,
                                    );
                                }

                                // Search match highlights
                                let scale =
                                    page_rect.width() / layout.w_pt;
                                for (m_idx, m) in
                                    self.search_matches.iter().enumerate()
                                {
                                    if m.page_idx == page_idx {
                                        let is_current =
                                            m_idx == self.current_match_idx;
                                        let fill_color = if is_current {
                                            Color32::from_rgba_unmultiplied(
                                                255, 140, 0, 170,
                                            )
                                        } else {
                                            Color32::from_rgba_unmultiplied(
                                                255, 210, 0, 90,
                                            )
                                        };
                                        let stroke = if is_current {
                                            Stroke::new(
                                                1.5,
                                                Color32::from_rgb(
                                                    255, 240, 120,
                                                ),
                                            )
                                        } else {
                                            Stroke::NONE
                                        };

                                        for r in &m.rects {
                                            let x1 = page_rect.min.x
                                                + r[0] * scale;
                                            let x2 = page_rect.min.x
                                                + r[2] * scale;
                                            let y1 = page_rect.min.y
                                                + (layout.h_pt - r[3])
                                                    * scale;
                                            let y2 = page_rect.min.y
                                                + (layout.h_pt - r[1])
                                                    * scale;
                                            let match_rect =
                                                Rect::from_min_max(
                                                    Pos2::new(x1, y1),
                                                    Pos2::new(x2, y2),
                                                )
                                                .expand(1.2);
                                            page_painter.rect(
                                                match_rect,
                                                CornerRadius::same(2),
                                                fill_color,
                                                stroke,
                                                StrokeKind::Outside,
                                            );
                                        }
                                    }
                                }

                                // Text interaction & selection
                                let is_interacting = response.drag_started()
                                    || response.dragged()
                                    || response.double_clicked()
                                    || self.selected_page == Some(page_idx);

                                if is_interacting
                                    && !self
                                        .page_text_cache
                                        .contains_key(&page_idx)
                                {
                                    let info = doc.get_page_text(page_idx);
                                    self.page_text_cache
                                        .insert(page_idx, Arc::new(info));
                                }

                                // Arc::clone is cheap (atomic increment) vs
                                // the old .cloned() which deep-copied all
                                // character data. Methods need &mut self
                                // while page_info borrows page_text_cache.
                                if let Some(page_info) = self
                                    .page_text_cache
                                    .get(&page_idx)
                                    .map(Arc::clone)
                                {
                                    self.handle_selection_interaction(
                                        &response,
                                        &page_info,
                                        page_idx,
                                        page_rect,
                                        window_rect,
                                        &mut should_copy,
                                    );
                                    self.draw_selection_highlights(
                                        page_painter,
                                        &page_info,
                                        page_idx,
                                        page_rect,
                                    );
                                } else if response.clicked()
                                    && !response.drag_stopped()
                                {
                                    self.selected_page = None;
                                    self.selection_start_char = None;
                                    self.selection_end_char = None;
                                }
                            }
                        });

                        // 1px page separator
                        if !layout.is_dual && row_idx + 1 < pages_count {
                            let sep_color = if self.dark_mode {
                                Color32::from_rgba_unmultiplied(
                                    255, 255, 255, 18,
                                )
                            } else {
                                Color32::from_rgba_unmultiplied(0, 0, 0, 25)
                            };
                            ui.painter().line_segment(
                                [
                                    Pos2::new(0.0, ui.cursor().top()),
                                    Pos2::new(available_w, ui.cursor().top()),
                                ],
                                Stroke::new(1.0, sep_color),
                            );
                        }
                    }
                },
            );

            // Smooth inertial scrolling
            let actual_scroll = scroll_output.state.offset.y;
            let wheel_delta = ctx.input(|i| i.smooth_scroll_delta.y);
            self.handle_scroll(
                &ctx,
                actual_scroll,
                wheel_delta,
                input.ctrl_down,
                window_rect,
                &layout,
            );

            // Track position & session history
            let ghost_text = self.track_scroll_position(
                actual_scroll,
                wheel_delta,
                &layout,
                pages_count,
                window_rect,
            );

            // Ghost indicators (page number + zoom)
            self.draw_ghost_indicators(ui, &ctx, window_rect, &ghost_text);

            // Overlays
            self.draw_goto_modal(ui, &ctx, window_rect, pages_count, &layout);
            self.draw_toc_modal(ui, &ctx, window_rect, &layout);
            self.draw_search_bar(ui, &ctx, window_rect, &layout);
        } else {
            self.draw_dropzone(ui, &ctx, window_rect);
        }

        // 4. Window chrome (drag, close, resize, grip, border)
        self.draw_chrome(ui, &ctx, window_rect);

        // 5. Execute copy if requested
        if should_copy {
            self.copy_selection(&ctx);
        }

        // 6. Toast notification
        self.draw_toast(ui, &ctx, window_rect);
    }
}
