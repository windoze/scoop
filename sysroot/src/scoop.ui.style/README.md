# scoop.ui.style

普通 Scoop 库，提供公共主题绘图协议、几何、语义参数，以及可复用的装饰与内容效果。`scoop.ui` 不依赖本库，基础控件和第三方自绘使用同一套公开接入。公共 Style、主题绘图、三套风格、控件消费和完整展示已完成 macOS aarch64 实施与 x86_64 补验，范围见[实施记录](../../../docs/ui/PROGRESS.md)；x86_64 使用 Apple Silicon 上的 Rosetta，Windows 后续。

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

Theme 提供普通 tokens 与各部件的 metrics／geometry，ThemePainter 消费同一份几何和视觉数据。基础能力包括表面、Checkbox 三态标记、Radio 标记，以及带轨道／滑块／可选端部按钮的 Scrollbar；这些入口只需要 Canvas，不依赖窗口、输入区域或挂载。三套风格是独立普通 Cone：

| Cone | 工厂 | 参数 |
| --- | --- | --- |
| scoop.ui.style.macos | macosTheme | ThemeMode.Light／Dark |
| scoop.ui.style.material | materialTheme | ThemeMode.Light／Dark |
| scoop.ui.style.fluent | fluentTheme | ThemeMode.Light／Dark |

第三方 Element 在 onMount 中保存 `scope.resource { ThemePainterCache() }`，在 paint 中使用当前 Props 的 Theme：

```scoop
val visual = CheckboxVisual(CheckMark.Mixed, InteractionVisual(focused = true))
val geometry = theme.checkboxGeometry(bounds, visual)
painters.get(theme).drawCheckbox(context.canvas, geometry, visual)
```

metrics 可查询建议／最小尺寸、padding、边框内缩及焦点外扩；geometry 使用调用者最终 bounds，返回实际外框、内容区、圆形、标记线段或滑块行程等类型化描述。Options 中显式给出的尺寸与样式在计算前解析。SurfaceGeometry.style 保存本次外观，contentStyle 只保留内容效果和 clip；在 drawSurface 的 Background 和 Foreground 阶段之间，用自己的 StylePainter 应用到实际内容。Radio 始终保持圆形，局部参数可覆盖尺寸、环宽、圆点内缩及颜色；完整 Radio 分组与滚动行为由控件／模型负责。

`theme.tokens` 提供 palette、typography、spacing、corners、effects 和 motion。文字覆盖用 SpanStyle.resolve；外框与内容效果继续使用本库 Style。Theme 不持有 native 资源；ThemePainterCache 保留当前 painter，换主题时先创建再替换，及时关闭旧资源。绘图中的临时 Canvas 不能保存，颜色和动画采样不会创建独立时钟。

应用持有稳定 ThemeSource，使用 `context(source) { ... }` 包围实际 Application.run／runWithAsyncio 及挂载入口。Component 可在挂载工厂的构造阶段保存来源，在 render 中读取 `source.current.value`；首轮 render 早于 onMount，不应等到 onMount 才初始化这个引用。更新主题 State 通知实际订阅者。局部主题通过明确 Props 传入，临时 Context 不会自动成为挂载树 Provider。三套主题、公共绘图部件和实际控件的完整用法见[主题展示](../scoop.ui.component/examples/themes/README.md)。

详细合同见[公共样式规范](../../../docs/specs/SCOOP-UI-STYLE-SPEC.md)、[图像滤镜规范](../../../docs/specs/SCOOP-GRAPHICS-EFFECTS-SPEC.md)和[主题绘图规范](../../../docs/specs/SCOOP-UI-THEME-SPEC.md)。普通绘图／缓存测试位于本 Cone，实际窗口与错误收尾位于 tests-cones/ui 和 tests-cones/errors，均使用 scoop.test。
