# Scoop Surface 基础库规范

后续共享 shortcut／caret 和输入序列归属的演进见 [控件基础设计](../ui/component-foundations/DESIGN.md)，按[控件基础实施记录](../ui/component-foundations/PROGRESS.md)逐批交付；下文描述对应批次的现行合同。

状态：第一期共享基础层及 macOS 后端已实现。更新：2026-11-03。本文件规定 window 输入前置、graphics 绘图／文本、Surface 宿主、动画和区域输入的当前合同；各项验证、执行环境和未测边界见 [实施记录](../surface/PROGRESS.md)。架构、平台接入和实施顺序见 [Surface 设计](../surface/DESIGN.md)。

## 1. 范围与依赖

第一期提供基本 2D 绘图与 Path、动画、文本布局、指定区域的输入响应。它们是后续 component 的基础；本期不提供组件树、声明式 DSL、通用布局容器、完整文本编辑器或手势识别系统。

基础 Cone 统一使用 `scoop.surface` 名称，类型为 SurfaceHost／SurfaceWindow／SurfaceWindowState，原生入口以 `scoop_surface_` 为前缀，不提供旧名称的兼容入口。上层由 [`scoop.ui` 自绘框架](SCOOP-UI-SPEC.md)和独立的 [`scoop.ui.component` 控件库](SCOOP-UI-COMPONENT-SPEC.md)组成；框架、公共文本适配、基础控件／容器、主题展示及后台下载组合已完成 macOS aarch64 实施和 x86_64 补验。本期新增独立[公共样式库](SCOOP-UI-STYLE-SPEC.md)及其[主题绘图／几何协议](SCOOP-UI-THEME-SPEC.md)，Surface 不依赖或识别这些上层类型。

字体和 Paragraph／TextLayout 已迁入独立 [`scoop.graphics.text`](SCOOP-GRAPHICS-TEXT-SPEC.md)，内容输入统一为 StyledText／SpanStyle，并提供行内对象的测量输入和位置结果。Surface 的帧、动画、输入区域与 Rect caret 不反向依赖 graphics.text 或 UI；使用文本的示例和测试直接声明依赖。

- `scoop.graphics` 是普通 Cone，提供几何、Canvas、Path、Image 及绘图目标，允许离屏使用，不依赖 `scoop.window`。绘图由 Skia 实现，文本由独立 graphics.text 接入。
- `scoop.surface` 是普通 Cone，依赖 graphics、window 及实际使用的 core／std／math／platform，提供 SurfaceHost、SurfaceWindow、帧调度、动画、InputArea 与区域焦点。两库的几何与动画所需数学函数复用 scoop.math。
- window 继续拥有原生窗口、输入归一、IME 协议、事件循环和 native lease；Surface 的平台适配拥有 surface 与呈现。共享层不依赖 AppKit、Metal 或其他窗口／GPU API。

当前实施目标为 `aarch64-apple-darwin`、`x86_64-apple-darwin`，以及 Windows x64 的 `x86_64-pc-windows-msvc-ucrt`、`x86_64-w64-windows-llvm-mingw-ucrt`、`x86_64-w64-windows-llvm-mingw-msvcrt`。Windows 阶段实现同一共享合同，Linux 留待后续；按 target 选择源码和原生依赖，不复制共享状态机，不为未实施平台加入空后端。采用现有语言、FFI、native 编译及资源释放规则，不新增编译器或 runtime 的 Surface 特殊通道。

## 2. 几何与绘制

公开几何采用有限 Double、内容区左上原点、向右／向下递增。Size 与 Rect 的尺寸非负；矩形端点及 native scalar 转换发生溢出时拒绝该操作。Canvas 的用户坐标为逻辑单位；DPI 到设备像素的转换只在渲染目标根部施加一次。桌面坐标不参与此转换。

Canvas 提供 save／restore、二维仿射变换、矩形与 Path clip、基本形状、Path、Image、文本布局绘制及组透明度。组透明度按离屏合成的整体结果应用，不能替换为各绘图操作分别乘 alpha。第一期颜色与窗口输出采用 sRGB、预乘 alpha；不承诺 HDR 或跨 GPU 的逐像素一致结果。

`path(fillRule: FillRule = FillRule.NonZero, configure: PathBuilder.() -> Unit = {}): Path` 在当前线程构造一个独立、不可变的 Path。配置块在 builder 创建成功后同步执行一次，操作按源码求值顺序追加；空配置生成空路径。正常返回或配置／快照失败均关闭临时 PathBuilder，异常原样传播，不发布部分结果；返回的 Path 由调用方负责 close／use。捕获的 builder 在作用域结束后已关闭，不可继续操作。

PathBuilder 的 `moveTo(point)`、`lineTo(point)`、`quadraticTo(control, end)`、`cubicTo(control1, control2, end)`、`closeContour()`、`addRect(rect)`、`addOval(rect)`、`addRoundRect(rect, radiusX, radiusY)`、`arcTo(oval, start, sweep, moveToStart = false)` 和 `reset()` 均返回 Unit，不提供链式构造。填充规则支持 NonZero／EvenOdd；arc 使用弧度，moveToStart 决定是否开始新轮廓。几何参数沿现有有限值和范围规则校验，圆角合同见[图像滤镜规范](SCOOP-GRAPHICS-EFFECTS-SPEC.md#2-图层与混合)。

需要增量修改、反复取得快照的调用方可显式创建 `PathBuilder(fillRule = FillRule.NonZero)` 并管理其 close／use。`build(): Path` 保留 builder，返回独立快照；`reset()` 清空几何但保留填充规则。后续追加、重置或关闭 builder 均不改变已交付的 Path。两种入口使用同一构造实现和 owner 线程规则；作用域入口不转移临时 builder 的所有权。

Path 提供 bounds、contains 和路径长度／位置查询；measure 返回独立 PathMeasure，原 Path 关闭后仍可查询。Paint 描述填充或描边、颜色／基础渐变、线宽与端点／连接样式。已交付的 Path、Image 和 Paint 描述不得因调用方后续修改 builder 而改变。

Canvas 是一次绘制作用域内的借用对象，不得跨帧保存或在其他线程调用；作用域结束后使用报状态异常。变换和 clip 的当前几何状态由 graphics 维护，并经低层接入接口提供给 Surface。绘制与区域放置不得分别维护两套可能不一致的变换。

`Canvas.withState<T>(block: (Canvas) -> T): T` 创建同步嵌套借用，继承当前坐标、clip 和组状态，不重复施加 DPI。block 获得独立有效期的子 Canvas；期间只能使用子 Canvas，父 Canvas 的操作及几何捕获报告状态错误。正常返回或抛错均恢复入口状态和保存层级，包括未配对的子级 save／saveLayer；子 Canvas 不能 restore 到入口以下，退出后拒绝操作。已绘制像素不回滚，异常帧是否提交由外层目标决定。

`scoop.graphics.native` 提供当前 Surface renderer 实际需要的公开 Unsafe 接入：原生渲染目标、受控的 Canvas 借用、提交／中止，以及几何状态快照。Skia C++ 类型不进入普通公共 API；Metal 等句柄只出现在按目标选择的 native 子包。跨 Cone 调用不能依赖另一 Cone 的 internal 构造器。

独立文本库使用同一 package 的 `@Unsafe fun <T> Canvas.withSkiaCanvas(independentTiles: Boolean = false, block: (Ptr<Unit>) -> T): T`。它在当前有效 Canvas 的 owner 线程同步借出实际 SkCanvas 指针，调用前保存原生画布状态，正常返回或异常退出都恢复原状态；不施加第二次 DPI 变换。指针只在 block 内有效，不能保存、释放、跨线程或挂起使用。block 只通过该指针进行 Skia 绘制，不重入同一 Canvas 的 Scoop 操作或几何捕获，不恢复到借用入口保存层以下，不自行提交目标或变更 GPU context。调用方承担这项 Unsafe 约束；graphics 的私有包装、资源析构和 clip 记录不向另一 Cone 开放。

SkSL／SkRuntimeEffect 的公开接入列为后续 graphics 增强，见[SkSL 设计](../graphics/sksl/DESIGN.md)。本期先按[图像滤镜与图层规范](SCOOP-GRAPHICS-EFFECTS-SPEC.md)补齐 Skia Gaussian blur、alpha 阴影、混合与图层滤镜，供离屏和公共 UI style 消费；这批已在 macOS aarch64 实现并通过 Raster／Metal 定向验证，x86_64 的 Rosetta 补验也已通过，详见 [UI 实施记录](../ui/PROGRESS.md)。RuntimeEffect／shader 填充仍未接入，不作为基础效果前置。后续实现通过既有 Skia 后端与普通 FFI 扩展，不隐式创建时间驱动器，也不改变 Canvas 借用或第 4.1 节的按需停帧规则。

### 2.1 绘制记录与兼容离屏图像

`Canvas.recordDrawing(bounds, block)` 在独立局部坐标中记录不可变 Drawing。bounds 必须有限，作为该记录的明确裁剪范围；不继承来源 Canvas 的变换、clip 或 DPI，记录原点为 (0, 0)，一个单位为一个逻辑单位。来源 Canvas 在回调期间处于借用状态，只有回调取得的记录 Canvas 可用；正常或异常退出均结束借用，未配对的 save／saveLayer 正常收尾。记录不修改来源像素，不保存 Scoop 回调，也不分配覆盖 bounds 的 bitmap。

Drawing 公开 bounds、isClosed 和 close；`Canvas.drawDrawing(drawing)` 在当前变换与 clip 下重放命令。它保留 Path、Image、字体和滤镜等实际原生依赖，来源包装或绘图回调结束不会使其失效。重放仍可能重新光栅化，不能替代像素缓存。记录与直接绘图使用相同混合语义；需要隔离的调用方显式使用 saveLayer。

Drawing.canRasterizeTiles 表示记录能否在独立裁剪块中光栅化。常规形状、图像、文字及不带 ImageFilter 的图层可以；记录内的 ImageFilter 使该值为 false，因为后端可能根据裁剪后的输入选择不同降采样位置，扩大滤镜邻域不能消除这种差异。嵌套 Drawing 继承被重放记录的限制。false 不妨碍正常重放，调用方应把滤镜放在完整内容的合成阶段，或按实际目标重放整个记录。

Unsafe withSkiaCanvas 增加可选 independentTiles 参数，默认 false；原生接入方只有在其命令可独立裁剪、没有依赖整层输入范围的效果时才声明 true。未声明的原生借用使当前记录的 canRasterizeTiles 为 false，不影响普通直接 Canvas。graphics.text 的纯字形／装饰绘图按此合同声明 true；行内组件仍由 UI 单独呈现。查询遵守 Drawing 的线程及关闭规则。

`Canvas.createSurface(pixelWidth, pixelHeight, scale = 1.0)` 创建与来源真实绘图目标兼容的 OffscreenSurface，采用相同颜色空间、像素格式和 GPU context，独立拥有像素与画布状态，不继承来源变换或 clip。尺寸为正整数，scale 为可表示的正数，只在离屏 draw 根部施加一次。OffscreenSurface 提供 draw、snapshot、readPixels、isClosed 和 close，Canvas 有效期与普通目标一致，借用期间拒绝关闭目标。

OffscreenSurface.snapshot 返回不可变合成 Image；GPU 路径保留 GPU 图像和所需 context，后续离屏写入不改变旧图像，不做 CPU 读回或等待。GPU 绘制与采样使用同一 context 的命令顺序；外部呈现目标的提交仍恢复其外部使用状态，普通离屏提交不要求呈现。现有 RenderTarget.snapshot／RasterSurface.snapshot 的独立 CPU 图像合同保持，readPixels 仍是显式同步点。

`Canvas.isCompatible(image)`／`isCompatible(drawing)` 检查实际后端资源能否在该画布使用。CPU 资源可被各后端消费；GPU 合成 Image 和 GPU 来源的 Drawing 只在同一实际 context 中使用，不自动读回、复制到其他 context 或重建资源。不兼容的绘制报参数错误，关闭及跨线程查询沿普通资源错误处理。资源持有引用不会延长 Canvas 借用，已排入 GPU 的使用由 Skia 保留至完成。

`Canvas.pixelTransform` 返回包含目标 DPI 的当前仿射变换；`localClipBounds` 返回当前 clip 在局部坐标中的保守包围盒，空裁剪为 None，路径孔洞不能仅凭此矩形判断命中。这两个只读查询用于像素网格与绘制区域选择，不替代 CanvasGeometry 的精确输入几何。`drawImageRegion(image, source, destination, sampling, opacity)` 绘制图像子区域，source 必须位于图像像素范围内；Linear 可采样相邻源像素，供有边缘扩展的缓存块拼接，来源图像外仍按图像边缘处理。

以上资源使用实际 Skia 记录器、兼容 surface 和图像快照，沿既有 owner 线程与 close／use 规则释放。本期在 macOS 实施；新增 x86_64 验证在全部 UI 基础交互功能完成后统一进行，Windows 验收留待后续。

## 3. 字体与文本布局

字体、Paragraph／TextLayout、多样式 run、位置查询和绘制的合同统一见 [文本排版规范](SCOOP-GRAPHICS-TEXT-SPEC.md)。公共类型及 Canvas.drawText 扩展位于 scoop.graphics.text；graphics 本身不引用 TextLayout，也不负责其原生释放。

Surface 只接收局部 Rect caret，并通过区域几何转换为窗口 IME 候选窗位置。文本编辑状态由应用拥有，应用在 graphics.text 的 TextRange 与 window 的 Utf8Range 之间显式转换；输入路由不保留段落或完整文档，也不接管撤销、剪贴板或 TextField。

## 4. Surface 宿主与帧

SurfaceHost 包装 ApplicationHandler，管理所附着的 SurfaceWindow，使用现有 Application.run 或 runWithAsyncio。窗口事件、用户绘制、动画采样和区域输入均在窗口 owner 线程串行执行；不启动第二个主循环或另一套任务树。CloseRequested 的接受策略仍由应用决定。

SurfaceHost 必须在 Closing／Exiting 及用户回调失败时关闭相关 Surface 资源；不能要求应用额外转发清理事件才能释放 lease。绘制和输入回调不挂起、不在内部锁中运行，也不从原生通知递归进入。

`SurfaceContent : Closeable` 定义同步 `paint(FrameContext)` 及默认不执行操作的 `onFrameComplete(FrameCompletion)`，表示一个由 SurfaceWindow 拥有的内容根。`SurfaceHost.attach(window, content)` 在通过 owner 线程检查后接收新 content 的所有权；窗口状态、重复附着、renderer 创建或配置失败时，也负责关闭 content 并回滚本次取得的资源。错误线程调用在转移所有权前拒绝。一个 content 只能交给一个窗口，close 必须幂等。原有 painter 闭包重载通过不拥有额外资源的 SurfaceContent 适配，继续服务直接绘图。

关闭覆盖 SurfaceHost 的事件处理器、内容 paint／帧完成通知和输入回调。回调内部请求关闭时先进入 Closing 并停止新工作，当前回调栈及 Canvas／frame 借用退出后才关闭 content；随后释放 renderer 和 lease。最外层 `SurfaceHost.use` 因而同时覆盖内容根、原生窗口关闭和异常退出。attach 尚未暴露给调用方的失败结果立即回滚，不等待另一个原生事件。

主题绘图只消费 Canvas、几何和视觉数据，不创建 State／输入／任务；系统明暗跟随复用 window 的现有主题通知。公共 style、主题和滤镜没有独立时钟；参数动画复用 Surface Animatable，静态阴影／光晕／模糊不增加周期任务或帧需求。隐含的背景采样不属于普通内容滤镜；Backdrop blur 留待后续。

每个 SurfaceWindow 合并重绘请求。只有 dirty 或活动动画需要新帧时才向平台请求帧机会；普通空闲保持 Wait。requestRedraw、WaitUntil 与 prePresentNotify 均不代表 vsync。

平台适配接收窗口配置与帧需求，通知下一次可尝试绘制的机会，并返回 Ready(Frame) 或 Deferred。Deferred 是正常暂不可绘制状态；保留 dirty，等待后续原生机会或窗口状态改变，不忙重试。隐藏、遮挡、零尺寸、待配置及关闭状态不得申请或提交无效帧。

帧开始使用 MonotonicTimeSource 的 TimeMark，一帧内所有动画消费同一个采样时间。平台的预测呈现时间留在呈现层，不冒充输入时间或通用 FrameClock 时间。

`FrameContext.withState<T>(block: (FrameContext) -> T): T` 沿用 Canvas.withState，为子绘制提供拥有相同窗口、尺寸、时间及本帧输入收集器的 FrameContext。子 context 的 place 使用其子 Canvas 的当前几何；子作用域结束只恢复绘制状态，输入 placement 仍随整个帧成功提交后发布。

一帧依次完成：取得可用目标、采样动画、执行必要文本布局及绘制、建立 Area placement、提交渲染、发布本帧命中几何。失败／中止不发布半帧几何；中止必须释放已取得的目标。帧内新增的失效请求保留到下一帧。

原生窗口在绘制回调中已改变尺寸或 backing scale 时，旧目标必须在提交前中止，即使对应 GeometryChanged 尚未进入业务事件处理器。平台 renderer 核对实际目标所依赖的原生尺寸与 scale；不把排队的 WindowMetrics 当成原生窗口尚未变化的证明，也不要求窗口请求同步递归派发事件。

发布的几何对应最近成功提交的画面，不保证 GPU 已经显示该画面。首期不为输入同步而等待 GPU 或显示扫描完成。平台提交接受之后的 GPU 失败作为渲染错误处理。

`FrameCompletion(time, size, submission)` 保存本次实际绘制使用的采样时间、窗口内容区逻辑尺寸及 `FrameSubmission.Accepted／Discarded`。Accepted 表示提交已被接受且本帧输入几何已经发布；Discarded 表示绘制正常返回，但目标失效、窗口隐藏等原因使该帧未被接受，旧几何继续有效。这是同步帧提交结果，不是 GPU 完成或屏幕扫描通知。

只有真正调用过 paint、正常结束绘制并完成 frame 资源释放，且 SurfaceWindow 仍为 Open 时，才调用一次 onFrameComplete。Deferred／隐藏期间没有进入 paint 的机会不产生通知；paint／提交／资源释放异常或关闭会跳过通知，按正常错误与关闭路径释放内容。通知不承担资源收尾职责。回调执行时全部 Canvas 借用均已失效，Accepted 的焦点／hover 调整已经完成；回调可以请求焦点、产生业务更新或请求下一帧，不能继续绘制或使用旧 FrameContext 的 Canvas。回调中关闭窗口仍延迟到当前回调退出后释放 content；回调抛错按内容回调失败处理，关闭相关资源。通知本身不产生新帧需求。

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

`Animatable.isFrameEnabled` 缺省 true，控制该值是否要求连续帧，不改变 Running／Paused 状态或起始时间。调用方已知内容不参与场景时可设为 false；恢复 true 先按当前单调时间采样，再请求必要帧。关闭期间的改目标、snap、取消仍正常改变逻辑值，但不单独请求绘制。`AnimationTimeline.hasWork` 包含所有活动动画和待交付完成回调，`hasFrameWork` 只包含启用帧的活动值或完成回调，二者不能混用。

SurfaceWindow 用 WindowDeadline 安排不参与帧驱动的有限动画最近完成时间；窗口整体不可绘制时适用于全部有限动画。有限重复与 spring 的结束时间按原解析计划计算，纳秒取整不提前结束；无法表示的期限在启动或恢复时拒绝。零时长完成回调同样在后续采样边界交付。无限重复的离屏动画不安排周期定时器，其他必要采样或恢复可见时直接采样当前进度。定时采样与真实帧使用同一时间轴及完成队列；取消、重新启动和关闭阻止旧完成回调，完成回调新建的工作留到下一轮。这个机制不把正常暂停变成离屏策略，也不推测内容布局矩形是否覆盖全部绘制效果。

本期不引入可观察属性系统。应用明确更新状态／请求帧，动画仅失效实际变化所需的绘制或文本布局。

`AnimationTimeline.group(frameEnabled = true)` 创建同一时间轴拥有的 AnimationGroup。group.value 与 timeline.value 支持相同值类型；组拥有这些值，close 关闭其剩余动画，时间轴关闭同时关闭组。组的 isFrameEnabled 与每个值的 isFrameEnabled 相与，不覆盖值的显式设置；恢复组时按同一个当前时间采样，完成回调仍排队。组没有独立时钟或嵌套调度器，供 UI 将一个挂载作用域的动画与平滑滚动一起退出离屏帧需求。创建、读写与关闭均使用时间轴 owner 线程。

## 6. 区域、输入与焦点

InputArea 是 SurfaceWindow 内的稳定命中对象，保存焦点／文本能力、caret 和 ignoreSelf。其 `scope: InputScope` 是该区域的逻辑路由节点；独立 InputScope 的 area 为 None，区域节点的 area 为 Some(对应 InputArea)。所有路由节点共用 InputScope 的 parent、enabled、captureHandler、handler 和 defaultAction。SurfaceWindow.input 是没有几何的窗口根；createScope／createArea 的 parent 缺省为该根。关闭区域会关闭其 scope，关闭 scope 使相应路由及后代失去交互资格，但不代替调用者关闭子资源。

parent 是稳定的逻辑关系，不由 FrameContext.place 决定。设置 parent 时检查同窗口、活动父节点和无祖先环；窗口根不能重新挂到其他节点。None 表示普通节点重新连接窗口根。更新立即影响后续事件和资格收尾，不等待下一帧。每帧 place 只提供区域及 shape，记录 Canvas 当前变换、有效 clip 及放置顺序；同一区域一帧至多放置一次，逻辑父区域不必先绘制。placement 随成功提交整体发布。

enabled = false 使节点及其后代退出交互；ignoreSelf 只忽略区域自身命中，仍允许子区域。命中按可接收区域的逆放置顺序选择唯一目标；不可逆变换不命中。路径中的区域须有有效已发布几何，独立 scope 不需要几何。父关系不隐式裁剪，shape 可以独立于可见颜色或 alpha，不做像素颜色拾取。

处理器使用 `(RoutedEventContext, InputEvent) -> Unit`。上下文的 target／currentTarget 是 InputScope，phase 为 Capture／Target／Bubble／Default／Notification；time 保留输入或观察时间。指针事件中，有几何区域的 localPosition 是 Some(局部 Point)，无几何作用域为 None；窗口位置从 InputPointer.state 读取，不伪造 scope 的局部坐标。上下文只能在当前同步回调中使用，处理器返回后失效。

可路由的 Move、Button、Wheel 和 Keyboard 先固定目标、路径及各节点坐标，再按 capture（根到目标父）、target、bubble（父到根）、default（目标到根）派发。captureHandler 接收 capture，handler 接收 target／bubble，defaultAction 接收默认阶段。stopPropagation 只停止后续传播；preventDefault 只取消默认阶段；consume 同时执行两者。默认阶段不因单独 stopPropagation 而取消，也不会重新寻找下方重叠兄弟。每次实际调用读取最新处理器，跳过已关闭／禁用的节点；回调中的 parent 修改不改写当前快照。

每条可路由输入完成后，沿原路径由内向外调用仍有效节点的 `onEventFinished(InputEvent)`。这是不可拦截的同步收尾通知，无路由上下文；传播被停止、默认行为被取消或处理器抛错时也执行，原异常与收尾异常按现有清理错误规则传播。关闭／禁用节点由 Cancel 等终结通知收尾。控件可据此清除已结束的按压或按键序列，不能在这里重新执行被取消的激活。

主按钮按下的默认阶段先在当前 hover 路径中最内层可聚焦区域请求焦点，再执行该节点的 defaultAction，仍未消费时继续祖先；拖出后追加按键不会将焦点送到已离开的捕获目标。传播处理器可用 preventDefault 取消这次默认焦点变化；控件自己的按压、释放和激活由其默认行为实现。窗口活动但没有焦点区域时，Keyboard 从窗口根 scope 开始；窗口未获 OS 焦点时不执行键盘默认动作。指针没有命中区域时也向窗口根交付普通事件，根的 localPosition 为 None，供窗口级观察与默认动作使用。

Entered／Left、FocusChanged／FocusVisibilityChanged、ModifiersChanged、Cancelled 和 TextInput 使用 Notification 阶段，不可取消。此时三个取消方法均不改变派发或状态收尾。Entered／Left 按 hover 路径差异通知；FocusChanged 与 TextInput 只交付所属区域；ModifiersChanged 完整通知当前焦点路径，缺省为活动窗口根。Cancel 向整条旧路径交付，不因中途节点消费或退休遗漏祖先收尾。close 或禁用发生在处理器内时，退出派发后再交付必要的 Cancel／Left／失焦通知。Left 与 Cancel 只有时间及终结原因，没有局部位置。

第一期沿用 window 的鼠标与滚轮模型，不虚构 touch contact。首次按钮按下固定该次鼠标按钮序列的目标与路径，未命中区域时固定窗口根路径，直到最后一个已按按钮释放或收到取消；中途移动／抬起继续送到该路径，使用最新有效 placement 换算局部坐标。区域移除／禁用、窗口失焦或捕获丢失结束序列，交付 Cancel，不能合成可能触发点击的正常 Up。

hover 根据当前位置和已发布几何单独计算；鼠标不动而区域随动画移动时也须更新。窗口 PointerEvent.Left 不等于拖动取消。PixelDelta 按该次输入携带的 scale 转为逻辑距离，LineDelta 保留单位，不能假定固定行高。

Surface 维护逻辑 focusedArea；键盘、修饰键与 TextInputBatch 路由给当前活动焦点。OS 失焦使其暂不活动并结束按下／组合状态，重新获得焦点时可恢复仍有效的区域；移除／禁用则清除逻辑焦点。文字插入只来自 TextInputBatch，不能同时从 LogicalKey.Character 插入。

requestFocus 只对已发布、可交互、具有非空可见几何且允许焦点的区域成功；canFocus 查询同一条件，文本区域自动允许焦点。它只改变 Surface 内逻辑焦点，不强制抢占 OS 窗口焦点。已聚焦区域仅被 clip 完全裁掉时保留逻辑焦点和活动输入会话；不再放置、变换不可逆、禁用或关闭仍清除焦点。保持当前会话与接收新聚焦请求使用不同条件。

InputArea.windowBounds／visibleBounds 返回最近成功帧的窗口逻辑坐标 Rect 值，分别表示未裁剪形状的包围盒和与显式 clip、窗口视口相交后的包围盒。没有已发布几何或区域已关闭时返回 None；空可见交集或不可逆变换的 visibleBounds 为 None。查询不返回可关闭的几何借用，也不认为后绘制的普通兄弟自动遮断前方焦点目标。InputScope.isEnabledInHierarchy 只查询逻辑祖先的关闭／禁用状态，不假定已有几何。

InputArea.pointerPosition 把窗口内最后观测的指针位置通过最近成功帧的几何映射为局部 Point。它不要求指针命中该区域；窗口尚无指针、指针已离开窗口、区域关闭／未放置或变换不可逆时返回 None。它供成功提交后刷新 hover 子部件等状态使用，不产生输入事件，不替代 RoutedEventContext.localPosition 固定的本次事件坐标，也不再次进行 DPI 换算。

InputArea.toLocal(windowPoint)／toWindow(localRect) 使用同一成功帧几何，供拖选重定位和子区域显露使用；没有有效放置时返回 None，toLocal 对不可逆变换返回 None，不保存调用方的布局对象。visibleCaretBounds 返回当前文档／caret 对应成功帧中、经过区域形状和实际 clip 的窗口逻辑包围盒；尚无匹配帧、不可见或已经关闭时为 None。

InputScope.activity 返回该逻辑子树当前的 InputActivity，包含 hasFocus、hasPress、hasScrollRoute、hasTextInput 及 isEmpty；分别查询逻辑焦点、按钮序列起点、尚保留的滚动路由起点和文本输入会话的归属。它不把 hover 或普通鼠标移动算作活动，关闭后为空，查询使用 owner 线程。hasScrollRoute 包含接触 Ended 后等待可能惯性的候选；该候选直到 momentum 结束／取消、新滚动替换、路径失效或窗口终结才释放，每窗口至多一个，不等同于动画或持续帧需求。

InputActivity 另含 hasBoundary，表示子树内仍持有至少一个 InputBoundary 登记，包括非当前域和等待首帧的模态。observeActivity 跟踪这些登记的实际身份，增加、移除及逻辑归属变化都会通知；非当前域关闭不改变当前路由，但仍更新活动归属。边界所属条目在取得首个区域焦点前也必须保持可实现，避免缓存使模态屏障存在而内容永不呈现。真实条目删除仍按普通关闭规则退休。

`InputScope.observeActivity(handler: () -> Unit): Closeable` 订阅实际归属改变，在当前输入派发及路由收尾后通知，可在回调中读取新 activity。同一子树内起点 A 移到 B 也通知，即使活动标记没有改变；没有归属改变的移动和重绘不通知。登记时不回调，不为每个 InputScope 自动建立观察者。关闭登记或所属 scope 后停止通知，调用方可以在回调中关闭登记、变更焦点或禁用节点；后续变化仍经同一收尾通路处理，不递归交付未稳定的路由状态。UI 按需子项用它维护交互固定，不复制一套焦点／指针／IME 状态。

graphics.native.CanvasGeometry.bounds／visibleBounds 提供上述查询所需的几何值；形状与任意 path clip 的相交复用 Skia PathOps，不以若干采样点或矩形重叠猜测。快照内部按需复用一次可见交集计算，新的帧使用新的快照。bounds 保留裁剪前的位置，便于后续滚动显露；此查询不更改画布或绘制内容。

SurfaceWindow.focusRevision 是每窗口焦点请求序号。合法区域的显式 requestFocus（包括不能兑现和重复请求同一区域）及 clearFocus 都推进序号；OS 激活变化、自动清理失效目标不构成新的用户焦点请求。reserveFocusRequest() 推进并返回同一个窗口序号，但不改变当前焦点，供上层登记尚未有可用几何的请求或记录被当前域拒绝的显式请求；不能为这些请求另设一套不可比较的版本。上层待处理请求记录序号，后续请求到来后取消旧意图，不用反复比较 focusedArea 猜测用户是否选择了同一控件。

InputArea.focusRevision 保存该区域最近一次成功聚焦请求的窗口序号，初始及关闭后为 None。鼠标默认聚焦与程序化请求使用同一记录；失败请求、失焦、clearFocus 和 OS 激活变化不重写该区域的成功历史。导航组据此恢复最近使用的有效成员，不安装第二套焦点处理器。比较同窗口记录时使用当前窗口序号减去记录的无符号距离，最近者距离最小。

`FocusOrigin.Pointer／Keyboard／Programmatic` 记录同一聚焦入口的来源。`InputArea.requestFocus(origin = Programmatic)` 仅在成功时更新 `focusOrigin`（初始为 Programmatic）；默认鼠标聚焦使用 Pointer，非 synthetic 的实际键盘按下将当前区域来源改为 Keyboard，失败请求及 OS 失焦／恢复不重写来源。按键引起的来源改变不推进 focusRevision，也不创建另一份焦点目标；显式聚焦请求沿原有修订号规则。相同目标且指示未变的重复请求不产生额外帧需求。

`InputArea.focusVisibility: FocusVisibility = Automatic` 支持 Automatic／Always／Never。`isFocusVisible` 仅在未关闭且 isFocusActive 时为真：Automatic 对键盘／程序聚焦以及 textInput 区域显示，Pointer 聚焦普通区域不显示；Always／Never 显式覆盖。仅指示变化时发送不可取消的 `InputEvent.FocusVisibilityChanged(Boolean)` Notification，不重发 FocusChanged、不重建 IME。样式策略更新及真实按键都使用该通知，禁用、隐藏、卸载和 OS 失焦仍沿原焦点合同退出。

每个区域用 `setCaret(rect: Rect?, revision: Long? = None)` 保存局部 caret 和可选文档修订；None 明确表示没有可用几何，初始为 None。应用可传入 TextLayout.caret 的结果，Surface 不保留段落或编辑文档。区域因移除而取消的按钮序列在全部原生按钮释放前保持空路径，不能让剩余 Move／Up 或追加按钮突然命中其他区域。

InputArea.textInput 可变；开启使区域可聚焦，关闭只结束输入法连接而保留仍可用的键盘焦点，供 readOnly 切换使用。`restartTextInput(): Boolean` 结束当前输入连接，等待 Disabled 后重新接入同一仍合格区域；返回值表示调用前该区域是否实际拥有正在开启、已开启或正在关闭的原生会话，需要等待其 Disabled。没有会话，或原生仍在关闭另一个区域的会话时返回 false，新区域随后首次 Enabled 可直接使用。入口不清除逻辑焦点，也不把旧连接的尾部 Edit 改投给新的连接。实际 native connection 随 Enabled／Disabled 保留，带 origin 的 Edit 必须匹配当前会话，迟到旧连接的批次丢弃。

区域本帧不再放置时，在成功发布该帧几何后按区域缺席处理，退出命中并执行所需取消／失焦；失败帧继续保留最近已提交几何。区域对象本身不因此关闭。UI 可用这条既有放置规则处理被省略号隐藏的行内控件，无需给 Surface 增加文本对象或组件生命周期。

更换文本接收者先关闭旧接收者的 IME，以 Disabled 为旧会话结束边界，再设置新候选窗位置并开启新接收者；没有活动旧接收者时直接开启。Enabled 先于新接收者的 Edit。结束前的 Edit 仍路由旧接收者，已注销则丢弃，不得写进新区域。逻辑 focus 可以先改变，排队编辑的归属不能随之被改写；窗口关闭的资源释放不等待业务 IME 通知。

frame.place 同时捕获该次 caret／revision 和区域几何，caret 的可见交集使用相同 Canvas、区域形状和真实 clip，复用 Graphics 几何查询。成功提交后才发布窗口逻辑包围矩形；不能把新的局部 caret 与旧帧变换拼接。文档修订或 caret 已变化但新帧尚未接受、窗口配置已改变、区域完全裁剪或缺席时，原生几何立即设为 unavailable。Discarded 保留旧场景，只有它仍对应当前 caret／文档时才可继续使用；原生文档快照始终由编辑器在编辑生效时独立发布。

共享 caret 适配通过 window 的公共入口工作：基础接入调用 setImeCursorArea，清除调用 clearImeCursorArea；带 revision 的接入使用当前 TextInputConnection 的 publishTextInputCaret，等待原生文档与批次进度匹配后发布。没有新的帧不会重复发布已经成功接受的同一几何，旧连接的缓存不阻止新连接更新。macOS 的 AppKit 与 Windows 的 TSF 后端遵守同一连接／revision 合同，Surface 不保留忽略连接／revision 的另一套候选位置路径。

### 6.1 组合默认行为

InputScope.addDefaultAction(handler) 为该节点登记额外默认行为，返回随 scope 生命周期退休的 Closeable；关闭登记后不会再调用它。每个节点先调用 defaultAction，再按登记顺序调用额外行为，然后匹配该节点的 ShortcutScope；窗口根最后执行所属 SurfaceHost 的快捷键回退。消费后停止。这个固定顺序服务焦点导航、弹层关闭和快捷键，不暴露任意优先级整数。

进入节点的默认阶段时取得登记快照；中途新增的登记从后续事件开始，已取消的登记跳过。每次回调都有单独的 RoutedEventContext 借用，前一个回调返回后其上下文即失效。作用域关闭、禁用和原路径快照继续遵守本节路由规则。

### 6.2 输入边界与按下预处理

InputBoundary(input: InputScope) 在该窗口安装输入域，按建立顺序叠放，最新未关闭边界生效；同一 scope 同时只登记一个边界。它是显式 Closeable，提供 input、isClosed、isCurrent。SurfaceWindow.inputBoundary 返回当前边界或 None。域改变不改变逻辑 parent，也不重新发布几何；关闭非当前边界不改变现有输入域，关闭当前边界恢复前一个未关闭边界或窗口根。

新域立即限制可接受目标。命中、程序化 focus、Keyboard／ModifiersChanged 的缺省目标和普通 capture／target／bubble／default 均停在当前 input；没有命中或焦点时从该域根接收输入，不能落回背景窗口根。路径中的区域只在域内要求已提交的可逆几何，域外的区域几何不再成为 portal 模态的前提；逻辑祖先的关闭／禁用仍使后代失效。InputScope.isInputAllowed 查询这种不依赖几何的逻辑准入，isEnabledInHierarchy 仍只查询关闭／启用状态。UI 的待聚焦请求也必须先满足当前域准入。

域切换结束旧按下路径，退出当前派发后向完整旧路径发送 InputCancelReason.BoundaryChanged；按键仍保持按下时使用空路径，直到全部释放，不能让剩余 Move／Up 或追加按钮落向新域。已失效焦点按普通规则退出并关闭旧 IME；终结通知和旧 IME 会话的队列收尾继续遵守原归属，不把它们当作新的背景用户输入。一次输入回调内安装边界后，后续域外传播与默认处理器立即跳过；冻结路径不会扩展成另一条新路径。

input 所属 scope 被关闭或禁用时，未关闭的 InputBoundary 继续阻止背景输入，但不再向失效域根交付事件。该规则供 UI 在移除弹层的帧成功提交前保留屏障；调用方在适当提交边界显式 close。窗口输入路由关闭时结束全部边界，无需再等帧。

InputScope.beforePointerPress: (InputPress) -> Boolean 默认为 false，仅在当前域根（无边界时为窗口根）执行。InputPress 是只读值，包含 target、pointer、button 和 clickCount；target、路由及坐标先固定，预处理在本次首次按钮按下的传播与 hover 通知前运行。返回 true 消费整个按下序列，该次 Down 和后续 Move／Up 不交付目标；返回 false 沿原路径继续，不重新命中或合成事件。已有按钮序列中的追加按下不再次执行预处理。回调抛错也使尚未交付的序列保持空路径，再按原异常规则收尾。

预处理用于窗口／模态域中的最上层弹层外部点击决策。它不是任意优先级过滤器链，不替换控件自己的 capture、默认动作或 onEventFinished；边界变化仍可使已固定目标失效，不能利用返回 false 穿透当前域。

### 6.3 作用域快捷键

ShortcutScope(input: InputScope, bindings: Array<ShortcutBinding>) 在指定输入节点的默认阶段匹配快捷键；一个输入节点只拥有一个活动 ShortcutScope，关闭后可重新创建。enabled 控制整个快捷键集合，bindings 使用不可变 Array 整体更新；调用方通过 setter 更新配置，不暴露可变底层数组。UI 可以用 ComponentScope.resource 管理其生命周期，无需第二条事件派发链。

HostShortcutScope(host: SurfaceHost, bindings: Array<ShortcutBinding> = []) 提供同一宿主各窗口共用的最后回退。每个宿主至多一个活动登记，关闭后可重新创建；可在 attach 窗口之前登记，后续附着窗口自动接入，关闭单个窗口不关闭宿主绑定。它提供与 ShortcutScope 相同的 enabled、bindings、isClosed 和 close；配置只在登记／整体更新时检查一次，不为每个窗口复制或重验集合。SurfaceHost.close 立即停用该登记，已进入 canExecute 的动作在执行前再次确认其仍有效。

宿主回退沿窗口根的同一次默认阶段执行，读取当前绑定，不从原始 ApplicationEvent 再派发键盘。任何活动 InputBoundary 都阻止它，包括以窗口根自身为域的边界；窗口根的本地 ShortcutScope 仍属于域内行为。canExecute 中安装边界、关闭 scope／宿主或使原路径失效后不执行旧动作。宿主绑定只覆盖该 SurfaceHost 已附着的窗口，不接管未交付给应用或其他应用的系统组合键。

ShortcutBinding 保存 window 的 ShortcutChord，以及 enabled、allowRepeat、canExecute 和 action。key／modifiers 的描述、构造检查、Primary 解析、匹配及标签均由 window 提供；Surface 不保存平台键位表。绑定更新以 chord.equivalentTo 检查重复，派发以 chord.matches 匹配事件；通用作用域和派发位于共享源码。

默认只响应真实 key-down；key-up 和 synthetic 不执行，repeat 必须显式允许。enabled=false 或 canExecute=false 继续向外层寻找。同一 scope 内同时启用的相同规范化组合在配置时报告参数错误，失败更新保留原集合；logical 与 physical 在实际布局下同时匹配且均可执行时报告歧义，不依赖登记顺序任选一个。成功执行前 consume，随后调用一次 action。

控件 defaultAction 及额外行为先于本节点快捷键，内层先于外层。文本焦点下，祖先的普通快捷键要求明确 Super／Control 应用组合或非文本 NamedKey；裸字符、Space、Dead 和未知文本键不能被祖先抢走。altGraph=Some(true) 的文本输入受到保护；未知时保守保护可能的 Control+Alt 文本输入，已确定不是 AltGr 时允许明确的 Ctrl+Alt 绑定。输入目标自身可定义本地快捷键。识别出的编辑命令由编辑目标认领；canExecute=false 的一般快捷键不等于已经被编辑目标消费的命令。

快捷键只处理 Window 实际交付的 Keyboard，不插入文字，也不在 interpretKeyEvents 之前截取 IME。没有焦点区域时可在活动窗口根工作；模态域的路由截断同样约束快捷键。系统未交付的组合不在本库控制范围内。本批提供 macOS 接入，Windows 后续补充。

### 6.4 滚轮残量与连续手势

Wheel 的 InputScrollDelta 始终保留该次输入的完整原始单位：Lines 不换成固定像素，Pixels 已按事件 scale 完成一次逻辑像素换算。每次派发另保存一份可消费残量；`RoutedEventContext.scrollRemaining: InputScrollDelta?` 读取它，非 Wheel 为 None。传播阶段读取的是尚未消费的值，默认阶段由内向外读取前面节点留下的部分，原 InputEvent 不被改写。stopPropagation 不消费滚动量；preventDefault／consume 停止后续默认行为，Contain 策略可在处理本层后用 preventDefault 截断剩余量。

`context.consumeScroll(lineStep: Double, apply: (graphics.Point) -> graphics.Point)` 只允许在 Wheel 的 Default 阶段调用。lineStep 为调用方明确提供的有限正逻辑步长；Lines 先按它换成窗口逻辑距离，再按接收区域的变换换到局部坐标，Pixels 不再应用 scale。无几何 scope 使用窗口逻辑坐标。apply 接收局部有符号位移，返回本容器实际消费的同向、未超量位移；零残量不调用 apply。返回值须有限，各轴允许正常浮点舍入误差，非法结果报告参数错误。消费量换回原单位后从残量扣除，下一容器使用自己的变换及 lineStep；旋转／剪切下仍按同一坐标变换保留向量残量。

同一次 Wheel 的位置和几何变换在传播前固定，与 localPosition 使用同一 placement。apply 中的布局更新不更换本次换算依据。上下文及 apply 为同步借用，不允许在同一个 apply 内递归消费；回调结束后上下文失效。默认动作已被取消时不再调用 apply。

无 phase、无 momentum 的离散 Wheel 按当次命中选择路由，与鼠标按钮捕获独立。连续 Wheel 从首个非 None phase 固定逻辑目标与路径，MayBegin 到 Began 沿用该路径；同一手势及其动量后续事件只用新的已提交几何换算位置，不因内容滚动或指针经过兄弟区域而改投。接触 Ended 后保存这条路径以接收随后的 momentum Began；等待动量不请求帧，也不算仍在进行的交互。新的接触手势或离散 Wheel 重新命中，结束前一条仍活动的滚动；正常 momentum Ended／Cancelled 释放路由。

`InputPointerEvent.ScrollCancelled(InputCancelReason, InputTime)` 是独立的 Notification，向完整旧滚动路径交付，不取消鼠标按钮捕获；`InputCancelReason.ScrollInterrupted` 表示另一段滚动开始。区域缺席／关闭／禁用／重挂、模态边界切换、窗口失焦和关闭分别沿用 AreaChanged／BoundaryChanged／Window／Closed。处理器内发生这些变化时，立即停止旧滚动的后续普通交付，退出当前派发后再完整通知旧路径；一段已取消的 phase／momentum 保持空路径到结束，不在后续 Changed 或惯性阶段改投背景兄弟。原生 Wheel 自带 Ended／Cancelled 仍按原事件交付，onEventFinished 保证其收尾不会被传播或默认取消吞掉。

### 6.5 可组合处理器与按下序列接管

`InputScope.addEventHandler(handler)` 登记额外的 Target／Bubble／Notification 处理器；`addEventFinished(handler)` 登记额外的事件完成通知。两者返回只注销该登记的 Closeable，不覆盖原 handler／onEventFinished，也不关闭调用方 area。一次交付使用开始时的登记快照，已注销项跳过；本次回调中新登记的项从下一次交付生效。普通处理器服从现有异常规则；Notification 与完成通知按清理规则继续交付其他有效登记。关闭 scope 后，已排队的终结通知先完成，再释放登记。

`InputScope.isHovered` 表示该 scope 是否在最近提交几何生成的逻辑 hover 路径中，关闭后为 false。它不从下一帧待提交矩形推算命中，也不把 hover 计入 InputActivity 的活动所有权。

`InputScope.claimPointer(button = MouseButton.Left): Boolean` 只接管当前窗口已经按下的序列。调用方必须是原按下路径上的活动 scope，指定按钮仍处于按下状态；不在原路径、已经取消或其他 scope 已取得归属时返回 false。第一次成功后，同一 owner 重复调用返回 true，不重复发送取消。同一原生按钮组合仍沿用第一个按下时固定的一条路径，所有按钮释放后才结束该序列。

接管先发布新的 owner 与路径，再对原路径中更深的节点各发送一次 `Cancelled(Claimed, time)`；回调可同步关闭／禁用／重挂节点，返回前重新确认新 owner 仍有效。当前事件已经冻结的坐标和完成通知保持不变，后续 move／release 从新 owner 向逻辑祖先路由。它不合成 Up，不把后续 release 投递到已取消子项，也不进行 OS 全局鼠标捕获。

`InputScope.cancelPointerClaim()` 只撤销调用方已经取得的归属，以 `ClaimCancelled` 原因终止整条序列；不会交还原子项。未取得归属时为空操作。窗口失焦／关闭、原生取消、路径失效和边界变化继续使用现有终结合同；迟到的 release 不能重新命中另一目标。普通 claim 不改变焦点、IME 或滚轮序列归属。

### 6.6 当前窗口指针观察

`InputScope.observePointer(handler: (PointerObservation) -> Unit): Closeable` 观察同一窗口，与命中路径及输入消费分开。PointerObservation 包含 Pointer(Window.PointerEvent)、FocusLost 和 Closed；普通指针事件及窗口失焦在原路由／reconcile 完成后交付，鼠标移到空白或离开窗口也可观察。没有可消费的 RoutedEventContext，也不安装 OS 全局监听或改变 capture／焦点。

派发前冻结观察登记，期间新建者不接收当前事件，已关闭者跳过；关闭只移除自己的登记，scope 退休自动清理，窗口／router 关闭对仍在登记的观察者交付一次终端 Closed。观察回调失败不跳过其余必要清理，最终传播错误。该接口服务有实际活动会话的 hover、typeahead 等组合，不要求每个 scope 常驻观察。

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


### 快捷键描述装配

`shortcutBinding(chord: ShortcutChord, action: () -> Unit, configure: ShortcutBindingBuilder.() -> Unit = {}): ShortcutBinding` 以完整组合和动作作为必需参数，配置 enabled=true、allowRepeat=false、canExecute={ true }。modifiers 配置归 window 的 shortcutChord，不再作为 Binding 的第二份键位状态。ShortcutBinding 的长参数构造内部化；action／canExecute 仅保存为普通函数，构造不调用它们。`shortcuts(configure: ShortcutsBuilder.() -> Unit = {}): Array<ShortcutBinding>` 按顺序收集 add(binding)、addAll(bindings) 和 binding(chord, action) 配置块，每个成员追加一次并返回 Unit。数组在发布时快照，输入节点和宿主 ShortcutScope 继续显式创建／更新／关闭。
