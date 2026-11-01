# Scoop 公共主题绘图与几何规范

日期：2026-11-02。状态：设计已确定，按 [UI 实施记录](../ui/PROGRESS.md)分批实现；本期先完成 macOS aarch64，最后统一补验 x86_64，Windows 后续。本文扩展 [公共样式规范](SCOOP-UI-STYLE-SPEC.md)，保留其内容效果、资源、稳定身份与停帧合同。

## 1. 本轮调整与分层

主题以公共绘图部件及其几何为核心。`scoop.ui.style` 定义 Theme／ThemePainter 协议、语义颜色／字体／尺寸／动画参数、类型化视觉状态、尺寸输入、几何结果，以及通用 Style／装饰适配。`scoop.ui.component` 负责持久状态、布局组合、输入与业务，不定义公共主题合同，也不依赖某个主题实现。

本轮明确替换此前三项设计：

1. 不再把公共主题及默认颜色／尺寸都放在 ComponentDefaults 或控件专用样式中；它们由 style 的公共 Theme 提供。
2. Button／Checkbox 不再直接用自己的一套背景、圆角、焦点和标记绘图。它们消费主题的公开几何与绘图；ButtonStyle／CheckboxStyle 只表达明确的局部覆盖和控件特有输入。
3. 控件不在构造时永久保存当时的 Theme 或解析后的外观。保存稳定 ThemeSource，在 render 中读取其当前 Theme，并把本轮主题及视觉输入通过 Props 交给持久 Element。

三组实现为普通独立 Cone：`scoop.ui.style.macos`、`scoop.ui.style.material`、`scoop.ui.style.fluent`。它们依赖 style 和实际使用的 graphics／graphics.text／surface，不依赖 component；style 不反向引用这三库。应用选择并依赖所需主题，主题展示示例显式依赖全部三库。`scoop.ui` 仍不依赖 style 或 component。

实现库可复用公共的几何、StylePainter 和普通绘图辅助代码。共同算法按实际三组调用复用，差异由各自的主题参数或绘图实现表达；控件中不得按 macOS／Material／Fluent 分支。自绘主题不替换 window 的原生装饰配置，也不声称与操作系统某个版本逐像素一致。

## 2. 主题对象、绘图对象与普通数据

Theme 是不可变的主题描述／协议实现，提供语义参数和各部件的 metrics／geometry 方法；不拥有需要应用逐次关闭的 Path／滤镜，不读取业务 State。ThemeSource 和 UI 消费适配与这些纯几何／绘图入口分开。

几何方法只接收有限的逻辑 bounds、尺寸选项和类型化视觉数据，不需要 Canvas，不访问 Application、InputArea、挂载节点或 Context。每类部件返回自己的结果类型，不用名称字典或通用无类型字段。绘制严格消费该结果中的尺寸、轮廓和区域，不能重新估算另一份布局。

原生缓存由 `ThemePainter : Closeable` 持有，通过 `Theme.createPainter()` 创建；其 drawSurface／drawCheckbox／drawRadio／drawScrollbar 只接收普通 Canvas、对应几何和视觉数据。这样离屏程序可以长期复用同一 painter，自绘 Element 则通过 ComponentScope 登记拥有可替换 painter 的 `ThemePainterCache`。cache 在主题实际变化时先构建新 painter，再替换并关闭旧值；创建失败保留旧值用于清理或下一次绘制。应用不为每个部件／每帧使用 use。

ThemePainter 和缓存限创建线程使用，close 幂等。绘图同步借用 Canvas，退出后不能保存该画布；正常及异常路径恢复嵌套状态。Theme／几何只是普通描述，不替代实际资源所有者。普通离屏用法为先查询 geometry，再把同一 geometry 交给对应 painter 的 draw 方法；不要求运行 UI。

## 3. 通用参数与覆盖

Theme 提供以下公共语义参数，使没有标准绘图部件的第三方控件也能选用相同外观：

- 颜色：画布、表面、提升表面、正文、次要文字、禁用文字、分隔／边框、强调、强调上的内容、焦点与阴影。
- 字体：正文、辅助文字和操作标签，直接使用 graphics.text 的 TextStyle；局部字体／字号／颜色覆盖复用 SpanStyle，段落设置继续使用 ParagraphStyle。
- 尺寸：基本间距、各部件建议／最小尺寸、内容内缩、边框宽度、圆角、焦点外扩和 Scrollbar 的实际度量。
- 效果：表面外框阴影和必要的 Style 内容效果参数。
- 动画：进入／离开状态和选择变化的 Surface AnimationSpec。主题只提供参数，绘图方法只消费本帧已采样的数值。

`ThemeMode.Light` 与 `ThemeMode.Dark` 只表达明暗；风格由所选实现独立决定。三套主题同时提供两种模式，不把 Dark 当成第四套风格，也不因切换模式改变组件类型。需要跟随系统的应用在现有 Window ThemeChanged 通知中更新 ThemeSource，主题库不轮询系统、不自行建立窗口监听或时钟。

Style 是完整的已确定描述；新增 `StyleOverride` 表达局部覆盖。未指定属性继续使用本轮主题，背景／边框等可空属性须区分“继承”和“显式清除”，采用类型化 `StyleValue<T>` 的 Inherit／Set，而不是用一个 None 混淆两者。数组效果显式覆盖时整体替换，顺序不变；需要组合时由调用者明确给出顺序。

控件局部样式和主题默认值在本轮几何计算前解析。圆角、边框内缩、padding 或建议尺寸的覆盖必须进入同一份最终几何；不能只改变画出的尺寸，却让布局或拖动继续用主题默认值。纯阴影／glow／blur 仍不改变布局尺寸、基线或命中区域。

## 4. 公共绘图部件

各部件的 InteractionVisual 使用明确的 enabled／hovered／pressed／focused 数据，部件另带自身选择或滚动输入。它们都是值描述，不是响应式 State；不存在组件回调、输入注册、任务或业务模型引用。

### 4.1 外框／表面

SurfaceMetrics 给出建议／最小尺寸、内容 padding、边框内缩及焦点外扩。语义用途区分普通表面、强调操作面和提升表面，不引用 Button 或 Frame 类型。SurfaceGeometry 至少返回外边界、圆角外轮廓、边框内区域、内容区域和可选焦点轮廓；轮廓为 Rect／圆角等普通描述，原生 Path 留在 painter。

Surface 的绘制支持 Background／Foreground／All 阶段。Background 绘制外框阴影和背景；Foreground 绘制边框及焦点外框；All 依次完成两者，供独立展示使用。控件在两阶段间绘制内容，完整子树的 Style 效果仍由通用装饰适配负责。这样背景、内容、边框的顺序与既有 Style 合同一致，不要求主题方法理解挂载树。

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

## 5. 三套风格与明暗

三套主题完整覆盖上述四类部件，差异必须同时包含颜色之外的可验证内容：

| 风格 | 首批方向 |
| --- | --- |
| macOS | 系统风格字体与紧凑尺寸，柔和圆角、细边框、清晰焦点外框，较细且无端部按钮的滚动条 |
| Material | 较大的触达／内容内缩、强调色选择状态、较大的圆角、明显提升阴影，较大的标记与滑块最小长度 |
| Fluent | 独立的字体回退与字号组合、中等尺寸和圆角、分层表面／细边框、不同焦点和 hover／pressed 表现，支持端部按钮的滚动条几何 |

这些是自绘预设，具体数字、颜色、轮廓与状态组合在各实现库中明确声明并测试。Light／Dark 调整整组语义颜色、边框／阴影强度和可读内容，而不只替换画布背景。共享字体描述使用系统存在的 family 回退列表；测试使用已有固定字体资源，不能依赖本机某一种商业字体一定安装。

## 6. 整体切换与局部主题

`ThemeSource` 是应用持有的稳定普通 class，包含 `State<Theme>` 当前值。应用将来源通过语言 Context 绑定在真实 Application.run／runWithAsyncio 及挂载入口；控件挂载时取得来源，在每次 render 中读取当前值，使实际消费者订阅。一次来源更新合并逻辑工作和必要帧，不遍历未订阅的任意业务对象。

控件只保存来源和持久交互状态。Theme、已解析局部覆盖及几何经 Props／同帧 measure／paint 更新；切换风格或明暗可以按新的 metrics 重新布局，但保留组件实例、State、Binding、InputArea 和焦点。没有结构变化时不能通过更换 Element 类型或增删装饰包装实现主题切换。painter 资源替换不等于组件卸载。

局部主题首期通过明确的 ThemeSource／Theme 参数和 Props 传入，组合控件显式转交需要继承的子项。不把临时 Context 当成 View 捕获或自动挂载树继承；需要子树 Provider 时另行设计。没有环境来源且未显式提供主题的控件，使用语言现有的缺失 Context 诊断，不偷偷建立全局可变默认主题。

输入使用 Surface 已成功提交的几何。控件在 paint 放置主题给出的轮廓／区域，必要的拖动数据随该帧成功发布；失败帧不能提前替换已发布的输入几何。逻辑 enabled 和最新处理器仍在 update 生效，绘制变化不推迟禁用。完整 ScrollView 的拖动／滚动行为不属于此绘图批次。

## 7. 效果、缓存与能耗

保留 [Style 规范第 3～4 节](SCOOP-UI-STYLE-SPEC.md#3-绘制布局与输入)的完整合同：外框阴影与内容 alpha 效果不同，有序 blur／drop shadow／glow、显式 clip、扩展输出和一次 DPI；Backdrop blur 后续独立实现。主题使用同一 graphics／Style 能力，不能复制滤镜实现或要求先完成 SkSL。

ThemePainter 缓存考虑本轮主题参数、几何、效果和本帧采样值，正确替换并关闭旧资源。首批不缓存子树像素，文字／图像变化与子项动画不会因主题对象未变而失效。Painter 不建立任何任务或时钟；动画参数由控件交给 Surface Animatable，完成、暂停、取消、隐藏或卸载后恢复等待。

静止验收同时观察 Component.render、Element.update／measure／paint，以及底层帧需求和提交；不能用截图不变代替停止工作。主题切换及真实参数变化可以唤醒一次必要更新，之后停止。原生窗口关闭和异常退出沿 ComponentScope／SurfaceContent 的既有所有权路径清理所有 painter、Path 和滤镜。

## 8. 实施与验收

顺序为：已完成 graphics 基础滤镜及通用嵌套 PaintContext → 公共 Style／装饰 → 主题协议与几何／绘图基础 → 三套主题及离屏展示 → 基础控件消费主题 → 完整展示示例和真实 UI 组合。原有布局、生命周期、异步进度目标继续推进。每批格式、检查与定向测试通过后提交、推送、merge origin/main，正常缓存保留。

主题展示示例并列显示四类公共部件和已实现控件的普通、hovered、pressed、disabled、selected／mixed 和 focused 状态；支持三风格、Light／Dark 和系统跟随。公共部件区域直接调用 Canvas 绘图，真实控件区域同时验证交互和主题切换。

所属 Cone 的 scoop.test 验证以下事实：

1. 几何无 Canvas／Application 可计算，建议／最小尺寸和内缩符合各部件合同；短轨道、最小滑块和端部按钮行程正确。
2. 真实离屏像素与同一几何一致，三套主题在尺寸、圆角、边框、阴影／焦点及明暗上有实际差异；DPI 只变换一次。
3. 控件仅使用公共主题接口，局部覆盖未指定的属性随主题变化；标记绘图可独立用于第三方内容。
4. 切换主题／效果保留实例、State、焦点和业务绑定，必要布局更新且输入几何按成功帧发布。
5. Surface 参数动画及最终停帧、缓存替换、绘图失败、卸载和原生关闭的资源释放。
6. 必要的真实 Metal／UI 组合及展示例子的交互切换。只记录实际运行的架构／后端；不新增普通 fixture 或执行全量测试。
