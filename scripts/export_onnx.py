"""
将 pix2tex (LaTeX-OCR) 模型导出为 ONNX 格式。
首次运行会自动下载模型权重（约 200MB）。
"""
import os
import sys
import torch
import torch.nn as nn

def main():
    output_path = os.path.join(os.path.dirname(__file__), "..", "models", "pix2tex.onnx")
    output_path = os.path.abspath(output_path)
    os.makedirs(os.path.dirname(output_path), exist_ok=True)

    if os.path.exists(output_path):
        print(f"模型文件已存在: {output_path}")
        return

    print("正在加载 pix2tex 模型（首次运行会下载权重）...")

    try:
        from pix2tex.cli import LatexOCR
        model_instance = LatexOCR()
        model = model_instance.model

        # pix2tex 的 encoder 部分接受图片输入
        # 我们导出整个模型的 encoder
        encoder = model.encoder
        encoder.eval()

        # pix2tex encoder 输入: [batch, channels=1, height=64, width=variable]
        dummy_input = torch.randn(1, 1, 64, 672)

        print(f"正在导出 ONNX 模型到: {output_path}")
        torch.onnx.export(
            encoder,
            dummy_input,
            output_path,
            input_names=["input"],
            output_names=["output"],
            dynamic_axes={
                "input": {0: "batch", 3: "width"},
                "output": {0: "batch", 1: "seq_len"},
            },
            opset_version=14,
            do_constant_folding=True,
        )
        print(f"导出成功! 文件大小: {os.path.getsize(output_path) / 1024 / 1024:.1f} MB")

    except Exception as e:
        print(f"encoder 导出失败 ({e})，尝试导出完整模型...")
        # 回退方案：尝试用 trace 方式导出
        try:
            from pix2tex.cli import LatexOCR
            ocr = LatexOCR()
            model = ocr.model
            model.eval()

            # 尝试直接 trace
            dummy = torch.randn(1, 1, 64, 672)
            with torch.no_grad():
                traced = torch.jit.trace(model.encoder, dummy)

            torch.onnx.export(
                traced,
                dummy,
                output_path,
                input_names=["input"],
                output_names=["output"],
                dynamic_axes={
                    "input": {0: "batch", 3: "width"},
                    "output": {0: "batch", 1: "seq_len"},
                },
                opset_version=14,
            )
            print(f"trace 导出成功! 文件大小: {os.path.getsize(output_path) / 1024 / 1024:.1f} MB")
        except Exception as e2:
            print(f"导出失败: {e2}")
            print("请手动获取 ONNX 模型文件，参见 models/README.md")
            sys.exit(1)

    # 验证 ONNX 模型
    try:
        import onnx
        model = onnx.load(output_path)
        onnx.checker.check_model(model)
        print("ONNX 模型验证通过 ✓")
    except Exception as e:
        print(f"ONNX 验证警告: {e}")

if __name__ == "__main__":
    main()
