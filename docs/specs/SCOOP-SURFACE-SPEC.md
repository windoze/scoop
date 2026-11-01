# Scoop Surface 基础库规范

状态：第一期共享基础层及 macOS 后端已实现。更新：2026-11-02。本文件规定 window 输入前置、graphics 绘图／文本、Surface 宿主、动画和区域输入的当前合同；各项验证、执行环境和未测边界见 [实施记录](../surface/PROGRESS.md)。架构、平台接入和实施顺序见 [Surface 设计](../surface/DESIGN.md)。

## 1. 范围与依赖

第一期提供基本 2D 绘图与 Path、动画、文本布局、指定区域的输入响应。它们是后续 component 的基础；本期不提供组件树、声明式 DSL、通用布局容器、完整文本编辑器或手势识别系统。

基础 Cone 统一使用 `scoop.surface` 名称，类型为 SurfaceHost／SurfaceWindow／SurfaceWindowState，原生入口以 `scoop_surface_` 为前缀，不提供旧名称的兼容入口。上层由 [`scoop.ui` 自绘框架](SCOOP-UI-SPEC.md)和独立的 [`scoop.ui.component` 控件库](SCOOP-UI-COMPONENT-SPEC.md)组成；框架及公共文本适配已在 macOS aarch64 交付，控件继续实施。本期新增独立[公共样式库](SCOOP-UI-STYLE-SPEC.md)，Surface 不依赖或识别这些上层类型。

字体和 Paragraph／TextLayout 已迁入独立 [`scoop.graphics.text`](SCOOP-GRAPHICS-TEXT-SPEC.md)，内容输入统一为 StyledText／SpanStyle，并提供行内对象的测量输入和位置结果。Surface 的帧、动画、输入区域与 Rect caret 不反向依赖 graphics.text 或 UI；使用文本的示例和测试直接声明依赖。

- `scoop.graphics` 是普通 Cone，提供几何、Canvas、Path、Image 及绘图目标，允许离屏使用，不依赖 `scoop.window`。绘图由 Skia 实现，文本由独立 graphics.text 接入。
- `scoop.surface` 是普通 Cone，依赖 graphics、window 及实际使用的 core／std／math／platform，提供 SurfaceHost、SurfaceWindow、帧调度、动画、InputArea 与区域焦点。两库的几何与动画所需数学函数复用 scoop.math。
- window 继续拥有原生窗口、输入归一、IME 协议、事件循环和 native lease；Surface 的平台适配拥有 surface 与呈现。共享层不依赖 AppKit、Metal 或其他窗口／GPU API。

当前实施目标为 `aarch64-apple-darwin`、`x86_64-apple-darwin`，以及 Windows x64 的 `x86_64-pc-windows-msvc-ucrt`、`x86_64-w64-windows-llvm-mingw-ucrt`、`x86_64-w64-windows-llvm-mingw-msvcrt`。Windows 阶段实现同一共享合同，Linux 留待后续；按 target 选择源码和原生依赖，不复制共享状态机，不为未实施平台加入空后端。采用现有语言、FFI、native 编译及资源释放规则，不新增编译器或 runtime 的 Surface 特殊通道。

## 2. 几何与绘制

公开几何采用有限 Double、内容区左上原点、向右／向下递增。Size 与 Rect 的尺寸非负；矩形端点及 native scalar 转换发生溢出时拒绝该操作。Canvas 的用户坐标为逻辑单位；DPI 到设备像素的转换只在渲染目标根部施加一次。桌面坐标不参与此转换。

Canvas 提供 save／restore、二维仿射变换、矩形与 Path clip、基本形状、Path、Image、文本布局绘制及组透明度。组透明度按离屏合成的整体结果应用，不能替换为各绘图操作分别乘 alpha。第一期颜色与窗口输出采用 sRGB、预乘 alpha；不承诺 HDR 或跨 GPU 的逐像素一致结果。

PathBuilder 构造 move／line／quadratic／cubic／arc／close，build 产生不可变 Path；包含 non-zero／even-odd 填充规则、bounds、contains 和路径长度／位置查询。Paint 描述填充或描边、颜色／基础渐变、线宽与端点／连接样式。已交付的 Path、Image 和 Paint 描述不得因调用方后续修改 builder 而改变。

Canvas 是一次绘制作用域内的借用对象，不得跨帧保存或在其他线程调用；作用域结束后使用报状态异常。变换和 clip 的当前几何状态由 graphics 维护，并经低层接入接口提供给 Surface。绘制与区域放置不得分别维护两套可能不一致的变换。

`Canvas.withState<T>(block: (Canvas) -> T): T` 创建同步嵌套借用，继承当前坐标、clip 和组状态，不重复施加 DPI。block 获得独立有效期的子 Canvas；期间只能使用子 Canvas，父 Canvas 的操作及几何捕获报告状态错误。正常返回或抛错均恢复入口状态和保存层级，包括未配对的子级 save／saveLayer；子 Canvas 不能 restore 到入口以下，退出后拒绝操作。已绘制像素不回滚，异常帧是否提交由外层目标决定。

`scoop.graphics.native` 提供当前 Surface renderer 实际需要的公开 Unsafe 接入：原生渲染目标、受控的 Canvas 借用、提交／中止，以及几何状态快照。Skia C++ 类型不进入普通公共 API；Metal 等句柄只出现在按目标选择的 native 子包。跨 Cone 调用不能依赖另一 Cone 的 internal 构造器。

独立文本库使用同一 package 的 `@Unsafe fun <T> Canvas.withSkiaCanvas(block: (Ptr<Unit>) -> T): T`。它在当前有效 Canvas 的 owner 线程同步借出实际 SkCanvas 指针，调用前保存原生画布状态，正常返回或异常退出都恢复原状态；不施加第二次 DPI 变换。指针只在 block 内有效，不能保存、释放、跨线程或挂起使用。block 只通过该指针进行 Skia 绘制，不重入同一 Canvas 的 Scoop 操作或几何捕获，不恢复到借用入口保存层以下，不自行提交目标或变更 GPU context。调用方承担这项 Unsafe 约束；graphics 的私有包装、资源析构和 clip 记录不向另一 Cone 开放。

SkSL／SkRuntimeEffect 的公开接入列为后续 graphics 增强，见[SkSL 设计](../graphics/sksl/DESIGN.md)。本期先按[图像滤镜与图层规范](SCOOP-GRAPHICS-EFFECTS-SPEC.md)补齐 Skia Gaussian blur、alpha 阴影、混合与图层滤镜，供离屏和公共 UI style 消费；这批已完成设计、待实施。RuntimeEffect／shader 填充仍未接入，不作为基础效果前置。后续实现通过既有 Skia 后端与普通 FFI 扩展，不隐式创建时间驱动器，也不改变 Canvas 借用或第 4.1 节的按需停帧规则。

## 3. 字体与文本布局

字体、Paragraph／TextLayout、多样式 run、位置查询和绘制的合同统一见 [文本排版规范](SCOOP-GRAPHICS-TEXT-SPEC.md)。公共类型及 Canvas.drawText 扩展位于 scoop.graphics.text；graphics 本身不引用 TextLayout，也不负责其原生释放。

Surface 只接收局部 Rect caret，并通过区域几何转换为窗口 IME 候选窗位置。文本编辑状态由应用拥有，应用在 graphics.text 的 TextRange 与 window 的 Utf8Range 之间显式转换；输入路由不保留段落或完整文档，也不接管撤销、剪贴板或 TextField。

## 4. Surface 宿主与帧

SurfaceHost 包装 ApplicationHandler，管理所附着的 SurfaceWindow，使用现有 Application.run 或 runWithAsyncio。窗口事件、用户绘制、动画采样和区域输入均在窗口 owner 线程串行执行；不启动第二个主循环或另一套任务树。CloseRequested 的接受策略仍由应用决定。

SurfaceHost 必须在 Closing／Exiting 及用户回调失败时关闭相关 Surface 资源；不能要求应用额外转发清理事件才能释放 lease。绘制和输入回调不挂起、不在内部锁中运行，也不从原生通知递归进入。

`SurfaceContent : Closeable` 定义同步 `paint(FrameContext)`，表示一个由 SurfaceWindow 拥有的内容根。`SurfaceHost.attach(window, content)` 在通过 owner 线程检查后接收新 content 的所有权；窗口状态、重复附着、renderer 创建或配置失败时，也负责关闭 content 并回滚本次取得的资源。错误线程调用在转移所有权前拒绝。一个 content 只能交给一个窗口，close 必须幂等。原有 painter 闭包重载通过不拥有额外资源的 SurfaceContent 适配，继续服务直接绘图。

关闭覆盖 SurfaceHost 的事件处理器、内容 paint 和输入回调。回调内部请求关闭时先进入 Closing 并停止新工作，当前回调栈及 Canvas／frame 借用退出后才关闭 content；随后释放 renderer 和 lease。最外层 `SurfaceHost.use` 因而同时覆盖内容根、原生窗口关闭和异常退出。attach 尚未暴露给调用方的失败结果立即回滚，不等待另一个原生事件。

公共 style 和滤镜没有独立时钟；参数动画复用 Surface Animatable，静态阴影／光晕／模糊不增加周期任务或帧需求。隐含的背景采样不属于普通内容滤镜；Backdrop blur 留待后续。

每个 SurfaceWindow 合并重绘请求。只有 dirty 或活动动画需要新帧时才向平台请求帧机会；普通空闲保持 Wait。requestRedraw、WaitUntil 与 prePresentNotify 均不代表 vsync。

平台适配接收窗口配置与帧需求，通知下一次可尝试绘制的机会，并返回 Ready(Frame) 或 Deferred。Deferred 是正常暂不可绘制状态；保留 dirty，等待后续原生机会或窗口状态改变，不忙重试。隐藏、遮挡、零尺寸、待配置及关闭状态不得申请或提交无效帧。

帧开始使用 MonotonicTimeSource 的 TimeMark，一帧内所有动画消费同一个采样时间。平台的预测呈现时间留在呈现层，不冒充输入时间或通用 FrameClock 时间。

`FrameContext.withState<T>(block: (FrameContext) -> T): T` 沿用 Canvas.withState，为子绘制提供拥有相同窗口、尺寸、时间及本帧输入收集器的 FrameContext。子 context 的 place 使用其子 Canvas 的当前几何；子作用域结束只恢复绘制状态，输入 placement 仍随整个帧成功提交后发布。

一帧依次完成：取得可用目标、采样动画、执行必要文本布局及绘制、建立 Area placement、提交渲染、发布本帧命中几何。失败／中止不发布半帧几何；中止必须释放已取得的目标。帧内新增的失效请求保留到下一帧。

原生窗口在绘制回调中已改变尺寸或 backing scale 时，旧目标必须在提交前中止，即使对应 GeometryChanged 尚未进入业务事件处理器。平台 renderer 核对实际目标所依赖的原生尺寸与 scale；不把排队的 WindowMetrics 当成原生窗口尚未变化的证明，也不要求窗口请求同步递归派发事件。

发布的几何对应最近成功提交的画面，不保证 GPU 已经显示该画面。首期不为输入同步而等待 GPU 或显示扫描完成。平台提交接受之后的 GPU 失败作为渲染错误处理。

### 4.1 空闲帧需求与原生等待

按需停帧是 Surface 的基本合同。最后的必要帧结束后，若没有新的 dirty、运行中动画或待处理完成回调，必须撤销该窗口的原生帧需求。平台适配暂停或解除帧通知，不能让 display link、帧等待回调、固定 timer 或代理投递链持续运行，只在回调内部跳过绘制。

静止期间不主动 acquire 绘制目标、调用 painter、提交 GPU 命令或重复 present。同一画面由平台保留；系统曝光、尺寸／DPI 变化等真实重绘请求仍可以安排必要帧，完成后再次停止。已经提交的 GPU 工作和有限的在途通知允许完成，但不得自行续订下一轮需求；停帧不要求同步 drain GPU，也不要求销毁可复用的图形资源。

每个窗口独立管理需求。只要 UI 没有工作，就不因另一窗口的动画、无关 I/O 通知或主循环再次进入 AboutToWait 而绘制它。Surface 不设置 Poll 或循环的短 WaitUntil 来等待绘制；应用存在其他任务时继续复用既有主循环等待策略。

隐藏、完全遮挡、零尺寸及其他暂不可呈现状态停止无效帧需求，保留 dirty 等待真实的帧机会或配置／可见性恢复，不忙轮询 acquire。原生恢复和新失效能重新请求工作；停止需求与通知交付的交错不能丢失有效请求，也不能形成永不停止的重绘链。

macOS 的需求撤销对应暂停 CAMetalDisplayLink；Windows 对应解除 DXGI frame-latency 的原生等待及不再需要的排队通知。未来移动端的前后台／可见性接入也必须停止相应帧订阅。上层 UI 队列、组件与布局的静止要求另见 [UI 规范第 6.1 节](SCOOP-UI-SPEC.md#61-静止时停止渲染工作)。

## 5. 动画

AnimationSpec 描述 tween／easing 或 spring；Animatable 持有当前值、目标、运行状态，以及 spring 所需速度。首批支持 Double、Point、Size、Color，复用实际需要的插值／向量转换协议。

动画按单调时间与起点求值，不用固定帧数或连续累加固定 dt 推进；跳帧直接采样当前进度。retarget 从当前时间对应的值开始；spring 同时保留适用的当前速度。提供启动／改目标、取消、暂停／恢复及有限或重复播放控制；零时长立即到达目标。

AnimationTimeline 接受一个 TimeSource 和失效回调，SurfaceWindow 使用同一个单调源并在真实帧内采样；离屏调用方可用 advanceTo 明确驱动。一个采样作用域内的启动、改目标和完成回调共用当前帧时间。完成回调在本轮全部动画采样结束后派发，新启动的动画进入下一轮；重入 advanceTo 拒绝。时间源必须一致，时间不得倒退。

Tween 支持线性与 cubic Bézier easing。Spring 采用质量为 1 的阻尼弹簧解析解，分别处理欠阻尼、临界阻尼和过阻尼；stiffness、dampingRatio、位移／速度阈值均为有限正数。依据解的衰减包络确定收敛时间，到达时精确落到目标并清零速度，不依赖采样间隔决定结束时机。计算无法用有限 Double 表达时报参数错误。

Repeat 指定有限正次数或持续重复。Restart 回到起点开始下一段；Reverse 在相邻段交换起终点，每段使用相同 spec。重复段从静止开始，首段可继承改目标前的 spring 速度；零总时长不允许重复。reverse() 从当前值向上一段的起点启动新的完整区段，保留可用速度。Size 和 Color 的转换将插值越界分别收敛到非负尺寸和 0..1 分量，Double／Point 不截断有限值。

SurfaceWindow 不可绘制时停止请求无效帧，但默认时间轴继续推进，恢复后按当前时间采样。只有显式暂停才冻结该动画的时间轴。取消或所属 Surface 关闭后不再产生完成业务回调；完成处理不递归启动同一轮采样。

本期不引入可观察属性系统。应用明确更新状态／请求帧，动画仅失效实际变化所需的绘制或文本布局。

## 6. 区域、输入与焦点

InputArea 是 SurfaceWindow 内的稳定对象，持有事件处理器、启用状态和焦点能力。每帧放置时提供命中 shape 与可选父区域，记录 Canvas 当前变换、有效 clip 及放置顺序；同一区域一帧最多放置一次，父区域须先放置。placement 随成功提交整体发布。

父子关系用于事件路径与局部坐标，不隐式裁剪子区域；裁剪只由已记录的 clip 决定。enabled = false 使该区域及后代退出交互；忽略自身命中仍允许子区域。命中按可接收输入区域的逆放置顺序选择目标；不可逆变换不命中。shape 可以独立于可见颜色或 alpha，不做像素颜色拾取。

事件先给目标，再沿固定父路径冒泡，Handled 停止传播；不会因此交给下方重叠兄弟。忽略自身命中不等于禁止子区域。回调可改变应用状态，但当前派发路径不因变化重算；已注销处理器不继续接收业务事件，取消／离开通知在安全的派发边界处理。

Pointer 的普通事件以及 Keyboard／ModifiersChanged 使用上述冒泡规则。Enter／Leave、FocusChanged 和 TextInput 直接交付所属区域；Cancel 是序列终结通知，交付整条旧路径，不因 Handled 遗漏祖先的收尾。close 或禁用发生在处理器内时，当前业务派发跳过已关闭／禁用的接收者，退出派发后再交付必要的 Cancel／Leave／失焦通知。Leave 与 Cancel 只有时间及终结原因，不伪造已经失效的局部坐标。

第一期沿用 window 的鼠标与滚轮模型，不虚构 touch contact。首次按钮按下固定该次鼠标按钮序列的目标与路径，未命中则固定空路径，直到最后一个已按按钮释放或收到取消；中途移动／抬起继续送到该路径，使用最新有效 placement 换算局部坐标。区域移除／禁用、窗口失焦或捕获丢失结束序列，交付 Cancel，不能合成可能触发点击的正常 Up。

hover 根据当前位置和已发布几何单独计算；鼠标不动而区域随动画移动时也须更新。窗口 PointerEvent.Left 不等于拖动取消。PixelDelta 按该次输入携带的 scale 转为逻辑距离，LineDelta 保留单位，不能假定固定行高。

Surface 维护逻辑 focusedArea；键盘、修饰键与 TextInputBatch 路由给当前活动焦点。OS 失焦使其暂不活动并结束按下／组合状态，重新获得焦点时可恢复仍有效的区域；移除／禁用则清除逻辑焦点。文字插入只来自 TextInputBatch，不能同时从 LogicalKey.Character 插入。

requestFocus 只对已发布、可交互且允许焦点的区域成功；文本区域自动允许焦点。它只改变 Surface 内逻辑焦点，不强制抢占 OS 窗口焦点。每个区域的 caret 保存为局部 Rect 值，应用可直接传入 TextLayout.caret 的结果，不让路由层保留段落或编辑文档。区域因移除而取消的按钮序列在全部原生按钮释放前保持空路径，不能让剩余 Move／Up 或追加按钮突然命中其他区域。

区域本帧不再放置时，在成功发布该帧几何后按区域缺席处理，退出命中并执行所需取消／失焦；失败帧继续保留最近已提交几何。区域对象本身不因此关闭。UI 可用这条既有放置规则处理被省略号隐藏的行内控件，无需给 Surface 增加文本对象或组件生命周期。

更换文本接收者先关闭旧接收者的 IME，以 Disabled 为旧会话结束边界，再设置新候选窗位置并开启新接收者；没有活动旧接收者时直接开启。Enabled 先于新接收者的 Edit。结束前的 Edit 仍路由旧接收者，已注销则丢弃，不得写进新区域。逻辑 focus 可以先改变，排队编辑的归属不能随之被改写；窗口关闭的资源释放不等待业务 IME 通知。

文本接收区域的 caret 经 TextLayout、区域变换转换为窗口逻辑矩形后调用 setImeCursorArea；使用轴对齐包围矩形，随布局、变换或窗口配置变化更新。编辑器仍拥有文字、selection 和 marked 状态。

## 7. 原生资源与错误

GPU context、窗口 surface、字体／图像／Path／文本的原生持有对象使用显式 Closeable／use 生命周期。托管包装的 GC release 只允许既有 runtime 合同许可的叶子释放；不借它调用 UI、等待 GPU、注销宿主回调或投递托管闭包。

graphics 与 graphics.text 的原生持有对象在创建线程串行使用和关闭；Surface 资源在窗口 owner，离屏资源不要求主线程。纯值可复制传递；结果逻辑上不可变不构成 Skia 内部缓存并发访问的承诺。

native 薄层复制跨调用存活的数据，或取得明确的原生引用；不能保存移动 GC 对象的裸地址。回调使用已有 ForeignCallback 生命周期。C++／Objective-C 异常与用户异常均不得跨 C ABI 展开。

关闭顺序为停止帧通知并退休回调、结束输入／动画、等待当前同步回调和帧借用退出、关闭 SurfaceContent、结束实际 GPU 使用、释放 surface／context 及平台引用、关闭 NativeWindowLease。已停止的 ApplicationProxy 不能成为资源释放的唯一途径。content.close 失败仍释放 renderer／lease，继续关闭其他窗口内容；清理失败遵循 ResourceUseException，不依赖后续一次事件或应用再调用 close 才完成收尾。

在当前业务回调或帧借用期间请求关闭时，先进入 Closing、停止新工作，待当前回调退出并中止帧后完成实际释放；owner 不得等待自身尚未退出的回调，也不得在 Canvas 仍借用时销毁其 surface。

参数／作用域错误使用现有参数／状态异常。Skia、GPU 或呈现失败以 GraphicsException 保留 operation、后端及可得的原生错误；窗口取得与操作失败保留 WindowException。第一期不自动换 renderer 或重建丢失的 GPU device。

## 8. macOS 实现与交付

macOS 使用 AppKit lease 中的 NSView 安装 CAMetalLayer，保持原有输入 view 和 NSTextInputClient；图形后端为 Skia Ganesh Metal。lease 的 NSView 是输入／呈现宿主，可以位于原 contentView 内；renderer 不假定两者相同，不改变原视图层级或 first responder。卸载恢复原有 layer 与 wantsLayer 设置；原 layer 为空时，允许 AppKit 随视图层级自动创建 backing layer，但不得保留 Surface 安装的 CAMetalLayer。通过 CAMetalDisplayLink 接入现有主线程 common modes，按需提供 drawable 与帧机会。主线程外的 GPU 完成工作仅释放其原生持有数据，不调用 Surface 业务。

采用 BGRA8、sRGB 与一致的预乘 alpha 配置，按 WindowMetrics 同步 contentsScale 和 drawableSize，以 top-left surface origin 绘制。Skia flush／submit 与 present 使用有序的 Metal command queue；CAMetalDisplayLink 的 drawable 使用普通 presentDrawable，由 display link 决定呈现节奏，不调用 atTime 或 afterMinimumDuration 变体。普通帧不 waitUntilCompleted；关闭时排空实际资源使用。

系统字体由 Skia 的 CoreText font manager 提供，段落算法和 HarfBuzz shaping 继续使用共享实现。Objective-C `.m` 负责 AppKit／QuartzCore／Metal 对象，C++ `.cpp` 负责 Skia；经 C ABI 的不透明句柄连接，不要求 Cone 新增 `.mm` 支持。

支持基线沿用 [发行配置](../../scripts/release.toml)。当前配置为 macOS 27.0.1／SDK 27.0，满足 CAMetalDisplayLink 的 macOS 14 API 要求；本设计不据此宣称支持 macOS 14，也不新增旧系统定时器后备实现。

Skia 与其 HarfBuzz／ICU 等依赖固定配套输入，由维护脚本显式准备为目标原生资源；普通应用使用预编译 Cone 和声明的链接依赖，不自动联网、下载或构建依赖。库行为测试归所属 Cone；实际 Metal 呈现、IME、拖动、混合 DPI 与主循环收尾另行验证并记录，不能用离屏测试代替。

## 9. Windows 实现与交付

Windows 使用现有 Win32 NativeWindowLease 的 HWND；Skia Ganesh Direct3D 12 负责绘制，DXGI flip swapchain 负责呈现，系统字体由 Skia DirectWrite font manager 提供。文本布局、UTF 索引、动画、区域输入和焦点继续使用共享实现。graphics 暴露目标专用 Unsafe `scoop.graphics.native.windows.D3DContext`，只接受同一设备、direct queue 和 BGRA8 resource；C++ 对象及 Skia ABI 不跨 Cone。

Surface 在 owner 上创建并使用 GPU context、swapchain 和呈现资源；不替换窗口、窗口过程、输入接收者或 IME context。目标使用 BGRA8、sRGB、top-left origin，与窗口透明策略一致的预乘 alpha；WindowMetrics 的物理尺寸及 scale 决定唯一的 DPI 根变换。DirectComposition 只承担需要的窗口合成，不成为共享层的组件或图层系统。

帧机会来自 DXGI frame-latency waitable object，按需求单次等待并合并 owner 通知。原生后台等待只投递通知，不读取托管 Surface 状态、不执行绘制。通知复用现有 Win32 消息循环，普通 run、asyncio 和系统嵌套循环遵守相同 owner 边界；不使用固定周期 timer、忙轮询或普通帧的 CPU fence 等待模拟帧时钟。隐藏、最小化和零尺寸停止帧需求；恢复后保留的 dirty 重新取帧。

Skia 对目标的提交必须完成 D3D12 resource state 到 PRESENT 的转换，再在同一 direct queue 的有序工作之后 Present。过期尺寸、暂不可呈现或未就绪返回 Deferred；设备移除及其他确定的 HRESULT 失败报告 GraphicsException，不能静默重试或切换 renderer。resize 退休旧 backbuffer，并在其实际 GPU 使用结束后重新配置；普通提交保持异步，显式读回、resize 和关闭才同步相关 GPU 使用。

关闭先解除原生等待和消息通知，结束正在使用的 frame，再排空相关 GPU 工作、卸载自己的合成目标、释放 swapchain／context，最后关闭 lease。已排队通知不能访问已释放 surface；关闭不依赖 ApplicationProxy 再执行业务闭包。三个目标分别构建相容的 C++／CRT 资源，并用所属 Cone 的 scoop.test 运行离屏、真实 D3D12、窗口、IME 与收尾测试；Linux 不进入本阶段的发布或验收。
