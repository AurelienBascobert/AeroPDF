use aeropdf::pdf_engine::get_pdfium;
use std::path::Path;
use std::time::Instant;

#[test]
fn test_bench_large_pdf() {
    let pdf_path = Path::new(r"C:\Users\aureb\Downloads\325462-sdm-vol-1-2abcd-3abcd-4.pdf");
    if !pdf_path.exists() {
        eprintln!("File not found: {:?}", pdf_path);
        return;
    }

    println!("=== TEST DE PERFORMANCE SUR GROS PDF ===");
    println!("Fichier: {:?}", pdf_path.file_name().unwrap());

    let t0 = Instant::now();
    let pdfium = get_pdfium().expect("Init pdfium");
    println!("1. Initialisation PDFium: {:.2?}", t0.elapsed());

    let t1 = Instant::now();
    let doc = pdfium.load_pdf_from_file(pdf_path.to_str().unwrap(), None).expect("Load doc");
    let load_time = t1.elapsed();
    println!("2. Chargement document brut (load_pdf_from_file): {:.2?}", load_time);

    let t2 = Instant::now();
    let page_count = doc.pages().len();
    println!("3. Nombre total de pages: {} (détecté en {:.2?})", page_count, t2.elapsed());

    let t_loaded = Instant::now();
    let _loaded_doc = aeropdf::pdf_engine::LoadedDoc::open(pdf_path);
    println!("=== Temps d'ouverture complet LoadedDoc::open (avec extraction): {:.2?}", t_loaded.elapsed());

    // Measure rendering page 0
    let page0 = doc.pages().get(0).expect("Page 0");
    let t3 = Instant::now();
    let render_config_ultra = pdfium_render::prelude::PdfRenderConfig::new()
        .set_target_width(1600)
        .use_lcd_text_rendering(true)
        .use_print_quality(true)
        .set_text_smoothing(true)
        .set_image_smoothing(true)
        .set_path_smoothing(true);

    let bitmap = page0.render_with_config(&render_config_ultra).expect("Render page 0");
    let _img = bitmap.as_image().expect("Image").to_rgba8();
    println!("4. Rendu de la page 1 Ultra-HD (1600px + LCD text + Print quality): {:.2?}", t3.elapsed());

    // Measure text extraction on page 0
    let t4 = Instant::now();
    let text0 = page0.text().expect("Text page 0");
    let char_count = text0.chars().len();
    println!("5. Extraction du texte de la page 1 ({} caractères): {:.2?}", char_count, t4.elapsed());

    // Measure rendering page 500 (deep in the document)
    if page_count > 500 {
        let page500 = doc.pages().get(500).expect("Page 500");
        let t5 = Instant::now();
        let bitmap500 = page500.render_with_config(&render_config_ultra).expect("Render page 500");
        let _img500 = bitmap500.as_image().expect("Image").to_rgba8();
        println!("6. Rendu aléatoire de la page 501 Ultra-HD: {:.2?}", t5.elapsed());

        let t6 = Instant::now();
        let text500 = page500.text().expect("Text page 500");
        println!("7. Extraction texte page 501 ({} caractères): {:.2?}", text500.chars().len(), t6.elapsed());
    }
}
