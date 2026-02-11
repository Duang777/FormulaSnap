// Property-based tests for FormulaPreview
// Task 12.7: Invalid LaTeX rendering error handling (fast-check)
// Validates: Requirements 4.5

import { describe, it, expect } from "vitest";
import fc from "fast-check";
import { renderLatex } from "./FormulaPreview";

describe("renderLatex - Property Tests", () => {
  it("Property 6: renderLatex never throws, always returns a result object", () => {
    fc.assert(
      fc.property(
        fc.string(),
        fc.boolean(),
        (latex, displayMode) => {
          // Should never throw, regardless of input
          const result = renderLatex(latex, displayMode);
          expect(result).toBeDefined();
          expect(typeof result.html).toBe("string");
          expect(result.error === null || typeof result.error === "string").toBe(true);
        }
      )
    );
  });

  it("Property 6: html and error are mutually exclusive (except empty input)", () => {
    fc.assert(
      fc.property(
        fc.string({ minLength: 1 }).filter((s) => s.trim().length > 0),
        fc.boolean(),
        (latex, displayMode) => {
          const result = renderLatex(latex, displayMode);
          // Either html is non-empty and error is null, or html is empty and error is non-null
          if (result.error !== null) {
            expect(result.html).toBe("");
          }
          if (result.html !== "") {
            expect(result.error).toBeNull();
          }
        }
      )
    );
  });

  it("Property 6: empty/whitespace input always returns empty html and no error", () => {
    fc.assert(
      fc.property(
        fc.stringOf(fc.constantFrom(" ", "\t", "\n", "\r")),
        fc.boolean(),
        (whitespace, displayMode) => {
          const result = renderLatex(whitespace, displayMode);
          expect(result.html).toBe("");
          expect(result.error).toBeNull();
        }
      )
    );
  });

  it("Property 6: valid simple LaTeX always renders successfully", () => {
    const validLatexArb = fc.constantFrom(
      "x", "y", "z", "a+b", "1+2", "x^2", "x_i",
      "\\alpha", "\\beta", "\\gamma",
      "\\frac{a}{b}", "\\sqrt{x}",
    );

    fc.assert(
      fc.property(validLatexArb, fc.boolean(), (latex, displayMode) => {
        const result = renderLatex(latex, displayMode);
        expect(result.html).not.toBe("");
        expect(result.error).toBeNull();
        expect(result.html).toContain("katex");
      })
    );
  });
});
