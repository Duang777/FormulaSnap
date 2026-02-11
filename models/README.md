# OCR 模型文件

FormulaSnap 使用 pix2tex (LaTeX-OCR) 的 ONNX 模型进行离线公式识别。

## 获取模型

请将 `pix2tex.onnx` 模型文件放置在此目录下。

### 方法一：从 pix2tex 项目导出

```bash
pip install pix2tex
python -c "
from pix2tex.cli import LatexOCR
import torch
model = LatexOCR()
# 导出为 ONNX 格式
dummy_input = torch.randn(1, 1, 64, 672)
torch.onnx.export(model.model, dummy_input, 'models/pix2tex.onnx',
                   input_names=['input'], output_names=['output'],
                   dynamic_axes={'input': {3: 'width'}, 'output': {1: 'seq_len'}})
"
```

### 方法二：从 Hugging Face 下载

访问 https://huggingface.co/breezedeus/pix2text-mfr 或类似仓库获取预转换的 ONNX 模型。

## 文件要求

- 文件名: `pix2tex.onnx`
- 输入: `[batch=1, channels=1, height=64, width=动态]` (float32, 灰度图, 归一化到 [0,1])
- 输出: token 索引 (int64) 或 logits (float32)
