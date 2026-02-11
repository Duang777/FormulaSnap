import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { useFormulaStore, wrapLatex } from "./formulaStore";

// ============================================================
// Mock @tauri-apps/api/core invoke
// ============================================================

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// ============================================================
// Helper: reset store between tests
// ============================================================

function resetStore() {
  useFormulaStore.getState().reset();
}

// ============================================================
// wrapLatex unit tests
// ============================================================

describe("wrapLatex", () => {
  it("wraps LaTeX in inline delimiters \\(...\\)", () => {
    const result = wrapLatex("x^2", "inline");
    expect(result).toBe("\\(x^2\\)");
  });

  it("wraps LaTeX in display delimiters \\[...\\]", () => {
    const result = wrapLatex("x^2", "display");
    expect(result).toBe("\\[x^2\\]");
  });

  it("preserves empty LaTeX string in inline mode", () => {
    const result = wrapLatex("", "inline");
    expect(result).toBe("\\(\\)");
  });

  it("preserves empty LaTeX string in display mode", () => {
    const result = wrapLatex("", "display");
    expect(result).toBe("\\[\\]");
  });

  it("preserves complex LaTeX content", () => {
    const latex = "\\frac{a}{b} + \\sqrt{c}";
    const result = wrapLatex(latex, "inline");
    expect(result).toBe("\\(\\frac{a}{b} + \\sqrt{c}\\)");
  });

  it("preserves LaTeX with special characters", () => {
    const latex = "\\int_{0}^{\\infty} e^{-x} dx";
    const result = wrapLatex(latex, "display");
    expect(result).toBe("\\[\\int_{0}^{\\infty} e^{-x} dx\\]");
  });
});

// ============================================================
// Store basic setters tests
// ============================================================

describe("FormulaStore - basic setters", () => {
  beforeEach(() => {
    resetStore();
    mockInvoke.mockReset();
  });

  it("has correct initial state", () => {
    const state = useFormulaStore.getState();
    expect(state.currentLatex).toBe("");
    expect(state.originalLatex).toBe("");
    expect(state.confidence).toBe(0);
    expect(state.screenshotData).toBeNull();
    expect(state.wrapMode).toBe("inline");
    expect(state.isCapturing).toBe(false);
    expect(state.isRecognizing).toBe(false);
    expect(state.isConverting).toBe(false);
    expect(state.error).toBeNull();
    expect(state.historyRecords).toEqual([]);
    expect(state.searchQuery).toBe("");
  });

  it("setLatex updates currentLatex", () => {
    useFormulaStore.getState().setLatex("x^2 + y^2");
    expect(useFormulaStore.getState().currentLatex).toBe("x^2 + y^2");
  });

  it("setWrapMode updates wrapMode", () => {
    useFormulaStore.getState().setWrapMode("display");
    expect(useFormulaStore.getState().wrapMode).toBe("display");
  });

  it("setError updates error", () => {
    useFormulaStore.getState().setError("Something went wrong");
    expect(useFormulaStore.getState().error).toBe("Something went wrong");
  });

  it("reset restores initial state", () => {
    const store = useFormulaStore.getState();
    store.setLatex("test");
    store.setWrapMode("display");
    store.setError("error");
    store.setCapturing(true);

    store.reset();

    const state = useFormulaStore.getState();
    expect(state.currentLatex).toBe("");
    expect(state.wrapMode).toBe("inline");
    expect(state.error).toBeNull();
    expect(state.isCapturing).toBe(false);
  });
});

// ============================================================
// Store Tauri command wrapper tests
// ============================================================

describe("FormulaStore - Tauri command wrappers", () => {
  beforeEach(() => {
    resetStore();
    mockInvoke.mockReset();
  });

  afterEach(() => {
    resetStore();
  });

  // ----------------------------------------------------------
  // startCapture
  // ----------------------------------------------------------

  describe("startCapture", () => {
    it("sets isCapturing to true during capture", async () => {
      mockInvoke.mockImplementation(
        () =>
          new Promise((resolve) => {
            // Check state while the promise is pending
            expect(useFormulaStore.getState().isCapturing).toBe(true);
            resolve([0, 1, 2, 3]);
          })
      );

      await useFormulaStore.getState().startCapture();
      expect(useFormulaStore.getState().isCapturing).toBe(false);
    });

    it("stores screenshot data on success", async () => {
      const imageData = [137, 80, 78, 71]; // PNG magic bytes
      mockInvoke.mockResolvedValue(imageData);

      await useFormulaStore.getState().startCapture();

      const state = useFormulaStore.getState();
      expect(state.screenshotData).toEqual(new Uint8Array(imageData));
      expect(state.isCapturing).toBe(false);
      expect(state.error).toBeNull();
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("Capture failed"));

      await useFormulaStore.getState().startCapture();

      const state = useFormulaStore.getState();
      expect(state.isCapturing).toBe(false);
      expect(state.error).toBe("Capture failed");
    });

    it("clears previous error on new capture", async () => {
      useFormulaStore.getState().setError("previous error");
      mockInvoke.mockResolvedValue([1, 2, 3]);

      await useFormulaStore.getState().startCapture();

      expect(useFormulaStore.getState().error).toBeNull();
    });
  });

  // ----------------------------------------------------------
  // captureRegion
  // ----------------------------------------------------------

  describe("captureRegion", () => {
    it("invokes capture_screen_region with region parameter", async () => {
      const region = { x: 10, y: 20, width: 300, height: 200 };
      mockInvoke.mockResolvedValue([1, 2, 3]);

      await useFormulaStore.getState().captureRegion(region);

      expect(mockInvoke).toHaveBeenCalledWith("capture_screen_region", {
        region,
      });
    });

    it("returns captured data as Uint8Array", async () => {
      const region = { x: 0, y: 0, width: 100, height: 100 };
      mockInvoke.mockResolvedValue([10, 20, 30]);

      const result = await useFormulaStore.getState().captureRegion(region);

      expect(result).toEqual(new Uint8Array([10, 20, 30]));
      expect(useFormulaStore.getState().screenshotData).toEqual(
        new Uint8Array([10, 20, 30])
      );
    });

    it("throws and sets error on failure", async () => {
      const region = { x: 0, y: 0, width: 0, height: 100 };
      mockInvoke.mockRejectedValue("Invalid region");

      await expect(
        useFormulaStore.getState().captureRegion(region)
      ).rejects.toThrow("Invalid region");

      expect(useFormulaStore.getState().error).toBe("Invalid region");
      expect(useFormulaStore.getState().isCapturing).toBe(false);
    });
  });

  // ----------------------------------------------------------
  // recognizeFormula
  // ----------------------------------------------------------

  describe("recognizeFormula", () => {
    it("invokes recognize_formula and updates state", async () => {
      const ocrResult = { latex: "x^2 + y^2 = z^2", confidence: 0.95 };
      mockInvoke.mockResolvedValue(ocrResult);

      const result = await useFormulaStore
        .getState()
        .recognizeFormula([1, 2, 3]);

      expect(mockInvoke).toHaveBeenCalledWith("recognize_formula", {
        image: [1, 2, 3],
      });
      expect(result).toEqual(ocrResult);

      const state = useFormulaStore.getState();
      expect(state.currentLatex).toBe("x^2 + y^2 = z^2");
      expect(state.originalLatex).toBe("x^2 + y^2 = z^2");
      expect(state.confidence).toBe(0.95);
      expect(state.isRecognizing).toBe(false);
    });

    it("sets isRecognizing during recognition", async () => {
      mockInvoke.mockImplementation(
        () =>
          new Promise((resolve) => {
            expect(useFormulaStore.getState().isRecognizing).toBe(true);
            resolve({ latex: "a", confidence: 0.5 });
          })
      );

      await useFormulaStore.getState().recognizeFormula([1]);
      expect(useFormulaStore.getState().isRecognizing).toBe(false);
    });

    it("throws and sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("OCR timeout"));

      await expect(
        useFormulaStore.getState().recognizeFormula([1])
      ).rejects.toThrow("OCR timeout");

      expect(useFormulaStore.getState().isRecognizing).toBe(false);
      expect(useFormulaStore.getState().error).toBe("OCR timeout");
    });
  });

  // ----------------------------------------------------------
  // copyToWord
  // ----------------------------------------------------------

  describe("copyToWord", () => {
    it("converts and copies all formats to clipboard", async () => {
      useFormulaStore.getState().setLatex("E = mc^2");

      mockInvoke
        .mockResolvedValueOnce("<math>...</math>") // convert_to_mathml
        .mockResolvedValueOnce("<m:oMath>...</m:oMath>") // convert_to_omml
        .mockResolvedValueOnce(undefined); // copy_formula_to_clipboard

      await useFormulaStore.getState().copyToWord();

      expect(mockInvoke).toHaveBeenCalledWith("convert_to_mathml", {
        latex: "E = mc^2",
      });
      expect(mockInvoke).toHaveBeenCalledWith("convert_to_omml", {
        latex: "E = mc^2",
      });
      expect(mockInvoke).toHaveBeenCalledWith("copy_formula_to_clipboard", {
        latex: "E = mc^2",
        omml: "<m:oMath>...</m:oMath>",
        mathml: "<math>...</math>",
      });

      const state = useFormulaStore.getState();
      expect(state.isConverting).toBe(false);
      // Success message is stored in error field as a notification
      expect(state.error).toBe("✓ 已复制！直接在 Word 中按 Ctrl+V 粘贴");
    });

    it("sets isConverting during conversion", async () => {
      useFormulaStore.getState().setLatex("x");

      mockInvoke.mockImplementation(
        () =>
          new Promise((resolve) => {
            expect(useFormulaStore.getState().isConverting).toBe(true);
            resolve("<result/>");
          })
      );

      await useFormulaStore.getState().copyToWord();
      expect(useFormulaStore.getState().isConverting).toBe(false);
    });

    it("falls back to LaTeX copy when conversion fails", async () => {
      useFormulaStore.getState().setLatex("\\invalid");
      useFormulaStore.getState().setWrapMode("inline");

      // First call (convert_to_mathml) fails
      mockInvoke
        .mockRejectedValueOnce(new Error("Conversion failed"))
        // Fallback: copy_latex_to_clipboard succeeds
        .mockResolvedValueOnce(undefined);

      await useFormulaStore.getState().copyToWord();

      // Should have called copy_latex_to_clipboard with wrapped LaTeX
      expect(mockInvoke).toHaveBeenCalledWith("copy_latex_to_clipboard", {
        latex: "\\(\\invalid\\)",
      });

      const state = useFormulaStore.getState();
      expect(state.isConverting).toBe(false);
      expect(state.error).toContain("转换失败");
    });
  });

  // ----------------------------------------------------------
  // copyLatex
  // ----------------------------------------------------------

  describe("copyLatex", () => {
    it("copies wrapped LaTeX in inline mode", async () => {
      useFormulaStore.getState().setLatex("a + b");
      useFormulaStore.getState().setWrapMode("inline");
      mockInvoke.mockResolvedValue(undefined);

      await useFormulaStore.getState().copyLatex();

      expect(mockInvoke).toHaveBeenCalledWith("copy_latex_to_clipboard", {
        latex: "\\(a + b\\)",
      });
      expect(useFormulaStore.getState().error).toBeNull();
    });

    it("copies wrapped LaTeX in display mode", async () => {
      useFormulaStore.getState().setLatex("a + b");
      useFormulaStore.getState().setWrapMode("display");
      mockInvoke.mockResolvedValue(undefined);

      await useFormulaStore.getState().copyLatex();

      expect(mockInvoke).toHaveBeenCalledWith("copy_latex_to_clipboard", {
        latex: "\\[a + b\\]",
      });
    });

    it("sets error on clipboard failure", async () => {
      useFormulaStore.getState().setLatex("x");
      mockInvoke.mockRejectedValue(new Error("Clipboard error"));

      await useFormulaStore.getState().copyLatex();

      expect(useFormulaStore.getState().error).toBe("Clipboard error");
    });
  });

  // ----------------------------------------------------------
  // saveToHistory
  // ----------------------------------------------------------

  describe("saveToHistory", () => {
    it("saves current state to history and returns ID", async () => {
      useFormulaStore.setState({
        currentLatex: "x^2",
        originalLatex: "x^2",
        confidence: 0.9,
        screenshotData: new Uint8Array([1, 2, 3]),
      });
      mockInvoke.mockResolvedValue(42);

      const id = await useFormulaStore.getState().saveToHistory();

      expect(id).toBe(42);
      expect(mockInvoke).toHaveBeenCalledWith(
        "save_history",
        expect.objectContaining({
          record: expect.objectContaining({
            original_latex: "x^2",
            confidence: 0.9,
            is_favorite: false,
            thumbnail: [1, 2, 3],
          }),
        })
      );
    });

    it("includes edited_latex when different from original", async () => {
      useFormulaStore.setState({
        currentLatex: "x^3",
        originalLatex: "x^2",
        confidence: 0.8,
        screenshotData: null,
      });
      mockInvoke.mockResolvedValue(1);

      await useFormulaStore.getState().saveToHistory();

      expect(mockInvoke).toHaveBeenCalledWith(
        "save_history",
        expect.objectContaining({
          record: expect.objectContaining({
            original_latex: "x^2",
            edited_latex: "x^3",
            thumbnail: undefined,
          }),
        })
      );
    });

    it("throws and sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("DB error"));

      await expect(
        useFormulaStore.getState().saveToHistory()
      ).rejects.toThrow("DB error");

      expect(useFormulaStore.getState().error).toBe("DB error");
    });
  });

  // ----------------------------------------------------------
  // searchHistory
  // ----------------------------------------------------------

  describe("searchHistory", () => {
    it("searches and updates historyRecords", async () => {
      const records = [
        {
          id: 1,
          created_at: "2024-01-01T00:00:00Z",
          original_latex: "x^2",
          confidence: 0.9,
          engine_version: "1.0",
          is_favorite: false,
        },
      ];
      mockInvoke.mockResolvedValue(records);

      await useFormulaStore.getState().searchHistory("x^2");

      expect(mockInvoke).toHaveBeenCalledWith("search_history", {
        query: "x^2",
      });
      expect(useFormulaStore.getState().historyRecords).toEqual(records);
      expect(useFormulaStore.getState().searchQuery).toBe("x^2");
    });

    it("sets error on search failure", async () => {
      mockInvoke.mockRejectedValue(new Error("Search failed"));

      await useFormulaStore.getState().searchHistory("test");

      expect(useFormulaStore.getState().error).toBe("Search failed");
    });
  });

  // ----------------------------------------------------------
  // toggleFavorite
  // ----------------------------------------------------------

  describe("toggleFavorite", () => {
    it("toggles favorite and updates local state", async () => {
      useFormulaStore.setState({
        historyRecords: [
          {
            id: 1,
            created_at: "2024-01-01T00:00:00Z",
            original_latex: "x",
            confidence: 0.9,
            engine_version: "1.0",
            is_favorite: false,
          },
          {
            id: 2,
            created_at: "2024-01-02T00:00:00Z",
            original_latex: "y",
            confidence: 0.8,
            engine_version: "1.0",
            is_favorite: true,
          },
        ],
      });
      mockInvoke.mockResolvedValue(undefined);

      await useFormulaStore.getState().toggleFavorite(1);

      const records = useFormulaStore.getState().historyRecords;
      expect(records[0].is_favorite).toBe(true);
      expect(records[1].is_favorite).toBe(true); // unchanged
    });

    it("sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("Toggle failed"));

      await useFormulaStore.getState().toggleFavorite(1);

      expect(useFormulaStore.getState().error).toBe("Toggle failed");
    });
  });

  // ----------------------------------------------------------
  // exportTex
  // ----------------------------------------------------------

  describe("exportTex", () => {
    it("invokes export_tex and returns bytes", async () => {
      const texBytes = [37, 33, 84, 69, 88]; // %!TEX
      mockInvoke.mockResolvedValue(texBytes);

      const result = await useFormulaStore
        .getState()
        .exportTex([1, 2], { add_time_comments: true });

      expect(mockInvoke).toHaveBeenCalledWith("export_tex", {
        ids: [1, 2],
        options: { add_time_comments: true },
      });
      expect(result).toEqual(new Uint8Array(texBytes));
    });

    it("throws and sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("Export failed"));

      await expect(
        useFormulaStore
          .getState()
          .exportTex([1], { add_time_comments: false })
      ).rejects.toThrow("Export failed");

      expect(useFormulaStore.getState().error).toBe("Export failed");
    });
  });

  // ----------------------------------------------------------
  // exportDocx
  // ----------------------------------------------------------

  describe("exportDocx", () => {
    it("invokes export_docx and returns bytes", async () => {
      const docxBytes = [80, 75, 3, 4]; // ZIP magic bytes
      mockInvoke.mockResolvedValue(docxBytes);

      const result = await useFormulaStore.getState().exportDocx([1, 2, 3]);

      expect(mockInvoke).toHaveBeenCalledWith("export_docx", {
        ids: [1, 2, 3],
      });
      expect(result).toEqual(new Uint8Array(docxBytes));
    });

    it("throws and sets error on failure", async () => {
      mockInvoke.mockRejectedValue(new Error("DOCX export failed"));

      await expect(
        useFormulaStore.getState().exportDocx([1])
      ).rejects.toThrow("DOCX export failed");

      expect(useFormulaStore.getState().error).toBe("DOCX export failed");
    });
  });
});
