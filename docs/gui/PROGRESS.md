# GUI 基础层实施记录

日期：2026-10-31。分支：`codex/gui-macos`。范围为共享架构和 macOS 两个目标；其他平台 renderer 留待后续。合同见 [设计](DESIGN.md)与 [规范](../specs/SCOOP-GUI-SPEC.md)。

## 当前状态

| 内容 | 状态 |
| --- | --- |
| window 鼠标序列取消、单调输入时间 | 已实现并验证 Darwin 两目标 |
| 固定 Skia／HarfBuzz／ICU 依赖与绘图 Cone | 已实现；CPU 与 Metal texture 验证通过 |
| GUI 宿主、共享帧调度、macOS Metal 呈现 | 已接入；桌面呈现测试待解锁会话后完成 |
| 字体、段落布局及索引／几何查询 | 已实现；Darwin 两目标离屏验证通过 |
| 动画与窗口帧时钟 | 已实现；Darwin 两目标确定性测试通过 |
| 输入区域、焦点和 IME 路由 | 待完成 |
| 组合示例、两目标集成验收 | 待完成 |

## window 前置能力

`InputTime.Event`／`Observed` 明确区分事件时间和接收时间，两者使用既有 `MonotonicTimeSource`。原生事件排队前保存秒／纳秒，键盘补发 release 重新采样，业务回调不重读时间。当前 AppKit 和 Win32 映射使用 Observed。

`PointerEvent.Cancelled` 结束窗口聚合的按下按钮集合，携带 FocusLost 或 CaptureLost 及独立时间，不伪造位置和正常 Up。AppKit 覆盖窗口／应用失焦及菜单跟踪，重复取消和取消后的迟到 Up 不重复交付；Left 继续正常拖动。为保持现有后端与共享 ABI 一致，Win32 同步字段、WM_CAPTURECHANGED／WM_CANCELMODE 和正常 ReleaseCapture 顺序；本轮没有执行 Windows 验证。

Darwin ARM64 和 Rosetta x86_64 的 `scoop.window` 各 4 容器、12 测试通过，包括真实 AppKit 输入方法、通知、IME、跨线程 lease 与关闭。`scoop.window.asyncio` 各 1 容器、1 测试覆盖原生 modal／menu 循环内的 I/O、根取消与 lease 收尾。x86_64 初次执行在 modal 观察断言失败、空闲复跑通过；测试辅助程序随后将短观察期的起点移至真正进入对应原生循环，避免 AppKit 冷启动消耗观察期。

Scoop 格式化、Cone check 及变更的 Objective-C 严格编译检查通过。验证全部使用 `scoop.test` 和普通测试 Cone，没有新增或执行 fixture；原始日志保存在仓库外。

协议调用与通知测试不等同于实际用户拖动、外部窗口抢焦点、输入法候选交互或混合 DPI 硬件验证。GUI 的显示链路和桌面验收仍须随后续批次完成。

## 绘图 Cone 与 native 资源

`scoop.graphics` 已提供 Point／Size／Rect／Transform／Color、Canvas、Paint、线性／径向渐变、不可变 Path 与 builder、PathMeasure、Image、RasterSurface。Canvas 的变换与 clip 由同一个 Skia 状态生成绘图和命中快照；快照保留独立 Path 引用，DPI 根缩放不进入逻辑几何。draw 回调结束或抛错时结束 Canvas 借用，恢复完整状态；跨线程、嵌套借用和借用中 close 均明确拒绝。

公开 `scoop.graphics.native` 提供 RenderTarget 和 CanvasGeometry，macOS 的 MetalContext 接收不透明 device／queue／texture，通过普通 C ABI 接入 Ganesh。目标独立保留 context，普通 submit 不等待 GPU；readPixels 和 drain 显式同步。窗口安装、CAMetalDisplayLink 与 present 仍属于后续 GUI 宿主批次。

固定 Skia revision `f326a1e25fc736704063922b9ce76b527b1c5efe` 及 DEPS 对应 HarfBuzz／ICU／libpng／libjpeg-turbo 源码、许可和 macOS 部署补丁已纳入 vendor。`build_skia.py` 显式构建两目标的 `libscoop_skia.a` 与头文件；依赖源自固定归档，命中先检查更新，普通应用不联网或运行 GN。系统 zlib 来自 Xcode SDK。发行准备与组装已接入这些资源。

macOS ARM64 和 Rosetta x86_64 的 graphics Cone 各 3 容器、10 测试通过：真实像素读回验证组透明度、描边端点、渐变、裁剪、PNG／JPEG 解码和图像快照；路径长度／曲线／多 contour 与资源生命周期使用同一 Skia 实现；独立 test-native Cone 创建真实 Metal texture，验证 Ganesh 提交、上下文复用、读回与一次 DPI 根转换。测试不需要 Application，尚不等于窗口呈现验收。

Scoop fmt／check、C++ 与 Objective-C 严格检查，以及 5 项既有发行维护脚本测试通过。两目标依赖实际从归档准备，后续相同输入直接复用；开发 SDK 为 macOS 27.2，最低部署仍为发行配置的 27.0.1。原始构建与测试日志留在仓库外。

## GUI 宿主与呈现接入

共享 GuiHost／GuiWindow 已接入 ApplicationHandler、按需帧调度、配置变化及 Closing／Exiting。平台边界为 RendererHost、WindowRenderer 和 RenderFrame；共享代码不包含 AppKit／Metal 类型。回调中关闭先停用需求，退出借用后中止帧并释放；绘制中 resize 会在提交前重新配置，退休旧目标。

macOS 安装 CAMetalLayer 到既有输入 view，保存并在卸载时恢复原 layer；同 host 的窗口共用 device／queue／Ganesh context。CAMetalDisplayLink 在 common modes 提供 drawable，经 ApplicationProxy 请求共享层绘制；普通帧只提交有序 GPU 工作，关闭时排空后退休回调和 lease。全部窗口卸载后释放 context，host 可再附着窗口。

已添加宿主、asyncio 和异常收尾三个普通 scoop.test Cone，Scoop 语义检查及 Darwin 两目标的 Objective-C 严格检查通过，两目标生产 `.slib` 构建成功。首次真实窗口测试停在首帧等待：系统显示休眠，唤醒后桌面工具仍明确报告 Mac 锁定，AppKit 将窗口报告为遮挡。此结果不计为呈现测试通过；待解锁后继续实际帧、空闲、多窗口、modal／menu 及异常验证。没有为测试绕过遮挡判断或启用定时器后备。

## 文本布局

graphics 已接入 FontCollection、TextStyle、Paragraph 与 TextLayout。共享 C++ 使用 SkParagraph／HarfBuzz／ICU；Darwin 文件只创建 CoreText font manager。显式字体数据、family 别名、fallback 开关、富文本 run、字距／词距、行高、方向／对齐、maxLines／ellipsis 及 OpenType feature／variation 均通过同一段落实现。Paragraph 保存字体集合快照，独立 layout 保留其完整来源，来源关闭不使结果失效。

文本快照构建 UTF-8／UTF-16 映射与 ICU grapheme 边界。行指标的 UTF-16 和 SkParagraph 可见范围的 UTF-8 在桥接处分别转换，对外统一为 Long 字节偏移。hitTest、caret、selection 和 Canvas.drawText 使用同一 native layout；软换行 affinity、bidi 多矩形及截断位置有明确结果。

Darwin ARM64 与 Rosetta x86_64 的 graphics Cone 各 4 容器、19 测试全部通过，其中 9 项文本测试覆盖固定字体测量、登记快照、独立重排、CJK／variable font、组合字符／emoji／CRLF、系统 fallback／混合 bidi、affinity、ellipsis、富文本像素、空文字／零宽及生命周期。测试字体从固定 Skia 资源提取，并保留相应 Apache／OFL 许可。

Scoop fmt／check、C++ 严格检查与 5 项发行维护脚本测试通过。依赖发布补齐了 SkUnicode 公共头实际引用的 `src/core/SkUTF.h`，两目标均由维护脚本重新准备。上述离屏与 Metal texture 验证不依赖桌面解锁，也不替代待完成的窗口呈现验收。

## 动画

AnimationTimeline 统一采样、调度和所有权，GuiWindow 将它接入同一帧 TimeMark、按需帧请求和关闭路径。Animatable 支持 Double／Point／Size／Color 和自定义向量转换，提供 tween／cubic Bézier、三种阻尼区间的解析 spring、改目标、暂停／恢复、取消、反向及重复。spring 以位移与速度包络决定结束时间，跳帧直接计算当前状态。

完成回调在全体采样之后交付，回调内操作共享当前帧时间；新动画进入后续帧。取消、重新定向或关闭后旧完成回调不会再次调用业务处理器。窗口隐藏时停用有效帧请求，时间默认继续推进，显式 pause 才冻结。

Scoop fmt／check 通过；ARM64 与 Rosetta x86_64 的 AnimationTest 各 7 项通过，覆盖同帧时间、跳帧、暂停、反向、无限重复、大步推进、spring 三种阻尼与速度连续性、约束值、零时长、安全回调边界、时间源／线程及关闭。窗口呈现仍待桌面解锁后集成验证。

## 区域输入发现的编译器前置修复

对嵌套输入事件进行完整模式匹配时，原 HIR 穷尽性算法会展开全部分支都忽略的 PointerState 字段，再检查后面的按钮状态。多个修饰键 Boolean 使这一过程产生指数级组合。采样确认耗时集中在 missing_witness 的 constructor 展开。

算法现先检查全部行共享 wildcard 列之后的尾部矩阵，保留每一行的关联；仅在尾部确有缺失时，按原顺序补一个合法头部 witness。没有增加预算、跳过诊断或修改 GUI 的匹配写法。

完整工作区 Clippy 和 2,097 项 HIR 测试通过，新增测试覆盖 32 个独立 Boolean、尾部关联、缺失示例顺序和空类型 product。普通 `tests/cones/pattern-columns` 的 Scoop 源码已在 ARM64 编译、链接和运行通过；x86_64 随配套 sysroot 更新后继续验证。没有使用 fixture。
