// ActionBar - 操作按钮栏组件
// "复制到 Word"、"复制 LaTeX"、"重试"、"保存"按钮
// 转换失败时的回退提示
// Validates: Requirements 5.1, 5.4, 5.5

import type { ActionBarProps } from "../../types";

export type { ActionBarProps };

export function ActionBar({
  onCopyToWord,
  onCopyLatex,
  onRetry,
  onSave,
  isConverting,
}: ActionBarProps) {
  return (
    <div className="action-bar flex items-center gap-3">
      <button
        onClick={onCopyToWord}
        disabled={isConverting}
        className="flex-1 px-6 py-3 text-sm font-medium text-white bg-gray-900 rounded-full shadow-button btn-soft disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
        aria-label="复制到 Word"
      >
        {isConverting ? (
          <>
            <span
              className="inline-block w-4 h-4 border-2 border-white border-t-transparent rounded-full animate-spin"
              role="status"
              aria-label="转换中"
            />
            转换中...
          </>
        ) : (
          "复制到 Word"
        )}
      </button>

      <button
        onClick={onCopyLatex}
        className="px-5 py-3 text-sm font-medium text-gray-700 bg-white rounded-full shadow-button btn-soft"
        aria-label="复制 LaTeX"
      >
        复制 LaTeX
      </button>

      <button
        onClick={onRetry}
        className="w-12 h-12 flex items-center justify-center text-gray-600 bg-white rounded-full shadow-button btn-soft"
        aria-label="重试"
      >
        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
      </button>

      <button
        onClick={onSave}
        className="w-12 h-12 flex items-center justify-center text-gray-600 bg-white rounded-full shadow-button btn-soft"
        aria-label="保存"
      >
        <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 5a2 2 0 012-2h10a2 2 0 012 2v16l-7-3.5L5 21V5z" />
        </svg>
      </button>
    </div>
  );
}

export default ActionBar;
