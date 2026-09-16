use eframe::egui::{self, Rect};
use std::time::Instant;

/// Result of keyboard / input handling for the current frame.
pub(crate) struct InputResult {
    /// If `true`, the caller should `return` immediately (e.g. Escape closed
    /// a modal or the window).
    pub should_return: bool,
    /// If `true`, the user requested a copy action (Ctrl+C / Ctrl+Ins / etc.).
    pub should_copy: bool,
    /// Whether the Ctrl (or Cmd) modifier is held.
    pub ctrl_down: bool,
}

impl super::AeroPdfApp {
    /// Process file drag-and-drop from the OS.
    pub(crate) fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        for file in dropped_files {
            let path = file.path();
            if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("pdf"))
            {
                self.load_file(path);
                break;
            }
        }
    }

    /// Process all keyboard shortcuts and modifier-key states. Returns an
    /// [`InputResult`] describing what the main loop should do next.
    pub(crate) fn handle_keyboard_shortcuts(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        window_rect: Rect,
    ) -> InputResult {
        let mut result = InputResult {
            should_return: false,
            should_copy: false,
            ctrl_down: false,
        };

        let is_typing = self.is_goto_open
            || self.is_search_open
            || self.is_toc_open
            || ctx.egui_wants_keyboard_input();

        let ctrl_down = ctx.input(|i| i.modifiers.ctrl || i.modifiers.command);
        let shift_down = ctx.input(|i| i.modifiers.shift);
        result.ctrl_down = ctrl_down;

        // ── Escape hierarchy ────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            if self.is_search_open {
                self.is_search_open = false;
                self.search_matches.clear();
                result.should_return = true;
                return result;
            }
            if self.is_toc_open {
                self.is_toc_open = false;
                result.should_return = true;
                return result;
            }
            if self.is_goto_open {
                self.is_goto_open = false;
                result.should_return = true;
                return result;
            }
            if self.is_fullscreen {
                self.is_fullscreen = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(false));
                result.should_return = true;
                return result;
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            result.should_return = true;
            return result;
        }

        // Q = quit (only when not typing)
        if !is_typing && ctx.input(|i| i.key_pressed(egui::Key::Q)) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            result.should_return = true;
            return result;
        }

        // ── F11: Plein Écran ────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            self.is_fullscreen = !self.is_fullscreen;
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(self.is_fullscreen));
            self.show_toast(if self.is_fullscreen {
                "Plein écran activé".into()
            } else {
                "Plein écran désactivé".into()
            });
        }

        // ── Ctrl+I / N: Mode Nuit ──────────────────────────────────
        let n_pressed = !is_typing && ctx.input(|i| i.key_pressed(egui::Key::N));
        let ctrl_i_pressed = ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::I));
        if n_pressed || ctrl_i_pressed {
            self.dark_mode = !self.dark_mode;
            self.page_textures.clear();
            {
                let (lock, _) = &*self.render_queue;
                lock.lock().unwrap().clear();
            }
            self.pending_renders.clear();
            self.show_toast(if self.dark_mode {
                "Mode Nuit activé".into()
            } else {
                "Mode Clair activé".into()
            });
            ctx.request_repaint();
        }

        // ── Ctrl+G: Saut à la page ─────────────────────────────────
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::G)) {
            self.is_goto_open = !self.is_goto_open;
            if self.is_goto_open {
                if let Some(doc) = &self.doc {
                    let (raw_w, raw_h) = doc.default_page_size;
                    let (w, h) = if self.rotation % 2 != 0 {
                        (raw_h, raw_w)
                    } else {
                        (raw_w, raw_h)
                    };
                    let base_w = ui.available_width();
                    let page_width = (base_w * self.zoom).round();
                    let page_height = (h * (page_width / w)).round();
                    let row_height = page_height + 1.0;
                    let cur_idx = ((self.last_known_scroll_y
                        + window_rect.height() * 0.3)
                        / row_height)
                        .floor() as usize;
                    self.goto_page_input = (cur_idx + 1).to_string();
                }
            }
        }

        // ── Ctrl+F: Recherche textuelle ─────────────────────────────
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::F)) {
            self.is_search_open = !self.is_search_open;
            if !self.is_search_open {
                self.search_matches.clear();
            }
            ctx.request_repaint();
        }

        // ── Ctrl+T / Ctrl+B: Sommaire / Table des matières ─────────
        if ctrl_down
            && ctx.input(|i| i.key_pressed(egui::Key::T) || i.key_pressed(egui::Key::B))
        {
            self.is_toc_open = !self.is_toc_open;
            if self.is_toc_open && self.toc_items.is_none() {
                if let Some(doc) = &self.doc {
                    self.toc_items = Some(doc.get_bookmarks());
                }
            }
            ctx.request_repaint();
        }

        // ── Ctrl+D: Mode Double Page ────────────────────────────────
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::D)) {
            self.dual_page_mode = !self.dual_page_mode;
            self.show_toast(if self.dual_page_mode {
                "Mode Double Page : Activé".into()
            } else {
                "Mode Page Simple : Activé".into()
            });
            ctx.request_repaint();
        }

        // ── R / Ctrl+R: Rotation ────────────────────────────────────
        let r_pressed = !is_typing && ctx.input(|i| i.key_pressed(egui::Key::R));
        let ctrl_r_pressed = ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::R));
        if r_pressed || ctrl_r_pressed {
            self.cycle_rotation(ctx);
        }

        // ── Ctrl+O: Ouvrir un fichier ───────────────────────────────
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::O)) {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("PDF Files", &["pdf"])
                .pick_file()
            {
                self.load_file(&path);
            }
        }

        // ── Ctrl+P: Imprimer ────────────────────────────────────────
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::P)) {
            if let Some(doc) = &self.doc {
                let path_str = doc.path.to_string_lossy().to_string();
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    let _ = std::process::Command::new("powershell")
                        .args([
                            "-NoProfile",
                            "-Command",
                            &format!(
                                "Start-Process -FilePath '{}' -Verb Print",
                                path_str
                            ),
                        ])
                        .creation_flags(CREATE_NO_WINDOW)
                        .spawn();
                }
                #[cfg(not(windows))]
                {
                    let _ = std::process::Command::new("lp")
                        .arg(&path_str)
                        .spawn();
                }
                self.show_toast("Impression lancée...".into());
            } else {
                self.show_toast("Aucun document à imprimer".into());
            }
        }

        // ── Copy detection ──────────────────────────────────────────
        result.should_copy = ctx.input(|i| {
            i.events.iter().any(|e| matches!(e, egui::Event::Copy))
                || (ctrl_down
                    && (i.key_pressed(egui::Key::C)
                        || i.raw.events.iter().any(|e| match e {
                            egui::Event::Key {
                                key: egui::Key::C,
                                pressed: true,
                                ..
                            } => true,
                            _ => false,
                        })))
                || (ctrl_down && i.key_pressed(egui::Key::Insert))
        });

        // ── Zoom shortcuts ──────────────────────────────────────────
        let scroll_delta = ctx.input(|i| i.smooth_scroll_delta);
        if ctrl_down && scroll_delta.y != 0.0 {
            let old_zoom = self.zoom;
            let factor = if scroll_delta.y > 0.0 { 1.10 } else { 0.90 };
            let new_zoom = (self.zoom * factor).clamp(0.5, 4.0);
            if (new_zoom - old_zoom).abs() > 0.001 {
                self.zoom = new_zoom;
                self.last_zoom_change = Instant::now();
                if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
                    let mouse_y = pos.y;
                    let content_y = self.last_known_scroll_y + mouse_y;
                    let scaled_y = content_y * (new_zoom / old_zoom);
                    self.target_scroll_y = Some((scaled_y - mouse_y).max(0.0));
                }
                ctx.request_repaint();
            }
        }
        if ctrl_down
            && ctx.input(|i| {
                i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)
            })
        {
            self.zoom = (self.zoom * 1.15).clamp(0.5, 4.0);
            self.last_zoom_change = Instant::now();
            ctx.request_repaint();
        }
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::Minus)) {
            self.zoom = (self.zoom * 0.85).clamp(0.5, 4.0);
            self.last_zoom_change = Instant::now();
            ctx.request_repaint();
        }
        if ctrl_down && ctx.input(|i| i.key_pressed(egui::Key::Num0)) {
            self.zoom = 1.0;
            self.last_zoom_change = Instant::now();
            self.show_toast("Zoom : 100% (Ajusté)".into());
            ctx.request_repaint();
        }

        // ── Keyboard navigation (not when typing) ───────────────────
        if !is_typing {
            let window_h = window_rect.height();
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                if shift_down {
                    self.target_scroll_y =
                        Some((self.last_known_scroll_y - window_h * 0.85).max(0.0));
                } else {
                    self.target_scroll_y =
                        Some(self.last_known_scroll_y + window_h * 0.85);
                }
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            } else if ctx.input(|i| {
                i.key_pressed(egui::Key::J) || i.key_pressed(egui::Key::ArrowDown)
            }) {
                self.target_scroll_y = Some(self.last_known_scroll_y + 80.0);
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            } else if ctx.input(|i| {
                i.key_pressed(egui::Key::K) || i.key_pressed(egui::Key::ArrowUp)
            }) {
                self.target_scroll_y =
                    Some((self.last_known_scroll_y - 80.0).max(0.0));
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            } else if ctx.input(|i| i.key_pressed(egui::Key::PageDown)) {
                self.target_scroll_y =
                    Some(self.last_known_scroll_y + window_h * 0.9);
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            } else if ctx.input(|i| i.key_pressed(egui::Key::PageUp)) {
                self.target_scroll_y =
                    Some((self.last_known_scroll_y - window_h * 0.9).max(0.0));
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            } else if ctx.input(|i| i.key_pressed(egui::Key::Home)) {
                self.target_scroll_y = Some(0.0);
                self.last_scroll_time = Instant::now();
                ctx.request_repaint();
            }
        }

        // ── Window drag: Middle-click or Alt+Click ──────────────────
        let alt_down = ctx.input(|i| i.modifiers.alt);
        if ctx.input(|i| {
            i.pointer.button_down(egui::PointerButton::Middle)
                || (alt_down && i.pointer.primary_down())
        }) {
            ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        result
    }
}
