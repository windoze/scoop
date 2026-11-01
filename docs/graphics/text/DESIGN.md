# scoop.graphics.text：富文本排版与行内对象设计

日期：2026-11-02。状态：独立字体和段落、StyledText／TextSpan／SpanStyle、行内对象及公共 UI 适配已在 macOS aarch64 实现；基础控件组合继续实施，x86_64 最后补验，进展见 [UI 实施记录](../../ui/PROGRESS.md)。目标合同见 [文本排版规范](../../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)，UI 对接见 [UI 规范第 7.5 节](../../specs/SCOOP-UI-SPEC.md#75-公共文本接入)，整体分层见 [UI 设计](../../ui/DESIGN.md)。

## 1. 从现有能力演进

已有的 FontCollection、Paragraph／TextLayout、行与基线、UTF 索引、caret／selection 和命中查询已迁入 `scoop.graphics.text`。内容输入已由公开 TextRun 数组演进为 StyledText／TextSpan／SpanStyle，统一表达嵌套样式继承。该库经公开的 `Canvas.withSkiaCanvas` 同步借用绘图目标，独立持有和释放字体／段落，不再包含 graphics 的私有 native 头文件。

新增需求是把字体、字号和样式混排提升为公共内容描述，并让图片及任意自绘控件参与段落断行与行高计算。TextLayout 作为排版结果保留；内容、样式继承和行内 metrics 是其上游输入，控件挂载及绘制是其下游使用者。

使用 `scoop.graphics.text` 表达图形领域的文本排版；`scoop.text` 已由 scoop.std 用于字符串操作、数值转换和 Hex，见[标准库规范](../../specs/SCOOP-STDLIB-SPEC.md)。排版库仍是独立 Cone，位于 `sysroot/src/scoop.graphics.text/`，与 graphics 通过普通依赖连接。

拆为普通 `scoop.graphics.text` Cone，依赖 graphics 的几何、颜色和画布；字体与文字 API 随之迁出 graphics。Canvas.drawText 扩展定义在 graphics.text，维持单向依赖。surface 继续接收 Rect caret，不持有文本内容，不因本次拆分依赖 graphics.text。ui 通过本 Cone 内的 `scoop.ui.text` 公共适配包使用 graphics.text。

这次不扩大到编辑文档、分页或浮动绕排。一个行内控件在父段落中占一个矩形；它内部可以继续组合文字和控件，但父段落不会把它拆成几行。绕排、分栏及区域续排的后续方向见[第 8 节](#8-后续增强图文绕排分栏与区域续排)，不作为本期拆分和行内对象接入的前置。

## 2. 成熟方案与本次选择

| 参考 | 关键机制 | Scoop 的采用点 |
| --- | --- | --- |
| Flutter TextSpan／WidgetSpan | UI 测量子控件，提交 PlaceholderDimensions，段落排版后返回位置；截断项可以没有放置位置 | 自然尺寸作为通用路径；基线进入测量结果，布局可见性与挂载分开 |
| Compose AnnotatedString／InlineTextContent | 显式 Placeholder 尺寸和对齐，内容 id 映射到 composable | 支持明确尺寸的行内盒子，key 与文字位置分离 |
| Apple TextKit attachment | attributed string 含附件，view provider 处理 view 接入 | 文本描述和 UI 实例分工；这不是 SwiftUI.Text 可以任意嵌入 View 的承诺 |
| WPF InlineUIContainer | Run／Span 之间可以包含 Button 等 UIElement | 样式范围与原子行内对象采用不同布局语义 |

Skia 固定版本 `f326a1e25fc736704063922b9ce76b527b1c5efe` 提供 addPlaceholder、PlaceholderStyle 及 getRectsForPlaceholders。接入按可见文本范围匹配逻辑对象，保留截断项和零尺寸项，已有 bidi、截断及图片的运行回归。实现时发现并通过[底层补丁](../../../sysroot/vendor/skia/patches/text-baselines.patch)统一实际绘制基线和对象共同的 ideographic 参照；补丁及目标资源见 [vendor 说明](../../../sysroot/vendor/skia/README.md)。实际 UI 控件组合仍按后续批次验收。

## 3. 描述与排版结果

作者层采用不可变 class 的 StyledText／TextSpan，使用普通函数构造内容。SpanStyle 只覆盖指定属性，解析后成为完整 TextStyle；TextRun 仅作为内部展开结果。片段不包含 native owner，描述复用不要求重复复制整段文字。

例如下面表达文字、局部样式、图片和控件的位置；行内对象的实际资源另外提供：

```kotlin
import scoop.graphics.text.*

val content =
    StyledText(
        spans =
            arrayOf(
                TextSpan.text("Total: "),
                TextSpan.span(
                    style = SpanStyle(fontSize = Some(20.0), weight = Some(700)),
                    children = arrayOf(TextSpan.text("128.00"))
                ),
                TextSpan.text(" "),
                TextSpan.inlineObject(InlineKey.Text("badge"), alternateText = "Member"),
                TextSpan.text(" "),
                TextSpan.inlineObject(InlineKey.Text("action"), alternateText = "Details")
            )
    )
```

文本层只收到 `badge`／`action` 的尺寸和对齐信息。直接绘图程序可在 badge 的排版位置画 Image；UI 可把 action 关联到持久 Button。图片和控件都遵守同一个对象盒子合同。

规范化文本用 U+FFFC 表示对象位置，并保存独立的 key／range。TextLayout 为每个对象返回带可选 box 的结果，保留不可见项。这样不必用字节偏移建立组件身份，也不会在截断时误把后一个控件放到前一个对象的位置。普通文本中的 U+FFFC 不带对象元数据。

替代文字用于普通文本导出，不替代原来的布局文本。caret、selection、bidi 和 UTF-8／UTF-16 映射继续使用实际布局输入，避免替代文字长度改变后破坏位置。

## 4. 与一棵 UI 挂载树连接

UI 的公开 Element 测量结果改为 LayoutMetrics，保存 Size 与可选的首行 alphabetic／ideographic baseline。Component 原样透传，Button／Frame 将子项基线加上真实偏移，缺少基线的元素返回 None。行内默认对齐使用可得的 alphabetic baseline，缺失时采用盒子底边；调用方可以指定其他对齐。

通用流程为：

1. render 提供 StyledText 与带稳定 key 的 InlineView 描述，通过公共适配生成 Element 的正常 children。
2. measure 先测量每个行内子项。自然尺寸使用零最小值、段落可用最大宽度及无界高度；固定尺寸使用显式 tight 约束。
3. 收集尺寸和基线，调用 Paragraph.layout；按同一 TextLayout 的对象结果放置可见子项，返回段落的尺寸与基线。
4. paint 绘制文字及已放置子项；子项继续使用自己的 Canvas、InputArea 和 Surface 动画接入。

每个子项一次测量仍然足够；子项不根据尚未产生的“这一行剩余宽度”反复协商。固定尺寸超过段落宽度时不自动缩小；需要缩小时显式选择合适尺寸。

Span 没有 UI 身份，所有行内子项在段落 Element 的直接子序列中按 InlineKey 匹配。在前方插字、改样式、换行、重排或被省略号隐藏时保留组件实例；只有内容中的对象被移除、key／形状改变或宿主卸载时才按正常规则卸载。

被截断子项继续参与逻辑更新和资源生命周期，但本帧没有 placement、不 paint，也不发布 InputArea。成功提交才清除上一次区域几何并结束相应的输入／焦点；失败帧保留最近成功提交的画面与几何。隐藏不等于组件卸载，也不自动停用整个动画或异步任务作用域。

## 5. 共享适配与控件的边界

`scoop.ui.text` 提供 InlineView 描述、普通 keyed 子描述构造及 ParagraphHost 辅助对象。ParagraphHost 是由 Element 登记的 Closeable 缓存与布局适配器，不是 View、现成控件、另一棵树或全局服务。它只调用公开的 MeasureContext／PaintContext 和 scoop.graphics.text；第三方自绘控件可以直接复用。

Label 接受纯文本或 StyledText，并把行内 View 作为普通 children 交给框架；默认字体、外观和是否裁剪仍由控件决定。Label 与 Button 无需各自实现一套字体缓存、placeholder 对应或基线推断。独立自绘示例不依赖控件库，也能用相同适配完成图文与交互混排。

ParagraphHost 只拥有自己的 Paragraph／TextLayout 和临时缓存，字体来源、子 View 和图片资源按明确借用关系使用。scope 在卸载时关闭宿主；替换布局不会关闭子组件。最外层 host.use 的所有权链保持。

## 6. 源码与 native 迁移

| 当前归属 | 目标归属及必要工作 |
| --- | --- |
| graphics 的字体、style、paragraph、layout Scoop 源码 | 移入 graphics.text；以 StyledText 为内容入口，迁移所有 import 和实际直接依赖 |
| graphics 的字体发现、SkParagraph 与索引接入 | 移入 graphics.text native；目标字体发现仍按 Darwin／Windows 选择 |
| Canvas.drawText 直接访问 graphics internal resource | 改为 graphics.text 扩展，通过 graphics.native 的同步画布借用与不透明 C ABI 接入 |
| SGObject／GraphicsResource 共享所有权实现 | 各库拥有自己的资源包装和释放入口；不复制对方私有对象布局，不靠跨 Cone internal 访问 |
| graphics tests 中的文本行为与字体资源 | 随实际 API 迁到 graphics.text 的 scoop.test；graphics 保留绘图目标、路径、图像及 native 画布合同测试 |
| Surface／示例／控件中实际使用字体和段落的源码 | 声明 text 直接依赖；Surface 核心的帧、动画和 Rect caret 接口保持 |

当前 Skia native 依赖可能同时服务两库；继续使用同一已固定的目标资源、普通 Cone 依赖及链接规则，不复制构建两份 Skia，也不增加 runtime 注册或文本专用产物通道。C ABI 借用只暴露实际文字绘制所需的画布，不建立通用渲染插件或显示列表。

迁移时一并更新当前 Surface 规范第 3 节、graphics README 和示例；文本合同转由文本规范维护。设计阶段在这些文档中标出目标与现状即可，不能提前把尚未迁移的可运行代码改成不存在的 import。项目未发布，实施不保留兼容别名或双轨 API。

## 7. 实施顺序与验收

| 批次 | 完整交付 | 验收重点 |
| --- | --- | --- |
| 1 | graphics.text Cone、现有字体／布局迁移及 Canvas 接入 | 现有文字行为继续可用，独立消费者离屏绘图，字体／段落／Canvas 生命周期 |
| 2 | StyledText、SpanStyle、行内 metrics／结果及位置语义 | 多字体字号、继承、图片、bidi、基线、截断、caret／selection 和替代文字 |
| 3 | UI LayoutMetrics、可选放置、公共 ParagraphHost | 自绘行内控件一次测量、真实输入、实例保留、隐藏后退出命中、尺寸动画重排 |
| 4 | Label／Button 与富文本组合 | 同段落文字、图片和按钮；窗口缩放、输入变化、图片尺寸变化及异常关闭 |

第一组组合场景采用固定字体：中英文与阿拉伯文、不同字号的强调文字、行内图片、具有自然尺寸的按钮。缩窄窗口触发换行和省略号，再恢复宽度，检查基线、对象 key、按钮 State 与输入区域；改变按钮内容或图片尺寸后检查重排。纯颜色或内部动画不改变 metrics 时复用布局。

每批用真实能力完成闭环，不提前导出不可调用的 API。文本普通行为归 graphics.text，UI 挂载／输入归 ui，控件外观与点击归 component。跨 Cone 绘图和原生窗口各使用必要的实际消费者，文档检查不冒充这些运行验收。

## 8. 后续增强：图文绕排、分栏与区域续排

调研日期：2026-11-01。本节记录后续阶段的设计方向，尚未实现或运行原型，不扩大第 7 节的交付范围，也不冻结新的公开类型和签名。

### 8.1 当前模型的复用范围

StyledText、样式继承、字体与 shaping、UTF-8 位置语义、InlineKey 和行内 metrics 可以继续复用。需要增强的是内容进入哪些区域、区域内如何断行，以及布局结果如何表达分散的位置；内容描述不必因为改为两栏而复制成两份。

当前 Paragraph.layout(width) 针对固定宽度的段落，行内对象是参与单行宽度、行高和基线计算的原子盒子。放入一张较高的行内图片会影响所在行的高度，不会自动让后面多行的文字沿图片轮廓绕排。Row 中放两个独立 Label 也不等于一段内容连续流入两栏。

### 8.2 区域排版需要表达的内容

- 区域有尺寸、位置和阅读顺序，内容从一个区域继续进入下一个区域。分栏首先使用有高度限制的矩形区域；绕排还需要排除区域，以及当前行高范围内可用的水平区间。同一行可能包含多个可用片段，不能始终假定一行对应一个连续矩形。
- 结果保留逻辑文本范围与实际行片段、所在区域、局部几何之间的映射。caret、selection、命中和行内对象位置都使用同一布局结果，并继续采用整份内容的 UTF-8 位置与 affinity，不能在每一栏重新从零计数。
- 区域放不下的内容产生续排位置及所需段落状态，与 maxLines／ellipsis 导致的显示截断分开。把溢出内容省略后再尝试恢复，无法代替连续排版。
- 断行跨区域时保持段落的 shaping、bidi 和边界上下文。不能先按字符数切分字符串，再把各片段当成独立段落排版；列宽和续排位置变化也需要重新判断受影响的行。
- 行内对象仍由稳定 InlineKey 关联到逻辑内容。UI 适配在同一逻辑宿主下保留子组件，把跨栏变化体现为 placement 变化；不能因为组件移到另一栏就改变其挂载父级并丢失 State。一个原子对象不因区域分片而重复挂载或同时放置多次。

首期设计的 ParagraphHost 继续服务普通段落。区域排版进入实施后增加相应的公共 UI 适配，复用子项、基线和命中协议。当前的每帧一次测量、至多一次放置不自动变成跨区域反复测量；如果对象自然尺寸要依赖不同区域的宽度，需要在该阶段明确测量策略并同步 UI 规范。

### 8.3 后端边界与后续批次

已核对的固定版本 SkParagraph 公共入口是 layout(width)，提供行、glyph、placeholder 与选择几何查询，但没有直接接收区域链和排除形状的布局入口，见[第 9 节的 Paragraph 源码](#9-调研来源)。等宽矩形分栏可以先评估复用整段排好的行并组织续排；变宽区域和跨多行绕排需要增强排行能力或引入适合区域排版的后端。只平移、裁剪同一份固定宽度 TextLayout 不能完成这类重新断行。现有字体、HarfBuzz／Unicode 处理和 Canvas 绘制仍可作为基础。

| 后续批次 | 目标 | 必要验证 |
| --- | --- | --- |
| 矩形区域续排 | 一份内容连续进入多栏，明确溢出与显示截断的区别 | 窗口宽高变化后的行序、内容无丢失／重复、跨栏 caret／selection 和 bidi |
| 绕排与变化的行宽 | 浮动图片或排除形状影响连续多行，结果可以包含多个行片段 | 不同图片尺寸和区域形状下的断行、基线、选择及命中几何 |
| UI 区域适配 | 文字、图片和真实行内控件共同进入区域布局 | 重排、跨栏与暂不可见期间保持 InlineKey／State；可见项的输入和焦点随成功帧更新 |

普通多栏和绕排先形成实际闭环，再按明确需求评估分页或更完整的文档排版；本次不增加分页器、编辑模型或通用文档框架。上述方向可以独立于 [GPU 与媒体增强](../../ui/DESIGN.md#13-后续增强gpu-互操作3d-与视频)推进。

## 9. 调研来源

- Flutter：[WidgetSpan](https://api.flutter.dev/flutter/widgets/WidgetSpan-class.html)、[子项测量与基线](https://api.flutter.dev/flutter/rendering/RenderInlineChildrenContainerDefaults/layoutInlineChildren.html)、[截断后的子项位置](https://api.flutter.dev/flutter/rendering/RenderInlineChildrenContainerDefaults/positionInlineChildren.html)。
- Jetpack Compose：[InlineTextContent](https://developer.android.com/reference/kotlin/androidx/compose/foundation/text/InlineTextContent)。
- Apple TextKit：[NSTextAttachment](https://developer.apple.com/documentation/uikit/nstextattachment)。
- WPF：[Flow document 与 InlineUIContainer](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/flow-document-overview)。
- Skia 固定版本：[ParagraphBuilder](https://github.com/google/skia/blob/f326a1e25fc736704063922b9ce76b527b1c5efe/modules/skparagraph/include/ParagraphBuilder.h)、[PlaceholderStyle](https://github.com/google/skia/blob/f326a1e25fc736704063922b9ce76b527b1c5efe/modules/skparagraph/include/TextStyle.h)、[Paragraph 查询](https://github.com/google/skia/blob/f326a1e25fc736704063922b9ce76b527b1c5efe/modules/skparagraph/include/Paragraph.h)。
