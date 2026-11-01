# 测试字体

以下文件原样取自已固定的 Skia revision `f326a1e25fc736704063922b9ce76b527b1c5efe` 的 `resources/fonts/`，仅用于本 Cone 的测试：

- `Roboto-Regular.ttf`：Copyright 2011 Google Inc. All Rights Reserved.，Apache License 2.0，完整条款见 `Roboto-LICENSE.txt`。
- `NotoSansCJK-VF-subset.otf.ttc`：Skia 的 CJK variable font 子集，仅包含“是”。Copyright 2014–2021 Adobe，Reserved Font Name “Source”，SIL Open Font License 1.1，完整条款见 `Noto-LICENSE.txt`。许可文本来自 Noto CJK 的 `Sans/LICENSE`。

测试用单独 family 别名登记这些字体，字体数据不进入生产 Cone 的默认字体集合。固定字体用于宽度、换行、variation 和绘图验证；系统 fallback 的测试只检查 Unicode 位置、方向和可用 glyph，不固定随操作系统变化的系统字体 metrics。
