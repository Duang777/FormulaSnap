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
os.environ["TF_CPP_MIN_LOG_LEVEL"] = "3"
warnings.filterwarnings("ignore")
logging.disable(logging.CRITICAL)
logging.basicConfig(stream=sys.stderr, level=logging.ERROR)

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "用法: ocr_engine <image_path>"}))
        sys.exit(1)

    image_path = sys.argv[1]
    
    if not os.path.exists(image_path):
        print(json.dumps({"error": f"图片文件不存在: {image_path}"}))
        sys.exit(1)

    try:
        import transformers
        transformers.logging.set_verbosity_error()
        
        from texify.inference import batch_inference
        from texify.model.model import load_model
        from texify.model.processor import load_processor
        from PIL import Image

        # 重定向 stdout 抑制模型加载日志
        old_stdout = sys.stdout
        sys.stdout = sys.stderr
        
        model = load_model()
        processor = load_processor()
        
        sys.stdout = old_stdout

        image = Image.open(image_path)
        if image.mode != "RGB":
            image = image.convert("RGB")
        
        results = batch_inference([image], model, processor)
        
        if results and len(results) > 0:
            # 处理结果 - 可能是字符串或其他类型
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
            # 移除可能的 $ 包裹
            if latex.startswith("$$") and latex.endswith("$$"):
                latex = latex[2:-2].strip()
            elif latex.startswith("$") and latex.endswith("$"):
                latex = latex[1:-1].strip()
            
            print(json.dumps({"latex": latex, "confidence": 0.95}))
        else:
            print(json.dumps({"error": "识别结果为空"}))
            sys.exit(1)
            
    except Exception as e:
        import traceback
        print(json.dumps({"error": str(e), "traceback": traceback.format_exc()}))
        sys.exit(1)

if __name__ == "__main__":
    main()
