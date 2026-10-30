# scoop.gui

普通 Scoop Cone，提供共享 GUI 宿主与按目标选择的平台 renderer。当前原生实现为 macOS 的 CAMetalLayer、CAMetalDisplayLink 和 Skia Ganesh Metal；共享代码只依赖 window 的事件、配置、代理和 lease 合同。

```scoop
import scoop.graphics.*
import scoop.gui.*
import scoop.window.*

fun main() {
    val application = Application(ApplicationOptions(LastWindowPolicy.Exit))
    val host = GuiHost { owner, active, event ->
        when (event) {
            case Resumed -> {
                val window = active.createWindow(WindowAttributes("Scoop GUI"))
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

`GuiWindow.requestFrame()` 合并重绘需求。空闲、隐藏、完全遮挡或无有效配置时暂停原生帧请求；恢复可绘制状态后继续处理 dirty。帧内再次请求重绘会保留到下一帧。`FrameContext.time` 使用平台单调时钟，一帧只采样一次；Canvas 仅在 painter 内有效。

关闭 GuiWindow 只卸载绘制层，Window 可继续存在或重新 attach。关闭 Window 或 Application 则由 GuiHost 自动退休 GUI 资源。回调内关闭先进入 Closing，退出 Canvas 借用后才释放；不会在回调内等待自己。用户异常关闭全部相关资源后沿 Application 的既有异常通路传播。

同一个 GuiHost 可直接用于 `scoop.window.asyncio` 的 `runWithAsyncio`；GUI 不创建第二套主循环或任务树。应用负责 CloseRequested 策略、自己的 Path／Image／文本资源生命周期及显式状态更新。

`GuiWindow.animations.value(initial)` 创建由窗口持有的 Animatable，支持 Double、Point、Size、Color，也可传入 AnimationConverter。`animateTo` 接受 Tween 或 Spring，可指定 Repeat；pause／resume、cancel、reverse 和 snapTo 明确控制生命周期。spring 改目标保留速度，隐藏期间不采样，恢复显示后按实际经过时间继续。窗口关闭会关闭其所有动画，并取消尚未交付的完成回调。

离屏使用 `AnimationTimeline(timeSource, invalidate)` 和 `advanceTo(time)`。同一轮全部值采样后才调用完成处理器，处理器中新启动的动画从该帧时间开始、后续帧继续。Tween 的 easing 包含线性和 cubic Bézier；spring 以质量 1 的解析解求值，不累加固定 dt。Size 和 Color 的结果限制在各自合法范围。Repeat 的次数计完整段数，Reverse 每段交换端点；零时长可以直接完成，但不能重复。

验证使用三个普通测试 Cone：

- 本 Cone 的 tests：动画的确定性采样、控制和生命周期；两个真实窗口、按需重绘、空闲、隐藏恢复、resize、回调中关闭、layer 恢复及重新 attach。`--filter Animation` 可独立运行不需要桌面的动画测试。
- `tests-cones/asyncio`：原生 modal／menu common modes 下的帧与 I/O，以及根取消后的 GUI 收尾。
- `tests-cones/errors`：painter 抛错后的多 surface 和 lease 释放。

可通过 `scoop test <Cone路径> --release --target <目标> --sysroot <sysroot>` 分别运行。真实显示测试需要已解锁、显示开启的桌面；显示休眠或锁屏时 AppKit 将窗口报告为遮挡，生产代码按合同停帧。当前交付状态与验证限制见 [实施记录](../../../docs/gui/PROGRESS.md)。
