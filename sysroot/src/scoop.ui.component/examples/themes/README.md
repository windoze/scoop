# 主题与公共绘图部件展示

这是使用正式控件库的普通 executable Cone，显式依赖 macOS、Material、Fluent 三套主题。运行前按仓库的 sysroot 准备流程构建这些库。

```sh
scoop run sysroot/src/scoop.ui.component/examples/themes --release --target aarch64-apple-darwin --sysroot /path/to/sysroot
```

顶部选择风格以及 Light／Dark／Follow system。风格与明暗分别保存，跟随系统使用窗口的 ThemeChanged 通知；手动选择只改变自绘内容，不修改系统设置或原生窗框。

表格直接使用 Theme 的几何查询和 ThemePainter 的 Canvas 绘图，展示表面、Checkbox、Radio、两轴 Scrollbar 和带标签的操作表面。列包括普通、悬停、按下、禁用、选中、混合和焦点；不适用的组合显示横线。Scrollbar 的 Selected 列展示另一滑块位置，完整滚动行为不属于这个示例。

下方使用真正的 Label、Button、Checkbox、Frame、Row、Column 和 Stack。点击计数按钮、切换自有状态 Checkbox，或给控件设置焦点后切换主题，可以观察尺寸、字体和外观更新，而计数与勾选状态保留。Enabled 控制计数按钮是否接收激活；Disabled 按钮始终禁用。

Drop shadow、Glow、Blur 依次作用于整个预览子树的内容 alpha，外框装饰仍由主题绘制。切换效果只更新现有 Frame 的 Props，不增删包装层。阴影和光晕不增加布局、基线或点击范围。表格中的悬停／按下状态是固定绘图输入，下方真实控件的状态过渡使用 Surface Animatable。

`GalleryPainter` 是示例自身的普通绘图辅助对象，可直接在 RasterSurface 上使用。它复用当前主题 painter 与文字布局，主题或字体 revision 改变时替换资源；挂载版本由 ComponentScope 统一关闭。应用只为共享字体及 UiHost 保留最外层 use。

本 Cone 的 scoop.test 包含主题来源与系统跟随、六种组合在两种 DPI 的离屏绘图，以及整个展示的真实 Metal／UI 切换、原生主题通知、资源释放和最终停帧。更细的几何／像素合同在 style 的 themes 测试 Cone，完整控件输入和焦点保持在 component 的 interaction 测试 Cone。

本期先在 macOS aarch64 完成，x86_64 最后统一补验，Windows 后续接入。
