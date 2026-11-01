# 自绘 Counter

只依赖 scoop.ui 及 graphics／surface／window 的普通 executable Cone，不依赖控件库。点击蓝色图形会增加组件的成员 State；下面的圆点显示计数，图形宽度通过 Surface spring 动画变化。没有交互且动画结束后不继续请求帧。

```sh
scoop run sysroot/src/scoop.ui/examples/custom --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
```

Counter 只返回 Element 描述，实例和 State 在更新时保留。Element 在 onMount 中创建 Path、输入区域和动画，全部登记到 ComponentScope；示例不为它们逐个 use。Path 构造时的临时 builder 在本地 use 中关闭。窗口根由 runUiApplication 管理，关闭窗口或抛错都会卸载组件。

本期先在 macOS aarch64 运行；x86_64 在本期功能完成后统一补验，Windows 后续接入。该示例演示自绘、状态和动画，不承担 Button／Checkbox 的完整键盘与点击语义。
