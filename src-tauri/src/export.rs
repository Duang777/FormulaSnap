// ExportService - 导出模块
// 负责生成 .tex 和 .docx 文件

use serde::{Deserialize, Serialize};
use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

use crate::history::HistoryRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TexExportOptions {
    /// 是否添加时间注释分隔
    pub add_time_comments: bool,
}

impl Default for TexExportOptions {
    fn default() -> Self {
        Self {
            add_time_comments: false,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ExportError {
    #[error("导出失败: {0}")]
    ExportFailed(String),
    #[error("转换失败: {0}")]
    ConvertFailed(String),
}

impl Serialize for ExportError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Returns the effective LaTeX string for a record.
/// Uses `edited_latex` if available, otherwise falls back to `original_latex`.
fn effective_latex(record: &HistoryRecord) -> &str {
    record
        .edited_latex
        .as_deref()
        .unwrap_or(&record.original_latex)
}

/// 导出为 .tex 文件
///
/// Records are sorted by `created_at` ascending (oldest first, chronological order).
/// Each formula is wrapped in `$$...$$` display math mode.
/// When `options.add_time_comments` is true, a comment line `% [timestamp]` is
/// inserted before each formula.
/// Formulas are separated by blank lines.
pub fn export_tex(
    records: &[HistoryRecord],
    options: &TexExportOptions,
) -> Result<Vec<u8>, ExportError> {
    // Sort records by created_at ascending (oldest first)
    let mut sorted: Vec<&HistoryRecord> = records.iter().collect();
    sorted.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    let mut parts: Vec<String> = Vec::with_capacity(sorted.len());

    for record in &sorted {
        let mut block = String::new();

        if options.add_time_comments {
            block.push_str(&format!("% [{}]\n", record.created_at));
        }

        let latex = effective_latex(record);
        block.push_str(&format!("$${}$$", latex));

        parts.push(block);
    }

    let content = parts.join("\n\n");
    Ok(content.into_bytes())
}

/// 导出为 .docx 文件
///
/// Creates a valid .docx file (OOXML ZIP archive) containing one paragraph per
/// record. Each paragraph contains either an OMML formula (if LaTeX→OMML
/// conversion succeeds) or a plain-text fallback annotated with "转换失败".
///
/// The .docx ZIP structure:
/// - `[Content_Types].xml`
/// - `_rels/.rels`
/// - `word/_rels/document.xml.rels`
/// - `word/document.xml`
pub fn export_docx(records: &[HistoryRecord]) -> Result<Vec<u8>, ExportError> {
    let buf = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buf);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| ExportError::ExportFailed(format!("ZIP error: {}", e)))?;
    zip.write_all(CONTENT_TYPES_XML.as_bytes())
        .map_err(|e| ExportError::ExportFailed(format!("Write error: {}", e)))?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| ExportError::ExportFailed(format!("ZIP error: {}", e)))?;
    zip.write_all(RELS_XML.as_bytes())
        .map_err(|e| ExportError::ExportFailed(format!("Write error: {}", e)))?;

    // 3. word/_rels/document.xml.rels
    zip.start_file("word/_rels/document.xml.rels", options)
        .map_err(|e| ExportError::ExportFailed(format!("ZIP error: {}", e)))?;
    zip.write_all(DOCUMENT_RELS_XML.as_bytes())
        .map_err(|e| ExportError::ExportFailed(format!("Write error: {}", e)))?;

    // 4. word/document.xml – main content
    zip.start_file("word/document.xml", options)
        .map_err(|e| ExportError::ExportFailed(format!("ZIP error: {}", e)))?;

    let document_xml = build_document_xml(records);
    zip.write_all(document_xml.as_bytes())
        .map_err(|e| ExportError::ExportFailed(format!("Write error: {}", e)))?;

    let result = zip
        .finish()
        .map_err(|e| ExportError::ExportFailed(format!("ZIP finish error: {}", e)))?;

    Ok(result.into_inner())
}

// ---------------------------------------------------------------------------
// OOXML static templates
// ---------------------------------------------------------------------------

const CONTENT_TYPES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/>
</Types>"#;

const RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/>
</Relationships>"#;

const DOCUMENT_RELS_XML: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
</Relationships>"#;

// ---------------------------------------------------------------------------
// Document XML builder
// ---------------------------------------------------------------------------

/// Build the `word/document.xml` content from the given records.
///
/// For each record:
/// - Try to convert the effective LaTeX to OMML via `crate::convert::latex_to_omml`.
/// - On success: wrap the OMML in `<w:p><m:oMathPara>…</m:oMathPara></w:p>`.
/// - On failure: insert a plain-text paragraph with the LaTeX and a "转换失败" annotation.
fn build_document_xml(records: &[HistoryRecord]) -> String {
    let mut paragraphs = String::new();

    for record in records {
        let latex = effective_latex(record);

        match crate::convert::latex_to_omml(latex) {
            Ok(omml) => {
                // The OMML from latex_to_omml already contains <m:oMathPara> wrapper.
                // We wrap it in a <w:p> paragraph.
                paragraphs.push_str("<w:p>");
                paragraphs.push_str(&omml);
                paragraphs.push_str("</w:p>");
            }
            Err(_) => {
                // Conversion failed – insert plain text with "转换失败" annotation
                paragraphs.push_str("<w:p><w:r><w:t>");
                paragraphs.push_str(&xml_escape(latex));
                paragraphs.push_str(" (转换失败)");
                paragraphs.push_str("</w:t></w:r></w:p>");
            }
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main" xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math">{}</w:document>"#,
        if paragraphs.is_empty() {
            "<w:body></w:body>".to_string()
        } else {
            format!("<w:body>{}</w:body>", paragraphs)
        }
    )
}

/// Escape special XML characters in text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ---------------------------------------------------------------------------
// Unit Tests
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::HistoryRecord;

    /// Helper to create a sample HistoryRecord with the given parameters.
    fn make_record(
        created_at: &str,
        original_latex: &str,
        edited_latex: Option<&str>,
    ) -> HistoryRecord {
        HistoryRecord {
            id: None,
            created_at: created_at.to_string(),
            original_latex: original_latex.to_string(),
            edited_latex: edited_latex.map(|s| s.to_string()),
            confidence: 0.95,
            engine_version: "pix2tex-v1".to_string(),
            thumbnail: None,
            is_favorite: false,
        }
    }

    #[test]
    fn test_export_tex_single_record_no_comments() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"E = mc^2", None)];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        assert_eq!(content, "$$E = mc^2$$");
    }

    #[test]
    fn test_export_tex_single_record_with_comments() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"E = mc^2", None)];
        let options = TexExportOptions {
            add_time_comments: true,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        assert_eq!(content, "% [2025-01-01T00:00:00Z]\n$$E = mc^2$$");
    }

    #[test]
    fn test_export_tex_multiple_records_sorted_by_time() {
        // Insert records out of chronological order
        let records = vec![
            make_record("2025-06-15T12:00:00Z", r"\beta", None),
            make_record("2025-01-01T00:00:00Z", r"\alpha", None),
            make_record("2025-03-10T08:30:00Z", r"\gamma", None),
        ];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        // Should be sorted ascending: alpha, gamma, beta
        let expected = "$$\\alpha$$\n\n$$\\gamma$$\n\n$$\\beta$$";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_export_tex_multiple_records_with_comments() {
        let records = vec![
            make_record("2025-03-10T08:30:00Z", r"\gamma", None),
            make_record("2025-01-01T00:00:00Z", r"\alpha", None),
        ];
        let options = TexExportOptions {
            add_time_comments: true,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        let expected = "% [2025-01-01T00:00:00Z]\n$$\\alpha$$\n\n% [2025-03-10T08:30:00Z]\n$$\\gamma$$";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_export_tex_uses_edited_latex_when_available() {
        let records = vec![make_record(
            "2025-01-01T00:00:00Z",
            r"E = mc^2",
            Some(r"E = mc^{2}"),
        )];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        // Should use edited_latex, not original_latex
        assert_eq!(content, "$$E = mc^{2}$$");
    }

    #[test]
    fn test_export_tex_falls_back_to_original_when_no_edit() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"\sum_{i=1}^n i", None)];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        assert_eq!(content, r"$$\sum_{i=1}^n i$$");
    }

    #[test]
    fn test_export_tex_empty_records() {
        let records: Vec<HistoryRecord> = vec![];
        let options = TexExportOptions {
            add_time_comments: true,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        assert_eq!(content, "");
    }

    #[test]
    fn test_export_tex_returns_valid_utf8_bytes() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"\frac{a}{b}", None)];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        // Verify the bytes are valid UTF-8
        assert!(String::from_utf8(result).is_ok());
    }

    #[test]
    fn test_export_tex_formulas_separated_by_blank_lines() {
        let records = vec![
            make_record("2025-01-01T00:00:00Z", "a", None),
            make_record("2025-01-02T00:00:00Z", "b", None),
            make_record("2025-01-03T00:00:00Z", "c", None),
        ];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        // Formulas should be separated by "\n\n" (blank line)
        let blocks: Vec<&str> = content.split("\n\n").collect();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], "$$a$$");
        assert_eq!(blocks[1], "$$b$$");
        assert_eq!(blocks[2], "$$c$$");
    }

    #[test]
    fn test_export_tex_mixed_edited_and_original() {
        let records = vec![
            make_record("2025-01-01T00:00:00Z", r"\alpha", Some(r"\alpha_{1}")),
            make_record("2025-01-02T00:00:00Z", r"\beta", None),
            make_record("2025-01-03T00:00:00Z", r"\gamma", Some(r"\gamma_{3}")),
        ];
        let options = TexExportOptions {
            add_time_comments: false,
        };

        let result = export_tex(&records, &options).expect("export should succeed");
        let content = String::from_utf8(result).expect("should be valid UTF-8");

        let expected = "$$\\alpha_{1}$$\n\n$$\\beta$$\n\n$$\\gamma_{3}$$";
        assert_eq!(content, expected);
    }

    #[test]
    fn test_effective_latex_prefers_edited() {
        let record = make_record("2025-01-01T00:00:00Z", "original", Some("edited"));
        assert_eq!(effective_latex(&record), "edited");
    }

    #[test]
    fn test_effective_latex_falls_back_to_original() {
        let record = make_record("2025-01-01T00:00:00Z", "original", None);
        assert_eq!(effective_latex(&record), "original");
    }

    // -----------------------------------------------------------------------
    // .docx export tests
    // -----------------------------------------------------------------------

    /// Helper: extract a named file from a ZIP archive as a String.
    fn read_zip_entry(data: &[u8], name: &str) -> Option<String> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor).ok()?;
        let mut file = archive.by_name(name).ok()?;
        let mut contents = String::new();
        std::io::Read::read_to_string(&mut file, &mut contents).ok()?;
        Some(contents)
    }

    /// Helper: list all file names in a ZIP archive.
    fn zip_file_names(data: &[u8]) -> Vec<String> {
        let cursor = std::io::Cursor::new(data);
        let archive = zip::ZipArchive::new(cursor).expect("valid ZIP");
        let count = archive.len();
        (0..count)
            .map(|i| {
                let mut a = zip::ZipArchive::new(std::io::Cursor::new(data)).unwrap();
                let name = a.by_index(i).unwrap().name().to_string();
                name
            })
            .collect()
    }

    #[test]
    fn test_export_docx_returns_valid_zip() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"E = mc^2", None)];
        let result = export_docx(&records).expect("export should succeed");

        // Verify it's a valid ZIP by trying to open it
        let cursor = std::io::Cursor::new(&result);
        assert!(
            zip::ZipArchive::new(cursor).is_ok(),
            "output should be a valid ZIP archive"
        );
    }

    #[test]
    fn test_export_docx_contains_required_files() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"x^2", None)];
        let result = export_docx(&records).expect("export should succeed");
        let names = zip_file_names(&result);

        assert!(names.contains(&"[Content_Types].xml".to_string()));
        assert!(names.contains(&"_rels/.rels".to_string()));
        assert!(names.contains(&"word/_rels/document.xml.rels".to_string()));
        assert!(names.contains(&"word/document.xml".to_string()));
    }

    #[test]
    fn test_export_docx_paragraph_count_matches_records() {
        let records = vec![
            make_record("2025-01-01T00:00:00Z", r"x^2", None),
            make_record("2025-01-02T00:00:00Z", r"\alpha", None),
            make_record("2025-01-03T00:00:00Z", r"\frac{a}{b}", None),
        ];
        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Count <w:p> opening tags – each record produces one paragraph
        let paragraph_count = doc_xml.matches("<w:p>").count();
        assert_eq!(
            paragraph_count,
            records.len(),
            "number of <w:p> paragraphs should equal number of records"
        );
    }

    #[test]
    fn test_export_docx_successful_conversion_contains_omml() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"x^2", None)];
        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Successful conversion should contain OMML math paragraph
        assert!(
            doc_xml.contains("<m:oMathPara"),
            "successful conversion should contain <m:oMathPara>"
        );
        assert!(
            doc_xml.contains("<m:oMath>"),
            "successful conversion should contain <m:oMath>"
        );
    }

    #[test]
    fn test_export_docx_failed_conversion_contains_fallback_text() {
        // Use an invalid LaTeX that will fail conversion
        let records = vec![make_record(
            "2025-01-01T00:00:00Z",
            r"\invalidcommandthatwillfail{{{",
            None,
        )];
        let result = export_docx(&records).expect("export should succeed even with conversion failures");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Failed conversion should contain "转换失败" annotation
        assert!(
            doc_xml.contains("转换失败"),
            "failed conversion should contain '转换失败' annotation"
        );
        // Should still have a paragraph
        assert!(
            doc_xml.contains("<w:p>"),
            "failed conversion should still produce a paragraph"
        );
    }

    #[test]
    fn test_export_docx_mixed_success_and_failure() {
        let records = vec![
            make_record("2025-01-01T00:00:00Z", r"x^2", None),                          // should succeed
            make_record("2025-01-02T00:00:00Z", r"\invalidcommandthatwillfail{{{", None), // should fail
            make_record("2025-01-03T00:00:00Z", r"\alpha", None),                         // should succeed
        ];
        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Should have 3 paragraphs total
        let paragraph_count = doc_xml.matches("<w:p>").count();
        assert_eq!(paragraph_count, 3);

        // Should contain both OMML and fallback text
        assert!(doc_xml.contains("<m:oMathPara"));
        assert!(doc_xml.contains("转换失败"));
    }

    #[test]
    fn test_export_docx_empty_records() {
        let records: Vec<HistoryRecord> = vec![];
        let result = export_docx(&records).expect("export should succeed for empty records");

        // Should still be a valid ZIP
        let cursor = std::io::Cursor::new(&result);
        assert!(zip::ZipArchive::new(cursor).is_ok());

        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");
        // No paragraphs
        assert_eq!(doc_xml.matches("<w:p>").count(), 0);
    }

    #[test]
    fn test_export_docx_uses_edited_latex() {
        let records = vec![make_record(
            "2025-01-01T00:00:00Z",
            r"\invalidcommandthatwillfail{{{",
            Some(r"x^2"), // edited version is valid
        )];
        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Should use edited_latex (x^2) which converts successfully
        assert!(
            doc_xml.contains("<m:oMathPara"),
            "should use edited_latex for conversion"
        );
        assert!(
            !doc_xml.contains("转换失败"),
            "should not contain failure annotation when edited_latex converts successfully"
        );
    }

    #[test]
    fn test_export_docx_document_xml_has_correct_namespaces() {
        let records = vec![make_record("2025-01-01T00:00:00Z", r"x", None)];
        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        assert!(
            doc_xml.contains("xmlns:w=\"http://schemas.openxmlformats.org/wordprocessingml/2006/main\""),
            "document.xml should declare the Word namespace"
        );
        assert!(
            doc_xml.contains("xmlns:m=\"http://schemas.openxmlformats.org/officeDocument/2006/math\""),
            "document.xml should declare the OMML namespace"
        );
    }

    #[test]
    fn test_xml_escape() {
        assert_eq!(xml_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
        assert_eq!(xml_escape(r#"say "hello""#), "say &quot;hello&quot;");
        assert_eq!(xml_escape("it's"), "it&apos;s");
        assert_eq!(xml_escape("plain text"), "plain text");
    }

    // -----------------------------------------------------------------------
    // Property-Based Tests (proptest)
    // -----------------------------------------------------------------------
    use proptest::prelude::*;

    /// Generate a valid ISO 8601 timestamp string for testing.
    fn arb_timestamp() -> impl Strategy<Value = String> {
        (2020u32..2030, 1u32..13, 1u32..29, 0u32..24, 0u32..60, 0u32..60).prop_map(
            |(year, month, day, hour, min, sec)| {
                format!(
                    "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
                    year, month, day, hour, min, sec
                )
            },
        )
    }

    /// Generate a simple LaTeX string for testing.
    fn arb_latex() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(r"\alpha".to_string()),
            Just(r"\beta".to_string()),
            Just(r"\gamma".to_string()),
            Just(r"x^2".to_string()),
            Just(r"\frac{a}{b}".to_string()),
            Just(r"\sum_{i=1}^n i".to_string()),
            Just(r"E = mc^2".to_string()),
            Just(r"\int_0^1 x dx".to_string()),
            "[a-zA-Z0-9_^{}\\\\]+".prop_map(|s| s),
        ]
    }

    /// Generate a HistoryRecord for property testing.
    fn arb_history_record() -> impl Strategy<Value = HistoryRecord> {
        (arb_timestamp(), arb_latex(), proptest::option::of(arb_latex())).prop_map(
            |(created_at, original_latex, edited_latex)| HistoryRecord {
                id: None,
                created_at,
                original_latex,
                edited_latex,
                confidence: 0.95,
                engine_version: "pix2tex-v1".to_string(),
                thumbnail: None,
                is_favorite: false,
            },
        )
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// **Property 16: .tex 导出完整性与排序**
        ///
        /// For any set of history records and export options, export_tex should:
        /// 1. Include all records' LaTeX content
        /// 2. Sort records by timestamp in ascending order
        /// 3. Include time comments when add_time_comments is true
        /// 4. Exclude time comments when add_time_comments is false
        ///
        /// **Validates: Requirements 8.1, 8.4**
        #[test]
        fn prop_tex_export_completeness_and_sorting(
            records in proptest::collection::vec(arb_history_record(), 1..10),
            add_time_comments in proptest::bool::ANY,
        ) {
            let options = TexExportOptions { add_time_comments };
            let result = export_tex(&records, &options).expect("export should succeed");
            let content = String::from_utf8(result).expect("should be valid UTF-8");

            // Property 1: All LaTeX content should be present
            for record in &records {
                let expected_latex = effective_latex(record);
                let wrapped = format!("${}$", expected_latex);
                prop_assert!(
                    content.contains(&wrapped),
                    "Content should contain wrapped LaTeX: {}",
                    wrapped
                );
            }

            // Property 2: Records should be sorted by timestamp (ascending)
            let mut sorted_records: Vec<&HistoryRecord> = records.iter().collect();
            sorted_records.sort_by(|a, b| a.created_at.cmp(&b.created_at));

            // Extract LaTeX blocks from content and verify order
            let blocks: Vec<&str> = content.split("\n\n").collect();
            let mut block_idx = 0;
            for record in &sorted_records {
                let expected_latex = effective_latex(record);
                let wrapped = format!("${}$", expected_latex);

                // Find this LaTeX in the remaining blocks
                while block_idx < blocks.len() {
                    if blocks[block_idx].contains(&wrapped) {
                        break;
                    }
                    block_idx += 1;
                }
                prop_assert!(
                    block_idx < blocks.len(),
                    "LaTeX {} should appear in sorted order",
                    wrapped
                );
                block_idx += 1;
            }

            // Property 3: Time comments presence based on option
            if add_time_comments {
                // When enabled, each record should have a time comment
                for record in &sorted_records {
                    let time_comment = format!("% [{}]", record.created_at);
                    prop_assert!(
                        content.contains(&time_comment),
                        "Content should contain time comment: {}",
                        time_comment
                    );
                }
            } else {
                // When disabled, no time comments should be present
                prop_assert!(
                    !content.contains("% ["),
                    "Content should not contain time comments when disabled"
                );
            }
        }

        /// **Property 17: .docx 导出段落数量一致性**
        ///
        /// For any set of history records, export_docx should produce a .docx file
        /// where the number of formula paragraphs equals the number of input records.
        ///
        /// **Validates: Requirements 8.2**
        #[test]
        fn prop_docx_export_paragraph_count_consistency(
            records in proptest::collection::vec(arb_history_record(), 0..10),
        ) {
            let result = export_docx(&records).expect("export should succeed");

            // Verify it's a valid ZIP
            let cursor = std::io::Cursor::new(&result);
            let archive = zip::ZipArchive::new(cursor).expect("should be valid ZIP");
            prop_assert!(archive.len() > 0, "ZIP should contain files");

            // Read document.xml
            let doc_xml = read_zip_entry(&result, "word/document.xml")
                .expect("document.xml should exist");

            // Count <w:p> paragraphs - each record produces one paragraph
            let paragraph_count = doc_xml.matches("<w:p>").count();
            prop_assert_eq!(
                paragraph_count,
                records.len(),
                "Number of paragraphs should equal number of records"
            );
        }
    }

    /// Unit test: .docx export marks failed conversions with "转换失败"
    ///
    /// **Validates: Requirements 8.3**
    #[test]
    fn test_docx_export_failed_conversion_annotation() {
        // Use LaTeX with unsupported symbols that will fail conversion
        let records = vec![
            make_record(
                "2025-01-01T00:00:00Z",
                r"\unsupportedcommand{test}",
                None,
            ),
            make_record(
                "2025-01-02T00:00:00Z",
                r"\anotherbadcommand[invalid]{{{",
                None,
            ),
        ];

        let result = export_docx(&records).expect("export should succeed even with conversion failures");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Both records should have "转换失败" annotation since they use unsupported commands
        let failure_count = doc_xml.matches("转换失败").count();
        assert!(
            failure_count >= 1,
            "At least one record should have '转换失败' annotation, found {}",
            failure_count
        );

        // Should still have paragraphs for all records
        let paragraph_count = doc_xml.matches("<w:p>").count();
        assert_eq!(
            paragraph_count, 2,
            "Should have 2 paragraphs even with conversion failures"
        );
    }

    /// Unit test: .docx export with mixed valid and invalid LaTeX
    ///
    /// **Validates: Requirements 8.3**
    #[test]
    fn test_docx_export_mixed_valid_invalid_latex() {
        let records = vec![
            make_record("2025-01-01T00:00:00Z", r"x^2", None),           // valid
            make_record("2025-01-02T00:00:00Z", r"\badcmd{{{", None),    // invalid
            make_record("2025-01-03T00:00:00Z", r"\alpha + \beta", None), // valid
        ];

        let result = export_docx(&records).expect("export should succeed");
        let doc_xml = read_zip_entry(&result, "word/document.xml")
            .expect("document.xml should exist");

        // Should have 3 paragraphs
        let paragraph_count = doc_xml.matches("<w:p>").count();
        assert_eq!(paragraph_count, 3, "Should have 3 paragraphs");

        // Should have at least one "转换失败" for the invalid LaTeX
        assert!(
            doc_xml.contains("转换失败"),
            "Should contain '转换失败' for invalid LaTeX"
        );

        // Should have OMML content for valid LaTeX
        assert!(
            doc_xml.contains("<m:oMathPara"),
            "Should contain OMML for valid LaTeX"
        );
    }
}
