# Scoop 公共主题绘图与几何规范

日期：2026-11-03。状态：公共主题协议、四类部件几何／绘图、三套风格及 Light／Dark、基础控件消费和完整展示已在 macOS aarch64 交付并完成 x86_64 补验，验证范围见 [UI 实施记录](../ui/PROGRESS.md)。x86_64 使用 Apple Silicon 上的 Rosetta，Windows 后续。本文扩展 [公共样式规范](SCOOP-UI-STYLE-SPEC.md)，保留其内容效果、资源、稳定身份与停帧合同。

2026-11-04：完整语义配色、显式减少动态与第四套 shadcn 预设已完成 macOS aarch64 定向验证，见 [shadcn 实施记录](../ui/shadcn/PROGRESS.md)。这一扩展的 x86_64 验证留在控件目录收尾。

## 1. 本轮调整与分层

主题以公共绘图部件及其几何为核心。`scoop.ui.style` 定义 Theme／ThemePainter 协议、语义颜色／字体／尺寸／动画参数、类型化视觉状态、尺寸输入、几何结果，以及通用 Style／装饰适配。`scoop.ui.component` 负责持久状态、布局组合、输入与业务，不定义公共主题合同，也不依赖某个主题实现。

本轮明确替换此前三项设计：

1. 不再把公共主题及默认颜色／尺寸都放在 ComponentDefaults 或控件专用样式中；它们由 style 的公共 Theme 提供。
2. Button／Checkbox 不再直接用自己的一套背景、圆角、焦点和标记绘图。它们消费主题的公开几何与绘图；ButtonStyle／CheckboxStyle 只表达明确的局部覆盖和控件特有输入。
3. 控件不在构造时永久保存当时的 Theme 或解析后的外观。保存稳定 ThemeSource，在 render 中读取其当前 Theme，并把本轮主题及视觉输入通过 Props 交给持久 Element。

四组实现为普通独立 Cone：`scoop.ui.style.macos`、`scoop.ui.style.material`、`scoop.ui.style.fluent`、`scoop.ui.style.shadcn`。它们依赖 style 和实际使用的 graphics／graphics.text／surface，不依赖 component；style 不反向引用主题库。应用选择并依赖所需主题。`scoop.ui` 仍不依赖 style 或 component。

四套主题的生产源码不使用 OS 条件。macos／fluent／material／shadcn 是自绘风格名称；字体可有偏好列表，但以 graphics.text 的通用 family fallback 收尾，缺少某个系统专有字体不要求上层选择平台。

实现库可复用公共的几何、StylePainter 和普通绘图辅助代码。共同算法按实际调用复用，差异由各自的主题参数或绘图实现表达；控件中不得按主题类型或名称分支。自绘主题不替换 window 的原生装饰配置，也不声称与操作系统某个版本逐像素一致。

## 2. 主题对象、绘图对象与普通数据

Theme 是不可变的主题描述／协议实现，提供语义参数和各部件的 metrics／geometry 方法；不拥有需要应用逐次关闭的 Path／滤镜，不读取业务 State。ThemeSource 和 UI 消费适配与这些纯几何／绘图入口分开。

几何方法只接收有限的逻辑 bounds、尺寸选项和类型化视觉数据，不需要 Canvas，不访问 Application、InputArea、挂载节点或 Context。每类部件返回自己的结果类型，不用名称字典或通用无类型字段。绘制严格消费该结果中的尺寸、轮廓和区域，不能重新估算另一份布局。

首批采用 `Theme.tokens`、`surfaceMetrics／checkboxMetrics／radioMetrics／scrollbarMetrics` 与对应 geometry 方法；各部件的 Options 表达显式尺寸和局部覆盖。几何结果同时保存本次解析的普通 Style／颜色快照，绘图使用这份结果，不重新解析另一套尺寸。InteractionVisual 的可选 hover／press 采样值和选择进度以 0～1 表达，省略时使用对应布尔视觉状态；有限输入收敛到该范围。改变采样值时重新计算普通几何描述，原生轮廓缓存仍只按真实轮廓变化替换。

四套预设共同使用 `ShapeTheme` 的圆角表面、勾选线段、圆环／圆点和滚动条几何算法，各库显式构造自己的参数。Theme 协议仍允许普通第三方实现独立的几何与 ThemePainter；控件不依赖 ShapeTheme。复用只覆盖这四类已有部件，不引入主题插件、节点注册或自动控件生成。

原生缓存由 `ThemePainter : Closeable` 持有，通过 `Theme.createPainter()` 创建；其 drawSurface／drawCheckbox／drawRadio／drawScrollbar 只接收普通 Canvas、对应几何和视觉数据。这样离屏程序可以长期复用同一 painter，自绘 Element 则通过 ComponentScope 登记拥有可替换 painter 的 `ThemePainterCache`。cache 在主题实际变化时先构建新 painter，再替换并关闭旧值；创建失败保留旧值用于清理或下一次绘制。应用不为每个部件／每帧使用 use。

ThemePainter 和缓存限创建线程使用，close 幂等。ThemePainterCache.clear 释放当前 painter 并回到可复用的空状态，供同一个 Element 从主题表面切换为普通装饰；close 则结束整个缓存的有效期。绘图同步借用 Canvas，退出后不能保存该画布；正常及异常路径恢复嵌套状态。Theme／几何只是普通描述，不替代实际资源所有者。普通离屏用法为先查询 geometry，再把同一 geometry 交给对应 painter 的 draw 方法；不要求运行 UI。

## 3. 通用参数与覆盖

Theme 提供以下公共语义参数，使没有标准绘图部件的第三方控件也能选用相同外观：

- 颜色：ThemePalette 的 background、card、popover、primary、secondary、muted、accent、destructive 都是完整的 `ColorPair(background: Color, foreground: Color)`。primary 表达强调操作；accent 表达候选／悬浮表面。border、input、ring、disabledText、shadow 是独立 Color。chart 是 `ChartPalette(one, two, three, four, five: Color)`；sidebar 是 `SidebarPalette(background, primary, accent: ColorPair, border, ring: Color)`。没有可空颜色或运行时补全。旧 canvas／text 迁到 background，surface 迁到 card，elevated 迁到 popover，accent／onAccent 迁到 primary，secondaryText 迁到 muted.foreground，focus 迁到 ring，error 迁到 destructive.background；旧字段不保留别名。
- 字体：正文、辅助文字和操作标签，直接使用 graphics.text 的 TextStyle；局部字体／字号／颜色覆盖复用 SpanStyle，段落设置继续使用 ParagraphStyle。
- 尺寸：基本间距、各部件建议／最小尺寸、内容内缩、边框宽度、圆角、焦点外扩和 Scrollbar 的实际度量。
- 效果：表面外框阴影和必要的 Style 内容效果参数。
- 动画：进入／离开状态和选择变化的 Surface AnimationSpec。主题只提供参数，绘图方法只消费本帧已采样的数值。

`ThemeMode.Light` 与 `ThemeMode.Dark` 只表达明暗；风格由所选实现独立决定。四套主题同时提供两种模式，不把 Dark 当成另一套风格，也不因切换模式改变组件类型。需要跟随系统的应用在现有 Window ThemeChanged 通知中更新 ThemeSource，主题库不轮询系统、不自行建立窗口监听或时钟。

Style 是完整的已确定描述；新增 `StyleOverride` 表达局部覆盖。未指定属性继续使用本轮主题，背景／边框等可空属性须区分“继承”和“显式清除”，采用类型化 `StyleValue<T>` 的 Inherit／Set，而不是用一个 None 混淆两者。数组效果显式覆盖时整体替换，顺序不变；需要组合时由调用者明确给出顺序。

控件局部样式和主题默认值在本轮几何计算前解析。圆角、边框内缩、padding 或建议尺寸的覆盖必须进入同一份最终几何；不能只改变画出的尺寸，却让布局或拖动继续用主题默认值。纯阴影／glow／blur 仍不改变布局尺寸、基线或命中区域。

### 3.1 部件选项的接收者配置

部件选项通过 `surfaceOptions(configure: SurfaceOptionsBuilder.() -> Unit = {})`、`checkboxOptions(configure: CheckboxOptionsBuilder.() -> Unit = {})`、`radioOptions(configure: RadioOptionsBuilder.() -> Unit = {})`、`scrollbarOptions(configure: ScrollbarOptionsBuilder.() -> Unit = {})` 构造，分别返回对应不可变 Options。每类 Options 提供 `copy(configure: 对应Builder.() -> Unit = {})`，基于当前完整覆盖值派生新值；值与 builder 的构造器内部化。

| Builder | 全部可写属性与默认值 |
| --- | --- |
| SurfaceOptionsBuilder | suggestedSize: Size? = None；minimumSize: Size? = None；padding: Insets? = None；style: StyleOverride = styleOverride()；focus: OutlineStyle? = None；validation: OutlineStyle? = None |
| CheckboxOptionsBuilder | size: Double? = None；minimumSize: Double? = None；markInset: Double? = None；style: StyleOverride = styleOverride()；markColor: Color? = None；focus: OutlineStyle? = None；validation: OutlineStyle? = None |
| RadioOptionsBuilder | size: Double? = None；minimumSize: Double? = None；dotInset: Double? = None；background: StyleValue<Paint?> = Inherit；border: StyleValue<BorderEdge?> = Inherit；dotColor: Color? = None；focus: OutlineStyle? = None；validation: OutlineStyle? = None |
| ScrollbarOptionsBuilder | thickness: Double? = None；minimumThumbLength: Double? = None；trackInset: Double? = None；thumbInset: Double? = None；buttonLength: StyleValue<Double?> = Inherit |

普通可选字段的 None 继续表示使用当前主题；Radio 的 background／border 和 Scrollbar 的 buttonLength 保留 Inherit、Set(None)、Set(value)。这三个覆盖字段各提供同名 Unit 设置方法，把同一槽写为 Set(value)；显式赋入 Inherit 恢复继承。

SurfaceOptionsBuilder／CheckboxOptionsBuilder 提供真实成员 `style(configure: StyleOverrideBuilder.() -> Unit): Unit`。它基于当前 style 修改，重复块累积、空块不重置；给 style 属性赋入完整值表示替换。嵌套和外层配置都同步执行一次，结束后修改被捕获的 builder 不改变结果。回调异常不发布部分结果；选项构造不解析某个主题、不创建 native 资源，尺寸与颜色仍在实际 geometry／metrics 边界检查。

### 3.2 完整主题与部件外观的接收者配置

四套预设的入口为 `macosTheme(mode: ThemeMode = ThemeMode.Light, configure: ShapeThemeBuilder.() -> Unit = {}): ShapeTheme`，materialTheme／fluentTheme／shadcnTheme 使用相同签名。先根据 mode 产生该风格的完整数据，再同步执行 configure 一次。返回的 ShapeTheme 继续实现 Theme；第三方 Theme 实现不需要采用这些 builder。

独立主题使用 `shapeTheme(tokens: ThemeTokens, surfaces: SurfaceParts, checkbox: CheckboxPart, radio: RadioPart, scrollbar: ScrollbarPart, configure: ShapeThemeBuilder.() -> Unit = {}): ShapeTheme`。五个必需参数都是完整描述，不使用未初始化字段、None 或虚构部件补足必需数据。

下列入口也接收尾随 `configure: 对应Builder.() -> Unit = {}` 并返回对应不可变值。表中的必需参数位于 configure 之前；其余可写属性从列出的领域默认值开始。

| 工厂 | 必需参数 | 其他可写属性与默认值 |
| --- | --- | --- |
| themeTypography | body／caption／action: TextStyle | 无 |
| themeEffects | 无 | raised／floating: Array<BoxShadow> = []；emphasis: Array<ContentEffect> = [] |
| themeMotion | 无 | hover／press／selection: AnimationSpec? = None；preference: MotionPreference = Full |
| themeTokens | palette: ThemePalette；typography: ThemeTypography；spacing: ThemeSpacing；corners: ThemeCorners；validation: OutlineStyle | mode: ThemeMode = Light；motion: ThemeMotion = themeMotion()；effects: ThemeEffects = themeEffects()；disabledOpacity: Double = 1.0 |
| surfacePart | metrics: SurfaceMetrics；fill／border／foreground: StateColors；focus: OutlineStyle | shadows: Array<BoxShadow> = [] |
| surfaceParts | plain／action／elevated: SurfacePart | 无 |
| markerAppearance | empty／selected／border／selectedBorder／mark: StateColors；focus: OutlineStyle | shadows: Array<BoxShadow> = [] |
| checkboxPart | metrics: CheckboxMetrics；appearance: MarkerAppearance | 无 |
| radioPart | metrics: RadioMetrics；appearance: MarkerAppearance | 无 |
| scrollbarPart | metrics: ScrollbarMetrics；track／thumb／button／arrow: StateColors | arrowWidth: Double = 1.5 |

每类 Builder 的全部可写属性就是对应值的字段，类型与表中参数／属性一致；ShapeThemeBuilder 的五个属性对应 shapeTheme 的必需参数。对应值均提供 `copy(configure: 对应Builder.() -> Unit = {})`，值和 builder 的构造器内部化。工厂和 copy 共用配置及快照实现。

ThemePalette 的上述十五个字段均为必需数据，没有可适用于任意主题的缺省配色。它保留一次给齐这些字段的完整值构造，并提供 `copy(configure: ThemePaletteBuilder.() -> Unit = {}): ThemePalette`；builder 的同名可写字段从现有配色初始化。ColorPair、ChartPalette、SidebarPalette、StateColors、ThemeSpacing、ThemeCorners、OutlineStyle 和各 Metrics 使用普通值构造。

`shadcnTheme(mode: ThemeMode = Light, configure: ShapeThemeBuilder.() -> Unit = {}): ShapeTheme` 遵循相同的完整描述及配置规则。Base Nova／Neutral 的颜色从上游 OKLCH 转为 sRGB，并保留 alpha；预设源码记录源 token。默认正文／操作文字为 14／20 逻辑单位、操作 weight 500；基准圆角 10，small／medium／large 为 6／8／10；常规表面边框 1、键盘焦点环 3 且 ring alpha 0.5。操作表面建议高 32、水平 padding 10；输入使用独立 input 边框；Checkbox 标记 16、圆角 4。所有长度均为逻辑单位，DPI 沿现有目标接入转换。其余 Button 变体在控件规范中定义，不增加按控件名查询的主题接口。

ThemeMotion 增加 `preference: MotionPreference = Full`（Full／Reduced）。`resolve(spec: AnimationSpec?): AnimationSpec?` 在 Reduced 时返回 None，其余返回原参数；控件在解析局部 transition 后调用，切换 Reduced 时结束当前过渡。该配置是应用显式偏好，不表示已接入 OS 设置。

嵌套成员均为真实成员并返回 Unit：

| Builder | 嵌套成员及 receiver |
| --- | --- |
| ShapeThemeBuilder | tokens: ThemeTokensBuilder；surfaces: SurfacePartsBuilder；checkbox: CheckboxPartBuilder；radio: RadioPartBuilder；scrollbar: ScrollbarPartBuilder |
| ThemeTokensBuilder | palette: ThemePaletteBuilder；typography: ThemeTypographyBuilder；motion: ThemeMotionBuilder；effects: ThemeEffectsBuilder |
| ThemeTypographyBuilder | body／caption／action: TextStyleBuilder |
| SurfacePartsBuilder | plain／action／elevated: SurfacePartBuilder |
| CheckboxPartBuilder／RadioPartBuilder | appearance: MarkerAppearanceBuilder |

所有嵌套配置基于当前字段调用其 copy，重复块累积，空块保留已有设置；直接给属性赋完整值表示替换。模式参数只负责选择预设起点；之后编辑 palette、typography 或某个部件，就只改变该显式字段，不隐式重新计算其他已经生成的文字颜色或部件外观，也不重放回调。需要统一改变多个描述时，由同一配置块明确设置它们。

构造仅发布普通描述，不读取 Context／State、不创建 painter／Path／字体资源或动画；回调异常沿普通调用传播，原值不变。Array 元素槽与子描述不可变，可安全共享；发布后修改被捕获的 builder 不改变结果。颜色、长度与视觉采样继续在原有实际消费边界检查，不增加另一套布局或绘图验证流程。

### 3.3 物理与逻辑几何

Insets(left, top, right, bottom) 始终表示逻辑长度单位中的物理四边。LogicalInsets(start, top, end, bottom) 是独立值类型；resolve(LayoutDirection) 先把 start／end 映射到左右，再返回已检查的 Insets。两者的 all(value) 生成四边相等的值，长度及总和有限非负。主题和通用几何消费已解析的物理 Insets；graphics 不依赖 UI 方向。

Surface／Checkbox 的 metrics 与 geometry 方法接收最后一个 direction: LayoutDirection = Ltr 参数。组件测量和 scene 传入同一有效方向，先解析 Style 的逻辑边框和圆角。SurfaceMetrics／CheckboxMetrics 保存 borderInsets: Insets 与 corners: CornerRadii；不保留原先单一 borderInset／cornerRadius 字段。contentInsets 逐边相加，geometry 使用 graphics.RoundedOutline.normalized／inset 计算相同的外框和内框。Geometry.style 保存本次已经解析为物理值的 Style，ThemePainter 不再推测方向。

公共 RoundedOutline 统一归 graphics，包含 bounds 和 radii。背景、边框、内容裁剪和焦点环复用这份几何；OutlineRing 的 border 使用一个真正均匀的 BorderEdge，焦点外框由 expanded(outset) 得到。Radio 的环宽和 Scrollbar 的统一半径仍按其实际形状语义表示；RadioOptions.border 使用可清除的 BorderEdge 覆盖，不为圆环引入四条独立边。

## 4. 公共绘图部件

各部件的 InteractionVisual 使用明确的 enabled／hovered／pressed／focusVisible／invalid 数据，部件另带自身选择或滚动输入。focusVisible 由统一焦点合同提供，不用逻辑 focused 推测；disabled 时不绘制焦点环。它们都是值描述，不是响应式 State；不存在组件回调、输入注册、任务或业务模型引用。

`OutlineStyle(color, width, outset, borderColor: Color? = None)` 描述一个环及可选的外框颜色替换；None 表示保留部件自身边框，width=0 可省略环。它替代仅限焦点命名的旧描述，普通焦点仍通过部件及 Options 的 focus 配置。ThemeTokens.validation 是完整必需的 OutlineStyle，Surface／Checkbox／Radio Options 的 validation 可局部覆盖。四套主题显式提供验证外观；不根据具体主题名称分支。

invalid 与 focusVisible 独立，前者是应用传入的展示状态，不改变焦点、输入资格或业务值。invalid 时选择 validation；否则只有 enabled 且 focusVisible 才选择 focus。禁用不显示普通焦点环；invalid 边框与环和控件一起合成，不单独再次降低 alpha。状态边框只替换颜色，保留同一份边宽，随后应用显式局部 Style 覆盖。SurfaceGeometry／CheckboxGeometry／RadioGeometry 的 ring: OutlineRing? 保存最终唯一的环，painter 直接绘制这个结果，不再自行重判焦点；invalid 与焦点不叠画两层环。metrics.focusOutset 为配置中焦点／验证两种环所需外扩的最大值，状态切换不改变内容布局。

shadcn 的焦点环替换边框为 ring token；验证边框为 destructive，Dark 使用 0.5 alpha。验证环宽及外扩为 3，Light／Dark 的 destructive alpha 分别为 0.2／0.4；其他三个主题保留原输入错误颜色和 2 宽、2 外扩、0.72 alpha 的基线。

ThemeTokens.disabledOpacity 控制禁用控件的整体合成透明度，有限值按 0～1 截取，默认 1.0 保留部件的 StateColors.disabled 配色。SurfaceGeometry／CheckboxGeometry／RadioGeometry 的 opacity: Double = 1.0 保存本帧解析值；启用时始终为 1。Surface 的 All 和 Checkbox／Radio painter 将整个绘制作为一层；使用 Surface Background／Foreground 的控件在自己的内容两侧包同一个合成层，两阶段不能各自重复应用 opacity。Scrollbar painter 将禁用透明度与本帧 visual.opacity 相乘。shadcn 为 0.5，并提供未衰减的禁用颜色；其他三个主题保持 1.0。透明度不改变测量或输入资格。

shadcn Plain 表面的禁用背景使用 input 颜色的 0.5／0.8 倍 alpha（Light／Dark），随后与内容一起应用 disabledOpacity。

### 4.1 外框／表面

SurfaceMetrics 给出建议／最小尺寸、内容 padding、边框内缩及焦点外扩。语义用途区分普通表面、强调操作面和提升表面，不引用 Button 或 Frame 类型。SurfaceGeometry 至少返回外边界、圆角外轮廓、边框内区域、内容区域和可选焦点轮廓；轮廓为 Rect／圆角等普通描述，原生 Path 留在 painter。

Surface 的绘制支持 Background／Foreground／All 阶段。Background 绘制外框阴影和背景；Foreground 绘制边框及焦点外框；All 依次完成两者，供独立展示使用。控件在两阶段间绘制内容，完整子树的 Style 效果仍由通用装饰适配负责。这样背景、内容、边框的顺序与既有 Style 合同一致，不要求主题方法理解挂载树。

SurfaceGeometry.contentStyle 保留同次解析的内容效果、圆角和 clip；需要内容的调用者在 Background／Foreground 之间通过自己的 StylePainter 应用它。All 没有内容子树，不把内容滤镜偷换为外框滤镜。Checkbox／Radio 标记绘制可把自身标记作为 Style 的内容，外框装饰与标记效果仍遵守相同顺序。

Button 根据主题建议尺寸、自己的标签 metrics 和内容内缩决定布局，然后用最终 bounds 计算几何；Frame 可组合相同外框能力。基线从实际子项加放置偏移得到，不由主题猜测字体基线。控件可把标签或留白纳入命中范围，焦点光晕不会自动扩大输入。

### 4.2 Checkbox 标记

CheckboxVisual 包含 `CheckMark.Off／On／Mixed` 与交互视觉状态；可选采样参数控制标记显现等外观，主题不决定何时切换。CheckboxMetrics 给出建议／最小标记尺寸、内缩与焦点外扩。CheckboxGeometry 明确标记外框、内部区域、勾选／混合标记使用区域和焦点轮廓。

绘制仅根据输入显示空框、勾选或混合横线及其状态外观，不读取 Checkbox.checked、不调用 onChange。菜单、树节点、表格和第三方自绘可直接使用同一能力。当前 Checkbox 控件的布尔 Binding 行为保持原范围，Mixed 绘图的交付不要求同时扩展它的业务值模型。

### 4.3 Radio 标记

RadioVisual 包含 selected 与交互状态及可选采样参数。RadioMetrics 给出建议／最小尺寸、环宽、圆点内缩与焦点外扩；RadioGeometry 返回外圆、内圆／圆点区域和焦点轮廓。圆用中心、半径或等价的普通矩形描述，实际绘制和控件命中可复用同一结果。

主题不建立分组、互斥状态或键盘导航。首批交付可独立调用的 Radio 绘图，完整 Radio 分组不成为当前基础控件的前置。

### 4.4 Scrollbar

Scrollbar 接收 Orientation、有限 position／visibleFraction 及部件视觉状态。position 和 visibleFraction 按 0～1 的归一比例表达，几何计算将合法有限输入收敛到范围内。hoveredPart／pressedPart 使用 Track／Thumb／Decrement／Increment 等类型化枚举，整体 enabled 独立表达。

ScrollbarMetrics 包含建议厚度、最小滑块长度、轨道／滑块内缩及可选端部按钮长度。ScrollbarGeometry 返回轨道、滑块、可选两端按钮的具体矩形／轮廓、滑块起始位置和可移动距离。先扣除端部按钮，再按 visibleFraction 计算并限制滑块长度；滑块行程是轨道可用主轴长度减去最终滑块长度。极短 bounds、全可见内容和零行程仍返回非负有限几何，不产生除零。

滑块位置使用最终行程和 position。主题厚度、最小长度或端部按钮改变后，绘制及调用者的拖动换算都使用这份结果。横／纵两种方向适用相同规则。主题不处理滚轮、拖动、键盘、自动重复或内容滚动，也不启动自动隐藏 timer；透明度等动画值由调用方采样后传入。

## 5. 四套风格与明暗

四套主题完整覆盖上述四类部件，差异必须同时包含颜色之外的可验证内容：

| 风格 | 首批方向 |
| --- | --- |
| shadcn | Base Nova／Neutral，14／20 文字、32 高操作表面、10 基准圆角、3 宽焦点环；完整 Light／Dark、图表与侧栏语义配色 |
| macOS | 系统风格字体与紧凑尺寸，柔和圆角、细边框、清晰焦点外框，较细且无端部按钮的滚动条 |
| Material | 较大的触达／内容内缩、强调色选择状态、较大的圆角、明显提升阴影，较大的标记与滑块最小长度 |
| Fluent | 独立的字体回退与字号组合、中等尺寸和圆角、分层表面／细边框、不同焦点和 hover／pressed 表现，支持端部按钮的滚动条几何 |

这些是自绘预设，具体数字、颜色、轮廓与状态组合在各实现库中明确声明并测试。Light／Dark 调整整组语义颜色、边框／阴影强度和可读内容，而不只替换画布背景。共享字体描述使用系统存在的 family 回退列表；测试使用已有固定字体资源，不能依赖本机某一种商业字体一定安装。

## 6. 整体切换与局部主题

`ThemeSource` 是应用持有的稳定普通 class，包含 `State<Theme>` 当前值。应用将来源通过语言 Context 绑定在真实 Application.run／runWithAsyncio 及挂载入口；控件挂载时取得来源，在每次 render 中读取当前值，使实际消费者订阅。一次来源更新合并逻辑工作和必要帧，不遍历未订阅的任意业务对象。

Component 的挂载工厂在该入口构造实例，因此可以在构造阶段保存 ThemeSource 引用。首轮 render 早于 onMount，render 所需的普通来源引用不能等到 onMount 才初始化；onMount 继续用于登记控制器、订阅和 native 资源。保存来源不等于保存当时的 Theme，每次 render 仍读取 source.current.value。

控件只保存来源和持久交互状态。Theme、已解析局部覆盖及几何经 Props／同帧 measure／paint 更新；切换风格或明暗可以按新的 metrics 重新布局，但保留组件实例、State、Binding、InputArea 和焦点。没有结构变化时不能通过更换 Element 类型或增删装饰包装实现主题切换。painter 资源替换不等于组件卸载。

局部主题首期通过明确的 ThemeSource／Theme 参数和 Props 传入，组合控件显式转交需要继承的子项。不把临时 Context 当成 View 捕获或自动挂载树继承；需要子树 Provider 时另行设计。没有环境来源且未显式提供主题的控件，使用语言现有的缺失 Context 诊断，不偷偷建立全局可变默认主题。

输入使用 Surface 已成功提交的几何。控件在 paint 放置主题给出的轮廓／区域，必要的拖动数据随该帧成功发布；失败帧不能提前替换已发布的输入几何。逻辑 enabled 和最新处理器仍在 update 生效，绘制变化不推迟禁用。完整 ScrollView 的拖动／滚动行为不属于此绘图批次。

## 7. 效果、缓存与能耗

保留 [Style 规范第 3～4 节](SCOOP-UI-STYLE-SPEC.md#3-绘制布局与输入)的完整合同：外框阴影与内容 alpha 效果不同，有序 blur／drop shadow／glow、显式 clip、扩展输出和一次 DPI；Backdrop blur 后续独立实现。主题使用同一 graphics／Style 能力，不能复制滤镜实现或要求先完成 SkSL。

ThemePainter 缓存考虑本轮主题参数、几何、效果和本帧采样值，正确替换并关闭旧资源。首批不缓存子树像素，文字／图像变化与子项动画不会因主题对象未变而失效。Painter 不建立任何任务或时钟；动画参数由控件交给 Surface Animatable，完成、暂停、取消、隐藏或卸载后恢复等待。

静止验收同时观察 Component.render、Element.update／measure／paint，以及底层帧需求和提交；不能用截图不变代替停止工作。主题切换及真实参数变化可以唤醒一次必要更新，之后停止。原生窗口关闭和异常退出沿 ComponentScope／SurfaceContent 的既有所有权路径清理所有 painter、Path 和滤镜。

## 8. 实施与验收

实施顺序为 graphics 基础滤镜及通用嵌套 PaintContext → 公共 Style／装饰 → 主题协议与几何／绘图基础 → 三套主题及离屏展示 → 基础控件消费主题 → 完整展示示例和真实 UI 组合。上述批次及原有布局、生命周期、异步进度目标均已完成本期 macOS 验收。每批格式、检查与定向测试通过后提交、推送、merge origin/main，正常缓存保留。

主题展示示例并列显示四类公共部件和已实现控件的普通、hovered、pressed、disabled、selected／mixed 和 focused 状态；支持三风格、Light／Dark 和系统跟随。公共部件区域直接调用 Canvas 绘图，真实控件区域同时验证交互和主题切换。

展示的默认布局应在约 670 逻辑单位高的内容区域内显示状态表格、真实控件和效果开关。通过调整示例的排列与留白满足这一要求，不缩放整棵 UI 来掩盖主题尺寸差异；更小窗口继续遵守容器的显式 clip，首期不因此增加 ScrollView。

所属 Cone 的 scoop.test 验证以下事实：

1. 几何无 Canvas／Application 可计算，建议／最小尺寸和内缩符合各部件合同；短轨道、最小滑块和端部按钮行程正确。
2. 真实离屏像素与同一几何一致，三套主题在尺寸、圆角、边框、阴影／焦点及明暗上有实际差异；DPI 只变换一次。
3. 控件仅使用公共主题接口，局部覆盖未指定的属性随主题变化；标记绘图可独立用于第三方内容。
4. 切换主题／效果保留实例、State、焦点和业务绑定，必要布局更新且输入几何按成功帧发布。
5. Surface 参数动画及最终停帧、缓存替换、绘图失败、卸载和原生关闭的资源释放。
6. 必要的真实 Metal／UI 组合及展示例子的交互切换。只记录实际运行的架构／后端；不新增普通 fixture 或执行全量测试。
