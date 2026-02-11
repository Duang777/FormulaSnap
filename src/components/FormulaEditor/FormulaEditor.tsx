// FormulaEditor - LaTeX 编辑器组件
// 可编辑 LaTeX 文本框，支持撤销（Ctrl+Z）/重做（Ctrl+Y）
// Validates: Requirements 4.1, 4.2, 4.3

import { useCallback, useRef } from "react";
import type { FormulaEditorProps } from "../../types";
import { useUndoRedo } from "./useUndoRedo";

export type { FormulaEditorProps };

export function FormulaEditor({
  latex,
  onChange,
  wrapMode,
  onWrapModeChange,
}: FormulaEditorProps) {
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  const { undo, redo, pushState } = useUndoRedo(latex, onChange);

  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLTextAreaElement>) => {
      const newValue = e.target.value;
      pushState(newValue);
      onChange(newValue);
    },
    [pushState, onChange]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
      // Ctrl+Z for undo
      if (e.ctrlKey && !e.shiftKey && e.key === "z") {
        e.preventDefault();
        undo();
        return;
      }
      // Ctrl+Y or Ctrl+Shift+Z for redo
      if (
        (e.ctrlKey && e.key === "y") ||
        (e.ctrlKey && e.shiftKey && e.key === "z")
      ) {
        e.preventDefault();
        redo();
        return;
      }
    },
    [undo, redo]
  );

  return (
    <div className="formula-editor flex flex-col gap-4 neu-card p-6">
      <div className="flex items-center justify-between">
        <label
          htmlFor="latex-editor"
          className="text-sm font-medium text-gray-700"
        >
          LaTeX 编辑器
        </label>
        <div className="flex items-center gap-3">
          <select
            value={wrapMode}
            onChange={(e) =>
              onWrapModeChange(e.target.value as "inline" | "display")
            }
            className="text-sm px-4 py-2 bg-white rounded-full shadow-button text-gray-600 border-0 focus:outline-none focus:ring-2 focus:ring-gray-300 cursor-pointer"
            aria-label="wrap mode"
          >
            <option value="inline">行内模式</option>
            <option value="display">行间模式</option>
          </select>
        </div>
      </div>
      <textarea
        ref={textareaRef}
        id="latex-editor"
        value={latex}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        className="w-full min-h-[120px] p-4 font-mono text-sm neu-inset resize-y focus:outline-none placeholder:text-gray-400"
        placeholder="输入或编辑 LaTeX 公式..."
        spellCheck={false}
        aria-label="LaTeX editor"
      />
      <div className="flex items-center gap-4 text-xs text-gray-400">
        <span>Ctrl+Z 撤销</span>
        <span>Ctrl+Y 重做</span>
      </div>
    </div>
  );
}

export default FormulaEditor;
