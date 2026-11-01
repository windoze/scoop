# UI 实施记录

日期：2026-11-02。实施依据为 [UI 设计](DESIGN.md)、[框架规范](../specs/SCOOP-UI-SPEC.md)、[文本排版规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)、[图像滤镜规范](../specs/SCOOP-GRAPHICS-EFFECTS-SPEC.md)、[公共样式规范](../specs/SCOOP-UI-STYLE-SPEC.md)、[主题绘图规范](../specs/SCOOP-UI-THEME-SPEC.md)及[基础控件规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)。本期先在 macOS aarch64 完成全部实施批次，再统一补验 x86_64，Windows 后续接入；图文绕排、GPU／3D／媒体互操作和 SkSL 仍属于设计中列出的后续增强。

## 1. 批次状态

| 批次 | 状态 | 交付范围 |
| --- | --- | --- |
| 0. 已确认设计与平台范围 | 已记录 | 自绘分层、State／Context／Binding、资源所有权、静止停帧、macOS 首期与后续增强 |
| 1. 文本公共能力 | aarch64 已完成 | 独立 graphics.text、Canvas 借用、StyledText／SpanStyle、行内 metrics／placement、位置与替代文字 |
| 2. 独立自绘闭环 | aarch64 已完成 | SurfaceContent 所有权、Component／Element／State／Props／Binding、挂载、布局、输入、UiHost 与独立自绘示例 |
| 3a. 公共文本适配 | aarch64 已完成 | ParagraphHost／InlineView、固定字体共享测试数据与独立自绘组合 |
| 3b. graphics 基础效果 | aarch64 已完成 | ImageFilter、Gaussian blur、alpha 阴影、图层／混合与圆角 Path |
| 3c. 公共样式 | aarch64 已完成 | Style／StyleOverride、稳定 styled、StylePainter、装饰／有序效果、真实 Metal／UI 停帧与异常清理 |
| 3d. 公共主题绘图 | aarch64 已完成 | Theme／ThemePainter、表面／Checkbox／Radio／Scrollbar 的几何与绘图，三风格及 Light／Dark |
| 3e. 简单控件 | aarch64 已完成 | Label／Button／Checkbox 消费公共主题，状态／绑定、局部覆盖、实例／焦点保留和真实停帧 |
| 4. 布局、组合与主题展示 | aarch64 已完成 | Row／Column／Stack／Frame、keyed 子项、基线与资源组合，公共部件和真实控件的完整主题展示 |
| 5. 宿主与生命周期组合 | 待实施 | 空闲停帧、动画收尾、隐藏更新、asyncio／HTTP 进度、取消和多窗口 |

已有 scoop.surface 的窗口、动画和 InputArea 实现作为基础，独立 UI 框架、公共文本适配、graphics 效果、公共样式／主题及基础控件／容器已交付；宿主和异步进度组合继续按批次推进。下文保存实际验证范围，不把设计示例和文档检查当成已交付功能。

## 2. 验证与提交方式

普通 API、资源和控件行为使用所属 Cone 的 scoop.test；真实 GPU／窗口组合使用现有 macOS 后端。默认 release，定向验证改变的能力和必要消费者，不执行全量测试。遇到编译器、runtime 或其他系统库缺口时先修订其规范并修复该层，再补相应定向回归，不在 UI 中实现规避分支。

每批先完成 aarch64 验证并提交，不把 x86_64 补验作为继续实现下一批的前置。已提前准备的部分 x86_64 资源和缓存保留，尚未进行本轮运行验收；本期 aarch64 功能完成后再更新资源并统一补验。

每项完整变更通过格式化、检查和定向测试后提交并推送，再 fetch／merge origin/main；合并改变相关代码时补充必要验证后继续。每轮清理不再使用的临时日志、探测文件和构建输出，仍需复用的正常构建缓存及本期目标资源继续保留。记录只保存验证范围、结果和实际限制。

2026-11-02 已同步 UI、控件、graphics／Surface、SkSL 的分层、顺序和当前状态：基础效果进入本期，SkSL／Backdrop blur 仍属后续。先补下层滤镜与通用绘制作用域，再交付公共样式及控件消费。其中 graphics 基础滤镜和公共 style 已完成，见第 3.8、3.10 节；主题及控件消费继续实施。

2026-11-02 后续主题调整：公共默认外观、部件绘图协议及几何移至 style，取代 ComponentDefaults 固定视觉值和控件自绘标记方案；ThemeSource 的 State／Context 支持整体切换，StyleOverride／SpanStyle 保留未指定属性的主题继承。新增三风格、明暗和公共部件展示，完整 Radio 分组／ScrollView 不成为绘图批次前置。既有滤镜、内容身份、资源与停帧要求保留。

## 3. 已完成变更

### 3.1 独立文本库与画布借用

字体、TextStyle／TextRun、Paragraph／TextLayout 及其 native 实现、固定测试字体迁入 scoop.graphics.text。该 Cone 自己释放字体与段落资源，Canvas.drawText 通过 graphics 的公开 Unsafe `Canvas.withSkiaCanvas` 同步绘制；正常返回和异常退出都恢复原生坐标、clip 和保存层级。文本库不包含 graphics 的私有 native 头文件，graphics 不反向引用文本类型。FontCollection 新增只在成功登记后递增的 revision。

Surface 主体仍只接收 Rect caret；使用文本的示例和焦点测试已改为直接依赖 graphics.text。已有 DirectWrite 工厂随文件归属一起移动，没有新增 Windows 行为或进行 Windows 验收。后续样式描述的交付见第 3.2 节。

本轮在 macOS aarch64 使用当前源码的工具链及显式准备的开发 sysroot，先完成 Scoop／C++ 格式检查、C++ 编译告警检查和两库测试源码的类型检查，再运行以下 release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.graphics | 13／13 | 原绘图／Path／Metal，加原生借用、异常恢复、过期与异线程拒绝 |
| scoop.graphics.text | 10／10 | 固定字体、CJK／bidi／emoji、独立布局、字体 revision、资源关闭与真实 Metal 文本 |
| scoop.surface 的 InputFocusTest | 5／5 | 文本 caret 与区域变换、IME／焦点及关闭 |
| Surface basics 示例 | 4／4 | 文本编辑、布局缓存和拖动收尾 |

画布恢复测试初次对 SkPath 的恰好边界点作了半开矩形的假设；改为检查明确位于 clip 外的点后，几何和实际像素断言均通过。本轮未修改编译器、core／std／test，未新增 fixture 或运行全量测试。x86_64 留待本期功能完成后补验。

### 3.2 内容描述与样式继承

StyledText／TextSpan.text／TextSpan.span 与 SpanStyle 已实现。嵌套片段在内容创建时展开并复用，未指定属性继承父样式，数组属性整体替换；lineHeight 区分继承、显式设置和清除。Paragraph 以基础 TextStyle 初始化默认属性，使空内容仍有正确的行高和起始 caret。公开 TextRun 输入已移除，Paragraph／TextLayout 保存同一不可变内容快照，String 便利构造继续可用。

toPlainText 支持完整导出及以 UTF-8 字节表示的标量边界范围；行内对象与替代文字的扩展见第 3.3 节。片段不拥有 native 资源，调用方无需逐片段 use。

macOS aarch64 的 Scoop 格式／类型检查和 C++ 格式／告警检查通过，release scoop.test 结果如下：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.graphics.text | 17／17 | 原文本／Metal，加嵌套字号与颜色、family 替换、行高清除、kerning／可变字体清除、空片段与 ligature、UTF-8 范围 |
| Surface caret 接入 | 1／1 | 排版 caret 的区域几何与所有权 |
| basics 的 DemoEditorTest | 2／2 | grapheme 编辑、组合输入与排版复用 |

本轮未修改编译器、core／std／test，未使用 fixture 或运行全量测试；x86_64 统一补验仍在全部 aarch64 功能之后。

### 3.3 行内对象与共同基线

TextSpan.inlineObject 以 InlineKey 标识每次出现，StyledText 保存规范化 U+FFFC 的 UTF-8 范围与替代文字，普通文字中的 U+FFFC 不成为对象。每次 layout 接收完整 metrics，按 key 校验并排序；对象的尺寸、六种对齐及两类基线参与真实 SkParagraph 排版。TextLayout 为每个对象保留 placement，可见项包含矩形、行号与方向，被截断项为 None，零尺寸项仍可见。

对象前后保留原子定位边界，hitTest 根据方向和位置选择两端，caret 与 selection 使用同一布局的实际对象盒子。toPlainText 按原 UTF-8 范围导出替代文字；更改尺寸生成独立布局，关闭字体来源或 Paragraph 不使旧结果失效。绘图调用方按位置绘制 Image 或自绘盒子，文本层不接管其资源。

运行验证发现 SkParagraph 的 baseline getter 与实际绘制坐标有舍入差异，ideographic placeholder 还会受前一对象扩大的行度量影响。已先修订文本合同，在 Skia 增加公共基线与共同文字参照的补丁，并让正常资源准备脚本应用。重新生成 aarch64 Skia 后，基线和多对象高度组合通过；bidi 用例同时修正了对 RTL／LTR 交界中性对象方向的错误预期。

Scoop／C++ 格式、类型和 native 告警检查，以及准备脚本的 Ruff 格式／lint 通过。aarch64 release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.graphics.text | 27／27 | 原文字／Metal，加对象 metadata、metrics 错误、图片绘制、两类基线、混合 bidi、maxLines／ellipsis、零尺寸与超宽、组合字符两侧定位、快照独立性 |
| Surface 的 InputFocusTest | 5／5 | caret 接入、区域变换、IME／焦点及关闭 |
| Surface basics 示例 | 4／4 | 编辑、布局缓存和拖动收尾 |

本轮未修改编译器、core／std／test，未使用 fixture 或运行全量测试。公共文本层已完成，下一步接入 SurfaceContent 所有权和独立自绘 UI 框架。x86_64 资源更新与补验仍在全部 aarch64 功能之后。

### 3.4 Surface 内容根的所有权

SurfaceHost.attach 现在可接收 SurfaceContent，在 owner 线程检查后接管内容，附着失败也关闭内容并回滚 renderer／lease。绘制、输入或宿主事件处理器内关闭先停止新工作，完整回调和 Canvas 借用退出后再关闭内容及底层资源。失败内容清理不阻止其他窗口收尾，多个错误通过 ResourceUseException 保留。现有 basics 示例的 DemoPanel 直接实现该接口，拥有字体、路径和文本缓存，最外层 host.use 覆盖窗口内容。

原生窗口回归发现 Metal 在 resize 已发生但 GeometryChanged 尚未交付时仍接受旧帧，已在 Surface 规范明确提交边界，并在 Metal renderer 提交前检查实际 backing 尺寸和 scale。没有在 UI 中补偿尺寸或递归派发窗口事件。

所有权测试按真实 Destroyed 事件推进，避免依赖主循环在无工作时额外产生 AboutToWait；asyncio 关闭检查允许 Surface 已在较早事件中退休。实现未为测试增加轮询、空任务或多余唤醒。

Scoop 格式／类型检查、Objective-C 格式及严格告警编译通过，aarch64 release scoop.test 结果如下：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| Surface content Cone | 1／1 | 所有权转移、错误线程、失败附着、回调与原生关闭、重挂载、隐藏根及退出 |
| Surface errors Cone | 1／1 | paint 和多个 close 错误合并、剩余内容与 lease 完整清理 |
| SurfaceHostTest | 1／1 | 按需帧、多窗口、原生 resize 中止、输入宿主恢复和重新附着 |
| Surface asyncio Cone | 1／1 | 原生 modal／menu 循环中的帧与 I/O，取消与关闭边界 |
| Surface native input Cone | 1／1 | 真实输入 view、区域与 IME 切换、回调内关闭 |
| basics 示例 | 4／4 | 现有文本编辑与拖动收尾，新内容根通过类型检查 |

UI 框架和基础控件尚未交付。本轮未修改编译器、core／std／test，未使用 fixture 或运行全量测试；x86_64 仍在全部 aarch64 功能完成后补验。

### 3.5 自绘所需的嵌套画布作用域

Canvas.withState 已提供独立有效期的嵌套借用，继承现有坐标及 clip，不重复 DPI；子作用域期间拒绝父画布操作，正常及异常返回恢复全部子级保存状态，子画布随即失效。FrameContext.withState 把同一窗口、帧时间和输入收集器传入子作用域，使区域 placement 使用与子组件绘制相同的几何。未增加另一份 UI 变换栈或暴露 graphics 内部构造器。

Scoop 格式／类型检查、C++ 格式与严格告警编译通过；重新发布 aarch64 graphics 及必要消费者后，release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.graphics | 16／16 | 嵌套绘制与几何、异常下未配对 saveLayer、子借用失效、线程限制、原有 native 借用及真实 Metal 的单次 DPI |
| Surface native input Cone | 1／1 | 子 FrameContext 的区域命中、局部坐标、IME caret、同帧时间与关闭 |
| graphics.text 的 MetalTextTest | 1／1 | 独立文本库继续通过画布原生借用绘制到 Metal |

本轮无编译器、core／std／test 改动，无 fixture 或全量测试；UI 框架继续实施，x86_64 统一留待最后。

### 3.6 独立自绘 UI 框架

新增普通 scoop.ui Cone，提供持久 Component／Element、State、Props／Binding、View 和延迟工厂、ComponentScope、约束与 LayoutMetrics、MeasureContext／PaintContext、UiHost／UiWindow。State 依赖只在 render 中收集；同一轮祖先与后代失效合并，条件读取或 Binding 换绑时移除旧订阅。key 与完整组件／Props 类型决定复用，普通 class 状态无需复制成 struct。

逻辑更新通过现有 ApplicationProxy 按需投递，不等待可见帧。隐藏窗口仍更新和卸载，SurfaceContent 拥有 UI 根；完整回调结束后才释放资源。绘制复用 FrameContext.withState，变换、clip 与区域放置使用同一几何；子项测量一次、可不放置，未放置项仍保留状态。动画直接使用 Surface 时间轴，不重跑未失效的 render。

Scoop 格式检查和生产／测试类型检查通过；当前 Scoop CLI 无独立 lint 子命令，本轮没有需要 Rust／C++ lint 的变更。aarch64 release scoop.test 结果如下：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| UiValueTest | 4／4 | 自有／共享委托、Binding、class 值、异线程读写拒绝、约束 |
| UiCompositionTest | 1／1 | 隐藏双窗口、Context 挂载、自有／共享 State、条件退订、更新合并、Props／Binding 换绑、key 重排与重新挂载、子先父资源关闭 |
| UI drawing Cone | 1／1 | 公开自绘接口、Rect／Path 命中、clip／坐标隔离、绘制顺序、基线、Surface 动画与停帧、可选放置、焦点退出、逻辑回调内关闭 |
| UI errors Cone | 1／1 | 12 种构造／render／挂载／update／测量／绘制及协议失败，多根清理、资源关闭一次、错误合并、原生 view 恢复 |

[自绘 Counter](../../sysroot/src/scoop.ui/examples/custom/README.md) 已通过生产类型检查和 aarch64 release 构建，只依赖 UI 及下层公开接口，实现成员 State、Path、输入和 spring 宽度动画。真实窗口交互由 drawing Cone 验证，未把示例构建计为额外运行测试。

本轮未修改编译器、core／std／test，未使用 fixture 或运行全量测试。多种容器、公共文本和 asyncio／HTTP 的组合仍在后续批次验收；x86_64 在 aarch64 全部功能完成后补验。

### 3.7 公共段落与行内子项适配

scoop.ui.text 已提供 InlineView、InlineViewAlignment、inlineChildren 和 ParagraphHost。行内出现以 InlineKey 创建普通 keyed 包装组件，内层 View 的 key 保留自身作用；ParagraphHost 只通过公开文本／UI 接口测量和放置，不增加 View 分支。自然尺寸、固定尺寸、子项基线或显式对齐均进入同一 TextLayout。

ParagraphHost 拥有可替换的 Paragraph／TextLayout，借用字体来源；内容身份、字体身份／revision、段落设置、宽度和实际 metrics 决定复用。成功取得新布局后关闭旧值，构造或排版失败保留原缓存。公开 layout 借用允许第三方控件复用文本查询；宿主关闭后不可继续访问。

Scoop 格式与生产／测试类型检查通过，aarch64 release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| UiTextCacheTest | 4／4 | 绑定校验及延迟构造、布局复用与尺寸变更、失败替换、字体 revision、借用关闭与 owner 线程 |
| UI text Cone | 1／1 | 公开适配、多字号文字、图片／交互子项、基线与固定尺寸、动画重排、换色复用、截断失焦与恢复状态、RTL／key 重排、内层 key 更换、资源所有权 |

固定字体由原数据目录新增的普通 text-fonts 测试 Cone 提供，避免复制字体或跨 Cone includeBytes。生产 UI 已重新构建到 aarch64 sysroot；未修改编译器、core／std／test，未使用 fixture 或运行全量测试。下一步接入独立基础控件库，x86_64 仍统一留待最后。

### 3.8 公共图像滤镜与图层

新增 ImageFilter.blur／dropShadow／dropShadowOnly 与有序 then 组合，Canvas.saveLayer 接入滤镜、整体透明度和 SrcOver／Src／DstIn／Plus 混合；PathBuilder.addRoundRect 提供同一圆角轮廓。实现直接使用固定 Skia 的 SkImageFilters，不引入 SkSL、图层布局框 crop、CPU 读回或新的时钟。组合及活动图层独立保留输入 native 引用，包装关闭不破坏尚未完成的绘制。

Scoop 格式／含测试的类型检查和 C++ 格式／严格告警检查通过。aarch64 release 的 scoop.graphics 共 24／24 通过，新增 7 项滤镜／几何行为和 1 项真实 Metal 组合：透明图片孔洞与半透明 alpha、只投影／带源、模糊外扩与零 sigma、有序效果差异、源 clip 与输出 clip、组混合、一次 DPI、输入关闭／错误线程与异常图层恢复。既有画布借用、形状、图像和 Metal 测试同时通过。

本轮没有编译器、core／std／test 改动，没有 fixture 或全量测试；公共 style 与控件接入继续在 aarch64 推进，x86_64 留待最后。

### 3.9 公共嵌套绘制上下文

PaintContext.withState 复用 FrameContext 的独立借用，保留同一尺寸、时间、输入父级和子项放置。子项绘制次数在同一 Element 的所有嵌套作用域间共享，避免装饰回调使 children 重复绘制；借用期间父上下文拒绝绘制／放置，结束后子上下文失效。普通和异常退出都恢复 Canvas，框架仍只有 Empty／Component／Element 三种描述。

Scoop 格式与 UI／独立 drawing Cone 的含测试类型检查通过。aarch64 release 的真实 UI drawing 组合 1／1 通过，新增覆盖嵌套子树绘制、父／子借用拒绝、重复 paintChildren 和未配对图层的异常恢复；原有动画、可选放置、输入冒泡与回调关闭继续通过。graphics／graphics.text／Surface 的 aarch64 公开资源已按更新的下层重新发布。无 fixture 或全量测试，公共 style 接着消费此入口。

### 3.10 公共样式与真实停帧

新增独立 scoop.ui.style：Style／StyleOverride 是普通不可变描述，styled 始终保留同一包装与内容位置，StylePainter 通过公开 Canvas／PaintContext 复用当前滤镜和轮廓。背景、向内边框、圆角、外框阴影与有序内容 Blur／DropShadow／Glow 按统一顺序绘制；显式 clip 使用同一轮廓限制输出和子输入。参数以局部逻辑单位表达，图层不按布局框隐式裁剪，效果不改变尺寸、基线或命中范围。

滤镜和几何先完整创建再替换，旧资源即时关闭；不把历代资源追加到 ComponentScope。没有子树像素缓存，文字、透明图片和子组件始终使用当帧内容。没有效果时钟或额外任务；Surface Animatable 直接提供采样值。

Scoop 格式和含测试类型检查、C++／Objective-C 格式与严格告警检查通过。aarch64 release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.ui.style | 10／10 | 外框与稀疏 alpha 阴影、真实文字及透明图片 glow／blur、顺序、clip／圆角／边框、像素外扩、一次 DPI 的 Metal、继承覆盖、缓存复用／失败替换／关闭与借用 |
| Style UI Cone | 1／1 | 添加／移除效果保留实例、State、基线和焦点，真实像素及阴影不扩大点击区、clip 限制子输入，动画完成／暂停／取消／隐藏和卸载后的停帧 |
| Style errors Cone | 1／1 | 绘制与清理同时失败、未配对图层恢复、图片／painter／过期画布释放、原生窗口关闭、失败帧零提交 |

停帧通过测试辅助 Cone 同时观察 render／update／measure／paint 计数、实际 Metal renderer 的帧需求、display-link 暂停和回调／提交计数；稳定区间所有计数不变。辅助观察不持有 renderer、不调度任务，生产实现没有为测试添加计时器或计数器。

补充圆角 clip 输入用例后曾有一次原生等待超时；增加阶段及完整错误链诊断后，完整测试和同一测试产物的复跑通过，未修改生产逻辑或放宽断言。该次超时的原因尚未定位，后续主题／控件原生组合继续沿用这些诊断。

本轮未修改编译器、core／std／test，没有新增 fixture 或运行全量测试。公共 style 已发布到 aarch64 开发 sysroot；下一批为 Theme／ThemePainter、四类公共几何及三套风格，x86_64 继续留待最后。

### 3.11 主题所需的公共文本覆盖解析

SpanStyle.resolve(parent) 由文本库公开，嵌套片段和主题／控件沿用同一继承规则，不复制字体描述或创建 native 资源。格式与含测试类型检查通过，aarch64 release 的 StyledTextTest 8／8 通过：新增更换 parent 后保留显式颜色、清除行高／feature、继承新字体与字号；既有嵌套排版和实际 glyph 绘图同时通过。graphics.text 已重新发布，无编译器／core／std／test 改动，无 fixture 或全量测试。

### 3.12 公共主题部件与三套风格

style 新增 Theme／ThemePainter、ThemeSource、ThemePainterCache 和类型化视觉状态、Options、metrics、geometry。表面返回外框、边框内区域、内容区和焦点轮廓；Checkbox／Radio 返回标记区域、线段／圆形；Scrollbar 返回轨道、滑块、端部按钮和实际行程。几何不依赖 Canvas／Application，绘图使用普通 Canvas 及同次解析的数据。Scope 拥有可替换 painter，替换先创建新值，失败保留旧值。

新增独立 macos／material／fluent 三个主题 Cone，均提供 Light／Dark。四类部件共用 ShapeTheme 算法，各主题显式提供字体／字重、尺寸／内缩、圆角、边框、阴影、焦点与状态颜色。macOS 为紧凑预设；Material 提供较大的尺寸、圆角和提升外观；Fluent 的滚动条带端部按钮。tokens 同时公开普通自绘可用的语义颜色、文字、间距、圆角、效果与 Surface 动画参数。主题和 painter 不创建业务状态、输入、任务或时钟。

Scoop 格式、生产与含测试类型检查通过，三套主题和更新后的 style／ui 已发布到 aarch64 开发 sysroot。themes 测试 Cone 的 release scoop.test 13／13 通过：四类几何与两轴短轨道、覆盖与继承、六种风格／明暗组合、Off／On／Mixed／hover／pressed／disabled／focus 像素、固定字体实际排版、Context 来源、失败创建及替换关闭、绘制异常恢复、异线程拒绝，以及 DPI 2 的真实 Metal 对照。

Metal 与 Raster 的圆周抗锯齿覆盖不同，初始逐像素最大误差断言把单个圆边缘差异误判为几何失败。检查原始图像后确认边界一致、内部最大差异 1、该 Radio 覆盖量差约 0.7%。最终测试分别约束均匀内部颜色、每个部件的像素边界及覆盖量，并保留整图误差检查；六组主题全部通过。临时原始像素及预览不提交。

控件整体切换、实例／焦点保留、主题动画与完整展示继续在下一批验证；本节的离屏与 Metal 部件测试不代替实际 UI 组合。无编译器／core／std／test 改动，无 fixture 或全量测试，x86_64 仍在 aarch64 全部功能完成后补验。

### 3.13 基础控件消费公共主题

独立 scoop.ui.component 已提供 Label／Button／Checkbox，生产依赖只有公开的 UI、style 及实际下层库，不依赖三套具体主题。Label 使用 ParagraphHost，Plain 的 SpanStyle 继承本轮主题，完整 StyledText 保留自己的模型；Button 的 action 字体与颜色显式转交给标签，实际基线透传。Checkbox 提供外部 Binding 与自有 State 两种入口，交互使用当前 Props 的绑定或回调。

Button／Checkbox 在 measure 中保存本帧采样后的主题几何和视觉输入，paint 使用同一结果。私有交互状态只处理指针、键盘和焦点；主题 painter 绘制表面、标记和焦点。局部状态覆盖通过 StyleOverride.then 有序组合，未指定属性继续跟随主题；hover／press／selection 复用 Surface Animatable，无独立时钟。

Scoop 格式及生产／含测试的类型检查通过，aarch64 开发 sysroot 已发布。release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.ui.style | 11／11 | 原样式／像素／资源测试，加局部覆盖的有序组合与显式清除 |
| scoop.ui.component | 3／3 | 控件状态覆盖、文字解析复用与主题变更、挂载环境来源和明确 Props 换绑 |
| controls UI Cone | 1／1 | 六种主题组合、真实文字与尺寸／基线、公共几何查询和绘图调用、实例／自有 State／焦点保留，指针与键盘、重复抑制、最新回调、Binding 换绑、禁用取消、效果切换和原生关闭 |

控件组合同时记录 render／update／measure／paint、公共主题绘图调用和原生帧需求／display-link／提交；动画结束后等待区间全部停止。每轮主题替换立即关闭旧 painter，原生关闭释放最后一组，外部字体仍有效。效果改变保持测量和基线，绘制与输入继续采用 Surface 成功提交的几何。

测试初次出现过无完整诊断的焦点等待超时；补充断言链后另定位到测试把原生事件入队误当成交付完成。现已在事件批次之后断言状态，不增加轮询或生产唤醒。requestFocus 仍遵守 window 的请求式合同；保留阶段、可见性、遮挡、焦点和帧计数诊断用于后续原生验收。

容器及完整主题展示接着实施，并已补充具体布局与 Frame 表面切换合同。当前提交不把未完成的展示草稿算作交付。无编译器／core／std／test 改动，无 fixture 或全量测试；x86_64 继续在 aarch64 完成后统一补验。

### 3.14 布局容器、富文本组合和主题展示

Row／Column／Stack／Frame 已通过公开 MeasureContext／PaintContext 接入。线性容器在主轴自然测量、交叉轴按约束对齐；Stack 按描述顺序叠放。Frame 提供明确尺寸、padding、对齐和 StyleOverride，可选择主题表面并透传真实子项基线。普通／主题表面沿同一节点更新，ThemePainterCache.clear 及时释放已不用的主题 painter，仍允许后续复用缓存。

完整[主题展示](../../sysroot/src/scoop.ui.component/examples/themes/README.md)使用同一 Canvas 绘图辅助对象展示公共部件及状态，下方使用真实控件、计数、自有勾选状态和有序效果。风格、明暗和系统跟随分别控制；来源只在实际主题变化时发布 State，原生 ThemeChanged 不会造成周期任务。视觉检查发现初版过高，已把默认内容区调整为 670 逻辑单位并压缩留白；未缩放整棵 UI 或增加滚动行为。实际点击验证了三种风格、明暗和效果切换，以及计数和勾选状态的保留。

Scoop 格式与生产／含测试类型检查通过，aarch64 开发 sysroot 已发布更新后的 style、三套主题与 component；展示 release 可执行文件已构建。release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.ui.component | 7／7 | 原覆盖／来源，加线性／叠放约束、对齐、溢出、Frame 固定尺寸与内缩、参数错误 |
| themes 的 ThemeResourceTest | 3／3 | painter 替换、clear 后复用、关闭与异线程拒绝，原失败创建和绘制借用 |
| controls layout UI Cone | 1／1 | keyed 重排保留 State／焦点、表面切换释放、Stack 绘制与命中顺序、圆角 clip、两种基线、同段富文本中的 Button／图片、RTL／截断／恢复、隐藏卸载与原生关闭 |
| themes 展示 Cone | 3／3 | 稳定来源与系统跟随、六种组合的 DPI 1／2 离屏输出、完整展示的真实 Metal／UI 切换、效果组合和停帧 |

布局组合初次等待超时记录到 requestFocus 后 focused=false、已有一次呈现；后续正常获得焦点，未改变 window 的请求式合同。另一个测试错误是把定宽行内按钮换行后的基线与首行比较，已用 Frame 明确该组合需要的宽度，再验证同一行实际基线。生产排版未增加估算或补偿路径。

布局和展示组合在安静区间同时断言 render／update／measure／paint、底层帧需求、display-link 暂停以及回调／提交计数停止；资源替换与原生关闭检查实际旧 painter、图片及借用字体的有效期。截图只用于排版检查，不代替这些断言。本轮没有编译器／core／std／test 改动，没有 fixture 或全量测试；原有异步进度与多窗口宿主组合继续推进，x86_64 仍统一留待最后。
