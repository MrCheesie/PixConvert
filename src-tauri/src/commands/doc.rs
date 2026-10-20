//! Phase 4 — Doc Studio backend (pandoc for conversion, ghostscript for PDF size).

use std::path::PathBuf;

use tauri::{AppHandle, Runtime};

use super::common::run_tool_named;
use super::{
    extension_of, find_on_path, normalize_format, output_path, validate_input, SUFFIX_COMPRESS,
    SUFFIX_CONVERT,
};

/// Extension → pandoc reader name. Anything not listed here is rejected with a
/// readable message instead of handing pandoc an unknown format.
fn pandoc_reader(ext: &str) -> Option<&'static str> {
    match ext {
        "md" | "markdown" | "mdown" | "mkd" => Some("markdown"),
        "txt" => Some("plain"),
        "html" | "htm" => Some("html"),
        "docx" => Some("docx"),
        "odt" => Some("odt"),
        "rtf" => Some("rtf"),
        "pptx" => Some("pptx"),
        "epub" => Some("epub"),
        "tex" | "latex" => Some("latex"),
        "rst" => Some("rst"),
        "org" => Some("org"),
        "ipynb" => Some("ipynb"),
        "csv" => Some("csv"),
        _ => None,
    }
}

/// Extension → pandoc writer name (+ the file extension we write).
fn pandoc_writer(format: &str) -> Option<(&'static str, &'static str)> {
    match format {
        "md" | "markdown" => Some(("markdown", "md")),
        "txt" => Some(("plain", "txt")),
        "html" | "htm" => Some(("html", "html")),
        "docx" => Some(("docx", "docx")),
        "odt" => Some(("odt", "odt")),
        "rtf" => Some(("rtf", "rtf")),
        "pptx" => Some(("pptx", "pptx")),
        "epub" => Some(("epub", "epub")),
        "tex" | "latex" => Some(("latex", "tex")),
        "pdf" => Some(("pdf", "pdf")),
        _ => None,
    }
}

/// PDF is not a pandoc writer: it shells out to a PDF engine via `-o`.
const PDF_ENGINES: &[&str] = &["pdflatex", "xelatex", "lualatex", "typst", "wkhtmltopdf", "weasyprint"];

fn find_pdf_engine() -> Option<&'static str> {
    PDF_ENGINES.iter().copied().find(|name| find_on_path(name).is_some())
}

#[tauri::command]
pub fn convert_doc<R: Runtime>(
    app: AppHandle<R>,
    input_path: String,
    output_format: String,
) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    let input_ext = extension_of(&input);
    let reader = pandoc_reader(&input_ext).ok_or_else(|| {
        format!(
            "Cannot read .{} files. Supported inputs: md, txt, html, docx, odt, rtf, pptx, epub, tex, rst, org, ipynb, csv.",
            if input_ext.is_empty() { "?" } else { &input_ext }
        )
    })?;

    let format = normalize_format(&output_format);
    let (writer, out_ext) = pandoc_writer(&format).ok_or_else(|| {
        format!(
            "Cannot write .{} files. Supported outputs: md, txt, html, docx, odt, rtf, pptx, epub, pdf, tex.",
            format
        )
    })?;

    let out = output_path(&input, SUFFIX_CONVERT, out_ext)?;
    run_pandoc(&app, &input, reader, writer, &out)?;
    Ok(done_message("Converted", &out))
}

#[tauri::command]
pub fn doc_to_pdf<R: Runtime>(app: AppHandle<R>, input_path: String) -> Result<String, String> {
    convert_doc(app, input_path, "pdf".to_string())
}

/// Shrink a PDF with ghostscript. Returns a readable error when gs is absent.
#[tauri::command]
pub fn compress_doc<R: Runtime>(app: AppHandle<R>, input_path: String) -> Result<String, String> {
    let input = PathBuf::from(&input_path);
    validate_input(&input)?;

    if find_on_path("gs").is_none() {
        return Err(
            "Ghostscript (gs) is not installed, so PDF compression is unavailable. Install it (brew install ghostscript) and try again."
                .to_string(),
        );
    }

    let out = output_path(&input, SUFFIX_COMPRESS, "pdf")?;
    let args = vec![
        "-sDEVICE=pdfwrite".to_string(),
        "-dCompatibilityLevel=1.4".to_string(),
        "-dPDFSETTINGS=/ebook".to_string(),
        "-dNOPAUSE".to_string(),
        "-dQUIET".to_string(),
        "-dBATCH".to_string(),
        format!("-sOutputFile={}", out.display()),
        input.display().to_string(),
    ];
    run_tool_named(&app, "gs", &args)?;
    Ok(done_message("Compressed PDF", &out))
}

fn run_pandoc<R: Runtime>(
    app: &AppHandle<R>,
    input: &std::path::Path,
    reader: &str,
    writer: &str,
    out: &std::path::Path,
) -> Result<(), String> {
    let mut args = vec![
        "-f".to_string(),
        reader.to_string(),
        "-t".to_string(),
        writer.to_string(),
        "-o".to_string(),
        out.display().to_string(),
    ];

    if writer == "pdf" {
        let engine = find_pdf_engine().ok_or_else(|| {
            format!(
                "PDF output needs a LaTeX engine or typst. Install one of: {} (pandoc will use it automatically).",
                PDF_ENGINES.join(", ")
            )
        })?;
        args.extend(["--pdf-engine".to_string(), engine.to_string()]);
    }

    args.push(input.display().to_string());
    run_tool_named(app, "pandoc", &args).map(|_| ())
}

fn done_message(verb: &str, out: &std::path::Path) -> String {
    let size = std::fs::metadata(out).map(|meta| meta.len()).unwrap_or(0);
    format!("{}: {} ({} bytes)", verb, out.display(), size)
}
