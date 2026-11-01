# scoop.ui.style.fluent

普通自绘主题库，提供 `fluentTheme(ThemeMode.Light)` 与 `fluentTheme(ThemeMode.Dark)`，返回 scoop.ui.style 的 Theme。风格采用独立字体回退与字重、中等尺寸和较小圆角、分层表面与细焦点外框、带端部按钮的滚动条；不依赖控件库，不创建输入区域、State 或任务，也不更改原生窗口装饰。

通过 Theme 查询表面、Checkbox／Radio 标记和 Scrollbar 的 metrics／geometry，再交给主题创建的 ThemePainter 绘制。离屏直接使用普通 Canvas；自绘 Element 通过 ComponentScope 持有 ThemePainterCache，换主题时替换并关闭旧 painter，应用无需逐组件 use。

主题工厂返回不可变的普通描述。ThemeSource.current 是整体切换入口；主题及模式变化保留组件身份，实际使用者按新几何布局。公共语义颜色、字体、尺寸、圆角、效果与动画参数通过 tokens 提供。字体使用 family 回退，字体实际存在与 shaping 由 graphics.text 负责。

四类部件共同使用公共 ShapeTheme 算法，各预设显式声明自己的参数，控件不按风格分支。完整 Radio 分组、滚动、动画驱动及点击行为仍属于控件或模型。

用法及资源／效果合同见 [style README](../scoop.ui.style/README.md) 和[主题规范](../../../docs/specs/SCOOP-UI-THEME-SPEC.md)。三组主题的比较、离屏像素、字体、资源和真实 Metal 验证位于 [themes 测试 Cone](../scoop.ui.style/tests-cones/themes)，当前结果见[实施记录](../../../docs/ui/PROGRESS.md)。本期先完成 macOS aarch64，x86_64 最后补验，Windows 后续。
