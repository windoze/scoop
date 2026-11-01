# scoop.surface

本库提供窗口呈现、帧调度、动画与区域输入。上层的 `scoop.ui` 提供不含现成控件的自绘框架，`scoop.ui.component` 使用其公开能力实现 Label／Button 等基础控件，见 [UI 设计](../../../docs/ui/DESIGN.md)；这两个上层库均尚未实现。

公共字体和段落 API 已迁入 [scoop.graphics.text](../scoop.graphics.text/README.md)，已实现 StyledText／SpanStyle 和行内对象，详细合同见[设计](../../../docs/graphics/text/DESIGN.md)。Surface 继续只管理呈现、动画和区域几何，文本或 UI 宿主拥有具体内容和资源。

普通 Scoop Cone，提供共享 Surface 宿主、动画、区域输入与按目标选择的平台 renderer。macOS 使用 CAMetalLayer／CAMetalDisplayLink 和 Skia Ganesh Metal；Windows 使用 D3D12、DXGI swapchain／frame-latency waitable object 和 DirectComposition。共享代码只依赖 window 的事件、配置、代理和 lease 合同。

```scoop
import scoop.graphics.*
import scoop.surface.*
import scoop.window.*

fun main() {
    val application = Application(ApplicationOptions(LastWindowPolicy.Exit))
    val host = SurfaceHost { owner, active, event ->
        when (event) {
            case Resumed -> {
                val window = active.createWindow(WindowAttributes("Scoop Surface"))
                owner.attach(window) { frame ->
                    frame.canvas.clear(Color.White)
                    frame.canvas.drawCircle(Point(80.0, 80.0), 40.0, Paint(Color(0.1, 0.4, 0.8)))
                }
            }
            case Window(id, WindowEvent.CloseRequested) -> active.requestExit()
            else -> {}
        }
    }
    host.use { current -> application.run(current) }
}
```

`SurfaceWindow.requestFrame()` 合并重绘需求。空闲、隐藏、系统报告遮挡或无有效配置时暂停原生帧请求；恢复可绘制状态后继续处理 dirty。Windows 的遮挡状态遵守 window 合同，只报告最小化或 DWM cloaked，不承诺检测其他顶层窗口的完整覆盖。帧内再次请求重绘会保留到下一帧。`FrameContext.time` 使用平台单调时钟，一帧只采样一次；Canvas 仅在 painter 内有效。

持有 Path／Image／文本等资源的内容可实现 `SurfaceContent : Closeable`，通过 `attach(window, content)` 把整个内容根交给 SurfaceWindow。content 提供 `paint(FrameContext)` 和幂等 `close()`；通过 owner 线程检查后即转移所有权，后续附着失败也由 Surface 关闭 content 并回滚资源。错误线程调用在接收所有权前拒绝。直接 painter 重载仍可用于不需要拥有内容资源的绘图。

关闭 SurfaceWindow 只卸载绘制层，Window 可继续存在或重新 attach。关闭 Window 或 Application 则由 SurfaceHost 自动退休 Surface 资源。SurfaceHost 事件处理器、paint 或输入回调内关闭先进入 Closing，整个业务回调和 Canvas 借用结束后，依次关闭 content、renderer 和 lease；不会在回调内等待自己。内容清理失败仍继续释放其余窗口与底层资源，并沿 Application 的既有异常通路传播。

同一个 SurfaceHost 可直接用于 `scoop.window.asyncio` 的 `runWithAsyncio`；Surface 不创建第二套主循环或任务树。应用负责 CloseRequested 策略、内容内部的资源组织及显式状态更新，最外层 host.use 覆盖所附着的全部内容根。

Windows 保留原 HWND、窗口过程、子窗口和 IME context，以 BGRA8／sRGB／预乘 alpha 呈现。DXGI 后台等待只向 owner 消息窗口投递一次通知，再经已有 proxy 请求 redraw。普通帧异步提交；尺寸变化先退休旧 frame，再释放 GPU 对旧 backbuffer 的使用并调整 swapchain。painter 内发生的原生尺寸变化会使当前帧放弃呈现和区域发布。关闭取消等待并移除待处理通知，排空 GPU 后卸载合成目标，最后释放 lease。

Windows 支持 `x86_64-pc-windows-msvc-ucrt`、`x86_64-w64-windows-llvm-mingw-ucrt` 和 `x86_64-w64-windows-llvm-mingw-msvcrt`，分别使用相容的 Skia／CRT 资源。应用消费已准备的 sysroot；维护者按[准备说明](../../../scripts/README.md)构建 native 依赖。设备或呈现错误以 GraphicsException 报告 operation 和可得的 HRESULT；不自动重建 device 或更换 renderer。

`SurfaceWindow.animations.value(initial)` 创建由窗口持有的 Animatable，支持 Double、Point、Size、Color，也可传入 AnimationConverter。`animateTo` 接受 Tween 或 Spring，可指定 Repeat；pause／resume、cancel、reverse 和 snapTo 明确控制生命周期。spring 改目标保留速度，隐藏期间不采样，恢复显示后按实际经过时间继续。窗口关闭会关闭其所有动画，并取消尚未交付的完成回调。

离屏使用 `AnimationTimeline(timeSource, invalidate)` 和 `advanceTo(time)`。同一轮全部值采样后才调用完成处理器，处理器中新启动的动画从该帧时间开始、后续帧继续。Tween 的 easing 包含线性和 cubic Bézier；spring 以质量 1 的解析解求值，不累加固定 dt。Size 和 Color 的结果限制在各自合法范围。Repeat 的次数计完整段数，Reverse 每段交换端点；零时长可以直接完成，但不能重复。

`SurfaceWindow.createArea` 创建窗口持有的稳定 InputArea。在 painter 内调用 `frame.place(area, rect或path, parent)`，沿 Canvas 当时的变换和 clip 记录本帧几何；只有渲染提交成功后才整体发布。区域可以每帧移动，父子关系用于事件冒泡，不自动裁剪子区域。区域不再放置、禁用或关闭后退出交互；所属窗口关闭时关闭其全部区域。

事件处理器返回 Ignored 或 Handled。普通指针、键盘和修饰键事件从命中目标向父区域冒泡，Handled 停止；Enter／Leave、焦点与文本直接交给对应区域。首次按钮按下固定整次多按钮序列的路径，拖出窗口仍可继续；区域失效或原生捕获取消会发送 Cancel，剩余按钮事件不会改投其他区域。移动与拖动提供窗口原始状态和各接收区域的最新局部坐标，滚轮区分行与逻辑像素。

`createArea(focusable = true)` 允许逻辑焦点，`textInput = true` 同时允许焦点和 IME。区域已成功放置后可调用 `requestFocus()`；它不抢占系统窗口焦点。`hasFocus` 表示保留的逻辑焦点，`isFocusActive` 还要求 OS 窗口当前获得焦点。文字只从 TextInputBatch 应用到编辑器自己的状态，不能再由 LogicalKey.Character 插入。用 `area.setCaret(rect)` 提交局部 caret，Surface 随已发布变换更新系统候选窗；同值设置不产生重绘。焦点切换等待旧 IME 的 Disabled 边界后再开启新接收者，已排队编辑保留原归属。

[组合示例](examples/basics/README.md)演示旋转 Path、hover、拖动 spring、文本布局及 IME 接收。它是普通 executable Cone，编辑状态由示例管理，未依赖 component。

验证使用以下普通测试 Cone：

- 本 Cone 的 tests：动画的确定性采样；实际 Skia 离屏几何上的区域命中、传播、拖动、焦点、IME 会话和关闭；两个真实窗口的按需重绘、空闲、隐藏／最小化恢复、resize、输入宿主保留及重新 attach。`--filter Animation` 和 `--filter Input` 可分别运行不需要桌面的测试；`--filter SurfaceHostTest` 运行真实窗口测试。
- `tests-cones/asyncio`：原生 modal／menu 循环下的帧与 I/O，Windows 还覆盖系统移动／缩放循环，以及根取消后的 Surface 收尾。
- `tests-cones/content`：附着失败及错误线程的所有权边界、回调内关闭、原生关闭、重新附着、从未绘制的隐藏内容和退出收尾。
- `tests-cones/errors`：paint 与多个 content.close 同时抛错、失败附着的回滚及全部 surface／lease 释放。
- `tests-cones/input`：在真实输入宿主上组合动画帧、区域鼠标事件、平台 preedit／commit、候选窗坐标、IME 切换和回调内关闭。Windows 的协议注入复用 window 测试 Cone 的 UTF-16 边界；该测试与安装的真实 IME 桌面操作分别记录。
- `tests-cones/presentation`：Windows 桌面上用两个重叠的真实窗口，采样透明区域和半透明绘图的最终合成像素。

可通过 `scoop test <Cone路径> --release --target <目标> --sysroot <sysroot>` 分别运行；Windows 安装后的命令名为 `sco`，仓库开发构建为 `scoop.exe`。真实显示测试需要已解锁、显示开启的桌面；显示休眠或锁屏时 AppKit 将窗口报告为遮挡，生产代码按合同停帧。当前交付状态与验证限制见 [实施记录](../../../docs/surface/PROGRESS.md)。

桌面测试须串行执行。`tests-cones/input` 还需要真实窗口焦点；macOS 可以拒绝后台进程的 activate 请求，此时在准备期将“Scoop region input”置于前台。测试给帧／焦点准备 30 秒，确认真实焦点后用独立的 5 秒期限验证输入会话，不用强制激活或伪造焦点绕过系统行为。超时会报告阶段、帧数、焦点和输入计数。需要桌面应用入口时，可将 `scoop test --no-run` 的测试可执行文件放入普通 `.app` 包装后从访达启动；无参数直接运行仍执行 scoop.test 的非 ignored 用例，设置 `SCOOP_TEST_RUN_DIR` 可记录原有结构化结果。
