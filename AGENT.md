# FormulaSnap 开发规范

## 项目概述

FormulaSnap 是一款 Windows 优先的离线桌面端工具，核心功能是截图数学公式 → 自动识别为 LaTeX → 一键复制到 Word 成为可编辑公式对象。

## 技术栈

- **框架**: Tauri v2（Rust + Web 前端）
- **前端**: React + TypeScript + Vite
- **样式**: Tailwind CSS
- **公式渲染**: KaTeX
- **本地存储**: SQLite（通过 Tauri 插件）
- **OCR**: 离线 Math OCR 模型（pix2tex / LaTeX-OCR）
- **转换**: LaTeX → MathML → OMML 转换链
- **剪贴板**: Windows 原生 Clipboard API（多格式写入）

## 项目结构

```
formulasnap/
├── src-tauri/              # Rust 后端
│   ├── src/
│   │   ├── main.rs
│   │   ├── capture.rs      # CaptureService: 全局热键、截图框选
│   │   ├── preprocess.rs   # PreprocessService: 裁边、增强、缩放
│   │   ├── ocr.rs          # OcrService: 图片→LaTeX+置信度
│   │   ├── convert.rs      # ConvertService: LaTeX→MathML→OMML
│   │   ├── clipboard.rs    # ClipboardService: 多格式写入剪贴板
│   │   ├── history.rs      # HistoryService: SQLite CRUD+搜索
│   │   └── export.rs       # ExportService: tex/docx 导出
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/                    # React 前端
│   ├── components/
│   │   ├── CaptureOverlay.tsx
│   │   ├── FormulaEditor.tsx
│   │   ├── FormulaPreview.tsx
│   │   ├── HistoryPanel.tsx
│   │   └── ActionBar.tsx
│   ├── hooks/
│   ├── stores/
│   ├── utils/
│   ├── App.tsx
│   └── main.tsx
├── package.json
├── tsconfig.json
├── vite.config.ts
├── tailwind.config.js
└── AGENT.md
```

## 编码规范

### Rust（后端）

- 使用 `Result<T, E>` 处理错误，避免 `unwrap()`
- 模块间通过 Tauri Command 暴露给前端
- 异步操作使用 `tokio`
- 日志使用 `log` crate
- 命名：snake_case（函数/变量），CamelCase（类型/结构体）

### TypeScript（前端）

- 严格模式 `strict: true`
- 组件使用函数式组件 + hooks
- 状态管理使用 zustand
- 命名：camelCase（变量/函数），PascalCase（组件/类型）
- 文件命名：组件 PascalCase，工具函数 camelCase

### 通用

- 提交信息格式：`type(scope): description`（如 `feat(ocr): add image preprocessing`）
- 每个模块需有对应的单元测试
- 属性测试（PBT）用于核心转换逻辑验证

## 模块接口定义

### CaptureService

```rust
/// 注册全局快捷键，进入截图模式
fn register_hotkey(shortcut: &str) -> Result<(), CaptureError>;
/// 框选截图，返回 PNG 字节
fn capture_region() -> Result<Vec<u8>, CaptureError>;
```

### OcrService

```rust
/// 图片识别为 LaTeX，返回结果和置信度
fn recognize(image: &[u8]) -> Result<OcrResult, OcrError>;

struct OcrResult {
    latex: String,
    confidence: f64, // 0.0 ~ 1.0
}
```

### ConvertService

```rust
/// LaTeX → MathML
fn latex_to_mathml(latex: &str) -> Result<String, ConvertError>;
/// LaTeX → OMML
fn latex_to_omml(latex: &str) -> Result<String, ConvertError>;
```

### ClipboardService

```rust
/// 多格式写入剪贴板（OMML + MathML + plain text）
fn copy_formula(latex: &str, omml: &str, mathml: &str) -> Result<(), ClipboardError>;
/// 仅复制 LaTeX 文本
fn copy_latex(latex: &str) -> Result<(), ClipboardError>;
```

### HistoryService

```rust
/// 保存识别记录
fn save(record: HistoryRecord) -> Result<i64, HistoryError>;
/// 搜索历史
fn search(query: &str) -> Result<Vec<HistoryRecord>, HistoryError>;
/// 切换收藏
fn toggle_favorite(id: i64) -> Result<(), HistoryError>;
```

## 关键约束

- 离线运行，不依赖网络
- 识别线程与 UI 线程分离
- 内存增量 < 300MB（连续 50 次识别）
- P95 识别延迟 ≤ 6 秒
- Word 粘贴成功率 ≥ 90%

## 测试策略

- 单元测试：每个 Service 模块
- 属性测试：LaTeX→OMML 转换的正确性
- 集成测试：截图→识别→复制完整链路
- 测试框架：Rust 用内置 test + proptest，前端用 vitest + fast-check
