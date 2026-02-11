# FormulaSnap 开发进度

> 最后更新: 2026-02-11

## 当前状态: ✅ 属性测试完成

## 已完成

### 前端 (React + TypeScript)
- [x] 类型定义 (`src/types/index.ts`)
- [x] Zustand Store + Tauri invoke 封装 (`src/store/formulaStore.ts`)
- [x] FormulaEditor 组件（撤销/重做）
- [x] FormulaPreview 组件（KaTeX 渲染）
- [x] CaptureOverlay 组件（框选截图）
- [x] ActionBar 组件（复制/重试/保存）
- [x] HistoryPanel 组件（搜索/收藏/复制/编辑）
- [x] useImageInput hook（粘贴 + 拖拽图片）
- [x] App Shell 主流程串联
- [x] 前端测试 195 个全部通过（含 fast-check 属性测试）

### Rust 后端
- [x] preprocess.rs — 裁边、对比度增强、缩放
- [x] convert.rs — LaTeX → MathML → OMML 转换链
- [x] clipboard.rs — 剪贴板写入（纯文本 MathML）
- [x] history.rs — SQLite CRUD + 搜索
- [x] export.rs — .tex / .docx 导出
- [x] ocr.rs — ONNX Runtime 推理模块（保留）
- [x] capture.rs — Win32 截图 + 全局快捷键
- [x] lib.rs — Tauri Commands 注册 + App 初始化
- [x] Rust 测试 188 通过，13 ignored（剪贴板/DB 并行测试）

### 属性测试 (2026-02-11 完成)
- [x] PreprocessService: 输出尺寸约束属性测试
- [x] ConvertService: XML 合法性、Pretty Print 结构保持、错误信息包含性
- [x] HistoryService: 保存/查询往返一致性、搜索完整性、收藏幂等性
- [x] ExportService: .tex 导出完整性与排序、.docx 段落数量一致性
- [x] OcrService: 置信度范围不变量
- [x] ClipboardService: 多格式写入完整性（需桌面会话）

### OCR 方案
- [x] 使用 Texify 模型（通过 Python 子进程调用）
- [x] 独立虚拟环境 `.venv-texify` 避免依赖冲突
- [x] `scripts/ocr_server.py` — Python OCR 服务脚本
- [x] Rust 端通过 base64 传递图片数据给 Python

### 剪贴板 Word 兼容性 (2026-02-10 修复)
- [x] 修复 `\limits` 命令预处理
- [x] 修复 `fix_subsup_order()` 正则表达式
- [x] 修复 MathML 解析：msup 嵌套 msub 转换为 msubsup
- [x] 添加 `\(` `\)` `\[` `\]` `$` 包装符移除
- [x] 添加 `\mathcal L` → `\mathcal{L}` 修复
- [x] 添加三重/双重大括号 `{{{x}}}` → `{x}` 修复
- [x] 添加空格函数名修复：`l o g` → `log`
- [x] **新方案：只写入 CF_UNICODETEXT 格式的 MathML**
  - 移除 CF_HTML 格式，Word 直接 Ctrl+V 即可识别 MathML 并转换为公式
  - 不再需要 Ctrl+Shift+V 或"仅保留文本"选项

### 编译与环境
- [x] `cargo build` 编译通过（0 errors, 0 warnings）
- [x] `cargo test` 153/158 通过（剪贴板测试需要桌面会话）
- [x] `cargo tauri dev` 应用启动成功

## 待测试

### 端到端流程
- [ ] 截图功能测试
- [ ] OCR 识别测试（需要 Texify 已安装在 .venv-texify）
- [ ] LaTeX 编辑 + 预览
- [ ] **复制到 Word 测试 — 直接 Ctrl+V 粘贴**
- [ ] 历史记录功能

## 使用说明

### 复制公式到 Word（MathML 方式）
1. 在应用中输入或识别公式
2. 点击"复制到 Word"按钮
3. 在 Word 中直接按 **Ctrl+V** 粘贴
4. Word 会自动识别 MathML 并转换为公式

### 备选方案：LaTeX 方式
1. 点击"复制 LaTeX"按钮
2. 在 Word 中：
   - 按 **Alt+=** 进入公式编辑模式
   - 按 **Ctrl+V** 粘贴
   - Word 会自动识别 LaTeX 并转换为公式

## 已知问题

### 剪贴板测试（部分）
- 状态: ⚠️ 环境依赖
- 原因: Win32 剪贴板 API 需要桌面会话上下文
- 影响: 不影响实际运行

### OCR 依赖
- 需要在 `.venv-texify` 虚拟环境中安装 Texify
- 首次运行会自动下载模型权重
