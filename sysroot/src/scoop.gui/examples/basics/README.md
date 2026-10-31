# GUI 基础层组合示例

一个普通 Scoop executable Cone，直接组合 `scoop.graphics`、`scoop.gui` 与 `scoop.window`，无需 component。

```sh
scoop run sysroot/src/scoop.gui/examples/basics --release --sysroot /path/to/sysroot
```

窗口展示旋转星形 Path、命中与绘制共用的变换、hover 颜色 tween、拖动结束后的 spring，以及文本区域的段落布局、IME 批次和候选窗矩形。拖动使用窗口坐标计算位移，区域本身可继续旋转；按下路径由 GUI 持续保留。

正常松开和失焦／捕获丢失造成的拖动取消都会回弹到起点，包括拖出窗口后通过键盘切换活动应用的情况。取消不等于正常 Up，回弹在收到取消时开始；窗口关闭时只清理状态，不启动新的动画。

文本与 selection／marked range 由示例自己的 DemoEditor 管理。编辑通过 TextInputBatch，左右键与退格按 grapheme 边界操作；宽度或文字改变才重新布局，光标移动复用旧布局。输入区用于短文本演示，尚未提供滚动、剪贴板、撤销或完整编辑器命令。

`scoop test <本目录> --release --sysroot <sysroot>` 运行不需要桌面的编辑与拖动回弹测试。运行示例和实际窗口呈现需要已解锁、显示开启的 macOS 或 Windows 会话；x86_64 在 Apple Silicon 上通过 Rosetta 验证时应记录执行环境。

Windows 安装版使用 `sco`；例如 `sco run sysroot/src/scoop.gui/examples/basics --release --target x86_64-w64-windows-llvm-mingw-ucrt --sysroot C:/path/to/sysroot`。也可选择 Microsoft UCRT 或 LLVM-MinGW MSVCRT 的完整目标名，前提是 sysroot 已准备对应资源。示例的输入、动画和编辑源码与 macOS 共用。
