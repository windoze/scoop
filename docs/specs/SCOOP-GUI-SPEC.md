# Scoop GUI 基础库规范

状态：实施中。日期：2026-10-31。本文件规定第一期共享基础层及 macOS 后端的目标合同，window 输入前置能力已实现，graphics／gui 的交付状态见 [实施记录](../gui/PROGRESS.md)。架构、平台接入和实施顺序见 [GUI 设计](../gui/DESIGN.md)。

## 1. 范围与依赖

第一期提供基本 2D 绘图与 Path、动画、文本布局、指定区域的输入响应。它们是后续 component 的基础；本期不提供组件树、声明式 DSL、通用布局容器、完整文本编辑器或手势识别系统。

- `scoop.graphics` 是普通 Cone，提供几何、Canvas、Path、Image、字体和文本布局，允许离屏使用，不依赖 `scoop.window`。绘图由 Skia 实现；文本采用 SkParagraph、HarfBuzz 和 SkUnicode 的 ICU 后端。
- `scoop.gui` 是普通 Cone，依赖 graphics、window 及实际使用的 core／std／math／platform，提供 GuiHost、GuiWindow、帧调度、动画、InputArea 与区域焦点。两库的几何与动画所需数学函数复用 scoop.math。
- window 继续拥有原生窗口、输入归一、IME 协议、事件循环和 native lease；GUI 的平台适配拥有 surface 与呈现。共享层不依赖 AppKit、Metal 或其他窗口／GPU API。

当前实施目标为 `aarch64-apple-darwin` 与 `x86_64-apple-darwin`。后续平台实现同一共享合同、按 target 选择自己的源码和原生依赖，不复制共享状态机，不为未实施平台加入空后端。采用现有语言、FFI、native 编译及资源释放规则，不新增编译器或 runtime 的 GUI 特殊通道。

## 2. 几何与绘制

公开几何采用有限 Double、内容区左上原点、向右／向下递增。Size 与 Rect 的尺寸非负；矩形端点及 native scalar 转换发生溢出时拒绝该操作。Canvas 的用户坐标为逻辑单位；DPI 到设备像素的转换只在渲染目标根部施加一次。桌面坐标不参与此转换。

Canvas 提供 save／restore、二维仿射变换、矩形与 Path clip、基本形状、Path、Image、文本布局绘制及组透明度。组透明度按离屏合成的整体结果应用，不能替换为各绘图操作分别乘 alpha。第一期颜色与窗口输出采用 sRGB、预乘 alpha；不承诺 HDR 或跨 GPU 的逐像素一致结果。

PathBuilder 构造 move／line／quadratic／cubic／arc／close，build 产生不可变 Path；包含 non-zero／even-odd 填充规则、bounds、contains 和路径长度／位置查询。Paint 描述填充或描边、颜色／基础渐变、线宽与端点／连接样式。已交付的 Path、Image 和 Paint 描述不得因调用方后续修改 builder 而改变。

Canvas 是一次绘制作用域内的借用对象，不得跨帧保存或在其他线程调用；作用域结束后使用报状态异常。变换和 clip 的当前几何状态由 graphics 维护，并经低层接入接口提供给 GUI。绘制与区域放置不得分别维护两套可能不一致的变换。

`scoop.graphics.native` 提供当前 GUI renderer 实际需要的公开 Unsafe 接入：原生渲染目标、受控的 Canvas 借用、提交／中止，以及几何状态快照。Skia C++ 类型不进入普通公共 API；Metal 等句柄只出现在按目标选择的 native 子包。跨 Cone 调用不能依赖另一 Cone 的 internal 构造器。

## 3. 字体与文本布局

FontCollection 管理显式字体及系统 fallback。Paragraph 保存自有文字、样式 run、段落设置和字体来源；`layout(width)` 产生完整 TextLayout，供 paint、命中、caret 和 selection 查询共同使用。宽度采用有限非负逻辑值；另提供明确的无换行布局请求，不用无穷浮点值表示它。

TextLayout 保存所属文字及布局所需资源，包含尺寸、基线、行范围、可见／截断范围、位置命中、caret 与 range 对应的矩形集合。双向文本的一个逻辑范围可以产生多个矩形。重新布局不得改变已交付的旧 TextLayout；native 内部对象可变时必须独立持有或复制结果。

文本位置采用 UTF-8 字节偏移，计数类型为 Long；范围为半开区间，端点位于 Unicode 标量边界。TextPosition 另含 affinity，区分软换行和双向边界处的视觉位置。GUI 在 graphics 文本位置与 window 的 Utf8Range 之间显式转换；graphics 不为复用窗口类型而依赖 window。

UTF-8／UTF-16 索引映射随文字构建并复用。标量、grapheme、shaping cluster 与 glyph 不得视为同一概念；用户 caret 不落在 grapheme 内部，利用 cluster／glyph 关系与 affinity 确定视觉位置，不能用 glyph 边界代替文本位置。shaping 与绘制使用同一字体、fallback 结果及 variation 设置。

第一期包括基本富文本 run、CJK／RTL／emoji、换行、对齐、行高、maxLines、ellipsis、测量与位置查询。HarfBuzz 负责 shaping；双向分析、分段、断行及段落组合由 SkUnicode／SkParagraph 处理。仅变换或整体透明度变化复用 TextLayout；文字、字体、字号或布局约束变化重新布局。

文本编辑状态与文本布局分离。GUI 可将窗口 TextInputBatch 路由给当前文本接收区域，并换算候选窗矩形；本期不增加完整 surrounding 文档桥接、撤销历史、剪贴板协议或 TextField。

## 4. GUI 宿主与帧

GuiHost 包装 ApplicationHandler，管理所附着的 GuiWindow，使用现有 Application.run 或 runWithAsyncio。窗口事件、用户绘制、动画采样和区域输入均在窗口 owner 线程串行执行；不启动第二个主循环或另一套任务树。CloseRequested 的接受策略仍由应用决定。

GuiHost 必须在 Closing／Exiting 及用户回调失败时关闭相关 GUI 资源；不能要求应用额外转发清理事件才能释放 lease。绘制和输入回调不挂起、不在内部锁中运行，也不从原生通知递归进入。

每个 GuiWindow 合并重绘请求。只有 dirty 或活动动画需要新帧时才向平台请求帧机会；普通空闲保持 Wait。requestRedraw、WaitUntil 与 prePresentNotify 均不代表 vsync。

平台适配接收窗口配置与帧需求，通知下一次可尝试绘制的机会，并返回 Ready(Frame) 或 Deferred。Deferred 是正常暂不可绘制状态；保留 dirty，等待后续原生机会或窗口状态改变，不忙重试。隐藏、遮挡、零尺寸、待配置及关闭状态不得申请或提交无效帧。

帧开始使用 MonotonicTimeSource 的 TimeMark，一帧内所有动画消费同一个采样时间。平台的预测呈现时间留在呈现层，不冒充输入时间或通用 FrameClock 时间。

一帧依次完成：取得可用目标、采样动画、执行必要文本布局及绘制、建立 Area placement、提交渲染、发布本帧命中几何。失败／中止不发布半帧几何；中止必须释放已取得的目标。帧内新增的失效请求保留到下一帧。

发布的几何对应最近成功提交的画面，不保证 GPU 已经显示该画面。首期不为输入同步而等待 GPU 或显示扫描完成。平台提交接受之后的 GPU 失败作为渲染错误处理。

## 5. 动画

AnimationSpec 描述 tween／easing 或 spring；Animatable 持有当前值、目标、运行状态，以及 spring 所需速度。首批支持 Double、Point、Size、Color，复用实际需要的插值／向量转换协议。

动画按单调时间与起点求值，不用固定帧数或连续累加固定 dt 推进；跳帧直接采样当前进度。retarget 从当前时间对应的值开始；spring 同时保留适用的当前速度。提供启动／改目标、取消、暂停／恢复及有限或重复播放控制；零时长立即到达目标。

GuiWindow 不可绘制时停止请求无效帧，但默认时间轴继续推进，恢复后按当前时间采样。只有显式暂停才冻结该动画的时间轴。取消或所属 GUI 关闭后不再产生完成业务回调；完成处理不递归启动同一轮采样。

本期不引入可观察属性系统。应用明确更新状态／请求帧，动画仅失效实际变化所需的绘制或文本布局。

## 6. 区域、输入与焦点

InputArea 是 GuiWindow 内的稳定对象，持有事件处理器、启用状态和焦点能力。每帧放置时提供命中 shape 与可选父区域，记录 Canvas 当前变换、有效 clip 及放置顺序；同一区域一帧最多放置一次，父区域须先放置。placement 随成功提交整体发布。

父子关系用于事件路径与局部坐标，不隐式裁剪子区域；裁剪只由已记录的 clip 决定。enabled = false 使该区域及后代退出交互；忽略自身命中仍允许子区域。命中按可接收输入区域的逆放置顺序选择目标；不可逆变换不命中。shape 可以独立于可见颜色或 alpha，不做像素颜色拾取。

事件先给目标，再沿固定父路径冒泡，Handled 停止传播；不会因此交给下方重叠兄弟。忽略自身命中不等于禁止子区域。回调可改变应用状态，但当前派发路径不因变化重算；已注销处理器不继续接收业务事件，取消／离开通知在安全的派发边界处理。

第一期沿用 window 的鼠标与滚轮模型，不虚构 touch contact。首次按钮按下固定该次鼠标按钮序列的目标与路径，未命中则固定空路径，直到最后一个已按按钮释放或收到取消；中途移动／抬起继续送到该路径，使用最新有效 placement 换算局部坐标。区域移除／禁用、窗口失焦或捕获丢失结束序列，交付 Cancel，不能合成可能触发点击的正常 Up。

hover 根据当前位置和已发布几何单独计算；鼠标不动而区域随动画移动时也须更新。窗口 PointerEvent.Left 不等于拖动取消。PixelDelta 按该次输入携带的 scale 转为逻辑距离，LineDelta 保留单位，不能假定固定行高。

GUI 维护逻辑 focusedArea；键盘、修饰键与 TextInputBatch 路由给当前活动焦点。OS 失焦使其暂不活动并结束按下／组合状态，重新获得焦点时可恢复仍有效的区域；移除／禁用则清除逻辑焦点。文字插入只来自 TextInputBatch，不能同时从 LogicalKey.Character 插入。

更换文本接收者先关闭旧接收者的 IME，以 Disabled 为旧会话结束边界，再设置新候选窗位置并开启新接收者；没有活动旧接收者时直接开启。Enabled 先于新接收者的 Edit。结束前的 Edit 仍路由旧接收者，已注销则丢弃，不得写进新区域。逻辑 focus 可以先改变，排队编辑的归属不能随之被改写；窗口关闭的资源释放不等待业务 IME 通知。

文本接收区域的 caret 经 TextLayout、区域变换转换为窗口逻辑矩形后调用 setImeCursorArea；使用轴对齐包围矩形，随布局、变换或窗口配置变化更新。编辑器仍拥有文字、selection 和 marked 状态。

## 7. 原生资源与错误

GPU context、窗口 surface、字体／图像／Path／文本的原生持有对象使用显式 Closeable／use 生命周期。托管包装的 GC release 只允许既有 runtime 合同许可的叶子释放；不借它调用 UI、等待 GPU、注销宿主回调或投递托管闭包。

第一期 graphics 原生持有对象在创建线程串行使用和关闭；GUI 资源在窗口 owner，离屏资源不要求主线程。纯值可复制传递；结果逻辑上不可变不构成 Skia 内部缓存并发访问的承诺。

native 薄层复制跨调用存活的数据，或取得明确的原生引用；不能保存移动 GC 对象的裸地址。回调使用已有 ForeignCallback 生命周期。C++／Objective-C 异常与用户异常均不得跨 C ABI 展开。

关闭顺序为停止帧通知并退休回调、结束输入／动画、结束实际 GPU 使用、释放 surface／context 及平台引用、关闭 NativeWindowLease。已停止的 ApplicationProxy 不能成为资源释放的唯一途径。清理失败继续释放其他资源，遵循 ResourceUseException。

在当前业务回调或帧借用期间请求关闭时，先进入 Closing、停止新工作，待当前回调退出并中止帧后完成实际释放；owner 不得等待自身尚未退出的回调，也不得在 Canvas 仍借用时销毁其 surface。

参数／作用域错误使用现有参数／状态异常。Skia、GPU 或呈现失败以 GraphicsException 保留 operation、后端及可得的原生错误；窗口取得与操作失败保留 WindowException。第一期不自动换 renderer 或重建丢失的 GPU device。

## 8. macOS 实现与交付

macOS 使用 AppKit lease 中的 NSView 安装 CAMetalLayer，保持原有输入 view 和 NSTextInputClient；图形后端为 Skia Ganesh Metal。通过 CAMetalDisplayLink 接入现有主线程 common modes，按需提供 drawable 与帧机会。主线程外的 GPU 完成工作仅释放其原生持有数据，不调用 GUI 业务。

采用 BGRA8、sRGB 与一致的预乘 alpha 配置，按 WindowMetrics 同步 contentsScale 和 drawableSize，以 top-left surface origin 绘制。Skia flush／submit 与 present 使用有序的 Metal command queue，普通帧不 waitUntilCompleted；关闭时排空实际资源使用。

系统字体由 Skia 的 CoreText font manager 提供，段落算法和 HarfBuzz shaping 继续使用共享实现。Objective-C `.m` 负责 AppKit／QuartzCore／Metal 对象，C++ `.cpp` 负责 Skia；经 C ABI 的不透明句柄连接，不要求 Cone 新增 `.mm` 支持。

支持基线沿用 [发行配置](../../scripts/release.toml)。当前配置为 macOS 27.0.1／SDK 27.0，满足 CAMetalDisplayLink 的 macOS 14 API 要求；本设计不据此宣称支持 macOS 14，也不新增旧系统定时器后备实现。

Skia 与其 HarfBuzz／ICU 等依赖固定配套输入，由维护脚本显式准备为目标原生资源；普通应用使用预编译 Cone 和声明的链接依赖，不自动联网、下载或构建依赖。库行为测试归所属 Cone；实际 Metal 呈现、IME、拖动、混合 DPI 与主循环收尾另行验证并记录，不能用离屏测试代替。
