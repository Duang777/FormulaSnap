// FormulaSnap Zustand Store
// 前端状态管理 - 封装所有 Tauri command 调用

import { create } from "zustand";
import { invoke } from "@tauri-apps/api/core";
import type {
  HistoryRecord,
  OcrResult,
  TexExportOptions,
  WrapMode,
} from "../types";

// ============================================================
// LaTeX Wrap Utility
// ============================================================

/**
 * Wraps a LaTeX string in the appropriate delimiters based on the mode.
 * - inline mode: \(...\)
 * - display mode: \[...\]
 *
 * Validates: Requirements 4.4, 5.5
 */
export function wrapLatex(latex: string, mode: WrapMode): string {
  if (mode === "inline") {
    return `\\(${latex}\\)`;
  }
  return `\\[${latex}\\]`;
}

// ============================================================
// CaptureRegion type (matches Rust CaptureRegion)
// ============================================================

export interface CaptureRegion {
  x: number;
  y: number;
  width: number;
  height: number;
}

// ============================================================
// Store Interface
// ============================================================

export interface FormulaState {
  // 当前编辑状态
  currentLatex: string;
  originalLatex: string;
  confidence: number;
  screenshotData: Uint8Array | null;
  wrapMode: WrapMode;

  // UI 状态
  isCapturing: boolean;
  isRecognizing: boolean;
  isConverting: boolean;
  error: string | null;

  // 历史
  historyRecords: HistoryRecord[];
  searchQuery: string;

  // Basic setters
  setLatex: (latex: string) => void;
  setWrapMode: (mode: WrapMode) => void;
  setError: (error: string | null) => void;
  setCapturing: (isCapturing: boolean) => void;
  setRecognizing: (isRecognizing: boolean) => void;
  setConverting: (isConverting: boolean) => void;
  setScreenshotData: (data: Uint8Array | null) => void;
  setHistoryRecords: (records: HistoryRecord[]) => void;
  setSearchQuery: (query: string) => void;
  reset: () => void;

  // Tauri command wrappers
  startCapture: () => Promise<void>;
  captureRegion: (region: CaptureRegion) => Promise<Uint8Array>;
  recognizeFormula: (image: number[]) => Promise<OcrResult>;
  copyToWord: () => Promise<void>;
  copyLatex: () => Promise<void>;
  saveToHistory: () => Promise<number>;
  searchHistory: (query: string) => Promise<void>;
  toggleFavorite: (id: number) => Promise<void>;
  exportTex: (ids: number[], options: TexExportOptions) => Promise<Uint8Array>;
  exportDocx: (ids: number[]) => Promise<Uint8Array>;
}

// ============================================================
// Initial State
// ============================================================

const initialState = {
  currentLatex: "",
  originalLatex: "",
  confidence: 0,
  screenshotData: null as Uint8Array | null,
  wrapMode: "inline" as WrapMode,
  isCapturing: false,
  isRecognizing: false,
  isConverting: false,
  error: null as string | null,
  historyRecords: [] as HistoryRecord[],
  searchQuery: "",
};

// ============================================================
// Store Implementation
// ============================================================

export const useFormulaStore = create<FormulaState>((set, get) => ({
  ...initialState,

  // ----------------------------------------------------------
  // Basic setters
  // ----------------------------------------------------------
  setLatex: (latex: string) => set({ currentLatex: latex }),
  setWrapMode: (mode: WrapMode) => set({ wrapMode: mode }),
  setError: (error: string | null) => set({ error }),
  setCapturing: (isCapturing: boolean) => set({ isCapturing }),
  setRecognizing: (isRecognizing: boolean) => set({ isRecognizing }),
  setConverting: (isConverting: boolean) => set({ isConverting }),
  setScreenshotData: (data: Uint8Array | null) => set({ screenshotData: data }),
  setHistoryRecords: (records: HistoryRecord[]) =>
    set({ historyRecords: records }),
  setSearchQuery: (query: string) => set({ searchQuery: query }),
  reset: () => set(initialState),

  // ----------------------------------------------------------
  // Tauri Command Wrappers
  // ----------------------------------------------------------

  /**
   * Start capture mode - invokes capture_screenshot on the backend.
   * Sets isCapturing state and handles errors.
   */
  startCapture: async () => {
    set({ isCapturing: true, error: null });
    try {
      const imageBytes = await invoke<number[]>("capture_screenshot");
      const data = new Uint8Array(imageBytes);
      set({ screenshotData: data, isCapturing: false });
    } catch (err) {
      set({
        isCapturing: false,
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  /**
   * Capture a specific screen region - invokes capture_screen_region.
   * Returns the captured PNG bytes as Uint8Array.
   */
  captureRegion: async (region: CaptureRegion) => {
    set({ isCapturing: true, error: null });
    try {
      const imageBytes = await invoke<number[]>("capture_screen_region", {
        region,
      });
      const data = new Uint8Array(imageBytes);
      set({ screenshotData: data, isCapturing: false });
      return data;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      set({ isCapturing: false, error: errorMsg });
      throw new Error(errorMsg);
    }
  },

  /**
   * Recognize formula from image bytes - invokes recognize_formula.
   * Sets isRecognizing state and updates currentLatex/confidence on success.
   */
  recognizeFormula: async (image: number[]) => {
    set({ isRecognizing: true, error: null });
    try {
      const result = await invoke<OcrResult>("recognize_formula", { image });
      set({
        isRecognizing: false,
        currentLatex: result.latex,
        originalLatex: result.latex,
        confidence: result.confidence,
      });
      return result;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      set({ isRecognizing: false, error: errorMsg });
      throw new Error(errorMsg);
    }
  },

  /**
   * Copy formula to Word - copies MathML to clipboard.
   * MathML is fixed to use msubsup for correct subscript/superscript rendering.
   * User needs to use Ctrl+Shift+V (Keep Text Only) to paste.
   *
   * Validates: Requirements 5.1, 5.2, 5.4
   */
  copyToWord: async () => {
    const { currentLatex } = get();
    set({ isConverting: true, error: null });
    try {
      // Convert LaTeX → MathML (with msubsup fix)
      const mathml = await invoke<string>("convert_to_mathml", {
        latex: currentLatex,
      });
      const omml = await invoke<string>("convert_to_omml", {
        latex: currentLatex,
      });
      
      // Write MathML to clipboard - Word recognizes it
      await invoke("copy_formula_to_clipboard", {
        latex: currentLatex,
        omml,
        mathml,
      });
      set({ 
        isConverting: false,
        // 提示用户直接粘贴
        error: "✓ 已复制！直接在 Word 中按 Ctrl+V 粘贴",
      });
      // 6秒后清除提示
      setTimeout(() => {
        const { error } = get();
        if (error?.includes("已复制")) {
          set({ error: null });
        }
      }, 6000);
    } catch (err) {
      // Fallback: copy plain LaTeX with wrap format and notify user
      const { wrapMode } = get();
      const wrappedLatex = wrapLatex(currentLatex, wrapMode);
      try {
        await invoke("copy_latex_to_clipboard", { latex: wrappedLatex });
        set({
          isConverting: false,
          error: "转换失败，已复制 LaTeX。在 Word 公式编辑器中粘贴",
        });
      } catch (fallbackErr) {
        const errorMsg =
          fallbackErr instanceof Error
            ? fallbackErr.message
            : String(fallbackErr);
        set({ isConverting: false, error: errorMsg });
      }
    }
  },

  /**
   * Copy LaTeX to clipboard with the current wrap format.
   * Uses wrapLatex to apply inline \(...\) or display \[...\] delimiters.
   *
   * Validates: Requirements 5.5, 4.4
   */
  copyLatex: async () => {
    const { currentLatex, wrapMode } = get();
    set({ error: null });
    try {
      const wrappedLatex = wrapLatex(currentLatex, wrapMode);
      await invoke("copy_latex_to_clipboard", { latex: wrappedLatex });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  /**
   * Save current formula to history - invokes save_history.
   * Returns the new record ID.
   */
  saveToHistory: async () => {
    const { currentLatex, originalLatex, confidence, screenshotData } = get();
    set({ error: null });
    try {
      const record: HistoryRecord = {
        created_at: new Date().toISOString(),
        original_latex: originalLatex,
        edited_latex: currentLatex !== originalLatex ? currentLatex : undefined,
        confidence,
        engine_version: "pix2tex-onnx-1.0",
        thumbnail: screenshotData ? Array.from(screenshotData) : undefined,
        is_favorite: false,
      };
      const id = await invoke<number>("save_history", { record });
      return id;
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      set({ error: errorMsg });
      throw new Error(errorMsg);
    }
  },

  /**
   * Search history records by keyword - invokes search_history.
   * Updates historyRecords and searchQuery state.
   */
  searchHistory: async (query: string) => {
    set({ searchQuery: query, error: null });
    try {
      const records = await invoke<HistoryRecord[]>("search_history", {
        query,
      });
      set({ historyRecords: records });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  /**
   * Toggle favorite status of a history record - invokes toggle_favorite.
   * Updates the local historyRecords state to reflect the change.
   */
  toggleFavorite: async (id: number) => {
    set({ error: null });
    try {
      await invoke("toggle_favorite", { id });
      // Update local state to reflect the toggled favorite
      const { historyRecords } = get();
      const updatedRecords = historyRecords.map((record) =>
        record.id === id
          ? { ...record, is_favorite: !record.is_favorite }
          : record
      );
      set({ historyRecords: updatedRecords });
    } catch (err) {
      set({
        error: err instanceof Error ? err.message : String(err),
      });
    }
  },

  /**
   * Export selected history records as .tex file - invokes export_tex.
   * Returns the .tex file bytes.
   */
  exportTex: async (ids: number[], options: TexExportOptions) => {
    set({ error: null });
    try {
      const bytes = await invoke<number[]>("export_tex", { ids, options });
      return new Uint8Array(bytes);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      set({ error: errorMsg });
      throw new Error(errorMsg);
    }
  },

  /**
   * Export selected history records as .docx file - invokes export_docx.
   * Returns the .docx file bytes.
   */
  exportDocx: async (ids: number[]) => {
    set({ error: null });
    try {
      const bytes = await invoke<number[]>("export_docx", { ids });
      return new Uint8Array(bytes);
    } catch (err) {
      const errorMsg = err instanceof Error ? err.message : String(err);
      set({ error: errorMsg });
      throw new Error(errorMsg);
    }
  },
}));
