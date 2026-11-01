# Surface 基础层实施记录

日期：2026-10-31。分支：`codex/gui-macos`。第一期共享架构与 macOS 部分已交付；其他平台 renderer 留待后续。公共合同见 [Surface 规范](../specs/SCOOP-SURFACE-SPEC.md)，架构与调研依据见 [设计](DESIGN.md)。

## 交付范围

| 内容 | 已交付能力 |
| --- | --- |
| `scoop.window` 前置 | 鼠标序列取消、有来源标记的单调输入时间 |
| `scoop.graphics` | 几何、Canvas、Paint／渐变、Path／PathMeasure、图像、离屏 surface 与 Metal target |
| 文本 | FontCollection、Paragraph、TextLayout；SkParagraph + HarfBuzz + SkUnicode／ICU；字体 fallback、段落重排、caret／selection／命中查询 |
| `scoop.surface` 宿主 | SurfaceHost／SurfaceWindow、按需帧调度、共享 renderer 合同、macOS Metal 呈现及有序关闭 |
| 动画 | 统一帧时间、Animatable、tween／Bézier／spring、改目标、暂停／恢复、重复／反向和取消 |
| 区域输入 | 稳定 InputArea、真实 Canvas 变换／clip、Rect／Path 命中、冒泡、鼠标序列、hover、焦点与 IME 路由 |
| 组合示例 | 独立 executable Cone：旋转 Path、悬停颜色、拖动回弹、短文本布局与输入 |

共享层不包含 AppKit／Metal 类型。平台入口通过 RendererHost、WindowRenderer 和 RenderFrame 连接原生 surface、帧通知与呈现；字体发现由目标适配。离屏绘图、文本、动画求值与区域算法不要求另建 Application。macOS 使用现有 window 输入 view 与 NativeWindowLease，安装 CAMetalLayer，通过 common modes 的 CAMetalDisplayLink 和既有 proxy／redraw 接通；同 host 共用 device／queue／Ganesh context。

Canvas 只在回调内借用；Path、文本布局和已发布几何保留所需原生引用。绘制成功提交后才发布区域场景，鼠标序列固定按下时的路径，拖动使用最新局部几何。逻辑焦点与 OS 活动焦点分开；IME 切换以旧 Disabled 为边界，已排队编辑保留原归属。关闭先停止新工作，退出业务回调与 Canvas 借用后再释放；普通帧不等待 GPU，显式读回和最终收尾才同步。

Skia 固定为 `f326a1e25fc736704063922b9ce76b527b1c5efe`，配套 HarfBuzz／ICU／图像 codec、许可及部署补丁纳入 vendor。维护脚本显式准备两目标的库与头文件，普通应用消费预编译 Cone 和 native 资源。文本对外统一使用 UTF-8 Long 字节索引，native 边界处理 UTF-16 与 grapheme 映射。

## 验证环境与结果

执行环境为 Apple M3 Ultra、macOS 27.0.1（26A434）；开发 SDK 27.2，最低部署 27.0.1。ARM64 原生运行，x86_64 通过 Rosetta 运行。以下均为 release 下实际执行成功的 `scoop.test`，没有把仅构建成功计为通过，也没有新增或运行 fixture。

| 测试范围 | ARM64 | Rosetta x86_64 | 关键覆盖 |
| --- | ---: | ---: | --- |
| graphics | 19／19 | 19／19 | Skia 像素读回、Path、clip、alpha、图像、真实 Metal texture；9 项文本测试覆盖固定字体、CJK／bidi／emoji、换行、ellipsis、索引与资源生命周期 |
| AnimationTest | 7／7 | 7／7 | 同帧时间、跳帧、三种阻尼、速度连续性、暂停／反向／重复、完成回调边界与关闭 |
| InputAreaTest／InputFocusTest | 14／14 | 14／14 | 真实 Skia 几何、变换／clip、路径孔洞、冒泡、拖动、静止 hover、焦点／IME、caret、线程及异常；离开窗口后失焦只取消一次并丢弃迟到 Up |
| SurfaceHostTest | 1／1 | 1／1 | 真实窗口帧、合并、空闲停帧、隐藏恢复、resize、多窗口、原 layer 恢复、重新 attach、绘制回调内关闭 |
| Surface asyncio Cone | 1／1 | 1／1 | 原生 modal／menu common modes 内帧与 I/O、根任务取消和 lease 收尾 |
| Surface errors Cone | 1／1 | 1／1 | painter 抛错后的多 surface／lease 清理 |
| Surface native input Cone | 1／1 | 1／1 | 两帧动画、真实输入 view、区域鼠标、preedit／commit 排队、IME 切换、候选窗坐标与回调内关闭 |
| 示例编辑与拖动 | 4／4 | 4／4 | IME 批次、selection／marked range、grapheme 编辑；拖动取消回弹、迟到移动／释放与关闭期间收尾 |
| window | 12／12 | 12／12 | 原生输入方法、通知、IME、跨线程 lease、关闭 |
| window.asyncio | 1／1 | 1／1 | 原生循环中的 I/O、根取消及收尾 |
| 穷尽性回归 Cone | 1／1 | 1／1 | 忽略嵌套 product 字段后的完整匹配、分支关联与真实编译运行 |

Scoop fmt／check、变更的 C++／Objective-C 严格编译检查通过；编译器前置修复通过完整工作区 Clippy、2,097 项 HIR 测试和 56 项工具链单测。固定依赖实际从归档准备；发行维护的定向测试通过。合并远端 main 的发行修复后，3 个相关脚本测试模块共执行 7 项，5 项通过、2 项按平台跳过。

原生显示测试串行执行。macOS 可以拒绝后台 activate 请求；window 和 Surface 输入测试把准备期与功能验证分开，30 秒内等待真实窗口焦点，再用独立的 5 秒期限验证输入和收尾。桌面入口使用普通 `.app` 包装已构建的原 scoop.test 二进制，从访达启动后记录其原有结构化结果，没有强制激活或伪造焦点。命令、启动条件见 [Surface README](../../sysroot/src/scoop.surface/README.md)。

## 桌面交互

ARM64 与 Rosetta 示例都已运行，观察到正确的 Skia 路径、旋转、阴影、中英文字与彩色 emoji。使用系统“简体拼音”逐键输入，显示带下划线的组合态，空格确认后得到“你好”；用户另确认系统候选窗位于文字光标附近。ARM64 示例还验证左右移动、退格删除完整 emoji，以及恢复焦点后继续进入中文组合态。

系统窗口放大使文字从两行重排为一行，还原后重新换行，caret 随同一布局更新。激活另一示例后，旧窗口保留正文和逻辑焦点，隐藏 caret；返回时恢复活动边框和 caret。Rosetta 示例关闭后进程退出，另一个示例继续工作。

桌面工具执行了星形命中、hover、拖动与释放。为区分拖动未发生和截图时回弹已结束，另在仓库外构造普通测试 Cone，复用原示例和生产库，仅在窗口标题记录实际区域事件。一次普通拖动收到按下、移动、释放；另一次拖至标题栏，在窗口内容坐标 `(650, -17)` 仍收到移动和释放，取消计数保持 0，随后星形回到原位。该记录来自桌面工具驱动的原生应用交互，不声称替代人工鼠标长时间操作。

用户随后用“按住星形拖出窗口 → 键盘切换活动应用 → 松开鼠标 → 返回窗口”发现示例未回弹。修复并更新 ARM64 桌面程序后，用户按同一顺序复验，确认已正常回弹。该人工组合验证与此前单独的拖动释放、应用焦点切换分开记录。

## 发现的问题与修复

- **输入缺口**：window 新增 `InputTime.Event`／`Observed` 及 `PointerEvent.Cancelled`。在原生接收处保存单调时间；失焦或原生捕获取消结束聚合按钮序列，迟到 Up 不再重复交付。正常离开内容区继续拖动。现有 Win32 后端同步共享字段和捕获事件，未增加新的 Windows renderer。
- **穷尽性分析耗时**：嵌套输入枚举匹配触发了对全部 wildcard product 字段的指数级展开。HIR 先检查共享 wildcard 列后的尾部矩阵，仅在确有缺失时构造合法 witness，保留行关联与诊断顺序；没有给 Surface 特判、跳过诊断或增加预算。
- **原生对象缓存串架构**：工具发现记录漏掉外层目标，同一 Apple Clang 下 ARM64／x86_64 误用同一 native 对象缓存键。现将 canonical triple 纳入既有更新键，真实 C 对象回归验证两个架构及各自复用；目标 include 重定位同时处理已存在目录的逻辑路径与物理路径差异。
- **display link 首帧异常**：CAMetalDisplayLink drawable 拒绝 `presentAtTime`。后端改为同一有序队列上的普通 `presentDrawable`，由 display link 控制节奏，移除预测时间缓存；两目标真实宿主测试随后通过。
- **测试对 AppKit 的假设**：输入 view 是 contentView 内的子 view；恢复原 nil layer 时，AppKit 可能自动建立 backing layer。探针现验证实际原层级、wantsLayer 和 Surface layer 移除，自定义原 layer 仍严格核对对象身份。焦点准备及原生 modal 观察起点也改为实际进入对应状态之后，不改生产激活或调度行为迁就测试。
- **拖出窗口后失焦不回弹**：window／Surface 已正确取消原按钮序列，取消后的松开不会交付正常 Up；示例原取消分支却只清理 dragging，留下窗口外的位置。示例现在共用 DemoDrag 处理释放与取消，窗口仍 Open 时立即回弹，Closing／Closed 只清理状态。两目标新增取消后回弹与关闭收尾测试；共享输入层新增离开窗口、失焦、重复原生取消和迟到 Up 的组合回归，确认取消只交付一次且恢复焦点后可开始新序列。

## 本期边界

本机只连接一块 2×、60 Hz 显示器，跨屏混合 DPI 尚无真实硬件验证；坐标与 scale 转换已有离屏／原生测试。x86_64 的结果来自 Rosetta，不等同于 Intel Mac 硬件验证。本轮没有执行 Windows／Linux Surface 验证，其他平台 renderer 仍留待独立实现。

示例只承担短文本编辑与基础能力组合；组件、容器布局、滚动、剪贴板、撤销、完整编辑器与完整 OS 文档镜像均属于后续范围。原始日志、逐次结构化结果、临时测试程序、缓存和演示可执行文件保存在仓库外，本文件只保留验证范围、结果与必要限制。

## 2026-11-01：Surface 更名与 UI 设计

底层 Cone、package、SurfaceHost／SurfaceWindow／SurfaceWindowState、C 入口、测试辅助接口、示例和文档已统一改名；没有兼容别名。上层另见 [UI 设计](../ui/DESIGN.md)、[框架规范](../specs/SCOOP-UI-SPEC.md)与[控件库规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)：`scoop.ui` 提供公开自绘协议，`scoop.ui.component` 独立实现 Label／Button 等控件，两个 Cone 均尚未实现。最外层 use、组件资源归属及待增加的 SurfaceContent 关闭合同已在设计中明确，本次不提前改变 surface 的绘制／关闭行为。

本次验证目标为 `aarch64-apple-darwin`，全部使用 release。Scoop 与原生源码已格式化，开发 sysroot 的系统库及四种 runtime 产物已完整准备；测试使用当前 runtime 源码和共享构建缓存。

| 验证范围 | 本次结果 |
| --- | --- |
| 基础库、示例、asyncio／errors／input 测试 Cone 类型检查 | 通过；Windows 专属 presentation 正文不计入 Darwin 验证 |
| 动画 | 7／7 通过 |
| 输入区域、逻辑焦点与几何 | 14／14 通过 |
| 示例拖动与文本编辑行为 | 4／4 通过 |
| 组合示例可执行文件 | 构建及链接通过 |
| 真实窗口宿主测试 | 实际运行超时，两个窗口帧数均为 0；当前桌面锁定 |
| errors／asyncio／input 三个独立桌面测试程序 | `--no-run` 构建及链接通过，未运行 |

系统会话两次报告 `CGSSessionScreenIsLocked=true`，锁屏环境无法完成真实帧与焦点验证；窗口宿主的超时不计为通过，待解锁后连同原生输入、模态循环及异常呈现收尾一起复验。本轮没有运行 Windows 或 Rosetta。25 项无需桌面的行为测试通过；原始日志、结构化结果及测试程序保存在仓库外。

## 2026-11-01：文本公共能力设计

新增 [scoop.graphics.text 规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)及[文本排版设计](../graphics/text/DESIGN.md)：独立字体／富文本／段落能力，支持样式继承、行内图片与控件的尺寸／基线／可见位置。UI 设计同步增加 LayoutMetrics、可选子项放置及公共 ParagraphHost，Label 使用相同接口。现有 SkParagraph 已有 placeholder 接口，但 Scoop 接入、native 拆分与控件组合尚未实现；本次仅更新设计，没有新增运行验收结论。
