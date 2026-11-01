# Scoop 公共 UI 样式规范

日期：2026-11-02。状态：graphics 滤镜／图层、通用嵌套 PaintContext 和公共 Style 已在 macOS aarch64 完成；主题绘图按更新后的设计继续实施。分批计划见 [UI 设计](../ui/DESIGN.md)与[实施记录](../ui/PROGRESS.md)。本期先完成 macOS aarch64，最后统一补验 x86_64，Windows 后续。

## 1. 分层与接入

`scoop.ui.style` 是独立普通 Cone，依赖实际使用的 ui、graphics、surface 及 core／std／platform。它定义[公共主题绘图与几何协议](SCOOP-UI-THEME-SPEC.md)、视觉状态、语义颜色／字体／尺寸／动画参数，同时提供通用 Style、外框装饰和内容效果的 UI 应用方式。graphics 提供[真正绘图、滤镜与图层](SCOOP-GRAPHICS-EFFECTS-SPEC.md)；ui 只提供挂载、State／Props、布局、输入和作用域，不依赖或识别 style／component。控件库负责交互和 ButtonStyle／CheckboxStyle 的明确局部覆盖，向主题提交类型化视觉状态并消费同一几何与绘图。主题合同不在 component 中定义。

文字继续使用 `scoop.graphics.text` 的 TextStyle／SpanStyle、ParagraphStyle；Theme 的公共 typography 也直接使用这些类型，Style 不定义另一套字体、段落或文本继承模型。第三方完全自绘组件和基础控件使用同一公开适配，没有专用 View 分支、控件节点或效果注册入口。

`styled(content, style = Style(), key = ViewKey.Position)` 始终建立相同具体类型的普通组件／元素及单个内容子项，包括空 Style。更新只替换 Props；在相同 key 和类型下，从无效果到有阴影、阴影移除、参数或效果顺序改变，都保留内容的实例和 State。调用者改变 content 自身的 key／类型时继续执行正常重挂载规则。

第三方 Element 可在 onMount 中 `scope.resource { StylePainter() }`，在 paint 中调用公共 `StylePainter.paint(context, style, content)`；也可提供显式局部 bounds。回调接收隔离后的 PaintContext，可绘制自己的内容并调用 paintChildren。StylePainter 同时提供 Canvas／Rect 入口，供真实离屏组合使用。隔离复用通用 PaintContext.withState／Canvas.withState，不访问 UI 内部节点。回调借用不能逃出作用域。

## 2. 描述与参数

Style 是普通不可变、可复用的完整描述 class，经 Props 传递，不拥有需要调用方 close 的 native 对象。主题的局部输入使用 StyleOverride，只替换明确指定的属性；StyleValue<T>.Inherit／Set 区分继承和显式清除。覆盖在最终几何计算前解析，具体协议见主题规范第 3 节。

| 输入 | 合同 |
| --- | --- |
| background | 可选 graphics Paint 填充，复用颜色／渐变描述；其 PaintingStyle 必须为 Fill |
| border | 可选 Border(width, color)，沿外框向内占用像素；不增加布局尺寸 |
| cornerRadius | 有限非负逻辑单位，应用于背景、边框、外框阴影和显式内容 clip |
| boxShadows | 有顺序的 `Array<BoxShadow>`，每项含 offset、sigmaX／sigmaY、color |
| contentEffects | 有顺序的 `Array<ContentEffect>`，首批为 Blur、DropShadow、Glow |
| clipContent | 默认 false；开启后把最终内容效果输出裁到外框轮廓，同时限制该作用域内的子输入几何 |

sigmaX／sigmaY 是 Gaussian 标准差，offset、边框宽度、圆角和 bounds 都是局部逻辑单位。有限性、非负尺寸和颜色沿 graphics 合同检查；不把半径冒充 sigma，不预乘 DPI。零 sigma 合法。Style 没有隐式 padding，布局留白由控件／容器的显式布局输入决定。

BoxShadow 沿填满的圆角外框 alpha 生成，与 background 是否存在、是否透明以及子内容形状无关。DropShadow 沿真实子树 alpha 生成，带原内容；不是外框的另一种写法。两者都不提供首批未实现的 spread／inset 语义。

Glow(sigmaX, sigmaY, color) 等价于零 offset 的带源 DropShadow：对源 alpha 模糊后用 color 着色，再将原内容以 SrcOver 合成到光晕上。color.alpha 决定光晕不透明度，没有另一个不明单位的 strength，也不默认采用加色或 HDR。文字直接以实际 glyph 绘图结果为源，无需转 Path；透明图片、行内对象和子组件采用相同通路。

## 3. 绘制、布局与输入

同一装饰节点依次执行：

1. 按数组顺序绘制各个外框阴影，后项覆盖前项。
2. 绘制圆角背景填充。
3. 绘制一次完整内容子树，按 contentEffects 的顺序处理；clipContent 为 true 时将最终输出裁到圆角外框。
4. 绘制向内的边框，保持边缘清晰。

本节点的内容效果只处理第 3 步，不处理自己的背景、外框阴影和边框；嵌套子节点的装饰属于该子树实际内容，因此会被处理。若要对完整装饰卡片做 blur，用外层 styled 包住它。内容回调发生一次，不为每种效果重画子树或重复放置 InputArea。

`[Blur(a), DropShadow(b)]` 表示先模糊内容，再根据模糊结果投影；`[DropShadow(b), Blur(a)]` 则将原内容与阴影一起模糊，结果不同。图层以透明黑开始，普通组合使用 SrcOver。空效果列表直接绘制内容，不分配效果图层。

clipContent 是滤镜输出 clip；Skia 可以读取输出所需的外围样本。若希望先裁内容、再让光晕向外扩展，可将带 clipContent 的内层 styled 作为外层 glow 的内容。该节点外框阴影不受自身 clipContent 截断，但仍受祖先 clip／目标范围限制。整个装饰也要裁剪时，由更外层的显式 clip 负责。

styled 把同样的约束传给唯一子项，原点放置，并原样返回其 LayoutMetrics（尺寸及两条可选基线）。效果像素不会扩大测量、基线、子项位置或点击范围；styled 不额外创建 InputArea。显式 clip 使用实际 Canvas 的同一 Path 几何，最终命中仍由 Surface 成功提交的 placement 决定。

图层不使用布局矩形作隐式 bounds／crop，也不把阴影向外扩展当成布局 padding。背景矩形大小为零时不绘制该外框装饰，子内容仍遵循其正常绘制合同。有限目标／祖先 clip 可以裁掉效果，调用方需通过布局明确留出可见空间。

## 4. 资源、更新与动画

StylePainter 是实际 Element 拥有的 Closeable 缓存，通过 ComponentScope 登记一次。它保留当前参数对应的滤镜和必要轮廓，参数未变复用；新结果完整构造后替换，及时关闭旧对象。失败关闭本次临时结果，旧缓存保持可清理。卸载、原生窗口关闭和异常退出沿正常 scope 路径释放，不把历代滤镜逐帧追加进 scope。

StylePainter 的一个 paint 回调结束前，不能重入同一 painter 或关闭它；这些调用报告状态错误。另一个 painter 可在内容回调内正常嵌套，Canvas／PaintContext 隔离各自借用，回调抛错后恢复原状态。

首批不缓存子树图像或显示列表。每个真实帧重画当帧内容，filter／轮廓缓存不代替内容更新，因此文字、透明图像替换、子项 State 和动画不会因为 Style 引用不变而滞后。未来若有实际需求增加像素缓存，失效必须包含内容、尺寸、DPI、效果参数与子项动画，不能只比较 Style。

静态样式没有时钟、timer、轮询或 UI 更新任务。资源存在本身不请求帧。参数动画由所属 Element 的 Surface Animatable 驱动，在 update／输入中设置真实变化的目标，在 paint 读取已采样值交给 StylePainter；不在 paint 重启动画，不要求每帧写 State 或重建父组件。普通 styled 的外部 Style 变化通过 Props 进入。

动画完成、暂停、取消、隐藏及卸载遵守既有 Surface 停帧合同。稳定区间同时停止逻辑更新、测量／绘制、平台帧需求和 GPU 提交；只检查画面相同不足以验收。替换滤镜不会引入额外驱动器。

## 5. 主题实现与验证

三套可切换的自绘主题由独立 scoop.ui.style.macos／material／fluent 实现，同一套公共协议覆盖表面、Checkbox 标记、Radio 标记和 Scrollbar。geometry 方法不依赖 Canvas；ThemePainter 的绘图只需要 Canvas、同一几何和普通视觉数据，资源由 scope 登记的可替换缓存拥有。完整主题来源、覆盖、几何和展示验收见[主题规范](SCOOP-UI-THEME-SPEC.md)，这些能力纳入本期。

### 5.1 效果验证与后续

普通行为放在本 Cone 的 scoop.test，使用真实 RasterSurface 及必要的 Metal／UI 组合：

- 对照圆角外框阴影与稀疏内容 alpha 阴影；文字、透明图像和带子装饰的 subtree glow／blur。
- 有序组合的不同像素、边框与背景顺序、源范围外的像素、显式 clip、零参数、透明颜色及一次 DPI。
- 无效果／有效果双向切换保留组件实例、State、尺寸、基线和输入区域；子项动画或内容变化即时反映。
- Surface 参数动画的真实帧与最终值，完成／暂停／取消／隐藏／卸载后逻辑及底层帧需求停止。
- 缓存复用、替换、构造／绘制异常、卸载和原生窗口关闭，确认资源及时释放。

Backdrop blur／毛玻璃是采样背后画面的独立能力，本批只记录为后续增强。普通内容 blur 不取得背景。SkSL 接入继续由 graphics 的[后续设计](../graphics/sksl/DESIGN.md)负责，将来可复用已建立的图层与适配，而不是重新引入效果节点或动画时钟。
