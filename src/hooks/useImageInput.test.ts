import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act, waitFor } from "@testing-library/react";
import {
  useImageInput,
  isValidImageType,
  extractImageFromClipboard,
  extractImageFromDrop,
  hasNonImageFileItems,
  hasNonImageDropFiles,
  readFileAsNumberArray,
  VALID_IMAGE_TYPES,
  INVALID_IMAGE_ERROR,
} from "./useImageInput";
import { useFormulaStore } from "../store/formulaStore";

// ============================================================
// Mock @tauri-apps/api/core invoke
// ============================================================

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

// ============================================================
// Helpers
// ============================================================

function resetStore() {
  useFormulaStore.getState().reset();
}

/** Create a mock DataTransfer with items */
function createMockDataTransfer(
  items: Array<{ kind: string; type: string; file?: File | null }>
): DataTransfer {
  const mockItems = items.map((item) => ({
    kind: item.kind,
    type: item.type,
    getAsFile: () => item.file ?? null,
  }));

  return {
    items: {
      length: mockItems.length,
      ...mockItems.reduce(
        (acc, item, i) => {
          acc[i] = item;
          return acc;
        },
        {} as Record<number, (typeof mockItems)[0]>
      ),
      [Symbol.iterator]: function* () {
        for (let i = 0; i < mockItems.length; i++) {
          yield mockItems[i];
        }
      },
    },
    files: {
      length: items.filter((i) => i.file).length,
      ...items
        .filter((i) => i.file)
        .reduce(
          (acc, item, i) => {
            acc[i] = item.file!;
            return acc;
          },
          {} as Record<number, File>
        ),
    },
  } as unknown as DataTransfer;
}

/** Create a mock File with given type and content */
function createMockFile(
  name: string,
  type: string,
  content: Uint8Array = new Uint8Array([1, 2, 3, 4])
): File {
  return new File([content as unknown as BlobPart], name, { type });
}

/** Create a mock ClipboardEvent */
function createPasteEvent(dataTransfer: DataTransfer | null): ClipboardEvent {
  const event = new Event("paste", {
    bubbles: true,
    cancelable: true,
  }) as ClipboardEvent;
  Object.defineProperty(event, "clipboardData", {
    value: dataTransfer,
    writable: false,
  });
  return event;
}

// ============================================================
// isValidImageType tests
// ============================================================

describe("isValidImageType", () => {
  it.each(VALID_IMAGE_TYPES)("returns true for %s", (type) => {
    expect(isValidImageType(type)).toBe(true);
  });

  it("returns false for text/plain", () => {
    expect(isValidImageType("text/plain")).toBe(false);
  });

  it("returns false for application/pdf", () => {
    expect(isValidImageType("application/pdf")).toBe(false);
  });

  it("returns false for empty string", () => {
    expect(isValidImageType("")).toBe(false);
  });

  it("returns false for image/svg+xml (not in valid list)", () => {
    expect(isValidImageType("image/svg+xml")).toBe(false);
  });
});

// ============================================================
// extractImageFromClipboard tests
// ============================================================

describe("extractImageFromClipboard", () => {
  it("returns null for null clipboardData", () => {
    expect(extractImageFromClipboard(null)).toBeNull();
  });

  it("returns image file for valid image item", () => {
    const file = createMockFile("test.png", "image/png");
    const dt = createMockDataTransfer([
      { kind: "file", type: "image/png", file },
    ]);
    expect(extractImageFromClipboard(dt)).toBe(file);
  });

  it("returns null when only text items present", () => {
    const dt = createMockDataTransfer([
      { kind: "string", type: "text/plain" },
    ]);
    expect(extractImageFromClipboard(dt)).toBeNull();
  });

  it("returns null for non-image file items", () => {
    const file = createMockFile("doc.pdf", "application/pdf");
    const dt = createMockDataTransfer([
      { kind: "file", type: "application/pdf", file },
    ]);
    expect(extractImageFromClipboard(dt)).toBeNull();
  });

  it("returns first valid image when multiple items present", () => {
    const pngFile = createMockFile("img.png", "image/png");
    const jpgFile = createMockFile("img.jpg", "image/jpeg");
    const dt = createMockDataTransfer([
      { kind: "string", type: "text/plain" },
      { kind: "file", type: "image/png", file: pngFile },
      { kind: "file", type: "image/jpeg", file: jpgFile },
    ]);
    expect(extractImageFromClipboard(dt)).toBe(pngFile);
  });
});

// ============================================================
// hasNonImageFileItems tests
// ============================================================

describe("hasNonImageFileItems", () => {
  it("returns false for null", () => {
    expect(hasNonImageFileItems(null)).toBe(false);
  });

  it("returns false when only image files present", () => {
    const dt = createMockDataTransfer([
      { kind: "file", type: "image/png", file: createMockFile("a.png", "image/png") },
    ]);
    expect(hasNonImageFileItems(dt)).toBe(false);
  });

  it("returns true when non-image file present", () => {
    const dt = createMockDataTransfer([
      { kind: "file", type: "application/pdf", file: createMockFile("a.pdf", "application/pdf") },
    ]);
    expect(hasNonImageFileItems(dt)).toBe(true);
  });

  it("returns false when only string items present", () => {
    const dt = createMockDataTransfer([
      { kind: "string", type: "text/plain" },
    ]);
    expect(hasNonImageFileItems(dt)).toBe(false);
  });
});

// ============================================================
// extractImageFromDrop / hasNonImageDropFiles tests
// ============================================================

describe("extractImageFromDrop", () => {
  it("returns null for null dataTransfer", () => {
    expect(extractImageFromDrop(null)).toBeNull();
  });

  it("returns valid image file from drop", () => {
    const file = createMockFile("photo.jpeg", "image/jpeg");
    const dt = createMockDataTransfer([
      { kind: "file", type: "image/jpeg", file },
    ]);
    expect(extractImageFromDrop(dt)).toBe(file);
  });

  it("returns null for non-image files", () => {
    const file = createMockFile("doc.txt", "text/plain");
    const dt = createMockDataTransfer([
      { kind: "file", type: "text/plain", file },
    ]);
    expect(extractImageFromDrop(dt)).toBeNull();
  });
});

describe("hasNonImageDropFiles", () => {
  it("returns false for null", () => {
    expect(hasNonImageDropFiles(null)).toBe(false);
  });

  it("returns true when non-image files present", () => {
    const file = createMockFile("doc.txt", "text/plain");
    const dt = createMockDataTransfer([
      { kind: "file", type: "text/plain", file },
    ]);
    expect(hasNonImageDropFiles(dt)).toBe(true);
  });

  it("returns false when only image files present", () => {
    const file = createMockFile("img.png", "image/png");
    const dt = createMockDataTransfer([
      { kind: "file", type: "image/png", file },
    ]);
    expect(hasNonImageDropFiles(dt)).toBe(false);
  });
});

// ============================================================
// readFileAsNumberArray tests
// ============================================================

describe("readFileAsNumberArray", () => {
  it("reads a blob as number array", async () => {
    const data = new Uint8Array([10, 20, 30, 40]);
    const blob = new Blob([data]);
    const result = await readFileAsNumberArray(blob);
    expect(result).toEqual([10, 20, 30, 40]);
  });

  it("returns empty array for empty blob", async () => {
    const blob = new Blob([]);
    const result = await readFileAsNumberArray(blob);
    expect(result).toEqual([]);
  });
});

// ============================================================
// useImageInput hook tests
// ============================================================

describe("useImageInput", () => {
  beforeEach(() => {
    resetStore();
    mockInvoke.mockReset();
  });

  afterEach(() => {
    resetStore();
  });

  it("returns onDragOver and onDrop handlers", () => {
    const { result } = renderHook(() => useImageInput());
    expect(typeof result.current.onDragOver).toBe("function");
    expect(typeof result.current.onDrop).toBe("function");
  });

  // ----------------------------------------------------------
  // Paste event tests (Req 2.1, 2.3)
  // ----------------------------------------------------------

  describe("paste handling", () => {
    it("processes valid image from paste event (Req 2.1)", async () => {
      const ocrResult = { latex: "x^2", confidence: 0.9 };
      mockInvoke.mockResolvedValue(ocrResult);

      const imageContent = new Uint8Array([137, 80, 78, 71]); // PNG-like
      const file = createMockFile("screenshot.png", "image/png", imageContent);
      const dt = createMockDataTransfer([
        { kind: "file", type: "image/png", file },
      ]);
      const pasteEvent = createPasteEvent(dt);

      renderHook(() => useImageInput());

      act(() => {
        document.dispatchEvent(pasteEvent);
      });

      // Wait for async FileReader + recognizeFormula to complete
      await waitFor(() => {
        expect(mockInvoke).toHaveBeenCalledWith("recognize_formula", {
          image: Array.from(imageContent),
        });
      });

      // Store should have screenshot data
      const state = useFormulaStore.getState();
      expect(state.screenshotData).toEqual(imageContent);
    });

    it("shows error for non-image file paste (Req 2.3)", async () => {
      const file = createMockFile("doc.pdf", "application/pdf");
      const dt = createMockDataTransfer([
        { kind: "file", type: "application/pdf", file },
      ]);
      const pasteEvent = createPasteEvent(dt);

      renderHook(() => useImageInput());

      act(() => {
        document.dispatchEvent(pasteEvent);
      });

      await waitFor(() => {
        const state = useFormulaStore.getState();
        expect(state.error).toBe(INVALID_IMAGE_ERROR);
      });

      // recognizeFormula should NOT have been called
      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it("does not interfere with text-only paste", async () => {
      const dt = createMockDataTransfer([
        { kind: "string", type: "text/plain" },
      ]);
      const pasteEvent = createPasteEvent(dt);

      renderHook(() => useImageInput());

      act(() => {
        document.dispatchEvent(pasteEvent);
      });

      // Allow any async handlers to settle
      await waitFor(() => {
        // No error, no invoke
        expect(useFormulaStore.getState().error).toBeNull();
      });
      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it("cleans up paste listener on unmount", () => {
      const addSpy = vi.spyOn(document, "addEventListener");
      const removeSpy = vi.spyOn(document, "removeEventListener");

      const { unmount } = renderHook(() => useImageInput());

      expect(addSpy).toHaveBeenCalledWith("paste", expect.any(Function));

      unmount();

      expect(removeSpy).toHaveBeenCalledWith("paste", expect.any(Function));

      addSpy.mockRestore();
      removeSpy.mockRestore();
    });
  });

  // ----------------------------------------------------------
  // Drop event tests (Req 2.2, 2.3)
  // ----------------------------------------------------------

  describe("drop handling", () => {
    it("processes valid image from drop event (Req 2.2)", async () => {
      const ocrResult = { latex: "y = mx + b", confidence: 0.85 };
      mockInvoke.mockResolvedValue(ocrResult);

      const imageContent = new Uint8Array([255, 216, 255]); // JPEG-like
      const file = createMockFile("photo.jpg", "image/jpeg", imageContent);
      const dt = createMockDataTransfer([
        { kind: "file", type: "image/jpeg", file },
      ]);

      const { result } = renderHook(() => useImageInput());

      const dropEvent = {
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        dataTransfer: dt,
      } as unknown as React.DragEvent;

      await act(async () => {
        await result.current.onDrop(dropEvent);
      });

      expect(dropEvent.preventDefault).toHaveBeenCalled();
      expect(dropEvent.stopPropagation).toHaveBeenCalled();
      expect(mockInvoke).toHaveBeenCalledWith("recognize_formula", {
        image: Array.from(imageContent),
      });
    });

    it("shows error for non-image file drop (Req 2.3)", async () => {
      const file = createMockFile("data.csv", "text/csv");
      const dt = createMockDataTransfer([
        { kind: "file", type: "text/csv", file },
      ]);

      const { result } = renderHook(() => useImageInput());

      const dropEvent = {
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        dataTransfer: dt,
      } as unknown as React.DragEvent;

      await act(async () => {
        await result.current.onDrop(dropEvent);
      });

      expect(useFormulaStore.getState().error).toBe(INVALID_IMAGE_ERROR);
      expect(mockInvoke).not.toHaveBeenCalled();
    });

    it("onDragOver prevents default to allow drop", () => {
      const { result } = renderHook(() => useImageInput());

      const dragOverEvent = {
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
      } as unknown as React.DragEvent;

      result.current.onDragOver(dragOverEvent);

      expect(dragOverEvent.preventDefault).toHaveBeenCalled();
      expect(dragOverEvent.stopPropagation).toHaveBeenCalled();
    });
  });

  // ----------------------------------------------------------
  // State preservation on error (Req 2.3)
  // ----------------------------------------------------------

  describe("state preservation on invalid input", () => {
    it("maintains current state when invalid file is pasted (Req 2.3)", async () => {
      // Set up some existing state
      useFormulaStore.setState({
        currentLatex: "existing formula",
        originalLatex: "existing formula",
        confidence: 0.95,
      });

      const file = createMockFile("doc.pdf", "application/pdf");
      const dt = createMockDataTransfer([
        { kind: "file", type: "application/pdf", file },
      ]);
      const pasteEvent = createPasteEvent(dt);

      renderHook(() => useImageInput());

      act(() => {
        document.dispatchEvent(pasteEvent);
      });

      await waitFor(() => {
        expect(useFormulaStore.getState().error).toBe(INVALID_IMAGE_ERROR);
      });

      const state = useFormulaStore.getState();
      // Existing state should be preserved
      expect(state.currentLatex).toBe("existing formula");
      expect(state.originalLatex).toBe("existing formula");
      expect(state.confidence).toBe(0.95);
    });

    it("maintains current state when invalid file is dropped (Req 2.3)", async () => {
      useFormulaStore.setState({
        currentLatex: "keep this",
        confidence: 0.8,
      });

      const file = createMockFile("script.js", "application/javascript");
      const dt = createMockDataTransfer([
        { kind: "file", type: "application/javascript", file },
      ]);

      const { result } = renderHook(() => useImageInput());

      const dropEvent = {
        preventDefault: vi.fn(),
        stopPropagation: vi.fn(),
        dataTransfer: dt,
      } as unknown as React.DragEvent;

      await act(async () => {
        await result.current.onDrop(dropEvent);
      });

      const state = useFormulaStore.getState();
      expect(state.error).toBe(INVALID_IMAGE_ERROR);
      expect(state.currentLatex).toBe("keep this");
      expect(state.confidence).toBe(0.8);
    });
  });
});
