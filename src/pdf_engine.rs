use image::RgbaImage;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// SAFETY: PdfDocument from pdfium-render is thread-safe as long as each page
// is accessed from a single thread at a time. Our architecture ensures this
// via the render queue which serializes page access.
const _: () = {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    fn _check() {
        _assert_send::<LoadedDoc>();
        _assert_sync::<LoadedDoc>();
    }
};

static PDFIUM: OnceLock<Pdfium> = OnceLock::new();

pub fn get_pdfium() -> Result<&'static Pdfium, String> {
    if let Some(p) = PDFIUM.get() {
        return Ok(p);
    }

    let mut candidates = Vec::new();

    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            candidates.push(exe_dir.join("pdfium.dll"));
        }
    }
    candidates.push(PathBuf::from("pdfium.dll"));
    candidates.push(PathBuf::from("../pdfium.dll"));
    candidates.push(PathBuf::from("../../pdfium.dll"));

    for path in &candidates {
        if path.exists() {
            if let Ok(bindings) = Pdfium::bind_to_library(path.to_str().unwrap_or_default()) {
                let _ = PDFIUM.set(Pdfium::new(bindings));
                return PDFIUM.get().ok_or_else(|| "Failed to set PDFium".to_string());
            }
        }
    }

    if let Ok(bindings) = Pdfium::bind_to_system_library() {
        let _ = PDFIUM.set(Pdfium::new(bindings));
        return PDFIUM.get().ok_or_else(|| "Failed to set PDFium".to_string());
    }

    Err("Impossible de trouver ou charger pdfium.dll. Assurez-vous que pdfium.dll est dans le même dossier que l'exécutable.".to_string())
}

#[derive(Clone, Debug)]
pub struct CharInfo {
    pub c: char,
    /// PDF point coordinates: [left, bottom, right, top]
    pub rect: [f32; 4],
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PageInfo {
    pub index: usize,
    pub width_pt: f32,
    pub height_pt: f32,
    pub chars: Vec<CharInfo>,
}

#[derive(Clone, Debug)]
pub struct BookmarkNode {
    pub title: String,
    pub page_index: usize,
    pub level: usize,
}

#[allow(dead_code)]
pub struct LoadedDoc {
    pub path: PathBuf,
    pub document: PdfDocument<'static>,
    pub page_count: usize,
    pub default_page_size: (f32, f32),
}

impl LoadedDoc {
    pub fn open(path: &Path) -> Result<Self, String> {
        let pdfium = get_pdfium()?;
        let document = pdfium
            .load_pdf_from_file(path.to_str().unwrap_or_default(), None)
            .map_err(|e| format!("Erreur d'ouverture du document: {:?}", e))?;

        let page_count = document.pages().len() as usize;
        let (default_width, default_height) = if let Ok(first_page) = document.pages().get(0) {
            (first_page.width().value, first_page.height().value)
        } else {
            (595.0, 842.0)
        };

        Ok(Self {
            path: path.to_path_buf(),
            document,
            page_count,
            default_page_size: (default_width, default_height),
        })
    }

    pub fn get_page_size(&self, page_index: usize) -> (f32, f32) {
        if let Ok(page) = self.document.pages().get(page_index as PdfPageIndex) {
            (page.width().value, page.height().value)
        } else {
            self.default_page_size
        }
    }

    pub fn get_page_text(&self, page_index: usize) -> PageInfo {
        if let Ok(page) = self.document.pages().get(page_index as PdfPageIndex) {
            let width_pt = page.width().value;
            let height_pt = page.height().value;

            let mut chars = Vec::new();
            if let Ok(text_page) = page.text() {
                for char_item in text_page.chars().iter() {
                    if let Some(c) = char_item.unicode_char() {
                        if let Ok(rect) = char_item.tight_bounds() {
                            chars.push(CharInfo {
                                c,
                                rect: [
                                    rect.left().value,
                                    rect.bottom().value,
                                    rect.right().value,
                                    rect.top().value,
                                ],
                            });
                        } else {
                            chars.push(CharInfo {
                                c,
                                rect: [0.0, 0.0, 0.0, 0.0],
                            });
                        }
                    }
                }
            }

            PageInfo {
                index: page_index,
                width_pt,
                height_pt,
                chars,
            }
        } else {
            PageInfo {
                index: page_index,
                width_pt: self.default_page_size.0,
                height_pt: self.default_page_size.1,
                chars: Vec::new(),
            }
        }
    }

    pub fn get_bookmarks(&self) -> Vec<BookmarkNode> {
        let mut result = Vec::new();
        let bookmarks = self.document.bookmarks();
        if let Some(mut curr) = bookmarks.root() {
            loop {
                Self::traverse_bookmark(&curr, 0, &mut result);
                match curr.next_sibling() {
                    Some(sibling) => curr = sibling,
                    None => break,
                }
            }
        }
        result
    }

    fn traverse_bookmark(bm: &PdfBookmark<'_>, level: usize, out: &mut Vec<BookmarkNode>) {
        let title = bm.title().unwrap_or_else(|| "Sans titre".to_string());
        let page_index = if let Some(dest) = bm.destination() {
            dest.page_index().unwrap_or(0) as usize
        } else if let Some(action) = bm.action() {
            match action {
                PdfAction::LocalDestination(local) => {
                    if let Ok(dest) = local.destination() {
                        dest.page_index().unwrap_or(0) as usize
                    } else {
                        0
                    }
                }
                _ => 0,
            }
        } else {
            0
        };

        out.push(BookmarkNode {
            title,
            page_index,
            level,
        });

        if let Some(mut child) = bm.first_child() {
            loop {
                Self::traverse_bookmark(&child, level + 1, out);
                match child.next_sibling() {
                    Some(sibling) => child = sibling,
                    None => break,
                }
            }
        }
    }

    pub fn search_page(&self, page_index: usize, query: &str) -> Vec<Vec<[f32; 4]>> {
        let mut results = Vec::new();
        if query.trim().is_empty() {
            return results;
        }
        if let Ok(page) = self.document.pages().get(page_index as PdfPageIndex) {
            if let Ok(text_page) = page.text() {
                let options = PdfSearchOptions::new();
                if let Ok(search) = text_page.search(query, &options) {
                    while let Some(segments) = search.find_next() {
                        let mut match_rects = Vec::new();
                        for i in 0..segments.len() {
                            if let Ok(segment) = segments.get(i) {
                                let b = segment.bounds();
                                match_rects.push([
                                    b.left().value,
                                    b.bottom().value,
                                    b.right().value,
                                    b.top().value,
                                ]);
                            }
                        }
                        if !match_rects.is_empty() {
                            results.push(match_rects);
                        }
                    }
                }
            }
        }
        results
    }

    pub fn render_page(&self, page_index: usize, target_width: u32, rotation: u8) -> Result<RgbaImage, String> {
        let page = self
            .document
            .pages()
            .get(page_index as PdfPageIndex)
            .map_err(|e| format!("Erreur récupération page {}: {:?}", page_index, e))?;

        let render_config = PdfRenderConfig::new()
            .set_target_width(target_width as i32)
            .use_lcd_text_rendering(true)
            .use_print_quality(true)
            .set_text_smoothing(true)
            .set_image_smoothing(true)
            .set_path_smoothing(true);

        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| format!("Erreur rendu page {}: {:?}", page_index, e))?;

        let dyn_img = bitmap
            .as_image()
            .map_err(|e| format!("Erreur conversion image: {:?}", e))?;

        let rgba = dyn_img.to_rgba8();
        let rotated = match rotation % 4 {
            1 => image::imageops::rotate90(&rgba),
            2 => image::imageops::rotate180(&rgba),
            3 => image::imageops::rotate270(&rgba),
            _ => rgba,
        };

        Ok(rotated)
    }
}
