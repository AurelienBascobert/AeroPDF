#[cfg(test)]
mod tests {
    use aeropdf::pdf_engine::LoadedDoc;
    use std::path::Path;

    #[test]
    fn test_load_sample_pdf() {
        let path = Path::new("sample.pdf");
        assert!(path.exists(), "sample.pdf must exist");

        let doc = LoadedDoc::open(path).expect("Should load sample.pdf successfully");
        assert_eq!(doc.page_count, 1, "sample.pdf should have 1 page");

        let page_info = doc.get_page_text(0);
        assert!(page_info.width_pt > 0.0);
        assert!(page_info.height_pt > 0.0);
        assert!(!page_info.chars.is_empty(), "Page should contain text characters");

        // Verify text content
        let all_text: String = page_info.chars.iter().map(|c| c.c).collect();
        println!("Extracted text: {}", all_text);
        assert!(all_text.contains("Dummy PDF file"), "Should contain Dummy PDF file");

        // Test rendering to image
        let img = doc.render_page(0, 800, 0).expect("Should render page to image");
        assert_eq!(img.width(), 800);
        assert!(img.height() > 0);
    }
}
