# 📐 FormulaSnap

一款桌面端公式识别工具，截图即可将数学公式转换为 LaTeX，支持一键复制到 Word。

## ✨ 功能特性

- **截图识别** - 框选屏幕区域，自动识别数学公式
- **实时预览** - LaTeX 编辑器 + 公式渲染预览
- **一键复制** - 复制为 Word 可粘贴的 OMML 格式或 LaTeX 源码
- **历史记录** - 自动保存识别历史，支持搜索和收藏
- **批量导出** - 导出为 `.tex` 或 `.docx` 文件

## 📦 安装

从 [Releases](https://github.com/Duang777/FormulaSnap/releases) 页面下载对应平台的安装包：

- Windows: `.msi` 或 `.exe`
- macOS: `.dmg`
- Linux: `.deb` 或 `.AppImage`

## 🚀 使用方法

1. 点击「截图识别」按钮
2. 框选包含公式的屏幕区域
3. 等待识别完成，在编辑器中调整
4. 点击「复制到 Word」或「复制 LaTeX」

## 🛠️ 本地开发

### 环境要求

- Node.js 18+
- Rust 1.70+
- pnpm 或 npm

### 安装依赖

```bash
npm install
```

### 启动开发服务器

```bash
cargo tauri dev
```

### 构建发布版本

```bash
cargo tauri build
```

## 📄 License

MIT
