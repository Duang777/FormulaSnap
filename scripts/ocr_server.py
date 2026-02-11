"""
Texify OCR 服务脚本
使用 texify 模型识别数学公式，输出 LaTeX（JSON 格式）

可以作为 Python 脚本运行，也可以用 PyInstaller 打包为独立可执行文件。
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

# 全局缓存模型（PyInstaller 打包后复用）
_model = None
_processor = None

def get_model_and_processor():
    """懒加载模型和处理器"""
    global _model, _processor
    if _model is None or _processor is None:
        import transformers
        transformers.logging.set_verbosity_error()
        
        from texify.model.model import load_model
        from texify.model.processor import load_processor
        
        # 重定向 stdout 抑制模型加载日志
        old_stdout = sys.stdout
        sys.stdout = sys.stderr
        
        _model = load_model()
        _processor = load_processor()
        
        sys.stdout = old_stdout
    
    return _model, _processor

def recognize(image_path: str) -> dict:
    """识别图片中的公式"""
    from texify.inference import batch_inference
    from PIL import Image
    
    model, processor = get_model_and_processor()
    
    image = Image.open(image_path)
    if image.mode != "RGB":
        image = image.convert("RGB")
    
    results = batch_inference([image], model, processor)
    
    if results and len(results) > 0:
        latex = results[0].strip()
        # 移除可能的 $ 包裹
        if latex.startswith("$$") and latex.endswith("$$"):
            latex = latex[2:-2].strip()
        elif latex.startswith("$") and latex.endswith("$"):
            latex = latex[1:-1].strip()
        
        return {"latex": latex, "confidence": 0.95}
    else:
        return {"error": "识别结果为空"}

def main():
    if len(sys.argv) < 2:
        print(json.dumps({"error": "用法: ocr_engine <image_path>"}))
        sys.exit(1)

    image_path = sys.argv[1]
    
    if not os.path.exists(image_path):
        print(json.dumps({"error": f"图片文件不存在: {image_path}"}))
        sys.exit(1)

    try:
        result = recognize(image_path)
        print(json.dumps(result))
        if "error" in result:
            sys.exit(1)
    except Exception as e:
        print(json.dumps({"error": str(e)}))
        sys.exit(1)

if __name__ == "__main__":
    main()
