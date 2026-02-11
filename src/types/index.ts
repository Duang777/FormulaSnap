// FormulaSnap 前端类型定义
// 与 Rust 后端类型保持一致

// ============================================================
// 核心数据类型
// ============================================================

/** OCR 识别结果（对应 Rust OcrResult） */
export interface OcrResult {
  latex: string;
  confidence: number; // 0.0 ~ 1.0
}

/** 历史记录（对应 Rust HistoryRecord） */
export interface HistoryRecord {
  id?: number;
  created_at: string; // ISO 8601
  original_latex: string;
  edited_latex?: string;
  confidence: number; // 0.0 ~ 1.0
  engine_version: string;
  thumbnail?: number[]; // PNG 缩略图（Rust Vec<u8> 序列化为 number[]）
  is_favorite: boolean;
}

/** .tex 导出选项（对应 Rust TexExportOptions） */
export interface TexExportOptions {
  add_time_comments: boolean;
}

/** 图像预处理选项（对应 Rust PreprocessOptions） */
export interface PreprocessOptions {
  auto_crop: boolean;
  enhance_contrast: boolean;
  target_height: number;
}

/** 截图配置（对应 Rust CaptureConfig） */
export interface CaptureConfig {
  shortcut: string; // 默认 "Ctrl+Shift+2"
}

// ============================================================
// 枚举与联合类型
// ============================================================

/** LaTeX 包裹模式 */
export type WrapMode = "inline" | "display";

// ============================================================
// 组件 Props 类型
// ============================================================

/** FormulaEditor 组件属性 */
export interface FormulaEditorProps {
  latex: string;
  onChange: (latex: string) => void;
  wrapMode: WrapMode;
  onWrapModeChange: (mode: WrapMode) => void;
}

/** FormulaPreview 组件属性 */
export interface FormulaPreviewProps {
  latex: string;
  displayMode: boolean;
}

/** HistoryPanel 组件属性 */
export interface HistoryPanelProps {
  onSelect: (record: HistoryRecord) => void;
  onCopyToWord: (record: HistoryRecord) => void;
  onCopyLatex: (record: HistoryRecord) => void;
}

/** ActionBar 组件属性 */
export interface ActionBarProps {
  onCopyToWord: () => void;
  onCopyLatex: () => void;
  onRetry: () => void;
  onSave: () => void;
  isConverting: boolean;
}

/** CaptureOverlay 组件属性 */
export interface CaptureOverlayProps {
  isActive: boolean;
  onCapture: (imageData: Uint8Array) => void;
  onCancel: () => void;
}
