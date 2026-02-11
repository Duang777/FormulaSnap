# 需求文档

## 简介

FormulaSnap 是一款 Windows 优先的离线桌面端工具，用于截图数学公式并自动识别为 LaTeX，支持一键复制到 Word 成为可编辑公式对象。产品基于 Tauri v2（Rust + React + TypeScript）构建，使用 SQLite 存储历史记录，KaTeX 渲染公式预览，离线 Math OCR 模型进行公式识别。

## 术语表

- **FormulaSnap**：本产品名称，离线桌面端公式截图识别工具
- **CaptureService**：截图服务模块，负责全局热键注册与屏幕区域框选截图
- **PreprocessService**：图像预处理模块，负责裁边、对比度增强、缩放等操作
- **OcrService**：离线公式识别模块，将图片转换为 LaTeX 字符串及置信度
- **ConvertService**：格式转换模块，将 LaTeX 转换为 MathML 和 OMML
- **ClipboardService**：剪贴板服务模块，负责将多种格式同时写入系统剪贴板
- **HistoryService**：历史记录模块，基于 SQLite 的 CRUD 与搜索功能
- **ExportService**：导出模块，负责生成 .tex 和 .docx 文件
- **LaTeX**：一种数学公式排版标记语言
- **OMML**：Office Math Markup Language，Microsoft Office 原生公式标记格式
- **MathML**：Mathematical Markup Language，W3C 标准的数学标记语言
- **KaTeX**：高性能的 LaTeX 数学公式渲染库
- **Confidence**：OCR 识别置信度，取值范围 0.0 ~ 1.0

## 需求

### 需求 1：全局快捷键截图

**用户故事：** 作为用户，我希望通过全局快捷键在任意窗口上层唤起截图框选，以便快速捕获屏幕上的数学公式。

#### 验收标准

1. WHEN 用户按下全局快捷键（默认 Ctrl+Shift+2），THE CaptureService SHALL 进入截图模式并显示十字光标供用户框选屏幕区域
2. WHEN 用户完成框选操作，THE CaptureService SHALL 在 300ms 内将截图传递给 OcrService 并在界面显示识别中状态
3. WHILE 截图模式处于激活状态，THE CaptureService SHALL 在所有窗口上层显示截图覆盖层
4. WHEN 用户在截图模式中按下 Escape 键，THE CaptureService SHALL 取消截图并恢复正常状态
5. WHERE 用户自定义了快捷键配置，THE CaptureService SHALL 使用用户配置的快捷键替代默认快捷键

### 需求 2：图片输入

**用户故事：** 作为用户，我希望除截图外还能通过粘贴或拖拽图片来输入公式，以便灵活地从不同来源获取公式图片。

#### 验收标准

1. WHEN 用户在应用窗口中按下 Ctrl+V 且剪贴板包含图片数据，THE FormulaSnap SHALL 将该图片传递给 OcrService 进行识别
2. WHEN 用户将图片文件拖拽到应用窗口，THE FormulaSnap SHALL 读取该图片并传递给 OcrService 进行识别
3. IF 粘贴或拖拽的内容不是有效图片格式，THEN THE FormulaSnap SHALL 显示明确的错误提示并保持当前状态不变

### 需求 3：离线公式识别

**用户故事：** 作为用户，我希望在完全离线的环境下将公式图片识别为 LaTeX，以便在无网络条件下正常使用。

#### 验收标准

1. WHEN OcrService 接收到公式图片，THE PreprocessService SHALL 对图片执行自动裁边和缩放到模型推荐尺寸的预处理操作
2. WHEN 预处理完成后，THE OcrService SHALL 在离线环境下将图片识别为 LaTeX 字符串并返回 0.0 到 1.0 之间的置信度值
3. WHEN OcrService 完成识别，THE FormulaSnap SHALL 在 P95 延迟不超过 6 秒的时间内返回结果（在常见笔记本 CPU 上）
4. WHILE OcrService 正在执行识别，THE FormulaSnap SHALL 在独立于 UI 的线程中运行识别任务以保持界面响应
5. IF OcrService 识别结果为空或识别失败，THEN THE FormulaSnap SHALL 显示"可能未检测到公式"的提示并提供重试按钮和手动编辑入口
6. IF OcrService 识别超过 10 秒未返回结果，THEN THE FormulaSnap SHALL 超时终止识别并提示用户可重试或手动输入

### 需求 4：LaTeX 编辑与实时渲染预览

**用户故事：** 作为用户，我希望编辑识别出的 LaTeX 并实时预览渲染效果，以便在识别不准确时快速修正公式。

#### 验收标准

1. WHEN OcrService 返回识别结果，THE FormulaSnap SHALL 在编辑区域显示可编辑的 LaTeX 文本并在预览区域使用 KaTeX 渲染公式
2. WHEN 用户修改 LaTeX 编辑框中的文本，THE FormulaSnap SHALL 实时更新 KaTeX 渲染预览
3. THE FormulaSnap SHALL 在 LaTeX 编辑框中支持撤销（Ctrl+Z）和重做（Ctrl+Y）操作
4. WHEN 用户切换输出包裹格式，THE FormulaSnap SHALL 在行内模式（`\(...\)`）和行间模式（`\[...\]`）之间切换 LaTeX 输出格式
5. IF KaTeX 渲染 LaTeX 时发生语法错误，THEN THE FormulaSnap SHALL 在预览区域显示错误提示而非空白内容

### 需求 5：复制到 Word 成为可编辑公式

**用户故事：** 作为用户，我希望一键复制公式后在 Word 中 Ctrl+V 直接粘贴为可编辑公式对象，以便高效地将公式插入文档。

#### 验收标准

1. WHEN 用户点击"复制到 Word"按钮，THE ConvertService SHALL 将 LaTeX 转换为 OMML 格式
2. WHEN ConvertService 完成 OMML 转换，THE ClipboardService SHALL 同时将 OMML、MathML 和纯文本 LaTeX 三种格式写入系统剪贴板
3. WHEN 用户在 Word（Microsoft 365 或 2019）中执行 Ctrl+V，THE ClipboardService 写入的剪贴板内容 SHALL 使 Word 显示为可编辑公式对象（成功率不低于 90%）
4. IF ConvertService 将 LaTeX 转换为 OMML 失败，THEN THE FormulaSnap SHALL 将纯文本 LaTeX 写入剪贴板并提示用户"在 Word 公式编辑器中粘贴并转换"
5. WHEN 用户点击"复制 LaTeX"按钮，THE ClipboardService SHALL 按当前包裹格式将 LaTeX 文本写入剪贴板

### 需求 6：LaTeX 与 OMML 格式转换

**用户故事：** 作为用户，我希望 LaTeX 能准确转换为 OMML 格式，以便复制到 Word 时公式结构完整。

#### 验收标准

1. THE ConvertService SHALL 将有效的 LaTeX 字符串转换为语法正确的 OMML 字符串
2. THE ConvertService SHALL 将有效的 LaTeX 字符串转换为语法正确的 MathML 字符串
3. THE ConvertService_PrettyPrinter SHALL 将 OMML 字符串格式化为可读的 XML 输出
4. FOR ALL 有效的 LaTeX 字符串，将 LaTeX 转换为 OMML 再将 OMML 解析回内部表示后，SHALL 产生与原始 LaTeX 语义等价的结构（往返一致性）
5. WHEN ConvertService 接收到包含不支持符号的 LaTeX，THE ConvertService SHALL 返回描述性错误信息指明不支持的具体符号
6. THE ConvertService SHALL 正确转换包含上下标、分式、根号、积分、求和、矩阵和希腊字母的 LaTeX 公式

### 需求 7：历史记录管理

**用户故事：** 作为用户，我希望自动保存每次识别结果并能搜索、收藏和复用历史公式，以便高效管理和重复使用常用公式。

#### 验收标准

1. WHEN OcrService 完成一次识别，THE HistoryService SHALL 自动保存识别记录，包含时间戳、原始 LaTeX、编辑后 LaTeX、置信度和引擎版本
2. WHEN 用户在搜索框输入关键词，THE HistoryService SHALL 返回 LaTeX 内容中包含该关键词的所有历史记录
3. WHEN 用户点击收藏按钮，THE HistoryService SHALL 切换该记录的收藏状态并持久化到 SQLite 数据库
4. WHEN 用户在历史面板中点击某条记录的"复制到 Word"按钮，THE FormulaSnap SHALL 执行与主界面相同的多格式剪贴板写入操作
5. WHEN 用户在历史面板中点击某条记录的"编辑"按钮，THE FormulaSnap SHALL 将该记录的 LaTeX 加载到编辑区域
6. WHERE 用户在设置中启用了"仅保存 LaTeX 不保存图片"选项，THE HistoryService SHALL 仅存储 LaTeX 文本而不保存截图缩略图

### 需求 8：导出功能

**用户故事：** 作为用户，我希望将多条历史公式批量导出为 .tex 或 .docx 文件，以便在其他工具中使用这些公式。

#### 验收标准

1. WHEN 用户选择多条历史记录并点击"导出 .tex"，THE ExportService SHALL 生成包含所有选中公式的 .tex 文件，公式按时间排序
2. WHEN 用户选择多条历史记录并点击"导出 .docx"，THE ExportService SHALL 生成 .docx 文件，其中每条公式为一个包含 OMML 公式对象的段落
3. IF ExportService 在生成 .docx 时某条公式的 OMML 转换失败，THEN THE ExportService SHALL 在该位置插入 LaTeX 纯文本并标注"转换失败"
4. WHEN 导出 .tex 文件时，THE ExportService SHALL 提供选项让用户选择是否在公式之间添加时间注释分隔

### 需求 9：稳定性与性能

**用户故事：** 作为用户，我希望应用在长时间使用中保持稳定且响应迅速，以便不中断工作流程。

#### 验收标准

1. THE FormulaSnap SHALL 在连续执行 50 次截图识别后保持正常运行且内存增量不超过 300MB
2. WHILE OcrService 正在执行识别任务，THE FormulaSnap SHALL 保持 UI 线程响应，用户可正常操作界面
3. IF FormulaSnap 发生意外崩溃，THEN THE HistoryService SHALL 确保崩溃前的所有已保存历史记录不丢失
4. WHEN 用户发起新的识别请求，THE FormulaSnap SHALL 提供取消当前正在进行的识别任务的能力
