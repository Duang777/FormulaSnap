import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";
import { FormulaPreview, renderLatex } from "./FormulaPreview";

// ============================================================
// renderLatex unit tests (pure function, no DOM needed)
// ============================================================

describe("renderLatex", () => {
  it("renders valid LaTeX to HTML", () => {
    const result = renderLatex("x^2", false);
    expect(result.html).toBeTruthy();
    expect(result.error).toBeNull();
    // KaTeX output should contain the rendered content
    expect(result.html).toContain("katex");
  });

  it("renders valid LaTeX in display mode", () => {
    const result = renderLatex("\\frac{a}{b}", true);
    expect(result.html).toBeTruthy();
    expect(result.error).toBeNull();
  });

  it("returns error for invalid LaTeX (unclosed brace)", () => {
    const result = renderLatex("\\frac{a}{", false);
    expect(result.error).toBeTruthy();
    expect(result.error).not.toBe("");
    expect(result.html).toBe("");
  });

  it("returns error for unknown command with strict mode", () => {
    // KaTeX with throwOnError: true throws on unclosed environments
    const result = renderLatex("\\begin{matrix}", false);
    expect(result.error).toBeTruthy();
    expect(result.html).toBe("");
  });

  it("returns empty html and no error for empty string", () => {
    const result = renderLatex("", false);
    expect(result.html).toBe("");
    expect(result.error).toBeNull();
  });

  it("returns empty html and no error for whitespace-only string", () => {
    const result = renderLatex("   ", false);
    expect(result.html).toBe("");
    expect(result.error).toBeNull();
  });

  it("renders complex formulas correctly", () => {
    const result = renderLatex(
      "\\int_{0}^{\\infty} e^{-x^2} dx = \\frac{\\sqrt{\\pi}}{2}",
      true
    );
    expect(result.html).toBeTruthy();
    expect(result.error).toBeNull();
  });

  it("renders Greek letters", () => {
    const result = renderLatex("\\alpha + \\beta = \\gamma", false);
    expect(result.html).toBeTruthy();
    expect(result.error).toBeNull();
  });
});

// ============================================================
// FormulaPreview component tests
// ============================================================

describe("FormulaPreview", () => {
  it("shows empty state when latex is empty", () => {
    render(<FormulaPreview latex="" displayMode={false} />);
    const emptyEl = screen.getByTestId("formula-preview-empty");
    expect(emptyEl).toBeInTheDocument();
    expect(emptyEl.textContent).toContain("预览区域");
  });

  it("renders valid LaTeX formula", () => {
    render(<FormulaPreview latex="x^2 + y^2" displayMode={false} />);
    const preview = screen.getByTestId("formula-preview");
    expect(preview).toBeInTheDocument();
    // Should contain KaTeX rendered HTML
    expect(preview.innerHTML).toContain("katex");
  });

  it("shows error for invalid LaTeX", () => {
    render(<FormulaPreview latex="\\frac{a}{" displayMode={false} />);
    const errorEl = screen.getByTestId("formula-preview-error");
    expect(errorEl).toBeInTheDocument();
    expect(errorEl.textContent).toContain("渲染错误");
  });

  it("renders in display mode", () => {
    render(<FormulaPreview latex="\\sum_{i=1}^{n} i" displayMode={true} />);
    const preview = screen.getByTestId("formula-preview");
    expect(preview).toBeInTheDocument();
  });

  it("shows error message content for syntax errors", () => {
    // Use truly invalid LaTeX that KaTeX will throw on (unclosed group)
    render(<FormulaPreview latex="\frac{a}{" displayMode={false} />);
    const errorEl = screen.getByTestId("formula-preview-error");
    // Error message should be non-empty and descriptive
    const errorText = errorEl.textContent || "";
    expect(errorText.length).toBeGreaterThan(0);
    expect(errorText).toContain("渲染错误");
  });

  it("updates preview when latex prop changes", () => {
    const { rerender } = render(
      <FormulaPreview latex="x" displayMode={false} />
    );
    expect(screen.getByTestId("formula-preview")).toBeInTheDocument();

    // Change to invalid LaTeX
    rerender(<FormulaPreview latex="\\frac{" displayMode={false} />);
    expect(screen.getByTestId("formula-preview-error")).toBeInTheDocument();

    // Change back to valid LaTeX
    rerender(<FormulaPreview latex="y^2" displayMode={false} />);
    expect(screen.getByTestId("formula-preview")).toBeInTheDocument();
  });
});
