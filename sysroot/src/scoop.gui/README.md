# scoop.gui

普通 Scoop Cone，提供共享 GUI 宿主、动画、区域输入与按目标选择的平台 renderer。当前原生实现为 macOS 的 CAMetalLayer、CAMetalDisplayLink 和 Skia Ganesh Metal；共享代码只依赖 window 的事件、配置、代理和 lease 合同。

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

`GuiWindow.createArea` 创建窗口持有的稳定 InputArea。在 painter 内调用 `frame.place(area, rect或path, parent)`，沿 Canvas 当时的变换和 clip 记录本帧几何；只有渲染提交成功后才整体发布。区域可以每帧移动，父子关系用于事件冒泡，不自动裁剪子区域。区域不再放置、禁用或关闭后退出交互；所属窗口关闭时关闭其全部区域。

事件处理器返回 Ignored 或 Handled。普通指针、键盘和修饰键事件从命中目标向父区域冒泡，Handled 停止；Enter／Leave、焦点与文本直接交给对应区域。首次按钮按下固定整次多按钮序列的路径，拖出窗口仍可继续；区域失效或原生捕获取消会发送 Cancel，剩余按钮事件不会改投其他区域。移动与拖动提供窗口原始状态和各接收区域的最新局部坐标，滚轮区分行与逻辑像素。

`createArea(focusable = true)` 允许逻辑焦点，`textInput = true` 同时允许焦点和 IME。区域已成功放置后可调用 `requestFocus()`；它不抢占系统窗口焦点。`hasFocus` 表示保留的逻辑焦点，`isFocusActive` 还要求 OS 窗口当前获得焦点。文字只从 TextInputBatch 应用到编辑器自己的状态，不能再由 LogicalKey.Character 插入。用 `area.setCaret(rect)` 提交局部 caret，GUI 随已发布变换更新系统候选窗；同值设置不产生重绘。焦点切换等待旧 IME 的 Disabled 边界后再开启新接收者，已排队编辑保留原归属。

验证使用四个普通测试 Cone：

- 本 Cone 的 tests：动画的确定性采样；实际 Skia 离屏几何上的区域命中、传播、拖动、焦点、IME 会话和关闭；两个真实窗口的按需重绘、空闲、隐藏恢复、resize、layer 恢复及重新 attach。`--filter Animation` 和 `--filter Input` 可分别运行不需要桌面的测试；`--filter GuiHostTest` 运行真实窗口测试。
- `tests-cones/asyncio`：原生 modal／menu common modes 下的帧与 I/O，以及根取消后的 GUI 收尾。
- `tests-cones/errors`：painter 抛错后的多 surface 和 lease 释放。
- `tests-cones/input`：在真实输入 view 上组合动画帧、区域鼠标事件、AppKit marked text／commit、候选窗坐标、IME 切换和回调内关闭。

可通过 `scoop test <Cone路径> --release --target <目标> --sysroot <sysroot>` 分别运行。真实显示测试需要已解锁、显示开启的桌面；显示休眠或锁屏时 AppKit 将窗口报告为遮挡，生产代码按合同停帧。当前交付状态与验证限制见 [实施记录](../../../docs/gui/PROGRESS.md)。
