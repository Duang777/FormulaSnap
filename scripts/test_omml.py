"""测试 OMML 转换"""
import subprocess
import sys

# 简单的 LaTeX
latex = r"e_{v}^{(1)}"

# 调用 Rust 程序测试
# 这里我们直接用 Python 的 latex2mathml 来看 MathML
import latex2mathml.converter
mathml = latex2mathml.converter.convert(latex)
print("MathML:")
print(mathml)
print()

# 手动构造预期的 OMML
expected_omml = '''<m:sSubSup>
  <m:sSubSupPr/>
  <m:e><m:r><m:t>e</m:t></m:r></m:e>
  <m:sub><m:r><m:t>v</m:t></m:r></m:sub>
  <m:sup><m:r><m:t>(1)</m:t></m:r></m:sup>
</m:sSubSup>'''
print("Expected OMML structure:")
print(expected_omml)
