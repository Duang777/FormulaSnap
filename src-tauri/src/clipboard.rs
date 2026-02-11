// ClipboardService - 剪贴板服务模块
// 使用纯文本格式写入 MathML，Word 可以直接识别并转换为公式

use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("剪贴板打开失败: {0}")]
    OpenFailed(String),
    #[error("格式写入失败: {0}")]
    WriteFailed(String),
}

impl Serialize for ClipboardError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// 多格式写入剪贴板
/// 只写入 CF_UNICODETEXT 格式的 MathML - Word 可以直接识别并转换为公式
/// 
/// 关键：不写入 CF_HTML，这样 Word 在 Ctrl+V 时只能使用纯文本格式，
/// 从而自动识别 MathML 并转换为公式
pub fn copy_formula(_latex: &str, _omml: &str, mathml: &str) -> Result<(), ClipboardError> {
    // Log what we're copying
    eprintln!("[clipboard] Copying formula to clipboard with CF_UNICODETEXT only (MathML)");
    eprintln!("[clipboard] MathML length: {} chars", mathml.len());
    
    // 只写入纯文本格式的 MathML
    // Word 会自动识别 MathML 并转换为公式
    copy_latex(mathml)?;
    
    eprintln!("[clipboard] MathML written as CF_UNICODETEXT successfully");
    
    Ok(())
}

/// 仅复制 LaTeX 文本（按包裹格式写入纯文本）
pub fn copy_latex(latex: &str) -> Result<(), ClipboardError> {
    // Open clipboard with retries
    let _clip = clipboard_win::Clipboard::new_attempts(10)
        .map_err(|e| ClipboardError::OpenFailed(e.to_string()))?;

    // Empty clipboard before writing
    clipboard_win::raw::empty()
        .map_err(|e| ClipboardError::WriteFailed(format!("清空剪贴板失败: {}", e)))?;

    // Write LaTeX as CF_UNICODETEXT without clearing (already emptied above)
    clipboard_win::raw::set_string_with(latex, clipboard_win::options::NoClear)
        .map_err(|e| ClipboardError::WriteFailed(format!("写入 LaTeX 文本失败: {}", e)))?;

    // Clipboard is closed automatically when _clip is dropped
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    // Strategy to generate random LaTeX-like strings
    fn latex_string_strategy() -> impl Strategy<Value = String> {
        prop::collection::vec(
            prop_oneof![
                Just("x".to_string()),
                Just("y".to_string()),
                Just("z".to_string()),
                Just("a".to_string()),
                Just("b".to_string()),
                Just("n".to_string()),
                Just("0".to_string()),
                Just("1".to_string()),
                Just("2".to_string()),
                Just("+".to_string()),
                Just("-".to_string()),
                Just("=".to_string()),
                Just("^{2}".to_string()),
                Just("_{i}".to_string()),
                Just("\\alpha".to_string()),
                Just("\\beta".to_string()),
                Just("\\gamma".to_string()),
                Just("\\frac{a}{b}".to_string()),
                Just("\\sqrt{x}".to_string()),
            ],
            1..5,
        )
        .prop_map(|parts| parts.join(" "))
    }

    // Strategy to generate MathML strings (what actually gets written to clipboard)
    fn mathml_string_strategy() -> impl Strategy<Value = String> {
        latex_string_strategy().prop_map(|latex| {
            // Generate a simple MathML wrapper for the content
            format!(
                r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mrow><mi>{}</mi></mrow></math>"#,
                latex.replace('<', "&lt;").replace('>', "&gt;")
            )
        })
    }

    // **Validates: Requirements 5.2**
    // Property 7: 剪贴板多格式写入完整性
    // For any valid LaTeX, OMML and MathML string combination, after calling copy_formula,
    // reading from clipboard should return the MathML content in CF_UNICODETEXT format.
    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        #[test]
        #[ignore] // Requires desktop session, may fail in CI
        fn prop_clipboard_multiformat_write_integrity(
            latex in latex_string_strategy(),
            mathml in mathml_string_strategy()
        ) {
            // Generate a simple OMML string (not actually used in current implementation)
            let omml = format!(
                r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:r><m:t>{}</m:t></m:r></m:oMath>"#,
                latex.replace('<', "&lt;").replace('>', "&gt;")
            );

            // Call copy_formula - this writes MathML to CF_UNICODETEXT
            let result = copy_formula(&latex, &omml, &mathml);
            prop_assert!(result.is_ok(), "copy_formula should succeed: {:?}", result.err());

            // Read back from clipboard and verify CF_UNICODETEXT contains MathML
            let read_back: Result<String, _> = clipboard_win::get_clipboard(clipboard_win::formats::Unicode);
            prop_assert!(read_back.is_ok(), "Should be able to read clipboard");

            let clipboard_content = read_back.unwrap();
            prop_assert_eq!(
                clipboard_content, mathml,
                "CF_UNICODETEXT should contain the MathML content"
            );
        }
    }

    #[test]
    fn test_copy_latex_writes_text() {
        let latex = r"E = mc^2";
        let result = copy_latex(latex);
        assert!(result.is_ok(), "copy_latex should succeed: {:?}", result.err());

        // Verify by reading back from clipboard
        let read_back: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read clipboard");
        assert_eq!(read_back, latex);
    }

    #[test]
    #[ignore = "Requires desktop session - clipboard access may fail in parallel tests"]
    fn test_copy_latex_with_wrap_format() {
        // Test with inline wrap format
        let latex = r"\(E = mc^2\)";
        let result = copy_latex(latex);
        assert!(result.is_ok(), "copy_latex should succeed with inline wrap");

        let read_back: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read clipboard");
        assert_eq!(read_back, latex);
    }

    #[test]
    #[ignore = "Requires desktop session - clipboard access may fail in parallel tests"]
    fn test_copy_latex_with_display_wrap() {
        // Test with display wrap format
        let latex = r"\[E = mc^2\]";
        let result = copy_latex(latex);
        assert!(result.is_ok(), "copy_latex should succeed with display wrap");

        let read_back: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read clipboard");
        assert_eq!(read_back, latex);
    }

    #[test]
    #[ignore = "Requires desktop session - clipboard access may fail in parallel tests"]
    fn test_copy_formula_writes_mathml() {
        let latex = r"\frac{a}{b}";
        let omml = r#"<m:oMath xmlns:m="http://schemas.openxmlformats.org/officeDocument/2006/math"><m:f><m:num><m:r><m:t>a</m:t></m:r></m:num><m:den><m:r><m:t>b</m:t></m:r></m:den></m:f></m:oMath>"#;
        let mathml = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mfrac><mi>a</mi><mi>b</mi></mfrac></math>"#;

        let result = copy_formula(latex, omml, mathml);
        assert!(
            result.is_ok(),
            "copy_formula should succeed: {:?}",
            result.err()
        );

        // Verify MathML (CF_UNICODETEXT) was written
        let read_text: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read unicode text from clipboard");
        assert_eq!(read_text, mathml);
    }

    #[test]
    fn test_copy_formula_empty_strings() {
        // Edge case: empty strings should still work (at least not crash)
        let result = copy_formula("", "", "");
        // Empty string for set_string may fail on Windows, but should not panic
        // The behavior depends on the clipboard-win crate's handling of empty strings
        // We just verify it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_copy_latex_empty_string() {
        let result = copy_latex("");
        // Empty string edge case - should not panic
        let _ = result;
    }

    #[test]
    #[ignore = "Requires desktop session - clipboard access may fail in parallel tests"]
    fn test_copy_formula_overwrites_previous() {
        // First write
        let result1 = copy_latex("first");
        assert!(result1.is_ok());

        // Second write should overwrite
        let result2 = copy_latex("second");
        assert!(result2.is_ok());

        let read_back: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read clipboard");
        assert_eq!(read_back, "second");
    }

    #[test]
    #[ignore = "Requires desktop session - clipboard access may fail in parallel tests"]
    fn test_copy_formula_unicode_content() {
        // Test with Unicode characters (Greek letters, math symbols)
        let latex = r"\alpha + \beta = \gamma";
        let omml = "<m:oMath><m:r><m:t>α+β=γ</m:t></m:r></m:oMath>";
        let mathml = "<math><mi>α</mi><mo>+</mo><mi>β</mi><mo>=</mo><mi>γ</mi></math>";

        let result = copy_formula(latex, omml, mathml);
        assert!(
            result.is_ok(),
            "copy_formula should handle Unicode: {:?}",
            result.err()
        );

        // Verify MathML was written
        let read_text: String = clipboard_win::get_clipboard(clipboard_win::formats::Unicode)
            .expect("Should read unicode text");
        assert_eq!(read_text, mathml);
    }
}
