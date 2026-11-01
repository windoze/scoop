# Scoop 基础控件库规范

日期：2026-11-01。状态：设计稿，`scoop.ui.component` 尚未实现。框架合同见 [UI 规范](SCOOP-UI-SPEC.md)，公共文本合同见 [文本排版规范](SCOOP-GRAPHICS-TEXT-SPEC.md)，分层与实施顺序见 [UI 设计](../ui/DESIGN.md)。

## 1. 定位与依赖

`scoop.ui.component` 是独立的普通 Scoop Cone，提供基于 `scoop.ui` 的自绘基础控件。它依赖框架及实际使用的 `scoop.graphics`、`scoop.graphics.text`、`scoop.surface`、`scoop.window` 和 core／std；使用下层类型的源码须声明相应直接依赖，不能借由框架的间接依赖访问它们。

`scoop.ui` 的生产代码、测试和自绘示例不依赖本库。本库只调用框架及下层库的 public 接口，不使用 UI 的 internal 节点、特殊控件注册表或按控件类型分支的入口。任何应用自己的控件库都可以使用相同的 Component／Element、测量／绘制 context、输入区域和动画能力。

框架决定挂载身份、树的更新和生命周期；本库决定控件外观、文字输入与裁剪策略、点击语义、布局算法及默认样式。公共文本模型、样式继承、shaping 和行内对象排版由 graphics.text 完成，段落与子组件的测量／缓存适配使用 scoop.ui.text；路径绘制、帧呈现和动画求值继续由 graphics／surface 完成。第一期控件全部自绘，不嵌入原生 Label／Button。

首期与 UI 框架相同，先在 macOS aarch64 完成实现与验证，最后统一补验 x86_64；Windows 接入后续单独推进。普通控件测试使用本 Cone 的 scoop.test，实施记录见 [UI 实施记录](../ui/PROGRESS.md)。

## 2. 控件与使用入口

第一批控件包括：

| 控件 | 职责 |
| --- | --- |
| Label | 纯文本或 StyledText、字体／样式输入及行内 View 绑定，复用公共段落测量与绘制 |
| Button | 标签、hover／pressed／focus／disabled 外观、指针与键盘激活 |
| Checkbox | 布尔 checked、外部 Binding 或自有状态、指针与 Space 切换及 disabled 外观 |
| Row／Column | 子项的单轴排列、spacing 与交叉轴对齐 |
| Stack | 子项叠放和对齐 |
| Frame | 单个子项的 padding、固定尺寸、对齐、背景和显式 clip |

这些控件是普通的持久 Component class，构造器接收各自的 Props；render 通过公开的 element 和 component 入口组织绘制元素及子控件。输入区域和布局策略可放在本库私有的 Element class 中，文本缓存复用公共 ParagraphHost；这些 class 仍只实现公开协议，不获得框架内部访问权。

常用入口提供返回 View 的普通函数，保留具体组件类型并使用延迟工厂。最小调用形式为：

```kotlin
import scoop.ui.*
import scoop.ui.component.button
import scoop.ui.component.column
import scoop.ui.component.label

class Counter: Component {
    private var count: Int by state(0)

    override fun render(): View =
        column(
            arrayOf(label(f"Count: {count}"), button(text = "Increment", onClick = { count += 1 })),
            spacing = 12.0
        )
}
```

label／button／checkbox／row／column／stack／frame 都属于 `scoop.ui.component`。它们不是编译器语法或框架内建节点，也不在 `scoop.ui` 中提供别名。工厂函数必须将变化的文字、回调、样式、Binding 和子描述放入 Props；不得把变化参数只捕获在首次构造闭包中。

显式 `component(props = ..., create = ::Label)` 与便利函数沿用相同的类型／key 匹配。改变 key 或组件类型重建实例；普通输入更新保留状态、区域和可复用缓存。子数组使用普通 Array<View>，函数体中的未使用表达式不会自动成为子项。

### 2.1 公共属性与状态归属

只读输入如 Button.enabled 作为普通值经 Props 更新；用户交互会修改的公开值如 Checkbox.checked 可以接收 Binding<T>；hover／pressed 等内部表现由持久组件的私有 State 保存。绑定读写同一份外部状态，不在控件内部复制一份值并靠回调保持同步。调用者可以提供自己的 State、Context 中 Store 持有的 State，或普通模型 getter／业务方法组成的 Binding。

控件每轮 render 读取当前 Props 中的 Binding，交互写入当前路径。更换 Binding 时按框架规则替换依赖，保留同一控件的挂载和内部交互状态；不能把变化绑定只存进首次构造的 delegate。公开计算属性的依赖来自其实际 State 读取；没有 State 的普通属性不会自动可观察。

便利函数返回 View 描述，调用者不通过它访问挂载实例或私有成员。跨控件联动通常由共同组合组件或业务模型持有状态，消费方在 render 中读取并计算输入。复杂控件可以公开普通状态模型，但不要求为每个布尔属性建立额外模型、全局 Store 或实例引用协议。

## 3. Label

Label 接受 String 加 TextStyle 的便利输入，也接受公共 StyledText、字体来源、ParagraphStyle 和 `Array<InlineView>`。便利输入转换为相同的纯文本描述，不建立另一套排版路径。默认字体、字号和颜色由本库选定；框架不提供全局默认主题或文本控件。调用方可以传入自己的字体及样式，其资源默认借用。

Label.render 使用 scoop.ui.text.inlineChildren 把行内对象绑定成普通 keyed children；自己的 Element 在 onMount 中通过 ComponentScope 登记 ParagraphHost，在 measure／paint 中委托公共适配。字体混排、SpanStyle 继承和内容索引由 graphics.text 负责，Label 不复制这些算法。

父约束给出有限最大宽度时按该宽度布局，无界时使用 layoutUnwrapped。最终 LayoutMetrics 的 size 遵守 Constraints，基线来自同一 TextLayout；绘制、对象放置及查询也使用这个结果，不另行估计文字宽度。换行、对齐、maxLines、ellipsis 和 Unicode 行为采用 graphics.text 合同。默认不裁剪超出外框的绘制，需要 clip 时显式包住整个文字与子项绘制作用域。

ParagraphHost 按内容、字体来源／revision、样式、段落设置、宽度和行内 metrics 管理缓存。框架每帧可以再次调用 measure，输入未变化时复用已有结果；行内图片尺寸、按钮文字或布局动画改变实际 metrics 时重新排版，纯位置、整体透明度或不改变尺寸的子项颜色变化复用布局。

行内子组件在插字、换行及省略号隐藏后保留 key、实例和状态；截断项不放置、不绘制，在该帧成功提交后退出输入和焦点。改变 key、移除对象或卸载 Label 才执行正常卸载。图片可以用普通自绘 Element 包装，使用与 Button 相同的行内协议；Label 不直接接管外部 Image 的所有权。

Label 本身不创建 InputArea，其行内控件可以拥有并放置自己的区域。文字选择、编辑文档、marked range、剪贴板和撤销不属于首期 Label；公共 graphics.text 的 hitTest／caret／selection 及替代文字导出仍可由其他控件使用。

## 4. Button

Button 的输入至少包括文字、onClick、enabled 及样式。它用普通状态保存 hover、pressed 和 focus 等交互表现，render 将这些值传给自己的 Element；文字可以组合 Label。绘制元素在 onMount 时取得 ComponentScope，创建一个允许焦点的 InputArea，在 paint 中使用实际布局矩形或明确的 Path 放置区域。

Button 通过公开 PaintContext 在背景、子标签与焦点前景之间安排绘制。需要将子项输入纳入按钮冒泡路径时，先放置自己的区域，再调用 paintChildren(parent = area)。控件外观变化不改变区域身份；单纯换色不重新创建组件或区域。

Button 的 LayoutMetrics 将标签基线加上实际垂直放置偏移，使整个按钮可以和周围文字按基线对齐。没有对应标签基线时返回 None，不根据字号估算另一条基线。

指针激活采用以下规则：

- 主按钮在有效区域内按下后进入按下序列；拖出暂时取消按下外观，移回可以恢复。
- 在同一有效区域内释放主按钮时触发一次 onClick；区域外释放不触发。
- Cancel、禁用、卸载或窗口失焦结束按下状态，不能合成会触发点击的普通 Up。
- 普通重绘或 Props 更新不产生点击，也不重启已有按下序列。

获得活动键盘焦点时，Enter／Space 的完整按下与释放激活一次；自动重复按键不产生额外激活。失焦、禁用或卸载取消键盘激活。指针和键盘激活状态须避免同一次交互重复调用回调。

enabled 和最新回调在 Element.update 的逻辑阶段生效。禁用不等待下一帧，隐藏时也立即退出交互；区域几何继续对应最近成功提交的画面。触发时调用当前 Props 中的 onClick，不延长旧闭包的生命周期。

动画是样式实现的可选行为。实现需要颜色、位置或大小过渡时，通过 scope.animation 取得 Surface Animatable；只有实际目标变化才调用 animateTo，普通 Props 刷新不能不断重启动画。measure／paint 读取本帧已采样值，动画完成不自行触发 onClick。

## 5. Checkbox

Checkbox 的 checked 是布尔值。最小入口为：

```text
fun checkbox(
    checked: Binding<Boolean>,
    enabled: Boolean = true,
    key: ViewKey = ViewKey.Position
): View
fun checkbox(
    initialChecked: Boolean = false,
    enabled: Boolean = true,
    key: ViewKey = ViewKey.Position
): View
```

外部绑定入口以当前 Binding 的 value 为显示和交互的状态来源。有效激活时读取当前值并写入其取反值，随后由底层 State 通知读取者；绑定的业务 setter 可以按自身规则接受或拒绝修改，控件不先保存另一个乐观值。外部修改 checked 只更新显示，不合成用户激活。

自有状态入口由持久的内部组件在首次构造时创建 State，initialChecked 只决定首次值；普通父级重建或再次传入初始值不重置它。该组件把自己的 State 作为 Binding 交给相同的 Checkbox 实现，不复制绘制和输入逻辑。两种入口采用不同组件形状，切换外部绑定／自有状态模式时按框架第 5 节重新挂载；同一外部绑定入口更换 Binding 对象则沿通常 Props 更新保留实例。

Checkbox 通过公开 Element 和 InputArea 自绘框、勾选标记及焦点外观。指针切换遵守 Button 的主按钮按下／区域内释放与取消规则；获得焦点后 Space 的一次完整按下／释放切换一次，自动重复不重复切换。禁用、失焦、卸载及区域取消结束活动序列，不改变 checked；Enter 不作为本控件的切换入口。enabled 和当前处理器在 Element.update 阶段生效，隐藏或没有 drawable 时同样处理。

例如，同一个状态驱动勾选与另一个控件的 enabled：

```kotlin
import scoop.ui.*
import scoop.ui.component.button
import scoop.ui.component.checkbox
import scoop.ui.component.column

class Form: Component {
    private val checked: State<Boolean> = state(false)

    override fun render(): View =
        column(
            arrayOf(
                checkbox(checked = checked.binding()),
                button(
                    text = "Clear selection",
                    enabled = checked.value,
                    onClick = { checked.value = false }
                )
            )
        )
}
```

Checkbox 在自身 render 中读取绑定，Form 在 render 中读取 checked.value，两者分别订阅同一 State。用户切换后，Form 重建描述并通过 Props 更新 Button.enabled；点击 Button 又更新同一状态。enabled 是派生输入，不另存一份 State，也不把普通赋值当作持续绑定。Checkbox 卸载时仅解除自己的订阅，Form 或 Store 持有的状态继续有效。

## 6. 容器与 Frame

Row／Column／Stack／Frame 全部通过普通 Element 的 MeasureContext 测量和放置子项，通过 PaintContext 绘制。框架只执行协议，不识别这些 class，也不内置它们的算法。

首期 Row／Column 给每个子项一次测量机会。子约束的最小值为零，主轴采用无界最大值取得子项自然尺寸，交叉轴传入可用最大值；子项仍须返回有限尺寸。主轴结果是子项尺寸与 spacing 之和，交叉轴采用最大子项尺寸，再按父约束收敛。子项按顺序排列，交叉轴按显式对齐方式放置。spacing 为有限非负值，尺寸求和或坐标溢出按通常参数错误报告。

Stack 对每个子项使用零最小值和同样的可用最大约束，取子项包围尺寸并按父约束收敛，再依对齐放置。绘制顺序与描述顺序一致，输入重叠选择继续由 Surface 的放置顺序决定。

Frame 先扣除 padding 得到子约束，再测量唯一子项，加回 padding 并按父约束确定自身尺寸。固定尺寸在父约束允许范围内生效；对子项的放置由 padding 和对齐决定。超出部分只有显式 clip 才裁剪，不因父子关系自动裁剪。背景在子项之前绘制，clip 使用同一个 Canvas 的几何状态。

各容器消费 measureChild 返回的 LayoutMetrics；Frame 把子项基线加上实际 y 偏移后返回。首期 Row／Column／Stack 未规定基线对齐策略，返回 None，不自动选取某个后代的基线。容器无截断策略时仍测量并放置所有子项，不能把父约束溢出当成隐式卸载。

首期不实现 flex／weight、grid、intrinsic 多轮协商、滚动或虚拟化。增加这些布局策略时继续使用或按具体需求扩展公开布局协议，不能在框架中增加本库专用类型分支。

## 7. 样式与资源归属

样式是本库的普通输入值，包含当前控件实际使用的颜色、字体、边框、圆角、间距和可选动画参数。默认样式、hover／pressed／focus／disabled 状态的视觉选择均属于本库；不在框架中建立控件主题、全局注册或控件样式继承系统。段落内 SpanStyle 的局部继承属于 graphics.text 的内容模型，复用其公共合同。

控件或其 Element 通过 ComponentScope.resource 登记自己创建的资源，或登记拥有可替换资源的缓存对象；输入区域和动画通过作用域的对应入口取得。卸载沿框架的子先父顺序清理，应用不为 Label／Button／Checkbox 逐个 use。借用的 Image／Path／字体、Binding 与外部模型不得由控件擅自关闭。

Element 构造、update、measure、paint 或回调失败沿框架与 Surface 的统一关闭路径传播。缓存创建及替换失败不得遗留新取得的资源；清理错误不能阻断其余对象收尾。控件库不另建窗口状态机、异常恢复树或动画时钟。

## 8. 验收要求

普通行为测试放在本 Cone 的 scoop.test，至少覆盖：

1. Label 使用实际 graphics.text／graphics 及公共 ParagraphHost 完成字体混排、继承、换行、尺寸与绘制；输入变化替换缓存，位置／透明度变化及重复测量复用缓存。
2. Button 指针按下、拖出／返回、区域外释放、取消、禁用、失焦，以及 Enter／Space 自动重复和激活取消。
3. Props 改变后使用最新文字、enabled 和回调；普通更新保留状态，key 改变后创建新实例。
4. Row／Column／Stack／Frame 的有限／无界约束、子项顺序、对齐、padding、显式 clip 及嵌套组合。
5. Counter 连续交互、两个独立实例、条件移除、keyed 列表重排，以及隐藏期间禁用和资源卸载。
6. 实际 Surface 动画驱动外观或布局，目标更新不重建实例，卸载后停止业务完成回调。
7. 自建／借用资源、缓存替换失败、回调内关闭、清理异常和真实窗口关闭。
8. 同一 Label 中的不同字号文字、行内图片和 Button：基线经 Button／Frame 正确传递，RTL 位置、自然／固定尺寸、窗口换行、截断及恢复后保持组件状态。
9. 行内按钮被截断后取消输入／清除焦点，失败帧保留已提交几何；按钮内容、图片尺寸与实际布局动画变化触发重排，缓存替换不关闭子组件。
10. Checkbox 的指针／Space 切换、自动重复抑制、禁用／失焦／取消；自有状态的初始值、重建保留和重新挂载重置。
11. 外部 Binding 与 Checkbox 自绘同步、Form 的 enabled 派生输入、反向更新同一状态、卸载不重置共享值；更换 Binding 后读写新目标并移除旧依赖，切换状态归属模式按组件形状重新挂载。

控件库必须能作为独立 Cone 使用已编译的公开框架接口构建和测试；独立自绘示例继续只依赖框架。类型、可见性及链接规则使用现有编译器与 Cone 合同，不为本库引入专用编译规则或重复每个控件的通用负例。
