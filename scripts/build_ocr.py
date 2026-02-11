"""
使用 PyInstaller 打包 OCR 模块为独立可执行文件
"""
import PyInstaller.__main__
import os
import sys

script_dir = os.path.dirname(os.path.abspath(__file__))
ocr_script = os.path.join(script_dir, "ocr_server.py")

PyInstaller.__main__.run([
    ocr_script,
    '--onedir',
    '--name=ocr_engine',
    '--distpath=../src-tauri/ocr_engine',
    '--workpath=../build/pyinstaller',
    '--specpath=../build',
    '--clean',
    '--noconfirm',
    # 隐藏控制台窗口
    '--noconsole',
    # 收集 texify 相关依赖
    '--collect-all=texify',
    '--collect-all=transformers',
    '--collect-all=tokenizers',
    '--collect-all=torch',
    '--collect-all=PIL',
    '--hidden-import=texify',
    '--hidden-import=texify.inference',
    '--hidden-import=texify.model.model',
    '--hidden-import=texify.model.processor',
])
