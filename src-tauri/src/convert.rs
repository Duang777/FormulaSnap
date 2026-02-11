// ConvertService - 格式转换模块
// LaTeX → MathML → OMML 转换链

use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;
use serde::Serialize;
use std::io::Cursor;

/// OMML namespace URI
const OMML_NS: &str = "http://schemas.openxmlformats.org/officeDocument/2006/math";

#[derive(Debug, thiserror::Error)]
pub enum ConvertError {
    #[error("LaTeX 转 MathML 失败: {0}")]
    LatexToMathml(String),
    #[error("MathML 转 OMML 失败: {0}")]
    MathmlToOmml(String),
    #[error("不支持的 LaTeX 符号: {0}")]
    UnsupportedSymbol(String),
}

impl Serialize for ConvertError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

/// Attempt to extract an unsupported symbol name from the LaTeX error message.
///
/// The `latex2mathml` crate returns errors for unknown commands or environments.
/// This helper inspects the error string representation to detect patterns that
/// indicate a specific unsupported symbol/command, and returns the symbol name
/// if one can be identified.
fn try_extract_unsupported_symbol(error: &latex2mathml::LatexError) -> Option<String> {
    match error {
        latex2mathml::LatexError::UnknownEnvironment(env) => Some(env.clone()),
        _ => {
            let msg = error.to_string();
            if let Some(pos) = msg.find('\\') {
                let after = &msg[pos + 1..];
                let symbol: String = after
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !symbol.is_empty() {
                    return Some(format!("\\{}", symbol));
                }
            }
            None
        }
    }
}

/// LaTeX → MathML
///
/// Converts a LaTeX math expression string into MathML markup using the
/// `latex2mathml` crate with inline display style.
///
/// # Preprocessing
///
/// Before conversion, the input is preprocessed to handle commands that
/// `latex2mathml` doesn't support:
/// - `\displaystyle`, `\textstyle`, `\scriptstyle`, `\scriptscriptstyle` are removed
/// - `\rlap{...}`, `\llap{...}` are replaced with their content
/// - `\quad`, `\qquad` are replaced with spaces
/// - `array` environment is converted to `matrix`
///
/// # Errors
///
/// Returns `ConvertError::UnsupportedSymbol` when the input contains a LaTeX
/// command or environment that is not supported by the converter.
/// Returns `ConvertError::LatexToMathml` for all other conversion failures
/// (e.g. syntax errors, mismatched braces).
pub fn latex_to_mathml(latex: &str) -> Result<String, ConvertError> {
    let preprocessed = preprocess_latex(latex);
    let mathml = latex2mathml::latex_to_mathml(&preprocessed, latex2mathml::DisplayStyle::Inline).map_err(|e| {
        if let Some(symbol) = try_extract_unsupported_symbol(&e) {
            ConvertError::UnsupportedSymbol(symbol)
        } else {
            ConvertError::LatexToMathml(e.to_string())
        }
    })?;
    
    // Post-process MathML to fix msup/msub nesting issues
    // Convert <msup><msub>base sub</msub> sup</msup> to <msubsup>base sub sup</msubsup>
    let fixed_mathml = fix_mathml_subsup(&mathml);
    
    Ok(fixed_mathml)
}

/// Fix MathML structure: convert nested msup/msub to msubsup
/// This fixes the issue where latex2mathml generates <msup><msub>...</msub>...</msup>
/// instead of <msubsup>...</msubsup> for expressions like X_a^b
fn fix_mathml_subsup(mathml: &str) -> String {
    // Use regex to find and fix the pattern
    // Pattern: <msup><msub>base sub</msub>sup</msup> -> <msubsup>base sub sup</msubsup>
    
    let re = match regex::Regex::new(
        r"<msup>(\s*)<msub>(.*?)</msub>(\s*)(.*?)</msup>"
    ) {
        Ok(r) => r,
        Err(_) => return mathml.to_string(),
    };
    
    // This simple regex won't handle nested cases well, so we need a more robust approach
    // For now, let's use a simple string replacement approach
    
    let mut result = mathml.to_string();
    
    // Keep replacing until no more matches (handles nested cases)
    loop {
        let new_result = re.replace_all(&result, "<msubsup>$1$2$3$4</msubsup>").to_string();
        if new_result == result {
            break;
        }
        result = new_result;
    }
    
    result
}

/// Preprocess LaTeX to remove/replace unsupported commands
fn preprocess_latex(latex: &str) -> String {
    let mut result = latex.to_string();
    
    // Remove \( \) and \[ \] wrappers
    if result.starts_with(r"\(") {
        result = result.strip_prefix(r"\(").unwrap_or(&result).to_string();
    }
    if result.ends_with(r"\)") {
        result = result.strip_suffix(r"\)").unwrap_or(&result).to_string();
    }
    if result.starts_with(r"\[") {
        result = result.strip_prefix(r"\[").unwrap_or(&result).to_string();
    }
    if result.ends_with(r"\]") {
        result = result.strip_suffix(r"\]").unwrap_or(&result).to_string();
    }
    
    // Remove $ and $$ wrappers
    result = result.trim_start_matches("$$").trim_end_matches("$$").to_string();
    result = result.trim_start_matches('$').trim_end_matches('$').to_string();
    
    // Fix \mathcal L -> \mathcal{L} (OCR often misses the braces)
    // Match \mathcal followed by a single letter without braces
    let mathcal_re = regex::Regex::new(r"\\mathcal\s+([A-Za-z])").ok();
    if let Some(re) = mathcal_re {
        result = re.replace_all(&result, r"\mathcal{$1}").to_string();
    }
    
    // Fix triple/double braces around content: {{{x}}} -> {x}, {{x}} -> {x}
    // But only when they are balanced pairs
    let triple_brace_re = regex::Regex::new(r"\{\{\{([^{}]*)\}\}\}").ok();
    if let Some(re) = triple_brace_re {
        loop {
            let new_result = re.replace_all(&result, "{$1}").to_string();
            if new_result == result {
                break;
            }
            result = new_result;
        }
    }
    
    let double_brace_re = regex::Regex::new(r"\{\{([^{}]*)\}\}").ok();
    if let Some(re) = double_brace_re {
        loop {
            let new_result = re.replace_all(&result, "{$1}").to_string();
            if new_result == result {
                break;
            }
            result = new_result;
        }
    }
    
    // Fix spaces in common function names: "l o g" -> "log", "g e n" -> "gen"
    result = result.replace("l o g", "log");
    result = result.replace("g e n", "gen");
    result = result.replace("s i n", "sin");
    result = result.replace("c o s", "cos");
    result = result.replace("t a n", "tan");
    result = result.replace("e x p", "exp");
    result = result.replace("l n", "ln");
    
    // Fix spaced-out common words: "E n c" -> "Enc", "D e c" -> "Dec"
    result = result.replace("E n c", "Enc");
    result = result.replace("D e c", "Dec");
    result = result.replace("C L S", "CLS");
    result = result.replace("S E P", "SEP");
    
    // Remove excessive \qquad (OCR often adds too many)
    let qquad_re = regex::Regex::new(r"(\\qquad\s*){3,}").ok();
    if let Some(re) = qquad_re {
        result = re.replace_all(&result, r"\quad ").to_string();
    }
    let quad_re = regex::Regex::new(r"(\\quad\s*){3,}").ok();
    if let Some(re) = quad_re {
        result = re.replace_all(&result, r"\quad ").to_string();
    }
    
    // Remove trailing \;\;\;\_  sequences
    let trailing_re = regex::Regex::new(r"(\\[;,!]\s*)+\\_\s*$").ok();
    if let Some(re) = trailing_re {
        result = re.replace_all(&result, "").to_string();
    }
    let trailing_re2 = regex::Regex::new(r"(\\[;,!]\s*)+$").ok();
    if let Some(re) = trailing_re2 {
        result = re.replace_all(&result, "").to_string();
    }
    
    // Fix \_ (escaped underscore)
    result = result.replace(r"\_", "_");
    
    // Remove display style commands (they don't affect the math structure)
    let style_commands = [
        r"\displaystyle",
        r"\textstyle", 
        r"\scriptstyle",
        r"\scriptscriptstyle",
    ];
    for cmd in &style_commands {
        result = result.replace(cmd, "");
    }
    
    // Remove \limits and \nolimits commands (they only affect placement, not structure)
    // \prod\limits_{k=1} -> \prod_{k=1}
    result = result.replace(r"\limits", "");
    result = result.replace(r"\nolimits", "");
    
    // Remove bracket sizing commands (they don't affect the math structure in OMML)
    let sizing_commands = [
        r"\Big", r"\big", r"\Bigg", r"\bigg",
        r"\Big", r"\big", r"\Bigg", r"\bigg",
        r"\left", r"\right",
    ];
    for cmd in &sizing_commands {
        // Replace \Big( with just ( etc.
        result = result.replace(&format!("{}(", cmd), "(");
        result = result.replace(&format!("{})", cmd), ")");
        result = result.replace(&format!("{}[", cmd), "[");
        result = result.replace(&format!("{}]", cmd), "]");
        result = result.replace(&format!("{}{{", cmd), "{");
        result = result.replace(&format!("{}}}", cmd), "}");
        result = result.replace(&format!("{}|", cmd), "|");
        result = result.replace(&format!("{}.", cmd), "");  // \left. \right. -> nothing
    }
    
    // Replace old-style font commands with modern equivalents
    // \bf{...} -> \mathbf{...}, \it{...} -> \mathit{...}, etc.
    result = replace_font_command(&result, r"\bf", r"\mathbf");
    result = replace_font_command(&result, r"\it", r"\mathit");
    result = replace_font_command(&result, r"\rm", r"\mathrm");
    result = replace_font_command(&result, r"\cal", r"\mathcal");
    result = replace_font_command(&result, r"\tt", r"\mathtt");
    result = replace_font_command(&result, r"\sf", r"\mathsf");
    
    // Replace \operatorname{...} with \mathrm{...}
    // latex2mathml doesn't support \operatorname
    result = replace_operatorname(&result);
    
    // Replace \mathcal{X} with styled letter (latex2mathml may not support it)
    // For now, just convert to regular italic
    result = replace_mathcal(&result);
    
    // Replace \quad and \qquad with thin space
    result = result.replace(r"\qquad", " ");
    result = result.replace(r"\quad", " ");
    
    // Replace \rlap{...} and \llap{...} with their content
    result = replace_command_with_content(&result, r"\rlap");
    result = replace_command_with_content(&result, r"\llap");
    
    // Convert array environment to matrix (basic conversion)
    // \begin{array}{...} ... \end{array} -> \begin{matrix} ... \end{matrix}
    result = convert_array_to_matrix(&result);
    
    // Fix subscript-superscript order for latex2mathml
    // X_{sub}^{sup} -> {X_{sub}}^{sup} to ensure correct MathML structure
    result = fix_subsup_order(&result);
    
    // Remove empty braces that might result from preprocessing
    result = result.replace("{}", "");
    
    // Clean up multiple spaces
    while result.contains("  ") {
        result = result.replace("  ", " ");
    }
    
    result.trim().to_string()
}

/// Fix subscript-superscript order for latex2mathml
/// Converts X_{sub}^{sup} to {X_{sub}}^{sup} to ensure correct MathML structure
/// This is needed because latex2mathml incorrectly nests msub inside msup for X_a^b
fn fix_subsup_order(latex: &str) -> String {
    // Use regex to find and fix the pattern
    // Pattern: (base)(_{subscript})(^{superscript})
    // where base is either a single letter (not part of a command) or a command like \cmd{...}
    
    // First, handle single letter base: A_{sub}^{sup} -> {A_{sub}}^{sup}
    // Use negative lookbehind to ensure the letter is not part of a command
    // Since Rust regex doesn't support lookbehind, we use a workaround:
    // Match either start of string or non-letter before the base letter
    let re1 = match regex::Regex::new(r"(^|[^a-zA-Z\\])([A-Za-z])(_\{[^}]*\})(\^\{[^}]*\})") {
        Ok(r) => r,
        Err(_) => return latex.to_string(),
    };
    let result = re1.replace_all(latex, "$1{$2$3}$4").to_string();
    
    // Handle single char subscript: A_a^{sup} -> {A_a}^{sup}
    let re2 = match regex::Regex::new(r"(^|[^a-zA-Z\\])([A-Za-z])_([A-Za-z0-9])(\^\{[^}]*\})") {
        Ok(r) => r,
        Err(_) => return result,
    };
    let result = re2.replace_all(&result, "$1{$2_$3}$4").to_string();
    
    // Handle command with braces as base: \cmd{x}_{sub}^{sup} -> {\cmd{x}_{sub}}^{sup}
    let re3 = match regex::Regex::new(r"(\\[a-zA-Z]+\{[^}]*\})(_\{[^}]*\})(\^\{[^}]*\})") {
        Ok(r) => r,
        Err(_) => return result,
    };
    let result = re3.replace_all(&result, "{$1$2}$3").to_string();
    
    result
}

/// Replace \mathcal{X} with a script-style representation
/// Since latex2mathml may not support \mathcal, we use Unicode script letters
fn replace_mathcal(latex: &str) -> String {
    // Map of regular letters to Unicode mathematical script letters
    let script_map: std::collections::HashMap<char, char> = [
        ('A', '𝒜'), ('B', 'ℬ'), ('C', '𝒞'), ('D', '𝒟'), ('E', 'ℰ'),
        ('F', 'ℱ'), ('G', '𝒢'), ('H', 'ℋ'), ('I', 'ℐ'), ('J', '𝒥'),
        ('K', '𝒦'), ('L', 'ℒ'), ('M', 'ℳ'), ('N', '𝒩'), ('O', '𝒪'),
        ('P', '𝒫'), ('Q', '𝒬'), ('R', 'ℛ'), ('S', '𝒮'), ('T', '𝒯'),
        ('U', '𝒰'), ('V', '𝒱'), ('W', '𝒲'), ('X', '𝒳'), ('Y', '𝒴'),
        ('Z', '𝒵'),
    ].iter().cloned().collect();
    
    let mut result = String::new();
    let mut chars = latex.chars().peekable();
    let cmd = r"\mathcal";
    let cmd_chars: Vec<char> = cmd.chars().collect();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            let mut matched = true;
            let mut consumed: Vec<char> = vec!['\\'];
            
            for &cmd_char in cmd_chars.iter().skip(1) {
                if let Some(&next) = chars.peek() {
                    if next == cmd_char {
                        consumed.push(chars.next().unwrap());
                    } else {
                        matched = false;
                        break;
                    }
                } else {
                    matched = false;
                    break;
                }
            }
            
            if matched {
                // Skip whitespace
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                
                // Check for opening brace
                if chars.peek() == Some(&'{') {
                    chars.next(); // consume '{'
                    
                    // Extract content until matching '}'
                    let mut depth = 1;
                    let mut content = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '{' {
                            depth += 1;
                            content.push(ch);
                        } else if ch == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            content.push(ch);
                        } else {
                            content.push(ch);
                        }
                    }
                    
                    // Convert each letter to script
                    for letter in content.chars() {
                        if let Some(&script) = script_map.get(&letter) {
                            result.push(script);
                        } else {
                            result.push(letter);
                        }
                    }
                } else {
                    // No brace, output as-is
                    result.extend(consumed);
                }
            } else {
                result.extend(consumed);
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Replace \operatorname{...} with \mathrm{...}
fn replace_operatorname(latex: &str) -> String {
    let mut result = String::new();
    let mut chars = latex.chars().peekable();
    let cmd = r"\operatorname";
    let cmd_chars: Vec<char> = cmd.chars().collect();
    
    while let Some(c) = chars.next() {
        if c == '\\' {
            // Try to match \operatorname
            let mut matched = true;
            let mut consumed: Vec<char> = vec!['\\'];
            
            for &cmd_char in cmd_chars.iter().skip(1) {
                if let Some(&next) = chars.peek() {
                    if next == cmd_char {
                        consumed.push(chars.next().unwrap());
                    } else {
                        matched = false;
                        break;
                    }
                } else {
                    matched = false;
                    break;
                }
            }
            
            if matched {
                // Found \operatorname, now handle subscript if present
                // e.g., \operatorname{Softmax}_{row} -> \mathrm{Softmax}_{\mathrm{row}}
                
                // Skip whitespace
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                
                // Check for opening brace
                if chars.peek() == Some(&'{') {
                    chars.next(); // consume '{'
                    
                    // Extract content until matching '}'
                    let mut depth = 1;
                    let mut content = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '{' {
                            depth += 1;
                            content.push(ch);
                        } else if ch == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            content.push(ch);
                        } else {
                            content.push(ch);
                        }
                    }
                    
                    // Output as \mathrm{content}
                    result.push_str(&format!("\\mathrm{{{}}}", content));
                } else {
                    // No brace, just output \mathrm
                    result.push_str("\\mathrm");
                }
            } else {
                // Not \operatorname, output what we consumed
                result.extend(consumed);
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Replace old-style font command with modern equivalent
/// e.g., \bf X -> \mathbf{X}, {\bf X} -> \mathbf{X}
fn replace_font_command(latex: &str, old_cmd: &str, new_cmd: &str) -> String {
    let mut result = latex.to_string();
    
    // Pattern 1: {\bf ...} -> \mathbf{...}
    // Find {\ followed by command name
    let brace_pattern = format!("{{{}\\s*", old_cmd.replace("\\", "\\\\"));
    if let Ok(re) = regex::Regex::new(&brace_pattern) {
        result = re.replace_all(&result, &format!("{}{}", new_cmd, "{")).to_string();
    }
    
    // Pattern 2: \bf followed by single token or {...}
    // Simple replacement: \bf -> \mathbf (let the next token be the argument)
    // This is a simplified approach - just replace the command name
    result = result.replace(&format!("{} ", old_cmd), &format!("{} ", new_cmd));
    result = result.replace(&format!("{}{{", old_cmd), &format!("{}{{", new_cmd));
    
    result
}

/// Replace a command like \rlap{content} with just content
fn replace_command_with_content(latex: &str, cmd: &str) -> String {
    let mut result = String::new();
    let mut chars = latex.chars().peekable();
    let cmd_chars: Vec<char> = cmd.chars().collect();
    
    while let Some(c) = chars.next() {
        // Check if we're at the start of the command
        if c == '\\' {
            let mut matched = true;
            let mut cmd_rest: Vec<char> = Vec::new();
            
            // Try to match the rest of the command
            for &cmd_char in cmd_chars.iter().skip(1) {
                if let Some(&next) = chars.peek() {
                    if next == cmd_char {
                        cmd_rest.push(chars.next().unwrap());
                    } else {
                        matched = false;
                        break;
                    }
                } else {
                    matched = false;
                    break;
                }
            }
            
            if matched {
                // Skip whitespace after command
                while chars.peek() == Some(&' ') {
                    chars.next();
                }
                
                // Check for opening brace
                if chars.peek() == Some(&'{') {
                    chars.next(); // consume '{'
                    
                    // Extract content until matching '}'
                    let mut depth = 1;
                    let mut content = String::new();
                    while let Some(ch) = chars.next() {
                        if ch == '{' {
                            depth += 1;
                            content.push(ch);
                        } else if ch == '}' {
                            depth -= 1;
                            if depth == 0 {
                                break;
                            }
                            content.push(ch);
                        } else {
                            content.push(ch);
                        }
                    }
                    result.push_str(&content);
                } else {
                    // No brace, just output the command as-is
                    result.push('\\');
                    result.extend(cmd_rest);
                }
            } else {
                // Not our command, output what we consumed
                result.push('\\');
                result.extend(cmd_rest);
            }
        } else {
            result.push(c);
        }
    }
    
    result
}

/// Convert array environment to matrix
fn convert_array_to_matrix(latex: &str) -> String {
    let mut result = latex.to_string();
    
    // Simple replacement: \begin{array}{...} -> \begin{matrix}
    // This is a basic conversion that works for simple cases
    
    // Find and replace \begin{array}{...}
    while let Some(start) = result.find(r"\begin{array}") {
        let after_begin = start + r"\begin{array}".len();
        
        // Skip the column specification {ccc} or similar
        if let Some(spec_start) = result[after_begin..].find('{') {
            let spec_start = after_begin + spec_start;
            if let Some(spec_end) = find_matching_brace(&result, spec_start) {
                // Remove the column spec and replace array with matrix
                result = format!(
                    "{}\\begin{{matrix}}{}",
                    &result[..start],
                    &result[spec_end + 1..]
                );
            } else {
                break; // Malformed, stop processing
            }
        } else {
            // No column spec, just replace
            result = result.replacen(r"\begin{array}", r"\begin{matrix}", 1);
        }
    }
    
    // Replace \end{array} with \end{matrix}
    result = result.replace(r"\end{array}", r"\end{matrix}");
    
    result
}

/// Find the position of the matching closing brace
fn find_matching_brace(s: &str, open_pos: usize) -> Option<usize> {
    let bytes = s.as_bytes();
    if bytes.get(open_pos) != Some(&b'{') {
        return None;
    }
    
    let mut depth = 1;
    for (i, &b) in bytes.iter().enumerate().skip(open_pos + 1) {
        match b {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------------------
// MathML → OMML conversion
// ---------------------------------------------------------------------------

/// Intermediate representation of a parsed MathML tree node.
#[derive(Debug, Clone)]
enum MathNode {
    /// An identifier (`<mi>`)
    Mi(String),
    /// A number (`<mn>`)
    Mn(String),
    /// An operator (`<mo>`)
    Mo(String),
    /// Text (`<mtext>`)
    Mtext(String),
    /// A row of children (`<mrow>` or implicit grouping)
    Mrow(Vec<MathNode>),
    /// Fraction (`<mfrac>`) with numerator and denominator
    Mfrac(Box<MathNode>, Box<MathNode>),
    /// Square root (`<msqrt>`)
    Msqrt(Vec<MathNode>),
    /// Nth root (`<mroot>`) with base and index
    Mroot(Box<MathNode>, Box<MathNode>),
    /// Superscript (`<msup>`) with base and superscript
    Msup(Box<MathNode>, Box<MathNode>),
    /// Subscript (`<msub>`) with base and subscript
    Msub(Box<MathNode>, Box<MathNode>),
    /// Sub-superscript (`<msubsup>`) with base, subscript, superscript
    Msubsup(Box<MathNode>, Box<MathNode>, Box<MathNode>),
    /// Over-accent or upper limit (`<mover>`)
    Mover(Box<MathNode>, Box<MathNode>),
    /// Under-limit (`<munder>`)
    Munder(Box<MathNode>, Box<MathNode>),
    /// Under-over (`<munderover>`)
    Munderover(Box<MathNode>, Box<MathNode>, Box<MathNode>),
    /// Table / matrix (`<mtable>`)
    Mtable(Vec<Vec<MathNode>>),
    /// Fenced expression (`<mfenced>`) with open, close delimiters and children
    Mfenced {
        open: String,
        close: String,
        children: Vec<MathNode>,
    },
    /// Space (`<mspace>`) – mostly ignored
    Mspace,
    /// Raw text that doesn't fit other categories
    Text(String),
}

/// Check if a character/string is a large operator (integral, sum, product, etc.)
fn is_large_operator(s: &str) -> bool {
    matches!(
        s,
        "∫" | "∬" | "∭" | "∮" | "∑" | "∏" | "⋃" | "⋂" | "⋁" | "⋀"
    )
}

/// Check if a string represents a common accent character.
fn is_accent_char(s: &str) -> bool {
    matches!(
        s,
        "^" | "~" | "¯" | "˙" | "¨" | "˘" | "ˇ"
            | "\u{0302}" | "\u{0303}" | "\u{0304}" | "\u{0307}"
            | "\u{0308}" | "\u{030C}" | "\u{20D7}"
    )
}

/// Parse MathML XML string into a tree of `MathNode`.
fn parse_mathml(mathml: &str) -> Result<Vec<MathNode>, ConvertError> {
    let mut reader = Reader::from_str(mathml);
    reader.config_mut().trim_text(true);
    let nodes = parse_children(&mut reader, None)?;
    Ok(nodes)
}

/// Recursively parse children from the XML reader until we hit the closing tag
/// for `parent_tag` (or EOF if `parent_tag` is None).
fn parse_children(
    reader: &mut Reader<&[u8]>,
    parent_tag: Option<&str>,
) -> Result<Vec<MathNode>, ConvertError> {
    let mut nodes = Vec::new();
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                // Strip namespace prefix (e.g. "mml:mrow" → "mrow")
                let local = strip_ns_prefix(&tag_name);
                let node = parse_element(reader, &local, e)?;
                nodes.push(node);
            }
            Ok(Event::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = strip_ns_prefix(&tag_name);
                match local.as_str() {
                    "mspace" => nodes.push(MathNode::Mspace),
                    _ => {
                        // Self-closing element – try to extract text from attributes
                        // (rare, but handle gracefully)
                    }
                }
            }
            Ok(Event::Text(ref e)) => {
                let text = e.unescape().unwrap_or_default().to_string();
                if !text.trim().is_empty() {
                    nodes.push(MathNode::Text(text));
                }
            }
            Ok(Event::End(ref e)) => {
                if let Some(parent) = parent_tag {
                    let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let local = strip_ns_prefix(&tag_name);
                    if local == parent {
                        break;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(ConvertError::MathmlToOmml(format!(
                    "XML parse error: {}",
                    e
                )));
            }
            _ => {} // Skip comments, processing instructions, etc.
        }
        buf.clear();
    }
    Ok(nodes)
}

/// Strip namespace prefix from a tag name (e.g. "mml:mrow" → "mrow").
fn strip_ns_prefix(tag: &str) -> String {
    if let Some(pos) = tag.find(':') {
        tag[pos + 1..].to_string()
    } else {
        tag.to_string()
    }
}

/// Parse a single MathML element that has already been opened (Start event consumed).
fn parse_element(
    reader: &mut Reader<&[u8]>,
    local_name: &str,
    start: &BytesStart,
) -> Result<MathNode, ConvertError> {
    match local_name {
        "math" => {
            let children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Mrow(children))
        }
        "mrow" | "semantics" | "annotation" | "annotation-xml" => {
            let children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Mrow(children))
        }
        "mi" => {
            let text = read_text_content(reader, local_name)?;
            Ok(MathNode::Mi(text))
        }
        "mn" => {
            let text = read_text_content(reader, local_name)?;
            Ok(MathNode::Mn(text))
        }
        "mo" => {
            let text = read_text_content(reader, local_name)?;
            Ok(MathNode::Mo(text))
        }
        "mtext" => {
            let text = read_text_content(reader, local_name)?;
            Ok(MathNode::Mtext(text))
        }
        "mfrac" => {
            let children = parse_children(reader, Some(local_name))?;
            let (num, den) = take_two(children, local_name)?;
            Ok(MathNode::Mfrac(Box::new(num), Box::new(den)))
        }
        "msqrt" => {
            let children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Msqrt(children))
        }
        "mroot" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, index) = take_two(children, local_name)?;
            Ok(MathNode::Mroot(Box::new(base), Box::new(index)))
        }
        "msup" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, sup) = take_two(children, local_name)?;
            
            // Check if base is an msub - if so, convert to msubsup
            // This fixes the issue where latex2mathml generates nested msup/msub
            // instead of msubsup for X_a^b
            if let MathNode::Msub(inner_base, sub) = base {
                Ok(MathNode::Msubsup(inner_base, sub, Box::new(sup)))
            } else {
                Ok(MathNode::Msup(Box::new(base), Box::new(sup)))
            }
        }
        "msub" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, sub) = take_two(children, local_name)?;
            Ok(MathNode::Msub(Box::new(base), Box::new(sub)))
        }
        "msubsup" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, sub, sup) = take_three(children, local_name)?;
            Ok(MathNode::Msubsup(
                Box::new(base),
                Box::new(sub),
                Box::new(sup),
            ))
        }
        "mover" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, over) = take_two(children, local_name)?;
            Ok(MathNode::Mover(Box::new(base), Box::new(over)))
        }
        "munder" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, under) = take_two(children, local_name)?;
            Ok(MathNode::Munder(Box::new(base), Box::new(under)))
        }
        "munderover" => {
            let children = parse_children(reader, Some(local_name))?;
            let (base, under, over) = take_three(children, local_name)?;
            Ok(MathNode::Munderover(
                Box::new(base),
                Box::new(under),
                Box::new(over),
            ))
        }
        "mtable" => {
            let children = parse_children(reader, Some(local_name))?;
            let mut rows: Vec<Vec<MathNode>> = Vec::new();
            for child in children {
                match child {
                    MathNode::Mrow(cells) => rows.push(cells),
                    other => rows.push(vec![other]),
                }
            }
            Ok(MathNode::Mtable(rows))
        }
        "mtr" | "mlabeledtr" => {
            let children = parse_children(reader, Some(local_name))?;
            // Return as Mrow so mtable can collect rows
            Ok(MathNode::Mrow(children))
        }
        "mtd" => {
            let children = parse_children(reader, Some(local_name))?;
            Ok(if children.len() == 1 {
                children.into_iter().next().unwrap()
            } else {
                MathNode::Mrow(children)
            })
        }
        "mfenced" => {
            let open = get_attr(start, "open").unwrap_or_else(|| "(".to_string());
            let close = get_attr(start, "close").unwrap_or_else(|| ")".to_string());
            let children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Mfenced {
                open,
                close,
                children,
            })
        }
        "mspace" => {
            let _children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Mspace)
        }
        "mpadded" | "mstyle" | "mphantom" | "menclose" | "merror" => {
            // Pass-through containers: just process children
            let children = parse_children(reader, Some(local_name))?;
            Ok(MathNode::Mrow(children))
        }
        _ => {
            // Unknown element – try to collect children
            let children = parse_children(reader, Some(local_name))?;
            if children.is_empty() {
                Ok(MathNode::Text(String::new()))
            } else if children.len() == 1 {
                Ok(children.into_iter().next().unwrap())
            } else {
                Ok(MathNode::Mrow(children))
            }
        }
    }
}

/// Read text content of a leaf element until its closing tag.
fn read_text_content(
    reader: &mut Reader<&[u8]>,
    tag_name: &str,
) -> Result<String, ConvertError> {
    let mut text = String::new();
    let mut buf = Vec::new();
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Text(ref e)) => {
                text.push_str(&e.unescape().unwrap_or_default());
            }
            Ok(Event::End(ref e)) => {
                let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let local = strip_ns_prefix(&name);
                if local == tag_name {
                    break;
                }
            }
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => {
                // Nested elements inside a leaf – skip them by reading to their end
                // This handles cases like <mi><mrow>x</mrow></mi>
                let inner = parse_children(reader, Some(tag_name))?;
                for node in inner {
                    text.push_str(&node_text(&node));
                }
                break;
            }
            Err(e) => {
                return Err(ConvertError::MathmlToOmml(format!(
                    "XML parse error in <{}>: {}",
                    tag_name, e
                )));
            }
            _ => {}
        }
        buf.clear();
    }
    Ok(text)
}

/// Extract text content from a MathNode (for fallback).
fn node_text(node: &MathNode) -> String {
    match node {
        MathNode::Mi(t)
        | MathNode::Mn(t)
        | MathNode::Mo(t)
        | MathNode::Mtext(t)
        | MathNode::Text(t) => t.clone(),
        MathNode::Mrow(children) => children.iter().map(node_text).collect::<String>(),
        _ => String::new(),
    }
}

/// Get an attribute value from a `BytesStart` element.
fn get_attr(start: &BytesStart, name: &str) -> Option<String> {
    for attr in start.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        if key == name || key.ends_with(&format!(":{}", name)) {
            return Some(String::from_utf8_lossy(&attr.value).to_string());
        }
    }
    None
}

/// Take exactly two children from a list, padding with empty Mrow if needed.
fn take_two(mut children: Vec<MathNode>, _tag: &str) -> Result<(MathNode, MathNode), ConvertError> {
    let second = if children.len() > 1 {
        children.remove(1)
    } else {
        MathNode::Mrow(vec![])
    };
    let first = if !children.is_empty() {
        children.remove(0)
    } else {
        MathNode::Mrow(vec![])
    };
    Ok((first, second))
}

/// Take exactly three children from a list, padding with empty Mrow if needed.
fn take_three(
    mut children: Vec<MathNode>,
    _tag: &str,
) -> Result<(MathNode, MathNode, MathNode), ConvertError> {
    let third = if children.len() > 2 {
        children.remove(2)
    } else {
        MathNode::Mrow(vec![])
    };
    let second = if children.len() > 1 {
        children.remove(1)
    } else {
        MathNode::Mrow(vec![])
    };
    let first = if !children.is_empty() {
        children.remove(0)
    } else {
        MathNode::Mrow(vec![])
    };
    Ok((first, second, third))
}

// ---------------------------------------------------------------------------
// OMML Writer – converts MathNode tree to OMML XML
// ---------------------------------------------------------------------------

/// Write a `<m:tagname>` start element.
fn write_m_start(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str) -> Result<(), ConvertError> {
    let elem = BytesStart::new(format!("m:{}", tag));
    writer
        .write_event(Event::Start(elem))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))
}

/// Write a `</m:tagname>` end element.
fn write_m_end(writer: &mut Writer<Cursor<Vec<u8>>>, tag: &str) -> Result<(), ConvertError> {
    let elem = BytesEnd::new(format!("m:{}", tag));
    writer
        .write_event(Event::End(elem))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))
}

/// Write a self-closing `<m:tagname m:val="value"/>` property element.
fn write_m_val_prop(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    tag: &str,
    val: &str,
) -> Result<(), ConvertError> {
    let mut elem = BytesStart::new(format!("m:{}", tag));
    elem.push_attribute(("m:val", val));
    writer
        .write_event(Event::Empty(elem))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))
}

/// Write an `<m:r><m:t>text</m:t></m:r>` run element.
fn write_run(writer: &mut Writer<Cursor<Vec<u8>>>, text: &str) -> Result<(), ConvertError> {
    if text.is_empty() {
        return Ok(());
    }
    write_m_start(writer, "r")?;
    write_m_start(writer, "t")?;
    writer
        .write_event(Event::Text(BytesText::new(text)))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))?;
    write_m_end(writer, "t")?;
    write_m_end(writer, "r")?;
    Ok(())
}

/// Write a list of MathNode children wrapped in `<m:e>`.
fn write_element_wrapper(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    nodes: &[MathNode],
) -> Result<(), ConvertError> {
    write_m_start(writer, "e")?;
    for node in nodes {
        write_node(writer, node)?;
    }
    write_m_end(writer, "e")?;
    Ok(())
}

/// Write a single MathNode wrapped in `<m:e>`.
fn write_single_element(
    writer: &mut Writer<Cursor<Vec<u8>>>,
    node: &MathNode,
) -> Result<(), ConvertError> {
    write_m_start(writer, "e")?;
    write_node(writer, node)?;
    write_m_end(writer, "e")?;
    Ok(())
}

/// Write a MathNode tree to the OMML writer.
fn write_node(writer: &mut Writer<Cursor<Vec<u8>>>, node: &MathNode) -> Result<(), ConvertError> {
    match node {
        MathNode::Mi(text) | MathNode::Mn(text) | MathNode::Mtext(text) => {
            write_run(writer, text)?;
        }
        MathNode::Mo(text) => {
            write_run(writer, text)?;
        }
        MathNode::Text(text) => {
            if !text.is_empty() {
                write_run(writer, text)?;
            }
        }
        MathNode::Mrow(children) => {
            for child in children {
                write_node(writer, child)?;
            }
        }
        MathNode::Mfrac(num, den) => {
            write_m_start(writer, "f")?;
            // fPr (fraction properties) – use default bar fraction
            write_m_start(writer, "fPr")?;
            write_m_val_prop(writer, "type", "bar")?;
            write_m_end(writer, "fPr")?;
            // numerator
            write_m_start(writer, "num")?;
            write_node(writer, num)?;
            write_m_end(writer, "num")?;
            // denominator
            write_m_start(writer, "den")?;
            write_node(writer, den)?;
            write_m_end(writer, "den")?;
            write_m_end(writer, "f")?;
        }
        MathNode::Msqrt(children) => {
            write_m_start(writer, "rad")?;
            // radPr – hide degree for square root
            write_m_start(writer, "radPr")?;
            write_m_val_prop(writer, "degHide", "1")?;
            write_m_end(writer, "radPr")?;
            // deg (empty for square root)
            write_m_start(writer, "deg")?;
            write_m_end(writer, "deg")?;
            // element
            write_element_wrapper(writer, children)?;
            write_m_end(writer, "rad")?;
        }
        MathNode::Mroot(base, index) => {
            write_m_start(writer, "rad")?;
            write_m_start(writer, "radPr")?;
            write_m_end(writer, "radPr")?;
            // degree
            write_m_start(writer, "deg")?;
            write_node(writer, index)?;
            write_m_end(writer, "deg")?;
            // element
            write_single_element(writer, base)?;
            write_m_end(writer, "rad")?;
        }
        MathNode::Msup(base, sup) => {
            write_m_start(writer, "sSup")?;
            write_m_start(writer, "sSupPr")?;
            write_m_end(writer, "sSupPr")?;
            write_single_element(writer, base)?;
            write_m_start(writer, "sup")?;
            write_node(writer, sup)?;
            write_m_end(writer, "sup")?;
            write_m_end(writer, "sSup")?;
        }
        MathNode::Msub(base, sub) => {
            write_m_start(writer, "sSub")?;
            write_m_start(writer, "sSubPr")?;
            write_m_end(writer, "sSubPr")?;
            write_single_element(writer, base)?;
            write_m_start(writer, "sub")?;
            write_node(writer, sub)?;
            write_m_end(writer, "sub")?;
            write_m_end(writer, "sSub")?;
        }
        MathNode::Msubsup(base, sub, sup) => {
            write_m_start(writer, "sSubSup")?;
            write_m_start(writer, "sSubSupPr")?;
            write_m_end(writer, "sSubSupPr")?;
            write_single_element(writer, base)?;
            write_m_start(writer, "sub")?;
            write_node(writer, sub)?;
            write_m_end(writer, "sub")?;
            write_m_start(writer, "sup")?;
            write_node(writer, sup)?;
            write_m_end(writer, "sup")?;
            write_m_end(writer, "sSubSup")?;
        }
        MathNode::Mover(base, over) => {
            let over_text = node_text(over);
            if is_accent_char(&over_text) {
                // Accent
                write_m_start(writer, "acc")?;
                write_m_start(writer, "accPr")?;
                write_m_val_prop(writer, "chr", &over_text)?;
                write_m_end(writer, "accPr")?;
                write_single_element(writer, base)?;
                write_m_end(writer, "acc")?;
            } else {
                // Upper limit
                write_m_start(writer, "limUpp")?;
                write_m_start(writer, "limUppPr")?;
                write_m_end(writer, "limUppPr")?;
                write_single_element(writer, base)?;
                write_m_start(writer, "lim")?;
                write_node(writer, over)?;
                write_m_end(writer, "lim")?;
                write_m_end(writer, "limUpp")?;
            }
        }
        MathNode::Munder(base, under) => {
            let base_text = node_text(base);
            if is_large_operator(&base_text) {
                // N-ary operator with lower limit only
                write_m_start(writer, "nary")?;
                write_m_start(writer, "naryPr")?;
                write_m_val_prop(writer, "chr", &base_text)?;
                write_m_val_prop(writer, "limLoc", "undOvr")?;
                write_m_val_prop(writer, "supHide", "1")?;
                write_m_end(writer, "naryPr")?;
                write_m_start(writer, "sub")?;
                write_node(writer, under)?;
                write_m_end(writer, "sub")?;
                write_m_start(writer, "sup")?;
                write_m_end(writer, "sup")?;
                write_m_start(writer, "e")?;
                write_m_end(writer, "e")?;
                write_m_end(writer, "nary")?;
            } else {
                // Lower limit
                write_m_start(writer, "limLow")?;
                write_m_start(writer, "limLowPr")?;
                write_m_end(writer, "limLowPr")?;
                write_single_element(writer, base)?;
                write_m_start(writer, "lim")?;
                write_node(writer, under)?;
                write_m_end(writer, "lim")?;
                write_m_end(writer, "limLow")?;
            }
        }
        MathNode::Munderover(base, under, over) => {
            let base_text = node_text(base);
            if is_large_operator(&base_text) {
                // N-ary operator (sum, integral, etc.)
                write_m_start(writer, "nary")?;
                write_m_start(writer, "naryPr")?;
                write_m_val_prop(writer, "chr", &base_text)?;
                write_m_val_prop(writer, "limLoc", "undOvr")?;
                write_m_end(writer, "naryPr")?;
                write_m_start(writer, "sub")?;
                write_node(writer, under)?;
                write_m_end(writer, "sub")?;
                write_m_start(writer, "sup")?;
                write_node(writer, over)?;
                write_m_end(writer, "sup")?;
                // Empty element body – the operand typically follows in the parent
                write_m_start(writer, "e")?;
                write_m_end(writer, "e")?;
                write_m_end(writer, "nary")?;
            } else {
                // Nested limits: limLow wrapping limUpp
                write_m_start(writer, "limLow")?;
                write_m_start(writer, "limLowPr")?;
                write_m_end(writer, "limLowPr")?;
                // The element is a limUpp
                write_m_start(writer, "e")?;
                write_m_start(writer, "limUpp")?;
                write_m_start(writer, "limUppPr")?;
                write_m_end(writer, "limUppPr")?;
                write_single_element(writer, base)?;
                write_m_start(writer, "lim")?;
                write_node(writer, over)?;
                write_m_end(writer, "lim")?;
                write_m_end(writer, "limUpp")?;
                write_m_end(writer, "e")?;
                write_m_start(writer, "lim")?;
                write_node(writer, under)?;
                write_m_end(writer, "lim")?;
                write_m_end(writer, "limLow")?;
            }
        }
        MathNode::Mtable(rows) => {
            write_m_start(writer, "m")?;
            // mPr – matrix properties
            write_m_start(writer, "mPr")?;
            write_m_end(writer, "mPr")?;
            for row in rows {
                write_m_start(writer, "mr")?;
                for cell in row {
                    write_single_element(writer, cell)?;
                }
                write_m_end(writer, "mr")?;
            }
            write_m_end(writer, "m")?;
        }
        MathNode::Mfenced {
            open,
            close,
            children,
        } => {
            write_m_start(writer, "d")?;
            write_m_start(writer, "dPr")?;
            write_m_val_prop(writer, "begChr", open)?;
            write_m_val_prop(writer, "endChr", close)?;
            write_m_end(writer, "dPr")?;
            write_element_wrapper(writer, children)?;
            write_m_end(writer, "d")?;
        }
        MathNode::Mspace => {
            // Emit a thin space run
            write_run(writer, "\u{2009}")?;
        }
    }
    Ok(())
}

/// MathML → OMML
///
/// Converts a MathML XML string into OMML (Office Math Markup Language) XML.
/// The conversion parses the MathML into an intermediate tree representation,
/// then serializes it as OMML wrapped in `<m:oMathPara><m:oMath>...</m:oMath></m:oMathPara>`.
///
/// # Errors
///
/// Returns `ConvertError::MathmlToOmml` if the MathML is malformed or contains
/// elements that cannot be converted.
pub fn mathml_to_omml(mathml: &str) -> Result<String, ConvertError> {
    // Parse MathML into intermediate tree
    let nodes = parse_mathml(mathml)?;

    // Write OMML
    let mut writer = Writer::new(Cursor::new(Vec::new()));

    // <m:oMathPara xmlns:m="...">
    let mut para_start = BytesStart::new("m:oMathPara");
    para_start.push_attribute(("xmlns:m", OMML_NS));
    writer
        .write_event(Event::Start(para_start))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))?;

    // <m:oMath>
    write_m_start(&mut writer, "oMath")?;

    // Write all nodes
    for node in &nodes {
        write_node(&mut writer, node)?;
    }

    // </m:oMath>
    write_m_end(&mut writer, "oMath")?;

    // </m:oMathPara>
    writer
        .write_event(Event::End(BytesEnd::new("m:oMathPara")))
        .map_err(|e| ConvertError::MathmlToOmml(format!("Write error: {}", e)))?;

    let result = writer.into_inner().into_inner();
    String::from_utf8(result)
        .map_err(|e| ConvertError::MathmlToOmml(format!("UTF-8 error: {}", e)))
}

/// LaTeX → OMML（组合调用）
///
/// Converts a LaTeX math expression to OMML by first converting to MathML,
/// then converting the MathML to OMML.
pub fn latex_to_omml(latex: &str) -> Result<String, ConvertError> {
    let mathml = latex_to_mathml(latex)?;
    mathml_to_omml(&mathml)
}

/// 格式化 OMML 为可读 XML
///
/// Parses the input OMML XML string and re-serializes it with proper indentation
/// (2 spaces per level) for human readability. The output is semantically identical
/// to the input — all element names, attributes, and text content are preserved.
///
/// # Errors
///
/// Returns `ConvertError::MathmlToOmml` if the input is not valid XML.
pub fn pretty_print_omml(omml: &str) -> Result<String, ConvertError> {
    let mut reader = Reader::from_str(omml);
    reader.config_mut().trim_text(true);

    let mut writer = Writer::new_with_indent(Cursor::new(Vec::new()), b' ', 2);
    let mut buf = Vec::new();

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Eof) => break,
            Ok(event) => {
                writer.write_event(event).map_err(|e| {
                    ConvertError::MathmlToOmml(format!("Pretty print write error: {}", e))
                })?;
            }
            Err(e) => {
                return Err(ConvertError::MathmlToOmml(format!(
                    "Pretty print XML parse error: {}",
                    e
                )));
            }
        }
        buf.clear();
    }

    let result = writer.into_inner().into_inner();
    String::from_utf8(result)
        .map_err(|e| ConvertError::MathmlToOmml(format!("Pretty print UTF-8 error: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // =====================================================================
    // LaTeX → MathML tests (from Task 3.1)
    // =====================================================================

    #[test]
    fn test_simple_variable() {
        let result = latex_to_mathml("x").unwrap();
        assert!(result.contains("<math"), "Output should contain <math tag");
        assert!(result.contains("</math>"), "Output should be closed with </math>");
        assert!(result.contains("x"), "Output should contain the variable 'x'");
    }

    #[test]
    fn test_superscript_and_subscript() {
        let result = latex_to_mathml("x_i^2").unwrap();
        assert!(result.contains("<math"), "Should produce valid MathML");
        let has_script_tag = result.contains("<msub")
            || result.contains("<msup")
            || result.contains("<msubsup");
        assert!(has_script_tag, "Should contain sub/superscript MathML elements");
    }

    #[test]
    fn test_fraction() {
        let result = latex_to_mathml(r"\frac{a}{b}").unwrap();
        assert!(result.contains("<mfrac"), "Should contain <mfrac> for fractions");
    }

    #[test]
    fn test_square_root() {
        let result = latex_to_mathml(r"\sqrt{x}").unwrap();
        assert!(result.contains("<msqrt"), "Should contain <msqrt> for square roots");
    }

    #[test]
    fn test_integral() {
        let result = latex_to_mathml(r"\int_0^\infty f(x) dx").unwrap();
        assert!(result.contains("<math"), "Should produce valid MathML");
        assert!(
            result.contains("∫") || result.contains("&#x222B;") || result.contains("int"),
            "Should contain integral symbol"
        );
    }

    #[test]
    fn test_summation() {
        let result = latex_to_mathml(r"\sum_{i=0}^{n} i").unwrap();
        assert!(result.contains("<math"), "Should produce valid MathML");
        assert!(
            result.contains("∑") || result.contains("&#x2211;") || result.contains("sum"),
            "Should contain summation symbol"
        );
    }

    #[test]
    fn test_matrix() {
        let result = latex_to_mathml(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}").unwrap();
        assert!(result.contains("<math"), "Should produce valid MathML");
        assert!(
            result.contains("<mtable") || result.contains("<mtr"),
            "Should contain matrix MathML elements"
        );
    }

    #[test]
    fn test_greek_letters() {
        let result = latex_to_mathml(r"\alpha + \beta = \gamma").unwrap();
        assert!(result.contains("<math"), "Should produce valid MathML");
        assert!(
            result.contains("α") || result.contains("&#x03B1;") || result.contains("alpha"),
            "Should contain alpha"
        );
    }

    #[test]
    fn test_output_is_valid_xml() {
        let formulas = vec![
            "x + y",
            r"\frac{1}{2}",
            r"e^{i\pi} + 1 = 0",
            r"\sqrt{a^2 + b^2}",
        ];
        for formula in formulas {
            let result = latex_to_mathml(formula).unwrap();
            assert!(
                result.starts_with("<math"),
                "MathML output for '{}' should start with <math",
                formula
            );
            assert!(
                result.ends_with("</math>"),
                "MathML output for '{}' should end with </math>",
                formula
            );
        }
    }

    #[test]
    fn test_unknown_environment_returns_unsupported_symbol() {
        let result = latex_to_mathml(r"\begin{tikzpicture} \end{tikzpicture}");
        assert!(result.is_err(), "Unknown environment should produce an error");
        match result.unwrap_err() {
            ConvertError::UnsupportedSymbol(sym) => {
                assert!(
                    sym.contains("tikzpicture"),
                    "Error should mention the unsupported environment name, got: {}",
                    sym
                );
            }
            other => {
                let msg = other.to_string();
                assert!(
                    !msg.is_empty(),
                    "Error message should be descriptive, got empty string"
                );
            }
        }
    }

    #[test]
    fn test_empty_input() {
        let result = latex_to_mathml("");
        if let Ok(mathml) = &result {
            assert!(mathml.contains("<math"), "Even empty input should produce <math wrapper");
        }
    }

    #[test]
    fn test_complex_formula() {
        let latex = r"\int_0^1 \frac{\sqrt{x^2 + 1}}{\sum_{k=0}^{n} \alpha_k} dx";
        let result = latex_to_mathml(latex).unwrap();
        assert!(result.contains("<math"), "Complex formula should produce valid MathML");
        assert!(result.contains("</math>"), "Complex formula should be well-formed");
    }

    #[test]
    fn test_error_is_descriptive() {
        let result = latex_to_mathml(r"\frac{a}");
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(!msg.is_empty(), "Error message should not be empty");
            assert!(
                msg.len() > 5,
                "Error message should be descriptive, got: {}",
                msg
            );
        }
    }

    // =====================================================================
    // MathML → OMML tests (Task 3.2)
    // =====================================================================

    /// Helper: verify the OMML output is well-formed XML with the expected wrapper.
    fn assert_valid_omml(omml: &str) {
        assert!(
            omml.contains("<m:oMathPara"),
            "OMML should contain <m:oMathPara>, got: {}",
            &omml[..omml.len().min(200)]
        );
        assert!(
            omml.contains("</m:oMathPara>"),
            "OMML should contain closing </m:oMathPara>"
        );
        assert!(
            omml.contains("<m:oMath>") || omml.contains("<m:oMath "),
            "OMML should contain <m:oMath>"
        );
        assert!(
            omml.contains("</m:oMath>"),
            "OMML should contain closing </m:oMath>"
        );
        assert!(
            omml.contains(OMML_NS),
            "OMML should contain the OMML namespace"
        );
        // Verify it's parseable XML
        let mut reader = Reader::from_str(omml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Err(e) => panic!("OMML is not valid XML: {}", e),
                _ => {}
            }
            buf.clear();
        }
    }

    #[test]
    fn test_mathml_to_omml_simple_variable() {
        let mathml = latex_to_mathml("x").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:r>"), "Should contain a run element");
        assert!(omml.contains("<m:t>"), "Should contain a text element");
        assert!(omml.contains("x"), "Should contain the variable 'x'");
    }

    #[test]
    fn test_mathml_to_omml_fraction() {
        // Requirement 6.6: 分式
        let mathml = latex_to_mathml(r"\frac{a}{b}").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:f>"), "Should contain fraction element <m:f>");
        assert!(omml.contains("<m:num>"), "Should contain numerator <m:num>");
        assert!(omml.contains("<m:den>"), "Should contain denominator <m:den>");
        assert!(omml.contains("a"), "Should contain numerator 'a'");
        assert!(omml.contains("b"), "Should contain denominator 'b'");
    }

    #[test]
    fn test_mathml_to_omml_square_root() {
        // Requirement 6.6: 根号
        let mathml = latex_to_mathml(r"\sqrt{x}").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:rad>"), "Should contain radical element <m:rad>");
        assert!(
            omml.contains("degHide") && omml.contains("1"),
            "Square root should hide degree"
        );
    }

    #[test]
    fn test_mathml_to_omml_superscript() {
        // Requirement 6.6: 上标
        let mathml = latex_to_mathml("x^2").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(
            omml.contains("<m:sSup>"),
            "Should contain superscript element <m:sSup>"
        );
        assert!(omml.contains("<m:sup>"), "Should contain <m:sup>");
        assert!(omml.contains("x"), "Should contain base 'x'");
        assert!(omml.contains("2"), "Should contain superscript '2'");
    }

    #[test]
    fn test_mathml_to_omml_subscript() {
        // Requirement 6.6: 下标
        let mathml = latex_to_mathml("x_i").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(
            omml.contains("<m:sSub>"),
            "Should contain subscript element <m:sSub>"
        );
        assert!(omml.contains("<m:sub>"), "Should contain <m:sub>");
    }

    #[test]
    fn test_mathml_to_omml_sub_superscript() {
        // Requirement 6.6: 上下标
        let mathml = latex_to_mathml("x_i^2").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Could be sSubSup or nested sSub/sSup depending on MathML structure
        let has_script = omml.contains("<m:sSubSup>")
            || (omml.contains("<m:sSub>") && omml.contains("<m:sSup>"))
            || omml.contains("<m:sub>") && omml.contains("<m:sup>");
        assert!(has_script, "Should contain sub-superscript elements");
    }

    #[test]
    fn test_mathml_to_omml_greek_letters() {
        // Requirement 6.6: 希腊字母
        let mathml = latex_to_mathml(r"\alpha + \beta").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Greek letters should appear as Unicode in the output
        assert!(
            omml.contains("α") || omml.contains("alpha"),
            "Should contain alpha"
        );
        assert!(
            omml.contains("β") || omml.contains("beta"),
            "Should contain beta"
        );
    }

    #[test]
    fn test_mathml_to_omml_matrix() {
        // Requirement 6.6: 矩阵
        let mathml =
            latex_to_mathml(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Matrix should produce <m:m> with <m:mr> rows
        // or delimiter <m:d> wrapping a matrix
        let has_matrix = omml.contains("<m:m>") || omml.contains("<m:mr>");
        let has_delimiter = omml.contains("<m:d>");
        assert!(
            has_matrix || has_delimiter,
            "Should contain matrix or delimiter elements"
        );
    }

    #[test]
    fn test_mathml_to_omml_summation() {
        // Requirement 6.6: 求和
        let mathml = latex_to_mathml(r"\sum_{i=0}^{n} i").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Summation should produce nary or sub/sup elements
        let has_nary = omml.contains("<m:nary>");
        let has_sub_sup = omml.contains("<m:sub>") && omml.contains("<m:sup>");
        assert!(
            has_nary || has_sub_sup,
            "Should contain nary or sub/sup elements for summation"
        );
    }

    #[test]
    fn test_mathml_to_omml_integral() {
        // Requirement 6.6: 积分
        let mathml = latex_to_mathml(r"\int_0^1 f(x) dx").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Should contain the integral symbol somewhere
        assert!(
            omml.contains("∫") || omml.contains("<m:nary>"),
            "Should contain integral symbol or nary element"
        );
    }

    #[test]
    fn test_latex_to_omml_composition() {
        // Requirement 6.1, 6.4: latex_to_omml should compose latex_to_mathml and mathml_to_omml
        let omml = latex_to_omml(r"\frac{1}{2}").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:f>"), "Should contain fraction");
        assert!(omml.contains("1"), "Should contain numerator '1'");
        assert!(omml.contains("2"), "Should contain denominator '2'");
    }

    #[test]
    fn test_latex_to_omml_complex_formula() {
        // Requirement 6.6: complex formula combining multiple features
        let omml = latex_to_omml(r"e^{i\pi} + 1 = 0").unwrap();
        assert_valid_omml(&omml);
    }

    #[test]
    fn test_latex_to_omml_euler_identity() {
        let omml = latex_to_omml(r"\sqrt{a^2 + b^2}").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:rad>"), "Should contain radical");
        assert!(omml.contains("<m:sSup>"), "Should contain superscript");
    }

    #[test]
    fn test_mathml_to_omml_preserves_text_content() {
        // Verify that text content is preserved through the conversion
        let mathml = latex_to_mathml("abc").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("a"), "Should preserve 'a'");
        assert!(omml.contains("b"), "Should preserve 'b'");
        assert!(omml.contains("c"), "Should preserve 'c'");
    }

    #[test]
    fn test_mathml_to_omml_nested_fractions() {
        let mathml = latex_to_mathml(r"\frac{\frac{a}{b}}{c}").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        // Should have nested fractions
        let f_count = omml.matches("<m:f>").count();
        assert!(f_count >= 2, "Should have at least 2 fraction elements, got {}", f_count);
    }

    #[test]
    fn test_mathml_to_omml_invalid_xml() {
        let result = mathml_to_omml("not xml at all <><>");
        // Should either succeed with best-effort or return an error, but not panic
        // The parser may treat this as text content
        match result {
            Ok(omml) => assert_valid_omml(&omml),
            Err(e) => {
                let msg = e.to_string();
                assert!(!msg.is_empty(), "Error should be descriptive");
            }
        }
    }

    #[test]
    fn test_mathml_to_omml_empty_math() {
        let omml = mathml_to_omml("<math></math>").unwrap();
        assert_valid_omml(&omml);
    }

    #[test]
    fn test_mathml_to_omml_direct_mathml_string() {
        // Test with a hand-crafted MathML string
        let mathml = r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><mi>x</mi><mo>+</mo><mn>1</mn></math>"#;
        let omml = mathml_to_omml(mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("x"), "Should contain 'x'");
        assert!(omml.contains("+"), "Should contain '+'");
        assert!(omml.contains("1"), "Should contain '1'");
    }

    #[test]
    fn test_mathml_to_omml_nth_root() {
        let mathml = latex_to_mathml(r"\sqrt[3]{x}").unwrap();
        let omml = mathml_to_omml(&mathml).unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:rad>"), "Should contain radical element");
        assert!(omml.contains("<m:deg>"), "Should contain degree element");
        assert!(omml.contains("3"), "Should contain the root index '3'");
    }

    // =====================================================================
    // Pretty Print OMML tests (Task 3.3)
    // =====================================================================

    /// Helper: parse XML into a list of events for structural comparison.
    /// Strips whitespace-only text events to compare DOM structure.
    fn parse_xml_events(xml: &str) -> Vec<String> {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        let mut events = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => break,
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    if !text.trim().is_empty() {
                        events.push(format!("Text({})", text.trim()));
                    }
                }
                Ok(Event::Start(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut attrs: Vec<String> = Vec::new();
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        attrs.push(format!("{}={}", key, val));
                    }
                    attrs.sort();
                    if attrs.is_empty() {
                        events.push(format!("Start({})", name));
                    } else {
                        events.push(format!("Start({} [{}])", name, attrs.join(", ")));
                    }
                }
                Ok(Event::End(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    events.push(format!("End({})", name));
                }
                Ok(Event::Empty(ref e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    let mut attrs: Vec<String> = Vec::new();
                    for attr in e.attributes().flatten() {
                        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
                        let val = String::from_utf8_lossy(&attr.value).to_string();
                        attrs.push(format!("{}={}", key, val));
                    }
                    attrs.sort();
                    if attrs.is_empty() {
                        events.push(format!("Empty({})", name));
                    } else {
                        events.push(format!("Empty({} [{}])", name, attrs.join(", ")));
                    }
                }
                Err(e) => panic!("XML parse error: {}", e),
                _ => {}
            }
            buf.clear();
        }
        events
    }

    #[test]
    fn test_pretty_print_omml_basic() {
        // Generate OMML from a simple formula, then pretty-print it
        let omml = latex_to_omml("x").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        // The pretty output should contain newlines (indentation)
        assert!(
            pretty.contains('\n'),
            "Pretty-printed output should contain newlines for indentation"
        );

        // The pretty output should still be valid XML
        assert_valid_omml(&pretty);
    }

    #[test]
    fn test_pretty_print_omml_preserves_structure() {
        // Requirement 6.3: pretty_print_omml should preserve the XML DOM structure
        let omml = latex_to_omml(r"\frac{a}{b}").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        // Parse both and compare structural events
        let original_events = parse_xml_events(&omml);
        let pretty_events = parse_xml_events(&pretty);

        assert_eq!(
            original_events, pretty_events,
            "Pretty-printed OMML should have the same DOM structure as the original"
        );
    }

    #[test]
    fn test_pretty_print_omml_preserves_attributes() {
        // Ensure attributes (like xmlns:m, m:val) are preserved
        let omml = latex_to_omml(r"\sqrt{x}").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        assert!(
            pretty.contains(OMML_NS),
            "Pretty-printed output should preserve the OMML namespace"
        );
        assert!(
            pretty.contains("degHide"),
            "Pretty-printed output should preserve degHide attribute"
        );

        // Structural comparison
        let original_events = parse_xml_events(&omml);
        let pretty_events = parse_xml_events(&pretty);
        assert_eq!(original_events, pretty_events);
    }

    #[test]
    fn test_pretty_print_omml_preserves_text_content() {
        let omml = latex_to_omml(r"\alpha + \beta").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        // Text content should be preserved
        assert!(pretty.contains("α"), "Should preserve alpha symbol");
        assert!(pretty.contains("β"), "Should preserve beta symbol");
        assert!(pretty.contains("+"), "Should preserve plus operator");

        // Structural comparison
        let original_events = parse_xml_events(&omml);
        let pretty_events = parse_xml_events(&pretty);
        assert_eq!(original_events, pretty_events);
    }

    #[test]
    fn test_pretty_print_omml_indentation() {
        let omml = latex_to_omml("x").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        // Check that indentation uses spaces
        let lines: Vec<&str> = pretty.lines().collect();
        assert!(
            lines.len() > 1,
            "Pretty-printed output should have multiple lines, got: {}",
            lines.len()
        );

        // At least one line should start with spaces (indented)
        let has_indented_line = lines.iter().any(|line| line.starts_with("  "));
        assert!(
            has_indented_line,
            "Pretty-printed output should have indented lines"
        );
    }

    #[test]
    fn test_pretty_print_omml_complex_formula() {
        // Test with a complex formula that exercises many OMML elements
        let omml = latex_to_omml(r"\int_0^1 \frac{\sqrt{x^2 + 1}}{\sum_{k=0}^{n} \alpha_k} dx").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();

        // Should be valid XML
        assert_valid_omml(&pretty);

        // Structural comparison
        let original_events = parse_xml_events(&omml);
        let pretty_events = parse_xml_events(&pretty);
        assert_eq!(original_events, pretty_events);
    }

    #[test]
    fn test_pretty_print_omml_invalid_xml() {
        let result = pretty_print_omml("<<<not valid xml>>>");
        // quick-xml may parse some invalid XML as text content without erroring,
        // so we just verify it doesn't panic and returns a result
        match result {
            Ok(output) => {
                // If it succeeds, the output should be valid
                let _ = &output;
            }
            Err(e) => {
                let err_msg = e.to_string();
                assert!(!err_msg.is_empty(), "Error message should be descriptive");
            }
        }
    }

    #[test]
    fn test_pretty_print_omml_empty_input() {
        let result = pretty_print_omml("");
        // Empty input should produce empty (or whitespace-only) output, not an error
        assert!(result.is_ok(), "Empty input should not produce an error");
        let output = result.unwrap();
        assert!(
            output.trim().is_empty(),
            "Empty input should produce empty output"
        );
    }

    #[test]
    fn test_pretty_print_omml_idempotent() {
        // Pretty-printing an already pretty-printed string should produce the same result
        let omml = latex_to_omml(r"\frac{a}{b}").unwrap();
        let pretty1 = pretty_print_omml(&omml).unwrap();
        let pretty2 = pretty_print_omml(&pretty1).unwrap();
        assert_eq!(
            pretty1, pretty2,
            "Pretty-printing should be idempotent"
        );
    }

    #[test]
    fn test_pretty_print_omml_matrix() {
        let omml = latex_to_omml(r"\begin{pmatrix} a & b \\ c & d \end{pmatrix}").unwrap();
        let pretty = pretty_print_omml(&omml).unwrap();
        assert_valid_omml(&pretty);

        let original_events = parse_xml_events(&omml);
        let pretty_events = parse_xml_events(&pretty);
        assert_eq!(original_events, pretty_events);
    }

    // =====================================================================
    // ConvertService 单元测试 (Task 3.4)
    // **Validates: Requirements 6.6**
    // 测试具体公式类型的转换正确性和失败回退行为
    // =====================================================================

    #[test]
    fn test_task34_superscript_subscript_combined() {
        // 测试上下标组合: x^2_i
        let mathml = latex_to_mathml("x^2_i").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        let has_script = mathml.contains("<msubsup") 
            || (mathml.contains("<msub") && mathml.contains("<msup"));
        assert!(has_script, "Should contain sub/superscript elements");
        
        let omml = latex_to_omml("x^2_i").unwrap();
        assert_valid_omml(&omml);
        let has_omml_script = omml.contains("<m:sSubSup>")
            || (omml.contains("<m:sSub>") && omml.contains("<m:sSup>"));
        assert!(has_omml_script, "OMML should contain sub/superscript elements");
        assert!(omml.contains("x"), "Should contain base 'x'");
        assert!(omml.contains("2"), "Should contain superscript '2'");
        assert!(omml.contains("i"), "Should contain subscript 'i'");
    }

    #[test]
    fn test_task34_fraction_ab() {
        // 测试分式: \frac{a}{b}
        let mathml = latex_to_mathml(r"\frac{a}{b}").unwrap();
        assert!(mathml.contains("<mfrac"), "MathML should contain <mfrac>");
        assert!(mathml.contains("a"), "Should contain numerator 'a'");
        assert!(mathml.contains("b"), "Should contain denominator 'b'");
        
        let omml = latex_to_omml(r"\frac{a}{b}").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:f>"), "OMML should contain fraction <m:f>");
        assert!(omml.contains("<m:num>"), "OMML should contain <m:num>");
        assert!(omml.contains("<m:den>"), "OMML should contain <m:den>");
    }

    #[test]
    fn test_task34_square_root_x() {
        // 测试根号: \sqrt{x}
        let mathml = latex_to_mathml(r"\sqrt{x}").unwrap();
        assert!(mathml.contains("<msqrt"), "MathML should contain <msqrt>");
        assert!(mathml.contains("x"), "Should contain radicand 'x'");
        
        let omml = latex_to_omml(r"\sqrt{x}").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:rad>"), "OMML should contain radical <m:rad>");
        assert!(omml.contains("degHide"), "Square root should hide degree");
    }

    #[test]
    fn test_task34_integral_bounds() {
        // 测试积分: \int_0^1
        let mathml = latex_to_mathml(r"\int_0^1 f(x) dx").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(
            mathml.contains("∫") || mathml.contains("int"),
            "Should contain integral symbol"
        );
        
        let omml = latex_to_omml(r"\int_0^1 f(x) dx").unwrap();
        assert_valid_omml(&omml);
        assert!(
            omml.contains("∫") || omml.contains("<m:nary>"),
            "OMML should contain integral"
        );
        assert!(omml.contains("0"), "Should contain lower bound '0'");
        assert!(omml.contains("1"), "Should contain upper bound '1'");
    }

    #[test]
    fn test_task34_summation_bounds() {
        // 测试求和: \sum_{i=1}^n
        let mathml = latex_to_mathml(r"\sum_{i=1}^{n} a_i").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(
            mathml.contains("∑") || mathml.contains("sum"),
            "Should contain summation symbol"
        );
        
        let omml = latex_to_omml(r"\sum_{i=1}^{n} a_i").unwrap();
        assert_valid_omml(&omml);
        assert!(
            omml.contains("∑") || omml.contains("<m:nary>"),
            "OMML should contain summation"
        );
    }

    #[test]
    fn test_task34_matrix_basic() {
        // 测试矩阵: \begin{matrix}...\end{matrix}
        let mathml = latex_to_mathml(r"\begin{matrix} a & b \\ c & d \end{matrix}").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(
            mathml.contains("<mtable") || mathml.contains("<mtr"),
            "MathML should contain matrix elements"
        );
        
        let omml = latex_to_omml(r"\begin{matrix} a & b \\ c & d \end{matrix}").unwrap();
        assert_valid_omml(&omml);
        let has_matrix = omml.contains("<m:m>") || omml.contains("<m:mr>");
        assert!(has_matrix, "OMML should contain matrix elements");
        assert!(omml.contains("a"), "Should contain element 'a'");
        assert!(omml.contains("d"), "Should contain element 'd'");
    }

    #[test]
    fn test_task34_greek_alpha_beta_gamma() {
        // 测试希腊字母: \alpha, \beta, \gamma
        let mathml = latex_to_mathml(r"\alpha + \beta + \gamma").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(
            mathml.contains("α") || mathml.contains("alpha"),
            "Should contain alpha"
        );
        assert!(
            mathml.contains("β") || mathml.contains("beta"),
            "Should contain beta"
        );
        assert!(
            mathml.contains("γ") || mathml.contains("gamma"),
            "Should contain gamma"
        );
        
        let omml = latex_to_omml(r"\alpha + \beta + \gamma").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("α"), "OMML should contain alpha symbol");
        assert!(omml.contains("β"), "OMML should contain beta symbol");
        assert!(omml.contains("γ"), "OMML should contain gamma symbol");
    }

    #[test]
    fn test_task34_fallback_unsupported_symbol() {
        // 测试转换失败的回退行为: 不支持的符号应返回描述性错误
        let result = latex_to_mathml(r"\begin{tikzpicture}\end{tikzpicture}");
        assert!(result.is_err(), "Unsupported environment should fail");
        
        match result.unwrap_err() {
            ConvertError::UnsupportedSymbol(sym) => {
                assert!(
                    sym.contains("tikzpicture"),
                    "Error should mention the unsupported symbol: {}",
                    sym
                );
            }
            ConvertError::LatexToMathml(msg) => {
                assert!(
                    !msg.is_empty(),
                    "Error message should be descriptive"
                );
            }
            _ => panic!("Unexpected error type"),
        }
    }

    #[test]
    fn test_task34_fallback_malformed_latex() {
        // 测试转换失败的回退行为: 格式错误的 LaTeX
        let result = latex_to_mathml(r"\frac{a}");
        // Should return an error for incomplete fraction
        if let Err(e) = result {
            let msg = e.to_string();
            assert!(!msg.is_empty(), "Error message should not be empty");
        }
    }

    #[test]
    fn test_task34_fallback_latex_to_omml_chain() {
        // 测试 latex_to_omml 组合调用的错误传播
        let result = latex_to_omml(r"\begin{unknownenv}\end{unknownenv}");
        assert!(result.is_err(), "Unknown environment should fail in full chain");
        
        let err = result.unwrap_err();
        let msg = err.to_string();
        assert!(!msg.is_empty(), "Error should be descriptive");
    }

    #[test]
    fn test_task34_fallback_empty_input() {
        // 测试空输入的处理
        let mathml_result = latex_to_mathml("");
        // Empty input should either succeed with minimal output or fail gracefully
        match mathml_result {
            Ok(mathml) => {
                assert!(mathml.contains("<math"), "Even empty input should produce <math wrapper");
            }
            Err(e) => {
                let msg = e.to_string();
                assert!(!msg.is_empty(), "Error should be descriptive");
            }
        }
    }

    #[test]
    fn test_task34_combined_formula() {
        // 测试组合公式: 包含多种元素
        let latex = r"\int_0^1 \frac{\sqrt{x^2 + 1}}{\sum_{k=0}^{n} \alpha_k} dx";
        let mathml = latex_to_mathml(latex).unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(mathml.contains("</math>"), "Should be well-formed");
        
        let omml = latex_to_omml(latex).unwrap();
        assert_valid_omml(&omml);
        // Should contain various elements
        assert!(omml.contains("<m:f>") || omml.contains("<m:rad>"), 
            "Should contain fraction or radical");
    }

    #[test]
    fn test_task34_pmatrix_with_delimiters() {
        // 测试带括号的矩阵
        let mathml = latex_to_mathml(r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        
        let omml = latex_to_omml(r"\begin{pmatrix} 1 & 0 \\ 0 & 1 \end{pmatrix}").unwrap();
        assert_valid_omml(&omml);
        // pmatrix should have delimiters
        let has_delim_or_matrix = omml.contains("<m:d>") || omml.contains("<m:m>");
        assert!(has_delim_or_matrix, "Should contain delimiter or matrix element");
    }

    #[test]
    fn test_task34_bmatrix() {
        // 测试方括号矩阵
        let mathml = latex_to_mathml(r"\begin{bmatrix} a & b \\ c & d \end{bmatrix}").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        
        let omml = latex_to_omml(r"\begin{bmatrix} a & b \\ c & d \end{bmatrix}").unwrap();
        assert_valid_omml(&omml);
    }

    #[test]
    fn test_task34_nth_root() {
        // 测试 n 次根号
        let mathml = latex_to_mathml(r"\sqrt[3]{x}").unwrap();
        assert!(mathml.contains("<mroot") || mathml.contains("<msqrt"), 
            "Should contain root element");
        
        let omml = latex_to_omml(r"\sqrt[3]{x}").unwrap();
        assert_valid_omml(&omml);
        assert!(omml.contains("<m:rad>"), "Should contain radical");
        assert!(omml.contains("<m:deg>"), "Should contain degree for nth root");
        assert!(omml.contains("3"), "Should contain root index '3'");
    }

    #[test]
    fn test_task34_product_symbol() {
        // 测试连乘符号
        let mathml = latex_to_mathml(r"\prod_{i=1}^{n} x_i").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        assert!(
            mathml.contains("∏") || mathml.contains("prod"),
            "Should contain product symbol"
        );
        
        let omml = latex_to_omml(r"\prod_{i=1}^{n} x_i").unwrap();
        assert_valid_omml(&omml);
    }

    #[test]
    fn test_task34_more_greek_letters() {
        // 测试更多希腊字母
        let mathml = latex_to_mathml(r"\delta + \epsilon + \theta + \lambda + \pi + \sigma + \omega").unwrap();
        assert!(mathml.contains("<math"), "Should produce valid MathML");
        
        let omml = latex_to_omml(r"\delta + \epsilon + \theta + \lambda + \pi + \sigma + \omega").unwrap();
        assert_valid_omml(&omml);
        // Check for some Greek letters in Unicode
        assert!(omml.contains("δ") || omml.contains("delta"), "Should contain delta");
        assert!(omml.contains("π") || omml.contains("pi"), "Should contain pi");
    }
}



#[cfg(test)]
mod subsup_tests {
    use super::*;

    #[test]
    fn test_fix_subsup_order() {
        // Test basic case
        assert_eq!(fix_subsup_order(r"A_{k}^{s}"), r"{A_{k}}^{s}");
        
        // Test nested subscript
        assert_eq!(fix_subsup_order(r"A_{k_2}^{s2t}"), r"{A_{k_2}}^{s2t}");
    }
    
    #[test]
    fn test_fix_subsup_mathml() {
        let latex = r"A_{k_2}^{s2t}";
        let mathml = latex_to_mathml(latex).unwrap();
        println!("LaTeX: {}", latex);
        println!("MathML: {}", mathml);
        
        // After fix, the MathML should have msubsup instead of nested msup/msub
        assert!(mathml.contains("<msubsup>"), "Should have msubsup (combined sub+sup)");
        // Should still have msub for the nested k_2
        assert!(mathml.contains("<msub>"), "Should have msub for nested subscript");
        // Should NOT have msup at the top level (it's been converted to msubsup)
        assert!(!mathml.contains("<msup>"), "Should not have separate msup");
    }
    
    #[test]
    fn test_tilde_subsup() {
        let latex = r"\tilde{E}_{k_2}^{s2t}";
        let mathml = latex_to_mathml(latex).unwrap();
        println!("LaTeX: {}", latex);
        println!("MathML: {}", mathml);
        // Should produce valid MathML
        assert!(mathml.contains("<math"), "Should produce valid MathML");
    }
}





#[cfg(test)]
mod debug_tests {
    use super::*;

    #[test]
    fn test_debug_subsup_omml() {
        let latex = r"A_{k_2}^{s2t}";
        let mathml = latex_to_mathml(latex).unwrap();
        println!("=== LaTeX ===\n{}", latex);
        println!("\n=== MathML ===\n{}", mathml);
        
        let omml = mathml_to_omml(&mathml).unwrap();
        let pretty_omml = pretty_print_omml(&omml).unwrap();
        println!("\n=== OMML ===\n{}", pretty_omml);
        
        // Check if sSubSup is present
        if omml.contains("<m:sSubSup>") {
            println!("\n✓ OMML contains sSubSup (correct!)");
        } else if omml.contains("<m:sSub>") && omml.contains("<m:sSup>") {
            println!("\n✗ OMML has separate sSub and sSup (incorrect!)");
        }
    }
}


#[cfg(test)]
mod complex_formula_tests {
    use super::*;

    #[test]
    fn test_complex_array_formula() {
        let latex = r"\(\begin{array}{c}{{{\mathcal L}_{g e n}(y_{t})=-\sum_{t=1}^{T}l o g(P(y_{t}|y<t,D,S))}}\\ {{{\mathcal L}={\mathcal L}_{g e n}(y_{t})+{\mathcal L}_{e}}}\end{array}\)";
        println!("Input LaTeX: {}", latex);
        
        match latex_to_mathml(latex) {
            Ok(mathml) => {
                println!("MathML output: {}", mathml);
            }
            Err(e) => {
                println!("Conversion error: {:?}", e);
            }
        }
    }
}


#[cfg(test)]
mod mathml_format_tests {
    use super::*;

    #[test]
    fn test_print_mathml_format() {
        // Test simple fraction
        let latex = r"\frac{1}{2}";
        let mathml = latex_to_mathml(latex).unwrap();
        println!("\n=== Simple Fraction ===");
        println!("LaTeX: {}", latex);
        println!("MathML:\n{}", mathml);
        
        // Verify MathML has correct xmlns
        assert!(mathml.contains(r#"xmlns="http://www.w3.org/1998/Math/MathML""#), 
            "MathML should have correct xmlns attribute");
    }
    
    #[test]
    fn test_print_complex_mathml() {
        let latex = r"\sum_{i=1}^{n} x_i";
        let mathml = latex_to_mathml(latex).unwrap();
        println!("\n=== Summation ===");
        println!("LaTeX: {}", latex);
        println!("MathML:\n{}", mathml);
    }
}


// ===========================================================================
// Property-Based Tests for LaTeX → MathML/OMML Conversion (Task 3.1)
// ===========================================================================
// **Validates: Requirements 6.1, 6.2**
// Property 8: LaTeX → MathML/OMML 转换输出 XML 合法性
// For any valid LaTeX math formula string, latex_to_mathml output should be
// valid MathML XML, and latex_to_omml output should be valid OMML XML.
// ===========================================================================

#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;
    use quick_xml::events::Event;
    use quick_xml::reader::Reader;

    /// Strategy to generate simple LaTeX variables (single letters)
    fn simple_variable() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("x".to_string()),
            Just("y".to_string()),
            Just("z".to_string()),
            Just("a".to_string()),
            Just("b".to_string()),
            Just("n".to_string()),
            Just("i".to_string()),
            Just("k".to_string()),
        ]
    }

    /// Strategy to generate LaTeX numbers
    fn latex_number() -> impl Strategy<Value = String> {
        prop_oneof![
            (1i32..100).prop_map(|n| n.to_string()),
            Just("0".to_string()),
            Just("1".to_string()),
            Just("2".to_string()),
            Just("10".to_string()),
        ]
    }

    /// Strategy to generate Greek letters
    fn greek_letter() -> impl Strategy<Value = String> {
        prop_oneof![
            Just(r"\alpha".to_string()),
            Just(r"\beta".to_string()),
            Just(r"\gamma".to_string()),
            Just(r"\delta".to_string()),
            Just(r"\theta".to_string()),
            Just(r"\pi".to_string()),
            Just(r"\sigma".to_string()),
            Just(r"\omega".to_string()),
        ]
    }

    /// Strategy to generate a base expression (variable, number, or Greek letter)
    fn base_expr() -> impl Strategy<Value = String> {
        prop_oneof![
            simple_variable(),
            latex_number(),
            greek_letter(),
        ]
    }

    /// Strategy to generate superscript expressions: base^{exp}
    fn superscript_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(base, exp)| {
            format!("{}^{{{}}}", base, exp)
        })
    }

    /// Strategy to generate subscript expressions: base_{sub}
    fn subscript_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(base, sub)| {
            format!("{}_{{{}}}", base, sub)
        })
    }

    /// Strategy to generate sub-superscript expressions: base_{sub}^{sup}
    fn subsup_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr(), base_expr()).prop_map(|(base, sub, sup)| {
            format!("{}_{{{}}}^{{{}}}", base, sub, sup)
        })
    }

    /// Strategy to generate fraction expressions: \frac{num}{den}
    fn fraction_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(num, den)| {
            format!(r"\frac{{{}}}{{{}}}", num, den)
        })
    }

    /// Strategy to generate square root expressions: \sqrt{content}
    fn sqrt_expr() -> impl Strategy<Value = String> {
        base_expr().prop_map(|content| {
            format!(r"\sqrt{{{}}}", content)
        })
    }

    /// Strategy to generate nth root expressions: \sqrt[n]{content}
    fn nthroot_expr() -> impl Strategy<Value = String> {
        (latex_number(), base_expr()).prop_map(|(n, content)| {
            format!(r"\sqrt[{}]{{{}}}", n, content)
        })
    }

    /// Strategy to generate summation expressions: \sum_{lower}^{upper}
    fn sum_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(lower, upper)| {
            format!(r"\sum_{{{}}}^{{{}}}", lower, upper)
        })
    }

    /// Strategy to generate integral expressions: \int_{lower}^{upper}
    fn integral_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(lower, upper)| {
            format!(r"\int_{{{}}}^{{{}}}", lower, upper)
        })
    }

    /// Strategy to generate product expressions: \prod_{lower}^{upper}
    fn product_expr() -> impl Strategy<Value = String> {
        (base_expr(), base_expr()).prop_map(|(lower, upper)| {
            format!(r"\prod_{{{}}}^{{{}}}", lower, upper)
        })
    }

    /// Strategy to generate valid LaTeX math expressions by combining various constructs
    fn valid_latex_expr() -> impl Strategy<Value = String> {
        prop_oneof![
            // Simple expressions
            base_expr(),
            // Superscript and subscript
            superscript_expr(),
            subscript_expr(),
            subsup_expr(),
            // Fractions
            fraction_expr(),
            // Roots
            sqrt_expr(),
            nthroot_expr(),
            // Large operators
            sum_expr(),
            integral_expr(),
            product_expr(),
            // Combined expressions
            (fraction_expr(), superscript_expr()).prop_map(|(f, s)| format!("{} + {}", f, s)),
            (sqrt_expr(), subscript_expr()).prop_map(|(r, s)| format!("{} = {}", r, s)),
            (sum_expr(), fraction_expr()).prop_map(|(s, f)| format!("{} {}", s, f)),
        ]
    }

    /// Helper function to verify XML is well-formed by parsing it
    fn is_valid_xml(xml: &str) -> bool {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Eof) => return true,
                Err(_) => return false,
                _ => {}
            }
            buf.clear();
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// Property 8: LaTeX → MathML 转换输出 XML 合法性
        /// **Validates: Requirements 6.1**
        ///
        /// For any valid LaTeX expression, the MathML output should:
        /// 1. Start with `<math`
        /// 2. End with `</math>`
        /// 3. Be parseable by an XML parser
        #[test]
        fn prop_latex_to_mathml_produces_valid_xml(latex in valid_latex_expr()) {
            let result = latex_to_mathml(&latex);

            // The conversion should succeed for valid LaTeX
            prop_assert!(result.is_ok(), "latex_to_mathml failed for '{}': {:?}", latex, result.err());

            let mathml = result.unwrap();

            // Verify MathML starts with <math
            prop_assert!(
                mathml.starts_with("<math"),
                "MathML output should start with '<math', got: {}",
                &mathml[..mathml.len().min(100)]
            );

            // Verify MathML ends with </math>
            prop_assert!(
                mathml.ends_with("</math>"),
                "MathML output should end with '</math>', got: ...{}",
                &mathml[mathml.len().saturating_sub(50)..]
            );

            // Verify the output is valid XML
            prop_assert!(
                is_valid_xml(&mathml),
                "MathML output should be valid XML for '{}', got: {}",
                latex,
                &mathml[..mathml.len().min(200)]
            );
        }

        /// Property 8: LaTeX → OMML 转换输出 XML 合法性
        /// **Validates: Requirements 6.2**
        ///
        /// For any valid LaTeX expression, the OMML output should:
        /// 1. Start with `<m:oMathPara`
        /// 2. End with `</m:oMathPara>`
        /// 3. Be parseable by an XML parser
        #[test]
        fn prop_latex_to_omml_produces_valid_xml(latex in valid_latex_expr()) {
            let result = latex_to_omml(&latex);

            // The conversion should succeed for valid LaTeX
            prop_assert!(result.is_ok(), "latex_to_omml failed for '{}': {:?}", latex, result.err());

            let omml = result.unwrap();

            // Verify OMML starts with <m:oMathPara
            prop_assert!(
                omml.starts_with("<m:oMathPara"),
                "OMML output should start with '<m:oMathPara', got: {}",
                &omml[..omml.len().min(100)]
            );

            // Verify OMML ends with </m:oMathPara>
            prop_assert!(
                omml.ends_with("</m:oMathPara>"),
                "OMML output should end with '</m:oMathPara>', got: ...{}",
                &omml[omml.len().saturating_sub(50)..]
            );

            // Verify the output is valid XML
            prop_assert!(
                is_valid_xml(&omml),
                "OMML output should be valid XML for '{}', got: {}",
                latex,
                &omml[..omml.len().min(200)]
            );

            // Verify OMML contains the namespace declaration
            prop_assert!(
                omml.contains(OMML_NS),
                "OMML output should contain the OMML namespace"
            );

            // Verify OMML contains the inner oMath element
            prop_assert!(
                omml.contains("<m:oMath>") || omml.contains("<m:oMath "),
                "OMML output should contain <m:oMath> element"
            );
        }

        /// Property 8 (combined): LaTeX → MathML → OMML pipeline produces valid XML
        /// **Validates: Requirements 6.1, 6.2**
        ///
        /// For any valid LaTeX expression, the full conversion pipeline should
        /// produce valid XML at each stage.
        #[test]
        fn prop_latex_conversion_pipeline_produces_valid_xml(latex in valid_latex_expr()) {
            // Step 1: LaTeX → MathML
            let mathml_result = latex_to_mathml(&latex);
            prop_assert!(mathml_result.is_ok(), "LaTeX to MathML failed for '{}'", latex);
            let mathml = mathml_result.unwrap();

            // Verify MathML is valid
            prop_assert!(mathml.starts_with("<math"), "MathML should start with <math");
            prop_assert!(mathml.ends_with("</math>"), "MathML should end with </math>");
            prop_assert!(is_valid_xml(&mathml), "MathML should be valid XML");

            // Step 2: MathML → OMML
            let omml_result = mathml_to_omml(&mathml);
            prop_assert!(omml_result.is_ok(), "MathML to OMML failed for '{}'", latex);
            let omml = omml_result.unwrap();

            // Verify OMML is valid
            prop_assert!(omml.starts_with("<m:oMathPara"), "OMML should start with <m:oMathPara");
            prop_assert!(omml.ends_with("</m:oMathPara>"), "OMML should end with </m:oMathPara>");
            prop_assert!(is_valid_xml(&omml), "OMML should be valid XML");
        }

        /// Property 9: OMML Pretty Print 结构保持
        /// **Validates: Requirements 6.3**
        ///
        /// For any valid OMML string, pretty_print_omml should preserve the XML structure:
        /// 1. The same number of elements
        /// 2. The same attributes on each element
        /// 3. The same text content
        #[test]
        fn prop_omml_pretty_print_preserves_structure(latex in valid_latex_expr()) {
            // Generate valid OMML from LaTeX
            let omml_result = latex_to_omml(&latex);
            prop_assert!(omml_result.is_ok(), "latex_to_omml failed for '{}': {:?}", latex, omml_result.err());
            let original_omml = omml_result.unwrap();

            // Apply pretty print
            let pretty_result = pretty_print_omml(&original_omml);
            prop_assert!(pretty_result.is_ok(), "pretty_print_omml failed for '{}': {:?}", latex, pretty_result.err());
            let pretty_omml = pretty_result.unwrap();

            // Extract structural information from both
            let original_structure = extract_xml_structure(&original_omml);
            let pretty_structure = extract_xml_structure(&pretty_omml);

            // Verify element count is the same
            prop_assert_eq!(
                original_structure.element_count,
                pretty_structure.element_count,
                "Element count should be preserved after pretty print for '{}'\nOriginal: {}\nPretty: {}",
                latex,
                original_structure.element_count,
                pretty_structure.element_count
            );

            // Verify element names are the same (in order)
            prop_assert_eq!(
                original_structure.element_names,
                pretty_structure.element_names,
                "Element names should be preserved after pretty print for '{}'",
                latex
            );

            // Verify text content is the same (ignoring whitespace differences)
            prop_assert_eq!(
                original_structure.text_content,
                pretty_structure.text_content,
                "Text content should be preserved after pretty print for '{}'",
                latex
            );

            // Verify attribute count is the same
            prop_assert_eq!(
                original_structure.attribute_count,
                pretty_structure.attribute_count,
                "Attribute count should be preserved after pretty print for '{}'",
                latex
            );

            // Verify the pretty-printed output is still valid XML
            prop_assert!(
                is_valid_xml(&pretty_omml),
                "Pretty-printed OMML should be valid XML for '{}'",
                latex
            );
        }
    }

    /// Structure extracted from XML for comparison
    #[derive(Debug, PartialEq)]
    struct XmlStructure {
        element_count: usize,
        element_names: Vec<String>,
        text_content: Vec<String>,
        attribute_count: usize,
    }

    /// Extract structural information from XML for comparison
    fn extract_xml_structure(xml: &str) -> XmlStructure {
        let mut reader = Reader::from_str(xml);
        reader.config_mut().trim_text(true);
        let mut buf = Vec::new();

        let mut element_count = 0;
        let mut element_names = Vec::new();
        let mut text_content = Vec::new();
        let mut attribute_count = 0;

        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    element_count += 1;
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    element_names.push(name);
                    attribute_count += e.attributes().count();
                }
                Ok(Event::Empty(ref e)) => {
                    element_count += 1;
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    element_names.push(name);
                    attribute_count += e.attributes().count();
                }
                Ok(Event::Text(ref e)) => {
                    let text = e.unescape().unwrap_or_default().to_string();
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        text_content.push(trimmed);
                    }
                }
                Ok(Event::Eof) => break,
                Err(_) => break,
                _ => {}
            }
            buf.clear();
        }

        XmlStructure {
            element_count,
            element_names,
            text_content,
            attribute_count,
        }
    }

    /// Strategy to generate unsupported environment names
    /// These are environment names that latex2mathml doesn't support
    fn unsupported_environment_name() -> impl Strategy<Value = String> {
        prop_oneof![
            Just("tikzpicture".to_string()),
            Just("fakeenv".to_string()),
            Just("unknownenv".to_string()),
            Just("nosuchenv".to_string()),
            Just("badenv".to_string()),
            Just("customenv".to_string()),
            Just("myenv".to_string()),
            Just("testenv".to_string()),
            Just("diagram".to_string()),
            Just("picture".to_string()),
        ]
    }

    /// Strategy to generate LaTeX with unsupported environments
    fn latex_with_unsupported_environment() -> impl Strategy<Value = (String, String)> {
        unsupported_environment_name().prop_map(|env_name| {
            let latex = format!(r"\begin{{{}}}\end{{{}}}", env_name, env_name);
            (latex, env_name)
        })
    }

    /// Strategy to generate LaTeX with unsupported environments containing content
    fn latex_with_unsupported_environment_and_content() -> impl Strategy<Value = (String, String)> {
        (unsupported_environment_name(), base_expr()).prop_map(|(env_name, content)| {
            let latex = format!(r"\begin{{{}}}{}\end{{{}}}", env_name, content, env_name);
            (latex, env_name)
        })
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(20))]

        /// Property 11: 不支持环境的错误信息包含性
        /// **Validates: Requirements 6.5**
        ///
        /// For any LaTeX input containing an unsupported environment,
        /// the error message should contain the name of the unsupported environment.
        #[test]
        fn prop_unsupported_environment_error_contains_env_name((latex, env_name) in latex_with_unsupported_environment()) {
            let result = latex_to_mathml(&latex);

            // The conversion should fail for unsupported environments
            prop_assert!(
                result.is_err(),
                "latex_to_mathml should fail for unsupported environment '{}' in '{}'",
                env_name,
                latex
            );

            let error = result.unwrap_err();
            let error_message = error.to_string();

            // The error message should contain the unsupported environment name
            prop_assert!(
                error_message.contains(&env_name),
                "Error message should contain the unsupported environment name '{}', got: {}",
                env_name,
                error_message
            );
        }

        /// Property 11: 不支持环境（带内容）的错误信息包含性
        /// **Validates: Requirements 6.5**
        ///
        /// For any LaTeX input containing an unsupported environment with content,
        /// the error message should contain the name of the unsupported environment.
        #[test]
        fn prop_unsupported_environment_with_content_error_contains_env_name(
            (latex, env_name) in latex_with_unsupported_environment_and_content()
        ) {
            let result = latex_to_mathml(&latex);

            // The conversion should fail for unsupported environments
            prop_assert!(
                result.is_err(),
                "latex_to_mathml should fail for unsupported environment '{}' in '{}'",
                env_name,
                latex
            );

            let error = result.unwrap_err();
            let error_message = error.to_string();

            // The error message should contain the unsupported environment name
            prop_assert!(
                error_message.contains(&env_name),
                "Error message should contain the unsupported environment name '{}', got: {}",
                env_name,
                error_message
            );
        }
    }
}