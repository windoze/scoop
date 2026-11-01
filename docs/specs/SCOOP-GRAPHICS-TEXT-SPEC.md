# Scoop 文本排版库规范

日期：2026-11-02。状态：独立文本能力及 UI 接入已完成 macOS aarch64 实施和 x86_64 补验，包括字体、StyledText／TextSpan／SpanStyle、Paragraph／TextLayout、行内对象及文字绘制查询。x86_64 使用 Apple Silicon 上的 Rosetta，范围见 [UI 实施记录](../ui/PROGRESS.md)。取舍、迁移和调研依据见 [文本排版设计](../graphics/text/DESIGN.md)。UI 接入见 [UI 规范第 7.5 节](SCOOP-UI-SPEC.md#75-公共文本接入)。

## 1. 定位与依赖

`scoop.graphics.text` 是普通 Scoop Cone，负责富文本内容、字体来源、shaping、段落排版及文字／行内对象的几何查询。直接绘图程序和 UI 控件使用同一套公共能力。它依赖实际使用的 core／std／platform 和 `scoop.graphics`，不依赖 window、surface、ui 或控件库。

Cone 名称为 `scoop:scoop.graphics.text`，公共 package 为 `scoop.graphics.text`，源码目录为 `sysroot/src/scoop.graphics.text/`。命名上属于 graphics 的文本排版领域，编译单元仍独立；依赖、导入和 internal 可见性遵守普通 Cone 规则，共享名称前缀不产生跨 Cone 内部访问权。`scoop.text` 是 scoop.std 已有的文本处理 package，负责字符串操作、数值文本转换和 Hex，见[标准库规范](SCOOP-STDLIB-SPEC.md)；其含义和现有 API 保持。

| 层 | 目标职责 |
| --- | --- |
| `scoop.graphics` | 几何、颜色、Canvas、Paint、Path、Image、绘图目标及 native 绘图接入 |
| `scoop.graphics.text` | 字体、StyledText／TextSpan、样式、Paragraph、TextLayout、行内对象排版及 Canvas.drawText 扩展 |
| `scoop.surface` | 窗口呈现、帧、动画、区域输入；caret 只接收局部 Rect |
| `scoop.ui.text` | `scoop.ui` Cone 内的公共文本适配包，连接段落排版与正常子组件测量／放置 |
| `scoop.ui.component` | Label、Button 等控件的外观、交互及使用入口 |

绘制扩展定义在 graphics.text，因此依赖保持 `scoop.graphics.text → scoop.graphics`；graphics 不引用 TextLayout。`scoop.ui.text` 不另建 Cone，也不提供现成控件。UI 框架使用 graphics.text 的地方声明直接依赖；独立绘图程序无需引入 UI 或 Application。

文本算法继续采用当前固定版本的 SkParagraph、HarfBuzz 与 SkUnicode／ICU，系统字体发现继续复用 CoreText／DirectWrite。拆分不要求新的文本引擎、语言语法、编译器处理、runtime ABI 或插件协议。

当前实施批次先以 macOS aarch64 的 CoreText 与真实离屏／Metal 绘图为验收目标，本期功能完成后统一补验 x86_64。拆分时一起移动已有的 DirectWrite 工厂及目标选择，避免保留两套文本所有权；Windows 的新增接入和运行验收留待后续。共享文本模型仍按本规范实现，目标平台的可用性以已交付的 Cone 源码和验证记录为准。

本期支持段落内多字体／字号／样式混排，以及不可分割的行内图片和控件盒子。完整编辑文档、撤销、剪贴板系统接入、accessibility 树、分页、浮动元素和跨多行图文绕排不属于本库本期交付；文本位置、替代文字与选择几何则在本期有明确合同。

图文绕排、分栏与区域续排列为后续增强，现有内容模型的复用范围、SkParagraph 的边界及实施方向见 [文本排版设计第 8 节](../graphics/text/DESIGN.md#8-后续增强图文绕排分栏与区域续排)。本期的 Paragraph.layout(width) 仍表示固定宽度的段落排版，不提前加入区域流式排版参数或空实现。

## 2. 内容与样式

`StyledText` 是无 native 资源的不可变 class，保存基础 TextStyle 与有序 `Array<TextSpan>`。TextSpan 同样是不可变描述，公开工厂只有以下内容形状：

| 工厂 | 内容 |
| --- | --- |
| `TextSpan.text(text)` | 普通 String |
| `TextSpan.span(style, children)` | 局部 SpanStyle 与有序子片段 |
| `TextSpan.inlineObject(key, alternateText)` | 一个有稳定 InlineKey 的行内对象及其替代文字 |

片段树只表达内容与样式，不是 UI 挂载树；Span 不产生组件身份或资源作用域。同一段落中的文字共同进行双向分析、shaping 和断行，不能把样式片段变成多个独立 Label 后拼接。Span 可以跨行；行内对象占一个不可拆分盒子。换行符沿既有 Paragraph 行为产生显式换行。

TextStyle 是完整样式，沿用当前的 family 顺序、字号、颜色、weight／width／slant、locale、字距、词距、行高、OpenType feature 和 variation。SpanStyle 对这些属性表达局部覆盖：未指定时继承最近父样式，最外层继承 StyledText.style；指定时替换该属性。数组属性整体替换，不隐式合并或追加。

局部覆盖必须区分继承、显式设置与清除可选值。例如 TextStyle.lineHeight 为 Option<Double>，SpanStyle 对应字段为 Option<Option<Double>>：None 继承，Some(None) 恢复字体自然行高，Some(Some(value)) 设置行高。空 feature／variation 数组可以显式清除继承设置。基础 TextStyle 的默认值不能冒充 SpanStyle 的未指定值。

`SpanStyle.resolve(parent: TextStyle): TextStyle` 公开同一套继承计算，返回普通完整描述，不创建排版或 native 资源。主题、控件和其他文本消费者可以把局部覆盖应用到当前默认样式；再次用新的 parent 解析时，未指定属性随 parent 更新。嵌套 TextSpan 也使用此方法，不另建主题字体模型或另一套解析规则。参数有效性仍由实际文本排版入口按既有合同检查。

进入段落引擎前，将描述展开为最终样式的文字 run 和行内对象记录。相邻等样式文字允许合并，空 Span 不插入额外字符。原始描述可以复用；不要求它是 struct、不强制深复制数组，也不为每个片段分配 native 资源。解析后的 TextRun 是排版输入的内部正常形态，不再另设一套平行的公开富文本模型。

纯空内容或仅含空片段时，段落的默认字体、行高与起始 caret 来自 StyledText.style；空的局部样式片段不制造一行或改变无文字段落的样式。段落构造使用基础样式设置默认文本属性，非空片段使用各自解析后的完整样式。

StyledText 公开规范化的 text，以及 `inlineObjects: Array<InlineObject>`；InlineObject 保存 key、range、alternateText，按逻辑顺序排列，供直接绘图和 UI 适配建立对应。内容规范化在本对象内计算并复用，不依赖字体或 shaping；调用方无需重新遍历文本、猜测编码位置或访问 internal 数据。

## 3. 行内对象与测量输入

InlineKey 是值类型，包含 Text(String) 和 Number(Long) 两种显式标识，在一个 StyledText 内唯一。它标识内容中的一次出现，不是字节位置、glyph 序号、视觉顺序或组件地址。重复使用同一图片或控件描述时，每个出现位置需要不同的 key。

文本层不保存 View、Component、InputArea、绘制回调或 Image 所有权。调用方负责取得对象实际尺寸；排版入口接收 `Array<InlineMetrics>`，每项包含 key、Size 与 InlineAlignment。有行内对象时，每个 key 恰好对应一份完整 metrics；重复、缺失和多余项作为参数错误处理，不用零尺寸补齐。无行内对象时传空数组。

InlineAlignment 的含义为：

| 对齐 | 几何含义 |
| --- | --- |
| `Baseline(kind, offset)` | kind 为 Alphabetic 或 Ideographic；对象顶部到该基线的距离为 offset，故对象顶部为段落基线减 offset |
| `AboveBaseline(kind)` | 对象底边贴该基线 |
| `BelowBaseline(kind)` | 对象顶边贴该基线 |
| Top／Bottom／Middle | 对齐排版引擎确定的所在行盒子的顶部、底部或中部 |

尺寸是有限非负逻辑值，基线偏移为有限有符号值；外框裁剪不要求基线一定落在外框内。零尺寸对象仍是实际内容，不代表被隐藏。对象的 ascent／descent 参与行高计算，不能仅在完成普通文字排版后叠画图片。

同一行的所有对象共用基线参照。两条基线之间的距离由该行文字的字体／行高度量决定，在计入对象高度前确定；没有文字时使用段落基础样式。对象可以扩大行盒子的 ascent／descent，但不能通过自身高度改变后续对象选择的基线距离。TextLayout 的首行基线与 TextLine.baseline、文字绘制及 InlineBox 使用同一实际坐标，包含排版引擎对行高的舍入结果；不能直接混用引擎未舍入的字体度量 getter 与绘制坐标。

对象前后的断行与双向排序交给段落引擎，采用对象替换字符的 Unicode 语义；本期不增加任意回调式断行策略。超过可用行宽的对象仍不可拆分，按段落的换行／截断规则处理，不自动缩小对象。需要限制尺寸的调用方在提供 metrics 前完成约束。

图片调用方可以由图片尺寸或显式目标尺寸给出 metrics，并在返回的位置调用 Canvas.drawImage。UI 可以先测量子控件，再提供同样的 metrics；两种路径使用相同的段落排版接口。

## 4. 字体与 Paragraph

FontCollection、TextStyle、ParagraphStyle、TextRange／TextPosition 等文本类型迁入 graphics.text。FontCollection 管理显式字体、family 别名和系统 fallback。成功改变字体登记时更新其只读 revision，供段落宿主判断字体输入是否变化；普通查询与失败登记不改变 revision。这只是本集合的缓存更新标记，不是全局身份或产物字段。

`Paragraph(fonts, content: StyledText, settings)` 保存展开后的内容、最终样式与字体来源快照。另提供 String 加 TextStyle 的普通便利构造，它产生相同的纯文本内容。之后字体登记、来源关闭或创建另一段落，不改变已有 Paragraph 和 TextLayout。

排版入口的签名形状为：

```text
fun layout(width: Double, inlines: Array<InlineMetrics> = []): TextLayout
fun layoutUnwrapped(inlines: Array<InlineMetrics> = []): TextLayout
```

width 是有限非负逻辑宽度；无界请求用 layoutUnwrapped，保留显式换行，不把 Infinity 传给引擎。对象尺寸属于此次排版输入，不固化在 Paragraph 的内容快照中。对象尺寸变化可以复用内容与字体快照，再创建新 TextLayout。

布局结果必须独立。后一次 layout、metrics 改变或关闭 Paragraph，都不能使旧 TextLayout 的文字、行、对象位置和绘制结果发生变化或失效。底层 SkParagraph 可变时，分别保留其实际结果及所需字体资源。

## 5. TextLayout 与行内对象结果

TextLayout 是拥有排版资源的 Closeable class。它保存内容快照、尺寸、首行 alphabetic／ideographic baseline、intrinsic 宽度、行信息、截断信息、grapheme 边界、未解析 glyph 数量，以及供绘制／命中／caret／selection 使用的同一份结果。几何都在段落局部逻辑坐标中。

新增 `inlineObjects: Array<InlinePlacement>`，按内容逻辑顺序为每个对象保留一项，包含：

- key：原 InlineKey。
- range：该对象在规范化文本中的 UTF-8 字节范围。
- box：Option<InlineBox>；Some 保存 rect、所在行索引和方向，None 表示该对象被 maxLines／ellipsis 排除，没有可绘制位置。

结果数量不因截断而缩短；None 是明确的布局结果。零宽或零高对象仍可得到 Some。对象位置来自排版引擎，调用方不能按数组顺序累加宽度推测 x，也不能将逻辑顺序当成 RTL／混合 bidi 下的视觉顺序。

native 的可见 placeholder 矩形需要结合原始对象记录及可见范围还原成上述结果；不能把一份缩短后的矩形数组直接 zip 到所有 UI 子项。身份、可见性与几何在同一 TextLayout 中保持对应。

maxLines／ellipsis 决定排版可见性；Canvas clip 只决定某次绘制的裁剪，不反向修改 TextLayout。布局宽度不自动变成绘制裁剪。被排版截掉的对象没有 box，未被截掉但超出外框的内容由调用方显式决定 clip。

## 6. 文本位置、命中与替代文字

规范化文本按内容顺序连接普通文字，每个行内对象插入一个 U+FFFC，并另存 key／range 元数据。它在 UTF-8 中占三个字节；不能把一个对象当作一个字节。普通文本中已有的 U+FFFC 只有字符语义，没有对象元数据就不是可挂载对象。

对外位置继续使用 Long UTF-8 字节偏移和 TextAffinity；范围为半开区间，端点须在 Unicode 标量边界。UTF-8／UTF-16、grapheme、cluster 与 glyph 的关系由接入层维护；描述树路径和对象 key 不取代文本位置。新增内容或换行可以改变位置，但不会改变未替换对象的 key。

既有纯文字查询继续适用：Upstream／Downstream 选择边界前／后的视觉 run，在软换行处可对应不同行；caret 返回具有指定 thickness 的逻辑矩形。TextLayout 同时提供 grapheme 边界、各行 range／contentRange／visibleRange、基线和 unresolved glyph 数量。空文字有字体决定的行高与起始 caret，无换行请求仍保留显式换行。超过实际 ICU／SkParagraph 索引表示范围的文本在接入处报参数错误。

每个行内对象作为一个原子文本单元，其两端保留为定位边界，不与相邻纯文字合并成不可分的定位单元；纯文字仍按 grapheme 定位。caret 只在对象前后，不落入其编码内部。hitTest 在对象盒内选择其前／后文本位置，结合方向、距离与 affinity 处理；需要把指针交给实际控件时，UI 仍走该控件的 InputArea，不能用文字 hitTest 代替控件事件分发。

selection 返回可见文字和已选中对象的 TextBox；完全覆盖对象的范围包含其实际盒子，被截断部分没有选择几何。一个逻辑范围在 bidi 下仍可返回多个矩形。被截断位置的 caret 为 None，空文本的行高及起始 caret 保持既有行为。

`StyledText.toPlainText(range)` 接受上述规范化文本范围，将其中对象替换为 alternateText；无参形式导出全部内容。替代文字不参与原段落的 shaping、断行或位置计数，导出的字符串使用自己的索引。空替代文字合法，但应由调用方根据图片或控件含义明确选择。辅助功能和复制的上层实现可以消费该数据，本库不接管剪贴板或 UI 语义树。

## 7. 绘制、缓存与所有权

定义在 `scoop.graphics.text` 的 Canvas.drawText 绘制 TextLayout 中的文字，行内对象留给调用方按 inlineObjects 绘制或放置。调用方使用同一个结果提供对象位置、文字选择及输入几何，不能分别测量两份段落。对象的绘制不触发文本引擎回调，也不在排版中调用组件 render／paint。

跨 Cone 绘制通过 `scoop.graphics.native` 提供的同步、受限 Unsafe 画布借用接入；有效期沿用原 Canvas。适配经不透明 C ABI 句柄连接同一固定版本的 Skia，不复制 SGCanvas／SGObject 的私有布局，不把 C++ 类型暴露为 Scoop API。graphics.text 自己持有并释放段落／字体对象；graphics 不按文本类型分派或为 graphics.text 建立资源注册表。借用不允许保存画布、跨线程／挂起使用，文字绘制结束后恢复调用方几何状态。

具体入口为 `Canvas.withSkiaCanvas`，签名及原生借用约束见 [Surface 规范第 2 节](SCOOP-SURFACE-SPEC.md#2-几何与绘制)。Canvas.drawText 在这段借用内绘制同一 TextLayout，只使用公开的 Point／Rect／Color 值布局及 Skia 头文件，不包含 graphics 的私有 native 头文件。

FontCollection、Paragraph 和 TextLayout 在创建线程串行使用与关闭；不可变结果不自动获得底层 Skia 缓存的并发保证。native 持有跨调用需要的数据或原生引用，不能保存移动 GC 对象的裸地址。关闭幂等，失败与清理继续遵守既有 GraphicsException、参数／状态异常和 ResourceUseException 合同。

缓存由实际宿主拥有。内容、样式、段落设置或字体集合／revision 改变时重建 Paragraph；宽度、行内尺寸或基线改变时重排。仅位置、Canvas 变换、整体透明度及不改变尺寸的行内绘制变化复用排版。颜色等已经保存在 Paragraph 中的样式变化可以走重建路径，本期不要求另建绘制样式增量系统。

复用以同一不可变内容／配置、字体来源及 revision、相同布局参数为最低合同；不要求对两个新描述执行全树深比较。新的内容描述可以按输入变化处理。每帧测得的有限尺寸和基线使用实际值判断，不能仅因生成了新 metrics 数组而反复重排。

公共 UI 宿主按 UI 规范登记一个可替换的 Closeable 缓存，创建新值成功后替换并及时关闭旧值，失败时关闭新取得资源。关闭 TextLayout 或 Paragraph 不会关闭行内组件、Image、输入区域或动画；这些资源继续由实际拥有者管理。应用不为每个 TextSpan 或行内对象书写 use。

## 8. 实施与验收

独立 Cone、字体和段落资源所有权、公开 Canvas 借用、统一内容描述、样式继承、行内 metrics／placement 和相关查询已实现，并完成测试与 Surface 示例迁移，不保留旧 package 别名或公开 run 作者接口。Paragraph 的输入为 StyledText 或 String／TextStyle 的便利构造，Paragraph／TextLayout 公开同一内容快照；toPlainText 按 UTF-8 标量边界替换对象的 alternateText。Skia 的基线补丁由正常目标资源准备过程应用，具体验证范围以实施记录为准。

普通行为测试使用所属 Cone 的 scoop.test，至少覆盖：

1. 固定字体的多字体／字号／颜色／weight／slant 混排，嵌套 Span 继承、局部覆盖、显式清除与空片段。
2. CJK、阿拉伯文与混合 bidi、组合字符、emoji、fallback、显式换行、零宽和无界请求；旧 Paragraph／TextLayout 的快照独立性。
3. 行内图片与控件尺寸输入、alphabetic／ideographic 对齐、不同高度对象影响行高、超宽对象及零尺寸对象。
4. 多对象在 RTL／混合 bidi 中的 key／位置对应；maxLines／ellipsis 保留完整结果数组，并正确标出不可见项。
5. UTF-8／UTF-16 映射、对象前后 caret、对象命中与 selection、替代文字导出，以及普通 U+FFFC 不成为附件。
6. 内容／字体登记／宽度／对象 metrics 更新和缓存复用；借用资源、来源提前关闭、构造／重排失败及资源替换。
7. 独立 Cone 只依赖 graphics.text／graphics，完成真实 Skia 离屏图文绘制与查询，验证跨 Cone 画布借用和目标 native 链接。

UI 自绘嵌入、控件状态保留、截断后的输入退出、动画重排和窗口关闭分别归 ui／component 的组合验收。测试围绕上述真实能力，不复制通用编译器负例，也不新增仅用于测试的排版后端。
