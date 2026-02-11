// HistoryPanel - 历史记录面板组件
// 历史记录列表（缩略图 + 渲染预览 + 时间）
// 搜索框、收藏按钮、复制/编辑操作
// Validates: Requirements 7.2, 7.3, 7.4, 7.5

import { useMemo, useCallback, useState } from "react";
import katex from "katex";
import "katex/dist/katex.min.css";
import type { HistoryPanelProps, HistoryRecord } from "../../types";
import { useFormulaStore } from "../../store/formulaStore";

export type { HistoryPanelProps };

// ============================================================
// Helper: Render LaTeX to HTML string for preview
// ============================================================

/**
 * Renders LaTeX to an HTML string using KaTeX.
 * Returns both the rendered HTML and any error message.
 */
export function renderLatexToHtml(latex: string): {
  html: string;
  error: string | null;
} {
  if (!latex.trim()) {
    return { html: "", error: null };
  }
  try {
    const html = katex.renderToString(latex, {
      displayMode: false,
      throwOnError: true,
      strict: false,
    });
    return { html, error: null };
  } catch (err) {
    const errorMessage =
      err instanceof Error ? err.message : "未知渲染错误";
    return { html: "", error: errorMessage };
  }
}

// ============================================================
// Helper: Format timestamp for display
// ============================================================

export function formatTimestamp(isoString: string): string {
  try {
    const date = new Date(isoString);
    if (isNaN(date.getTime())) {
      return isoString;
    }
    return date.toLocaleString();
  } catch {
    return isoString;
  }
}

// ============================================================
// Helper: Format confidence as percentage
// ============================================================

export function formatConfidence(confidence: number): string {
  return `${Math.round(confidence * 100)}%`;
}

// ============================================================
// HistoryRecordItem sub-component
// ============================================================

interface HistoryRecordItemProps {
  record: HistoryRecord;
  onSelect: (record: HistoryRecord) => void;
  onCopyToWord: (record: HistoryRecord) => void;
  onCopyLatex: (record: HistoryRecord) => void;
  onToggleFavorite: (record: HistoryRecord) => void;
}

export function HistoryRecordItem({
  record,
  onSelect,
  onCopyToWord,
  onCopyLatex,
  onToggleFavorite,
}: HistoryRecordItemProps) {
  const latex = record.edited_latex || record.original_latex;
  const { html, error } = useMemo(() => renderLatexToHtml(latex), [latex]);
  const [expanded, setExpanded] = useState(false);

  // Build thumbnail data URL if thumbnail bytes are available
  const thumbnailUrl = useMemo(() => {
    if (record.thumbnail && record.thumbnail.length > 0) {
      const bytes = new Uint8Array(record.thumbnail);
      const blob = new Blob([bytes], { type: "image/png" });
      return URL.createObjectURL(blob);
    }
    return null;
  }, [record.thumbnail]);

  // Truncate latex for display
  const truncatedLatex = useMemo(() => {
    if (latex.length > 30) {
      return latex.substring(0, 30) + "...";
    }
    return latex;
  }, [latex]);

  return (
    <div
      className="history-record bg-white rounded-2xl shadow-soft transition-all duration-200 hover:shadow-card overflow-hidden"
      data-testid="history-record"
    >
      {/* Clickable header */}
      <div 
        className="p-4 cursor-pointer"
        onClick={() => setExpanded(!expanded)}
        data-testid="history-record-header"
      >
        <div className="flex items-center gap-3">
          {/* Thumbnail */}
          {thumbnailUrl && (
            <img
              src={thumbnailUrl}
              alt="缩略图"
              className="w-10 h-10 object-contain rounded-lg bg-surface-100 flex-shrink-0"
              data-testid="history-thumbnail"
            />
          )}
          
          {/* Info */}
          <div className="flex-1 min-w-0">
            <div className="text-xs font-mono text-gray-500 truncate" title={latex}>
              {truncatedLatex}
            </div>
            <div className="flex items-center gap-2 mt-1 text-xs text-gray-400">
              <span data-testid="history-timestamp">
                {formatTimestamp(record.created_at)}
              </span>
              <span data-testid="history-confidence">
                {formatConfidence(record.confidence)}
              </span>
            </div>
          </div>

          {/* Expand indicator */}
          <svg 
            className={`w-4 h-4 text-gray-400 transition-transform duration-200 ${expanded ? 'rotate-180' : ''}`} 
            fill="none" 
            stroke="currentColor" 
            viewBox="0 0 24 24"
          >
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M19 9l-7 7-7-7" />
          </svg>
        </div>
      </div>

      {/* Expandable preview */}
      {expanded && (
        <div className="px-4 pb-4 animate-fade-in">
          {/* Formula preview */}
          <div className="p-4 bg-surface-100 rounded-xl mb-3 flex items-center justify-center min-h-[60px] overflow-auto">
            {error ? (
              <div
                className="text-gray-400 text-xs"
                data-testid="history-preview-error"
              >
                渲染错误
              </div>
            ) : html ? (
              <div
                className="katex-preview"
                data-testid="history-preview"
                dangerouslySetInnerHTML={{ __html: html }}
              />
            ) : (
              <div className="text-gray-400 text-xs">无内容</div>
            )}
          </div>

          {/* Action buttons */}
          <div className="flex items-center gap-2">
            <button
              onClick={(e) => { e.stopPropagation(); onToggleFavorite(record); }}
              className={`w-9 h-9 flex items-center justify-center rounded-full transition-all duration-200 ${
                record.is_favorite
                  ? "bg-gray-900 text-white"
                  : "bg-surface-200 text-gray-500 hover:bg-surface-300"
              }`}
              aria-label={record.is_favorite ? "取消收藏" : "收藏"}
              data-testid="history-favorite-btn"
            >
              <svg className="w-4 h-4" fill={record.is_favorite ? "currentColor" : "none"} stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M11.049 2.927c.3-.921 1.603-.921 1.902 0l1.519 4.674a1 1 0 00.95.69h4.915c.969 0 1.371 1.24.588 1.81l-3.976 2.888a1 1 0 00-.363 1.118l1.518 4.674c.3.922-.755 1.688-1.538 1.118l-3.976-2.888a1 1 0 00-1.176 0l-3.976 2.888c-.783.57-1.838-.197-1.538-1.118l1.518-4.674a1 1 0 00-.363-1.118l-3.976-2.888c-.784-.57-.38-1.81.588-1.81h4.914a1 1 0 00.951-.69l1.519-4.674z" />
              </svg>
            </button>

            <button
              onClick={(e) => { e.stopPropagation(); onCopyToWord(record); }}
              className="flex-1 py-2 text-xs font-medium text-gray-700 bg-surface-200 rounded-full hover:bg-surface-300 transition-all duration-200"
              aria-label="复制到 Word"
              data-testid="history-copy-word-btn"
            >
              Word
            </button>

            <button
              onClick={(e) => { e.stopPropagation(); onCopyLatex(record); }}
              className="flex-1 py-2 text-xs font-medium text-gray-700 bg-surface-200 rounded-full hover:bg-surface-300 transition-all duration-200"
              aria-label="复制 LaTeX"
              data-testid="history-copy-latex-btn"
            >
              LaTeX
            </button>

            <button
              onClick={(e) => { e.stopPropagation(); onSelect(record); }}
              className="w-9 h-9 flex items-center justify-center text-gray-500 bg-surface-200 rounded-full hover:bg-surface-300 transition-all duration-200"
              aria-label="编辑"
              data-testid="history-edit-btn"
            >
              <svg className="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
              </svg>
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

// ============================================================
// HistoryPanel main component
// ============================================================

export function HistoryPanel({
  onSelect,
  onCopyToWord,
  onCopyLatex,
}: HistoryPanelProps) {
  const historyRecords = useFormulaStore((s) => s.historyRecords);
  const searchQuery = useFormulaStore((s) => s.searchQuery);
  const searchHistory = useFormulaStore((s) => s.searchHistory);
  const toggleFavorite = useFormulaStore((s) => s.toggleFavorite);
  const setSearchQuery = useFormulaStore((s) => s.setSearchQuery);

  const handleSearchChange = useCallback(
    (value: string) => {
      setSearchQuery(value);
      searchHistory(value);
    },
    [setSearchQuery, searchHistory]
  );

  const handleToggleFavorite = useCallback(
    (record: HistoryRecord) => {
      if (record.id != null) {
        toggleFavorite(record.id);
      }
    },
    [toggleFavorite]
  );

  // Client-side filtering as backup (store search is the primary mechanism)
  const filteredRecords = useMemo(() => {
    if (!searchQuery.trim()) return historyRecords;
    const q = searchQuery.toLowerCase();
    return historyRecords.filter(
      (r) =>
        r.original_latex.toLowerCase().includes(q) ||
        (r.edited_latex && r.edited_latex.toLowerCase().includes(q))
    );
  }, [historyRecords, searchQuery]);

  return (
    <div
      className="history-panel flex flex-col h-full"
      data-testid="history-panel"
    >
      {/* Header */}
      <div className="p-5">
        <h2 className="text-base font-semibold text-gray-800 mb-4">
          历史记录
        </h2>
        {/* Search box */}
        <div className="relative">
          <svg className="absolute left-4 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
          </svg>
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => handleSearchChange(e.target.value)}
            placeholder="搜索..."
            className="w-full pl-11 pr-4 py-3 text-sm neu-inset focus:outline-none placeholder:text-gray-400"
            aria-label="搜索历史记录"
            data-testid="history-search-input"
          />
        </div>
      </div>

      {/* Records list */}
      <div className="flex-1 overflow-y-auto px-5 pb-5 space-y-3">
        {filteredRecords.length === 0 ? (
          <div
            className="flex flex-col items-center justify-center py-16 text-gray-400"
            data-testid="history-empty"
          >
            <svg className="w-12 h-12 mb-3 opacity-30" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1} d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
            </svg>
            <span className="text-sm">
              {searchQuery.trim()
                ? "未找到匹配记录"
                : "暂无历史记录"}
            </span>
          </div>
        ) : (
          filteredRecords.map((record, index) => (
            <HistoryRecordItem
              key={record.id ?? index}
              record={record}
              onSelect={onSelect}
              onCopyToWord={onCopyToWord}
              onCopyLatex={onCopyLatex}
              onToggleFavorite={handleToggleFavorite}
            />
          ))
        )}
      </div>
    </div>
  );
}

export default HistoryPanel;
