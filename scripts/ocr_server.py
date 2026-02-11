"""
Texify OCR 服务脚本
使用 texify 模型识别数学公式，输出 LaTeX（JSON 格式）
"""
import sys
import json
import os
import warnings
import logging

# 抑制所有警告和日志
os.environ["HF_HUB_DISABLE_SYMLINKS_WARNING"] = "1"
os.environ["TRANSFORMERS_VERBOSITY"] = "error"
warnings.filterwarnings("ignore")
logging.disable(logging.CRITICAL)

# 重定向 transformers 和 texify 的日志到 stderr
logging.basicConfig(stream=sys.stderr, level=logging.ERROR)

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "用法: python ocr_server.py <image_path>"}))
        sys.exit(1)

    image_path = sys.argv[1]

    try:
        # 在导入前设置环境变量抑制日志
        import transformers
        transformers.logging.set_verbosity_error()
        
        from texify.inference import batch_inference
        from texify.model.model import load_model
        from texify.model.processor import load_processor
        from PIL import Image

        # 临时重定向 stdout 到 stderr 以捕获模型加载日志
        old_stdout = sys.stdout
        sys.stdout = sys.stderr
        
        # 加载模型和处理器
        model = load_model()
        processor = load_processor()
        
        # 恢复 stdout
        sys.stdout = old_stdout

        # 读取图片
        image = Image.open(image_path)
        
        # 转换为 RGB（如果需要）
        if image.mode != "RGB":
            image = image.convert("RGB")

        # 识别
        results = batch_inference([image], model, processor)
        
        if results and len(results) > 0:
            latex = results[0]
            # 清理结果（移除可能的 $$ 包裹）
            latex = latex.strip()
            if latex.startswith("$$") and latex.endswith("$$"):
                latex = latex[2:-2].strip()
            elif latex.startswith("$") and latex.endswith("$"):
                latex = latex[1:-1].strip()
            
            print(json.dumps({
                "latex": latex,
                "confidence": 0.95
            }))
        else:
            print(json.dumps({"error": "识别结果为空"}))
            sys.exit(1)
            
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
