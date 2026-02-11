// App Shell - 主流程串联
// 截图 → 预处理 → OCR → 编辑/预览 → 复制/保存完整流程
// Validates: Requirements 1.2, 3.5, 9.4

import { useCallback, useState, useEffect } from "react";
import { FormulaEditor } from "./components/FormulaEditor";
import { FormulaPreview } from "./components/FormulaPreview";
import { CaptureOverlay } from "./components/CaptureOverlay";
import { ActionBar } from "./components/ActionBar";
import { HistoryPanel } from "./components/HistoryPanel";
import { useImageInput } from "./hooks/useImageInput";
import { useFormulaStore } from "./store/formulaStore";
import type { HistoryRecord } from "./types";

/**
 * Helper: trigger a browser file download from a Uint8Array.
 */
function downloadFile(data: Uint8Array, filename: string, mimeType: string) {
  const blob = new Blob([data as BlobPart], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  document.body.removeChild(a);
  URL.revokeObjectURL(url);
}

export default function App() {
  // ============================================================
  // Store state
  // ============================================================
  const currentLatex = useFormulaStore((s) => s.currentLatex);
  const wrapMode = useFormulaStore((s) => s.wrapMode);
  const isCapturing = useFormulaStore((s) => s.isCapturing);
  const isRecognizing = useFormulaStore((s) => s.isRecognizing);
  const isConverting = useFormulaStore((s) => s.isConverting);
  const error = useFormulaStore((s) => s.error);
  const historyRecords = useFormulaStore((s) => s.historyRecords);

  const setLatex = useFormulaStore((s) => s.setLatex);
  const setWrapMode = useFormulaStore((s) => s.setWrapMode);
  const setCapturing = useFormulaStore((s) => s.setCapturing);
  const setError = useFormulaStore((s) => s.setError);
  const reset = useFormulaStore((s) => s.reset);
  const captureRegion = useFormulaStore((s) => s.captureRegion);
  const recognizeFormula = useFormulaStore((s) => s.recognizeFormula);
  const copyToWord = useFormulaStore((s) => s.copyToWord);
  const copyLatex = useFormulaStore((s) => s.copyLatex);
  const saveToHistory = useFormulaStore((s) => s.saveToHistory);
  const searchHistory = useFormulaStore((s) => s.searchHistory);
  const exportTex = useFormulaStore((s) => s.exportTex);
  const exportDocx = useFormulaStore((s) => s.exportDocx);

  // ============================================================
  // Local UI state
  // ============================================================
  const [sidebarOpen, setSidebarOpen] = useState(true);

  // ============================================================
  // Image input (paste & drag-drop) - Req 2.1, 2.2, 2.3
  // ============================================================
  const { onDragOver, onDrop } = useImageInput();

  // ============================================================
  // Auto-dismiss error toast after 5 seconds
  // ============================================================
  useEffect(() => {
    if (!error) return;
    const timer = setTimeout(() => setError(null), 5000);
    return () => clearTimeout(timer);
  }, [error, setError]);

  // ============================================================
  // Load history on mount
  // ============================================================
  useEffect(() => {
    searchHistory("");
  }, [searchHistory]);

  // ============================================================
  // Capture flow - Req 1.2
  // ============================================================
  const handleStartCapture = useCallback(() => {
    setCapturing(true);
  }, [setCapturing]);

  const handleCaptureComplete = useCallback(
    async (regionData: Uint8Array) => {
      // Decode region coordinates from the Uint8Array
      const view = new DataView(regionData.buffer);
      const x = view.getInt32(0, true);
      const y = view.getInt32(4, true);
      const width = view.getInt32(8, true);
      const height = view.getInt32(12, true);

      try {
        // Req 1.2: Pass screenshot to OCR within 300ms and show recognizing state
        const imageData = await captureRegion({ x, y, width, height });
        await recognizeFormula(Array.from(imageData));
      } catch {
        // Errors are already set in the store by captureRegion/recognizeFormula
      }
    },
    [captureRegion, recognizeFormula]
  );

  const handleCaptureCancel = useCallback(() => {
    setCapturing(false);
  }, [setCapturing]);

  // ============================================================
  // Cancel recognition - Req 9.4
  // ============================================================
  const handleCancelRecognition = useCallback(() => {
    reset();
  }, [reset]);

  // ============================================================
  // ActionBar handlers
  // ============================================================
  const handleRetry = useCallback(() => {
    handleStartCapture();
  }, [handleStartCapture]);

  const handleSave = useCallback(async () => {
    try {
      await saveToHistory();
      // Refresh history list after saving
      await searchHistory("");
    } catch {
      // Error already set in store
    }
  }, [saveToHistory, searchHistory]);

  // ============================================================
  // HistoryPanel handlers
  // ============================================================
  const handleHistorySelect = useCallback(
    (record: HistoryRecord) => {
      // Req 7.5: Load record's LaTeX into editor
      const latex = record.edited_latex || record.original_latex;
      setLatex(latex);
    },
    [setLatex]
  );

  const handleHistoryCopyToWord = useCallback(
    async (record: HistoryRecord) => {
      const latex = record.edited_latex || record.original_latex;
      setLatex(latex);
      // Small delay to let state update, then trigger copy
      setTimeout(() => copyToWord(), 50);
    },
    [setLatex, copyToWord]
  );

  const handleHistoryCopyLatex = useCallback(
    async (record: HistoryRecord) => {
      const latex = record.edited_latex || record.original_latex;
      setLatex(latex);
      setTimeout(() => copyLatex(), 50);
    },
    [setLatex, copyLatex]
  );

  // ============================================================
  // Export handlers - Req 8.1, 8.2
  // ============================================================
  const handleExportTex = useCallback(async () => {
    const ids = historyRecords
      .filter((r) => r.id != null)
      .map((r) => r.id as number);
    if (ids.length === 0) {
      setError("没有可导出的历史记录");
      return;
    }
    try {
      const data = await exportTex(ids, { add_time_comments: true });
      downloadFile(data, "formulas.tex", "application/x-tex");
    } catch {
      // Error already set in store
    }
  }, [historyRecords, exportTex, setError]);

  const handleExportDocx = useCallback(async () => {
    const ids = historyRecords
      .filter((r) => r.id != null)
      .map((r) => r.id as number);
    if (ids.length === 0) {
      setError("没有可导出的历史记录");
      return;
    }
    try {
      const data = await exportDocx(ids);
      downloadFile(data, "formulas.docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document");
    } catch {
      // Error already set in store
    }
  }, [historyRecords, exportDocx, setError]);

  // ============================================================
  // Render
  // ============================================================
  return (
    <div
      className="h-screen flex flex-col overflow-hidden bg-surface-100"
      onDragOver={onDragOver}
      onDrop={onDrop}
      data-testid="app-shell"
    >
      {/* ====== Header ====== */}
      <header className="flex items-center justify-between px-6 py-4 flex-shrink-0">
        <div className="flex items-center gap-4">
          <h1 className="text-xl font-semibold text-gray-800 tracking-tight">📐 FormulaSnap</h1>
          <button
            onClick={handleStartCapture}
            className="px-5 py-2.5 text-sm font-medium text-white bg-gray-900 rounded-full btn-soft shadow-button"
            aria-label="截图"
            data-testid="capture-btn"
          >
            截图识别
          </button>
        </div>

        <div className="flex items-center gap-3">
          <button
            onClick={handleExportTex}
            className="w-10 h-10 flex items-center justify-center text-gray-600 bg-white rounded-full shadow-soft btn-soft"
            aria-label="导出 .tex"
            data-testid="export-tex-btn"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
            </svg>
          </button>
          <button
            onClick={handleExportDocx}
            className="w-10 h-10 flex items-center justify-center text-gray-600 bg-white rounded-full shadow-soft btn-soft"
            aria-label="导出 .docx"
            data-testid="export-docx-btn"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 16v1a3 3 0 003 3h10a3 3 0 003-3v-1m-4-4l-4 4m0 0l-4-4m4 4V4" />
            </svg>
          </button>
          <button
            onClick={() => setSidebarOpen(!sidebarOpen)}
            className="w-10 h-10 flex items-center justify-center text-gray-600 bg-white rounded-full shadow-soft btn-soft"
            aria-label={sidebarOpen ? "收起历史面板" : "展开历史面板"}
            data-testid="sidebar-toggle-btn"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
          </button>
        </div>
      </header>

      {/* ====== Main Content ====== */}
      <div className="flex flex-1 overflow-hidden px-6 pb-6 gap-6">
        {/* Left panel: Editor + Preview + ActionBar */}
        <main
          className="flex-1 flex flex-col overflow-y-auto gap-5"
          data-testid="main-panel"
        >
          {/* Recognizing state */}
          {isRecognizing && (
            <div
              className="flex items-center justify-center gap-4 p-6 neu-card animate-fade-in"
              data-testid="recognizing-indicator"
            >
              <span
                className="inline-block w-5 h-5 border-2 border-gray-800 border-t-transparent rounded-full animate-spin"
                role="status"
                aria-label="识别中"
              />
              <span className="text-gray-700 text-sm font-medium">
                正在识别公式...
              </span>
              <button
                onClick={handleCancelRecognition}
                className="ml-2 px-4 py-1.5 text-sm font-medium text-gray-600 bg-white rounded-full shadow-button btn-soft"
                aria-label="取消识别"
                data-testid="cancel-recognition-btn"
              >
                取消
              </button>
            </div>
          )}

          {/* Error display for OCR failure */}
          {!isRecognizing && error && error.includes("未检测到") && (
            <div
              className="flex items-center justify-between p-5 neu-card animate-slide-up"
              data-testid="ocr-empty-warning"
            >
              <span className="text-gray-600 text-sm">
                可能未检测到公式
              </span>
              <div className="flex items-center gap-3">
                <button
                  onClick={handleRetry}
                  className="px-4 py-1.5 text-sm font-medium text-gray-700 bg-white rounded-full shadow-button btn-soft"
                  aria-label="重试识别"
                >
                  重试
                </button>
                <span className="text-gray-400 text-xs">
                  或手动编辑
                </span>
              </div>
            </div>
          )}

          {/* Formula Editor */}
          <FormulaEditor
            latex={currentLatex}
            onChange={setLatex}
            wrapMode={wrapMode}
            onWrapModeChange={setWrapMode}
          />

          {/* Formula Preview */}
          <FormulaPreview
            latex={currentLatex}
            displayMode={wrapMode === "display"}
          />

          {/* Action Bar */}
          <ActionBar
            onCopyToWord={copyToWord}
            onCopyLatex={copyLatex}
            onRetry={handleRetry}
            onSave={handleSave}
            isConverting={isConverting}
          />
        </main>

        {/* Right panel: History sidebar */}
        {sidebarOpen && (
          <aside
            className="w-80 neu-card flex-shrink-0 overflow-hidden animate-fade-in"
            data-testid="history-sidebar"
          >
            <HistoryPanel
              onSelect={handleHistorySelect}
              onCopyToWord={handleHistoryCopyToWord}
              onCopyLatex={handleHistoryCopyLatex}
            />
          </aside>
        )}
      </div>

      {/* ====== Error Toast ====== */}
      {error && (
        <div
          className="fixed bottom-6 right-6 max-w-sm px-5 py-4 bg-gray-900 text-white text-sm rounded-2xl shadow-card z-50 flex items-center gap-3 animate-slide-up"
          data-testid="error-toast"
          role="alert"
        >
          <span className="flex-1">{error}</span>
          <button
            onClick={() => setError(null)}
            className="text-gray-400 hover:text-white transition-colors"
            aria-label="关闭错误提示"
          >
            <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M6 18L18 6M6 6l12 12" />
            </svg>
          </button>
        </div>
      )}

      {/* ====== Capture Overlay ====== */}
      <CaptureOverlay
        isActive={isCapturing}
        onCapture={handleCaptureComplete}
        onCancel={handleCaptureCancel}
      />
    </div>
  );
}
