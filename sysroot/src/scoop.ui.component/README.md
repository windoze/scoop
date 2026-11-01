# 自绘基础控件

`scoop.ui.component` 是普通 Scoop Cone。当前提供 Label、Button、Checkbox、Row／Column／Stack 和 Frame，使用公开的 Component／Element、ParagraphHost、Theme／ThemePainter、InputArea 和 Surface 动画；UI 框架没有这些控件的特殊节点。

应用选用一个主题实现，并在实际运行入口绑定稳定 ThemeSource：

```kotlin
import scoop.ui.*
import scoop.ui.component.button
import scoop.ui.style.ThemeMode
import scoop.ui.style.ThemeSource
import scoop.ui.style.macos.macosTheme

class Counter: Component {
    private var count: Int by state(0)
    public override fun render(): View =
        button(f"Count: ${count}", { count += 1 })
}

fun main() {
    val source = ThemeSource(macosTheme(ThemeMode.Light))
    context(source) {
        runUiApplication("Counter", { Counter() })
    }
}
```

控件挂载时保存来源，在 render 中订阅 `source.current.value`。应用在 UI owner 线程更新这一个 State 即可切换风格或明暗；工厂、内部 State 和焦点保留，尺寸及文字按新主题重新布局。可通过控件的 `theme` 参数明确使用另一来源。临时 Context 不会自动成为 View 子树的主题 Provider。

Checkbox 可接收 `State<Boolean>.binding()`，也可用 `checkbox(initialChecked = false)` 保留自己的状态。Button.enabled 是普通 Props，可在组合组件的 render 中从同一 checked State 计算。更换外部 Binding 后交互使用新绑定；initialChecked 只决定自有状态组件首次挂载的值。

Label 的 String 入口接受可选 SpanStyle，只覆盖其中明确的字体、字号、颜色等属性；完整 StyledText 使用公共富文本模型。Button 的标签复用 Label 与主题 action 字体，其基线来自实际文字布局。外部 FontCollection 只借用，默认字体资源由挂载作用域管理。

ButtonStyle／CheckboxStyle 使用公共的尺寸 Options 和 StyleOverride。Inherit 保留本轮主题值，Set(None) 明确清除可空属性。InteractionStyles 按 hovered、pressed、focused 顺序组合，禁用时只应用 disabled。`styled(content, style)` 可以给完整控件添加有序 glow、drop shadow 或 blur；空效果与有效果使用同一个持久装饰节点。

主题 painter、字体、段落和滤镜归 Element 的 ComponentScope 所有，应用无需为每个控件添加 use。主题和效果没有自己的时钟，控件动画只使用 Surface Animatable；静止时不安排更新或绘制。

Row／Column 按主轴排列，spacing 与交叉轴 AxisAlignment 通过 Props 更新；Stack 按描述顺序叠放。Frame 提供明确的尺寸、padding、两轴对齐和公共装饰，也可以选择 SurfaceKind 使用主题表面。未选表面时无需主题来源；从普通 Frame 切换为主题表面或反向切换都保留内容身份。Frame 透传经实际偏移后的基线，其他容器首期没有基线对齐策略。

有限宽度的 Label 使用定宽段落排版，无界宽度使用 layoutUnwrapped。行内按钮需要限定宽度时，可用 Frame.width 明确指定；文本行内子项仍只测量一次，不能要求先得到所在行的剩余宽度。

[主题展示示例](examples/themes/README.md)并列展示公共绘图部件，提供三风格、明暗、系统跟随和真实控件／效果切换。

[下载与后台校验示例](examples/download/README.md)展示 UI owner 的异步 HTTP 与真实后台 Thread 如何协作，通过 ApplicationProxy 合并发布进度；覆盖共享业务 State、组件自有状态、取消／替换以及应用退出后的线程回收。

规范见 [基础控件规范](../../../docs/specs/SCOOP-UI-COMPONENT-SPEC.md)和[主题绘图规范](../../../docs/specs/SCOOP-UI-THEME-SPEC.md)。普通覆盖／来源／布局测试在本 Cone，真实 Metal／输入／主题切换组合位于 `tests-cones/interaction`，容器与富文本组合位于 `tests-cones/layout`；实施范围与运行结果记录在 [UI 实施记录](../../../docs/ui/PROGRESS.md)。
