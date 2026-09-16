use crate::pdf_engine::LoadedDoc;
use eframe::egui;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex};
use std::time::Duration;

pub(crate) struct RenderTask {
    pub page_idx: usize,
    pub target_width: u32,
    pub dark_mode: bool,
    pub rotation: u8,
    pub doc: Arc<LoadedDoc>,
}

/// Result from the background render pool — either a successfully rendered
/// page or an error message so the UI can clean up `pending_renders`.
pub(crate) enum RenderResult {
    Success {
        page_idx: usize,
        target_width: u32,
        dark_mode: bool,
        rotation: u8,
        image: image::RgbaImage,
    },
    Error {
        page_idx: usize,
        message: String,
    },
}

/// Spawn a pool of `worker_count` background render threads sharing the same
/// queue. Each worker shuts down cleanly when `shutdown` is set to `true`.
pub(crate) fn spawn_render_pool(
    queue: Arc<(Mutex<VecDeque<RenderTask>>, Condvar)>,
    tx: mpsc::Sender<RenderResult>,
    shutdown: Arc<AtomicBool>,
    worker_count: usize,
) {
    for i in 0..worker_count {
        let queue = Arc::clone(&queue);
        let tx = tx.clone();
        let shutdown = Arc::clone(&shutdown);

        std::thread::Builder::new()
            .name(format!("aeropdf-renderer-{}", i))
            .spawn(move || {
                let (lock, cvar) = &*queue;
                loop {
                    let task = {
                        let mut q = lock.lock().unwrap();
                        loop {
                            if shutdown.load(Ordering::Relaxed) {
                                return;
                            }
                            if let Some(task) = q.pop_front() {
                                break task;
                            }
                            let (new_q, _) = cvar
                                .wait_timeout(q, Duration::from_millis(200))
                                .unwrap();
                            q = new_q;
                        }
                    };

                    let page_idx = task.page_idx;
                    match task.doc.render_page(
                        task.page_idx,
                        task.target_width,
                        task.rotation,
                    ) {
                        Ok(mut img) => {
                            if task.dark_mode {
                                for p in img.chunks_exact_mut(4) {
                                    p[0] = 255 - p[0];
                                    p[1] = 255 - p[1];
                                    p[2] = 255 - p[2];
                                    p[0] = (p[0] as f32 * 0.88 + 20.0) as u8;
                                    p[1] = (p[1] as f32 * 0.88 + 20.0) as u8;
                                    p[2] = (p[2] as f32 * 0.88 + 24.0) as u8;
                                }
                            }
                            let _ = tx.send(RenderResult::Success {
                                page_idx: task.page_idx,
                                target_width: task.target_width,
                                dark_mode: task.dark_mode,
                                rotation: task.rotation,
                                image: img,
                            });
                        }
                        Err(msg) => {
                            let _ = tx.send(RenderResult::Error {
                                page_idx,
                                message: msg,
                            });
                        }
                    }
                }
            })
            .expect("Failed to spawn render thread");
    }
}

impl super::AeroPdfApp {
    /// Enqueue a page for background rendering if it's not already rendered
    /// at a compatible resolution.
    pub(crate) fn request_render(
        &mut self,
        page_idx: usize,
        target_width: u32,
        high_priority: bool,
    ) {
        if let Some((_, cur_w)) = self.page_textures.get(&page_idx) {
            if (*cur_w as f32 - target_width as f32).abs() <= target_width as f32 * 0.25 {
                return;
            }
        }
        if self.pending_renders.contains(&page_idx) {
            return;
        }
        if let Some(doc) = &self.doc {
            self.pending_renders.insert(page_idx);
            let task = RenderTask {
                page_idx,
                target_width,
                dark_mode: self.dark_mode,
                rotation: self.rotation,
                doc: Arc::clone(doc),
            };
            let (lock, cvar) = &*self.render_queue;
            let mut q = lock.lock().unwrap();
            if high_priority {
                q.push_front(task);
            } else if q.len() < 12 {
                q.push_back(task);
            } else {
                self.pending_renders.remove(&page_idx);
            }
            cvar.notify_one();
        }
    }

    /// Drain completed render results from the background pool and upload
    /// textures to the GPU. Errors are cleaned up silently (page will be
    /// retried on next scroll).
    pub(crate) fn drain_render_results(&mut self, ctx: &egui::Context) {
        while let Ok(result) = self.render_rx.try_recv() {
            match result {
                RenderResult::Success {
                    page_idx,
                    target_width,
                    dark_mode,
                    rotation,
                    image,
                } => {
                    self.pending_renders.remove(&page_idx);
                    if dark_mode == self.dark_mode && rotation == self.rotation {
                        let size =
                            [image.width() as usize, image.height() as usize];
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            size,
                            image.as_raw(),
                        );
                        let texture = ctx.load_texture(
                            format!(
                                "page_{}_{}_{}", page_idx,
                                if dark_mode { "dark" } else { "light" },
                                rotation
                            ),
                            color_image,
                            egui::TextureOptions::LINEAR,
                        );
                        self.page_textures
                            .insert(page_idx, (texture, target_width));
                        ctx.request_repaint();
                    }
                }
                RenderResult::Error { page_idx, message } => {
                    self.pending_renders.remove(&page_idx);
                    eprintln!(
                        "[aeropdf] Render error on page {}: {}",
                        page_idx, message
                    );
                }
            }
        }
    }
}
