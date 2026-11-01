# scoop.ui.style

普通 Scoop 库，提供可复用的公共装饰与内容效果。`scoop.ui` 不依赖本库，基础控件和第三方自绘使用同一套公开接入。公共 Style 已在 macOS aarch64 验证，主题绘图及三套风格按[实施记录](../../../docs/ui/PROGRESS.md)继续接入；x86_64 最后补验，Windows 后续。

```toml
[dependencies]
"scoop:scoop.ui.style" = "0.1.0"
```

通过 Props 更新普通描述，应用不关闭 Style，也不为每个装饰或组件写 use：

```scoop
styled(content, Style(
    background = Some(Paint(Color.White)),
    border = Some(Border(1.0, Color(0.7, 0.7, 0.7))),
    cornerRadius = 8.0,
    boxShadows = [BoxShadow(Point(0.0, 3.0), 4.0, 4.0, Color.Black.withAlpha(0.2))],
    contentEffects = [ContentEffect.Glow(2.0, 2.0, Color(0.0, 0.4, 1.0, 0.5))]
))
```

styled 始终保留包装，包括空 Style；增加或移除效果不重建内容的实例和 State。它原样传递子项的约束、尺寸与基线，不创建 InputArea 或增加隐式 padding。点击范围来自内容本身；`clipContent = true` 使用同一圆角轮廓裁剪最终内容输出和子输入。

顺序是外框阴影、背景、经过有序效果的内容、向内的边框。BoxShadow 使用填满的圆角外框；DropShadow 使用真实内容 alpha；Glow 是零 offset 的着色 alpha 模糊与原内容 SrcOver 合成。`[Blur(...), DropShadow(...)]` 与相反顺序不同。文字直接使用实际绘制结果，透明图像和子组件采用同一路径；不需要 glyph 转 Path。

sigma 是 Gaussian 标准差，sigma 和 offset 使用局部逻辑单位。根 DPI 只施加一次，图层没有按布局框添加隐式 crop，因此向外的光晕和阴影可以显示。祖先 clip 和实际目标仍限制可见范围。普通内容 Blur 不采样背后的画面，Backdrop blur 另行设计。

完全自绘 Element 在 onMount 中保存 `scope.resource { StylePainter() }`，在 paint 中调用 `painter.paint(context, style) { content -> ... }`；回调使用提供的 PaintContext，可调用 paintChildren。也提供显式 Rect 和普通 Canvas 的入口，供离屏使用。原生滤镜／轮廓由 painter 缓存并及时替换关闭，组件卸载或窗口关闭由 scope 清理。回调内不能重入或提前关闭同一 painter，另一个 painter 可以正常嵌套。

StyleOverride 只替换明确指定的属性：StyleValue.Inherit 继承本轮输入，StyleValue.Set(None) 可以清除可空背景或边框，其余属性继续更新。字体、段落和混排仍使用 graphics.text 的 TextStyle／SpanStyle／ParagraphStyle。

没有独立效果时钟，也没有子树像素缓存。真实帧每次绘制当帧内容，仅复用当前滤镜与几何。参数动画在 Element 的 update／输入中设置 Surface Animatable，在 paint 中读取采样值；完成、暂停、取消或隐藏后按 Surface 规则停帧。

详细合同见[公共样式规范](../../../docs/specs/SCOOP-UI-STYLE-SPEC.md)、[图像滤镜规范](../../../docs/specs/SCOOP-GRAPHICS-EFFECTS-SPEC.md)和[主题绘图规范](../../../docs/specs/SCOOP-UI-THEME-SPEC.md)。普通绘图／缓存测试位于本 Cone，实际窗口与错误收尾位于 tests-cones/ui 和 tests-cones/errors，均使用 scoop.test。
