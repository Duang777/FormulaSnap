// FormulaPreview - KaTeX 渲染预览组件
// 使用 KaTeX 实时渲染 LaTeX，处理语法错误显示
// Validates: Requirements 4.1, 4.2, 4.5

import { useMemo } from "react";
import katex from "katex";
import "katex/dist/katex.min.css";
import type { FormulaPreviewProps } from "../../types";

export type { FormulaPreviewProps };

export interface KaTeXRenderResult {
  html: string;
  error: string | null;
  fixedLatex?: string;
}

/**
 * Attempts to fix common LaTeX syntax errors from OCR.
 * Returns the fixed LaTeX string.
 */
export function fixLatexSyntax(latex: string): string {
  let fixed = latex;

  // Detect garbage OCR output (too many repeated escape sequences like \, \; \!)
  // If more than 50% of the content is spacing commands, it's garbage
  const escapeCount = (fixed.match(/\\[,;!]/g) || []).length;
  const totalLength = fixed.length;
  if (escapeCount > 20 && escapeCount * 3 > totalLength * 0.5) {
    // This is garbage OCR output, return empty to show error
    return "";
  }

  // Remove \( \) and \[ \] wrappers (OCR sometimes includes these)
  fixed = fixed.replace(/^\\\(\s*/, "").replace(/\s*\\\)$/, "");
  fixed = fixed.replace(/^\\\[\s*/, "").replace(/\s*\\\]$/, "");

  // Remove $ or $$ wrappers (OCR sometimes includes these)
  fixed = fixed.replace(/^\$\$\s*/, "").replace(/\s*\$\$$/, "");
  fixed = fixed.replace(/^\$\s*/, "").replace(/\s*\$$/, "");

  // Remove unsupported commands
  const unsupportedCommands = [
    /\\displaystyle\s*/g,
    /\\textstyle\s*/g,
    /\\scriptstyle\s*/g,
    /\\scriptscriptstyle\s*/g,
    /\\cal\s*/g,
    /\\it\s*/g,
    /\\bf\s*/g,
    /\\rm\s*/g,
    /\\limits\s*/g,
    /\\nolimits\s*/g,
  ];
  for (const cmd of unsupportedCommands) {
    fixed = fixed.replace(cmd, "");
  }

  // Fix {{ before \end - common OCR mistake: {{\end{...} -> \end{...}
  fixed = fixed.replace(/\{+\\end\{/g, "\\end{");
  
  // Fix {{ before \begin - common OCR mistake: {{\begin{...} -> \begin{...}
  fixed = fixed.replace(/\{+\\begin\{/g, "\\begin{");

  // Fix multiple braces {{{ }}} -> { } (common OCR mistake)
  // Apply multiple times to handle nested cases
  let prevFixed = "";
  while (prevFixed !== fixed) {
    prevFixed = fixed;
    // Replace {{{ with {
    fixed = fixed.replace(/\{\{\{/g, "{");
    // Replace }}} with }
    fixed = fixed.replace(/\}\}\}/g, "}");
    // Replace {{ with {
    fixed = fixed.replace(/\{\{/g, "{");
    // Replace }} with }
    fixed = fixed.replace(/\}\}/g, "}");
  }

  // Fix \left and \right mismatches
  const leftCount = (fixed.match(/\\left[.\s({[\|]/g) || []).length;
  const rightCount = (fixed.match(/\\right[.\s)}\]|\|]/g) || []).length;
  
  if (leftCount > rightCount) {
    // Add missing \right. at the end
    for (let i = 0; i < leftCount - rightCount; i++) {
      fixed = fixed + "\\right.";
    }
  } else if (rightCount > leftCount) {
    // Add missing \left. at the beginning
    for (let i = 0; i < rightCount - leftCount; i++) {
      fixed = "\\left." + fixed;
    }
  }

  // Convert array to matrix (KaTeX doesn't support array well)
  fixed = fixed.replace(/\\begin\{array\}\{[^}]*\}/g, "\\begin{matrix}");
  fixed = fixed.replace(/\\end\{array\}/g, "\\end{matrix}");

  // Fix common OCR mistakes
  fixed = fixed.replace(/\\rlap\{([^}]*)\}/g, "$1");
  fixed = fixed.replace(/\\llap\{([^}]*)\}/g, "$1");
  
  // Remove excessive \qquad and \quad (OCR often adds too many)
  fixed = fixed.replace(/(\\qquad\s*){3,}/g, "\\quad ");
  fixed = fixed.replace(/(\\quad\s*){3,}/g, "\\quad ");
  fixed = fixed.replace(/\\qquad/g, "\\;");
  fixed = fixed.replace(/\\quad/g, "\\;");

  // Fix spaces in function names (OCR mistake): "g e n" -> "gen", "l o g" -> "log"
  fixed = fixed.replace(/g\s+e\s+n/g, "gen");
  fixed = fixed.replace(/l\s+o\s+g/g, "log");
  fixed = fixed.replace(/s\s+i\s+n/g, "sin");
  fixed = fixed.replace(/c\s+o\s+s/g, "cos");
  fixed = fixed.replace(/t\s+a\s+n/g, "tan");
  fixed = fixed.replace(/e\s+x\s+p/g, "exp");
  fixed = fixed.replace(/l\s+n/g, "ln");
  
  // Fix spaced-out common words: "E n c" -> "Enc", "D e c" -> "Dec"
  fixed = fixed.replace(/E\s+n\s+c/g, "Enc");
  fixed = fixed.replace(/D\s+e\s+c/g, "Dec");
  fixed = fixed.replace(/C\s+L\s+S/g, "CLS");
  fixed = fixed.replace(/S\s+E\s+P/g, "SEP");

  // Remove trailing backslash sequences like \;\;\;\;\_
  fixed = fixed.replace(/(\\[;,!]\s*)+\\_\s*$/g, "");
  fixed = fixed.replace(/(\\[;,!]\s*)+$/g, "");
  
  // Fix \_ (escaped underscore) - usually should just be removed or be _
  fixed = fixed.replace(/\\_/g, "_");

  // Remove empty braces
  fixed = fixed.replace(/\{\}/g, "");

  // Clean up multiple spaces
  fixed = fixed.replace(/\s+/g, " ").trim();

  return fixed;
}

/**
 * Renders LaTeX to HTML using KaTeX.
 * Returns both the rendered HTML and any error message.
 * Attempts to fix common syntax errors before rendering.
 *
 * Validates: Requirements 4.5
 */
export function renderLatex(
  latex: string,
  displayMode: boolean
): KaTeXRenderResult {
  if (!latex.trim()) {
    return { html: "", error: null };
  }

  // First try with original LaTeX
  try {
    const html = katex.renderToString(latex, {
      displayMode,
      throwOnError: true,
      strict: false,
    });
    return { html, error: null };
  } catch {
    // Try with fixed LaTeX
    const fixedLatex = fixLatexSyntax(latex);
    try {
      const html = katex.renderToString(fixedLatex, {
        displayMode,
        throwOnError: true,
        strict: false,
      });
      return { html, error: null, fixedLatex };
    } catch (err) {
      // Still failed, return error
      const errorMessage =
        err instanceof Error ? err.message : "未知渲染错误";
      return { html: "", error: errorMessage, fixedLatex };
    }
  }
}

export function FormulaPreview({ latex, displayMode }: FormulaPreviewProps) {
  const { html, error } = useMemo(
    () => renderLatex(latex, displayMode),
    [latex, displayMode]
  );

  if (!latex.trim()) {
    return (
      <div
        className="formula-preview flex items-center justify-center min-h-[120px] neu-card text-gray-400 text-sm"
        data-testid="formula-preview-empty"
      >
        <div className="text-center">
          <div className="text-3xl mb-2 opacity-30">∑</div>
          <div>预览区域</div>
        </div>
      </div>
    );
  }

  // Check if the fixed latex is empty (garbage OCR output)
  const fixedLatex = fixLatexSyntax(latex);
  if (!fixedLatex.trim() && latex.trim()) {
    return (
      <div
        className="formula-preview min-h-[120px] neu-card p-6"
        data-testid="formula-preview-error"
      >
        <div className="text-gray-700 text-sm font-medium mb-2">
          OCR 识别失败
        </div>
        <div className="text-gray-500 text-xs">
          识别结果无效，请重新截图或手动输入公式
        </div>
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="formula-preview min-h-[120px] neu-card p-6"
        data-testid="formula-preview-error"
      >
        <div className="text-gray-700 text-sm font-medium mb-2">
          渲染错误
        </div>
        <div className="text-gray-500 text-xs font-mono break-all neu-inset p-3">
          {error}
        </div>
      </div>
    );
  }

  return (
    <div
      className="formula-preview min-h-[120px] neu-card p-6 flex items-center justify-center overflow-auto"
      data-testid="formula-preview"
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

export default FormulaPreview;
