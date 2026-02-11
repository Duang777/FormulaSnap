# 实施计划：FormulaSnap - 属性测试完善阶段

## 概述

本阶段专注于完成之前跳过的可选属性测试任务，提高代码质量和稳定性。使用 proptest（Rust）和 fast-check（TypeScript）进行属性测试。

## 任务

- [x] 1. Rust 属性测试环境准备
  - [x] 1.1 添加 proptest 依赖到 Cargo.toml
    - 在 `[dev-dependencies]` 中添加 `proptest = "1"`
    - _Requirements: 全部属性测试_

- [x] 2. PreprocessService 属性测试
  - [x] 2.1 编写图像预处理输出尺寸约束属性测试
    - **Property 2: 图像预处理输出尺寸约束**
    - 使用 proptest 生成随机尺寸图片（宽度 1-2000，高度 1-2000）
    - 验证输出高度等于目标值（64px）且宽度为正整数
    - 验证宽高比保持不变（误差 < 1%）
    - 文件：`src-tauri/src/preprocess.rs` 添加 `#[cfg(test)]` 模块
    - **Validates: Requirements 3.1**

- [x] 3. ConvertService 属性测试
  - [x] 3.1 编写 LaTeX → MathML/OMML 转换输出 XML 合法性属性测试
    - **Property 8: LaTeX → MathML/OMML 转换输出 XML 合法性**
    - 使用 proptest 生成有效 LaTeX 字符串（分式、上下标、根号等组合）
    - 验证 MathML 输出以 `<math` 开头并以 `</math>` 结尾
    - 验证 OMML 输出以 `<m:oMathPara` 开头并以 `</m:oMathPara>` 结尾
    - 验证输出可被 XML 解析器解析
    - 文件：`src-tauri/src/convert.rs` 添加属性测试
    - **Validates: Requirements 6.1, 6.2**

  - [x] 3.2 编写 OMML Pretty Print 结构保持属性测试
    - **Property 9: OMML Pretty Print 结构保持**
    - 对任意有效 OMML 字符串，pretty_print 后再解析应得到相同的 XML 结构
    - 验证格式化前后的元素数量、属性、文本内容一致
    - 文件：`src-tauri/src/convert.rs` 添加属性测试
    - **Validates: Requirements 6.3**

  - [x] 3.3 编写不支持符号错误信息包含性属性测试
    - **Property 11: 不支持符号的错误信息包含性**
    - 生成包含不支持命令的 LaTeX（如 `\unsupported{x}`）
    - 验证错误信息中包含不支持的符号名称
    - 文件：`src-tauri/src/convert.rs` 添加属性测试
    - **Validates: Requirements 6.5**

  - [x] 3.4 编写 ConvertService 单元测试
    - 测试具体公式类型：上下标 `x^2_i`、分式 `\frac{a}{b}`、根号 `\sqrt{x}`
    - 测试积分 `\int_0^1`、求和 `\sum_{i=1}^n`、矩阵 `\begin{matrix}...\end{matrix}`
    - 测试希腊字母 `\alpha, \beta, \gamma`
    - 测试转换失败的回退行为
    - 文件：`src-tauri/src/convert.rs` 添加单元测试
    - **Validates: Requirements 6.6**

- [x] 4. HistoryService 属性测试
  - [x] 4.1 编写历史记录保存/查询往返一致性属性测试
    - **Property 12: 历史记录保存/查询往返一致性**
    - 生成随机 HistoryRecord（LaTeX、置信度、时间戳）
    - 保存后查询，验证所有字段一致
    - 文件：`src-tauri/src/history.rs` 添加属性测试
    - **Validates: Requirements 7.1**

  - [x] 4.2 编写历史搜索结果完整性与正确性属性测试
    - **Property 13: 历史搜索结果完整性与正确性**
    - 保存多条记录，搜索关键词
    - 验证返回结果包含所有匹配项且不包含不匹配项
    - 文件：`src-tauri/src/history.rs` 添加属性测试
    - **Validates: Requirements 7.2**

  - [x] 4.3 编写收藏状态切换幂等性属性测试
    - **Property 14: 收藏状态切换幂等性**
    - 对同一记录连续调用 toggle_favorite 两次
    - 验证状态恢复到初始值
    - 文件：`src-tauri/src/history.rs` 添加属性测试
    - **Validates: Requirements 7.3**

- [x] 5. ExportService 属性测试
  - [x] 5.1 编写 .tex 导出完整性与排序属性测试
    - **Property 16: .tex 导出完整性与排序**
    - 生成多条带时间戳的记录
    - 导出后验证所有 LaTeX 都存在且按时间排序
    - 文件：`src-tauri/src/export.rs` 添加属性测试
    - **Validates: Requirements 8.1, 8.4**

  - [x] 5.2 编写 .docx 导出段落数量一致性属性测试
    - **Property 17: .docx 导出段落数量一致性**
    - 导出 N 条记录，验证 .docx 中有 N 个公式段落
    - 文件：`src-tauri/src/export.rs` 添加属性测试
    - **Validates: Requirements 8.2**

  - [x] 5.3 编写 ExportService 单元测试
    - 测试 .docx 中转换失败项的"转换失败"标注
    - 使用包含不支持符号的 LaTeX 触发转换失败
    - 验证输出文档中包含"转换失败"文本
    - 文件：`src-tauri/src/export.rs` 添加单元测试
    - **Validates: Requirements 8.3**

- [x] 6. OcrService 属性测试
  - [x] 6.1 编写 OCR 置信度范围不变量属性测试
    - **Property 3: OCR 置信度范围不变量**
    - 对任意有效图片输入，验证返回的置信度在 0.0 到 1.0 之间
    - 注意：此测试需要 OCR 模型可用，可能需要 mock 或跳过
    - 文件：`src-tauri/src/ocr.rs` 添加属性测试
    - **Validates: Requirements 3.2**

- [x] 7. ClipboardService 属性测试
  - [x] 7.1 编写剪贴板多格式写入完整性属性测试
    - **Property 7: 剪贴板多格式写入完整性**
    - 生成随机 LaTeX 字符串
    - 调用 copy_formula 后读取剪贴板
    - 验证 CF_UNICODETEXT 格式包含 MathML
    - 注意：此测试需要桌面会话，CI 环境可能需要跳过
    - 文件：`src-tauri/src/clipboard.rs` 添加属性测试
    - **Validates: Requirements 5.2**

- [x] 8. Final Checkpoint
  - [x] 8.1 运行所有 Rust 测试
    - 执行 `cargo test` 确保所有测试通过
    - 修复任何失败的测试

  - [x] 8.2 运行所有前端测试
    - 执行 `npm test` 确保所有测试通过
    - 修复任何失败的测试

  - [x] 8.3 更新 PROGRESS.md
    - 记录属性测试完成状态
    - 更新测试覆盖率信息

## 备注

- 属性测试使用 proptest（Rust）和 fast-check（TypeScript）
- 需要桌面会话的测试（剪贴板）在 CI 环境中可能需要跳过
- 需要 OCR 模型的测试可能需要 mock 或条件编译
- 每个属性测试都应该有清晰的 `**Validates: Requirements X.X**` 注释
