"""
Texify OCR 服务脚本
使用 texify 模型识别数学公式，输出 LaTeX（JSON 格式）
"""
import sys
import json
import os
import warnings
import logging

# 保存原始 stdout，用于最后输出 JSON
_original_stdout = sys.stdout

# 立即重定向 stdout 到 stderr，防止任何导入时的输出污染 JSON
sys.stdout = sys.stderr

# 抑制所有警告和日志
os.environ["HF_HUB_DISABLE_SYMLINKS_WARNING"] = "1"
os.environ["TRANSFORMERS_VERBOSITY"] = "error"
os.environ["TF_CPP_MIN_LOG_LEVEL"] = "3"
warnings.filterwarnings("ignore")
logging.disable(logging.CRITICAL)

def output_json(data):
    """输出 JSON 到原始 stdout"""
    print(json.dumps(data), file=_original_stdout)

def main():
    if len(sys.argv) < 2:
        output_json({"error": "用法: ocr_engine <image_path>"})
        sys.exit(1)

    image_path = sys.argv[1]
    
    if not os.path.exists(image_path):
        output_json({"error": f"图片文件不存在: {image_path}"})
        sys.exit(1)

    try:
        import transformers
        transformers.logging.set_verbosity_error()
        
        from texify.inference import batch_inference
        from texify.model.model import load_model
        from texify.model.processor import load_processor
        from PIL import Image

        model = load_model()
        processor = load_processor()

        image = Image.open(image_path)
        if image.mode != "RGB":
            image = image.convert("RGB")
        
        results = batch_inference([image], model, processor)
        
        if results and len(results) > 0:
            result = results[0]
            if isinstance(result, str):
                latex = result
            elif hasattr(result, 'text'):
                latex = result.text
            elif isinstance(result, dict):
                latex = result.get('text', result.get('latex', str(result)))
            else:
                latex = str(result)
            
            latex = latex.strip()
            if latex.startswith("$$") and latex.endswith("$$"):
                latex = latex[2:-2].strip()
            elif latex.startswith("$") and latex.endswith("$"):
                latex = latex[1:-1].strip()
            
            output_json({"latex": latex, "confidence": 0.95})
        else:
            output_json({"error": "识别结果为空"})
            sys.exit(1)
            
    except Exception as e:
        output_json({"error": str(e)})
        sys.exit(1)

if __name__ == "__main__":
    main()
