# UI 实施记录

日期：2026-11-02。实施依据为 [UI 设计](DESIGN.md)、[框架规范](../specs/SCOOP-UI-SPEC.md)、[文本排版规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)及[基础控件规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)。本期先在 macOS aarch64 完成全部实施批次，再统一补验 x86_64，Windows 后续接入；图文绕排、GPU／3D／媒体互操作和 SkSL 仍属于设计中列出的后续增强。

## 1. 批次状态

| 批次 | 状态 | 交付范围 |
| --- | --- | --- |
| 0. 已确认设计与平台范围 | 已记录 | 自绘分层、State／Context／Binding、资源所有权、静止停帧、macOS 首期与后续增强 |
| 1. 文本公共能力 | aarch64 已完成 | 独立 graphics.text、Canvas 借用、StyledText／SpanStyle、行内 metrics／placement、位置与替代文字 |
| 2. 独立自绘闭环 | aarch64 已完成 | SurfaceContent 所有权、Component／Element／State／Props／Binding、挂载、布局、输入、UiHost 与独立自绘示例 |
| 3. 公共文本适配与简单控件 | aarch64 接入中 | ParagraphHost／InlineView 已完成；Label／Button／Checkbox 待实施 |
| 4. 布局与组合 | 待实施 | Row／Column／Stack／Frame、keyed 子项、基线与资源组合 |
| 5. 宿主与生命周期组合 | 待实施 | 空闲停帧、动画收尾、隐藏更新、asyncio／HTTP 进度、取消和多窗口 |

已有 scoop.surface 的窗口、动画和 InputArea 实现作为基础，独立 UI 框架已交付；公共文本适配、控件库和后续组合验证继续按批次推进。下文保存实际验证范围，不把设计示例和文档检查当成已交付功能。

## 2. 验证与提交方式

普通 API、资源和控件行为使用所属 Cone 的 scoop.test；真实 GPU／窗口组合使用现有 macOS 后端。默认 release，定向验证改变的能力和必要消费者，不执行全量测试。遇到编译器、runtime 或其他系统库缺口时先修订其规范并修复该层，再补相应定向回归，不在 UI 中实现规避分支。

每批先完成 aarch64 验证并提交，不把 x86_64 补验作为继续实现下一批的前置。已提前准备的部分 x86_64 资源和缓存保留，尚未进行本轮运行验收；本期 aarch64 功能完成后再更新资源并统一补验。

每项完整变更通过格式化、检查和定向测试后提交并推送，再 fetch／merge origin/main；合并改变相关代码时补充必要验证后继续。每轮清理不再使用的临时日志、探测文件和构建输出，仍需复用的正常构建缓存及本期目标资源继续保留。记录只保存验证范围、结果和实际限制。

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
