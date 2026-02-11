// Property-based tests for FormulaStore
// Task 12.7: LaTeX wrap format correctness (fast-check)
// Validates: Requirements 4.4, 5.5

import { describe, it, expect } from "vitest";
import fc from "fast-check";
import { wrapLatex } from "./formulaStore";

describe("wrapLatex - Property Tests", () => {
  it("Property 5: inline wrap always produces \\(...\\) delimiters", () => {
    fc.assert(
      fc.property(fc.string(), (latex) => {
        const result = wrapLatex(latex, "inline");
        expect(result).toBe(`\\(${latex}\\)`);
        expect(result.startsWith("\\(")).toBe(true);
        expect(result.endsWith("\\)")).toBe(true);
      })
    );
  });

  it("Property 5: display wrap always produces \\[...\\] delimiters", () => {
    fc.assert(
      fc.property(fc.string(), (latex) => {
        const result = wrapLatex(latex, "display");
        expect(result).toBe(`\\[${latex}\\]`);
        expect(result.startsWith("\\[")).toBe(true);
        expect(result.endsWith("\\]")).toBe(true);
      })
    );
  });

  it("Property 5: wrapped output length = input length + 4", () => {
    fc.assert(
      fc.property(
        fc.string(),
        fc.constantFrom("inline" as const, "display" as const),
        (latex, mode) => {
          const result = wrapLatex(latex, mode);
          expect(result.length).toBe(latex.length + 4);
        }
      )
    );
  });

  it("Property 5: original content is preserved inside delimiters", () => {
    fc.assert(
      fc.property(
        fc.string(),
        fc.constantFrom("inline" as const, "display" as const),
        (latex, mode) => {
          const result = wrapLatex(latex, mode);
          // Extract content between delimiters
          const inner = result.slice(2, result.length - 2);
          expect(inner).toBe(latex);
        }
      )
    );
  });
});
