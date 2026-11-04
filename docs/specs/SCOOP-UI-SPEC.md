# Scoop 自绘 UI 框架规范

后续测量、方向、公共交互、portal 退出及文本策略的演进见 [控件基础设计](../ui/component-foundations/DESIGN.md)，按[控件基础实施记录](../ui/component-foundations/PROGRESS.md)逐批交付；下文描述对应批次的现行合同。

日期：2026-11-03。状态：原有自绘框架、公共文本适配、独立基础控件／容器、主题展示及宿主／后台下载组合已完成 macOS aarch64 与 x86_64 Rosetta 验证，详见 [UI 实施记录](../ui/PROGRESS.md)。新增场景与基础交互能力按[本期实施记录](../ui/foundations/PROGRESS.md)分批验收。本文件规定框架公开合同；公共文本能力另见 [文本排版规范](SCOOP-GRAPHICS-TEXT-SPEC.md)，基础控件另见 [控件库规范](SCOOP-UI-COMPONENT-SPEC.md)，取舍、内部组织及实施顺序见 [UI 设计](../ui/DESIGN.md)。现有底层库以 `scoop.surface` 命名，已实现的绘图、窗口呈现及输入合同见 [Surface 规范](SCOOP-SURFACE-SPEC.md)。

## 1. 定位与依赖

生产 Scoop 源码不限定 Darwin，也不包含 OS native 代码或平台判断；键盘、文本服务、字体和呈现均依赖基础库的公共合同。当前只验证 macOS，其他后端补齐基础能力后以同一份上层源码重新构建。

`scoop.ui` 是普通 Scoop Cone，提供声明式组合、可观察状态、组件身份、自绘元素协议、测量／布局接入以及资源生命周期。它依赖实际使用的 core／std／math／platform、`scoop.graphics`、`scoop.graphics.text`、`scoop.surface` 和 `scoop.window`。窗口事件循环、呈现后端、动画求值、文本 shaping 和 GC 仍由对应下层负责。

框架不提供 Label、Button、Row／Column／Stack 等现成控件、布局策略及默认控件样式。它提供实现这些控件所需的公开能力。`scoop.ui.component` 是独立的普通 Cone，使用与第三方控件相同的公开接口构建基础控件；`scoop.ui` 不依赖、转导出或识别该控件库及独立 `scoop.ui.style` 中的类型。公共主题绘图／几何与装饰／效果由[样式库](SCOOP-UI-STYLE-SPEC.md)通过相同公开协议实现，实际滤镜与图层由 [graphics](SCOOP-GRAPHICS-EFFECTS-SPEC.md)负责。只依赖框架的应用可以完成自绘、输入和动画。

首期只实现和验证 macOS：先完成 aarch64 的各批功能，最后统一补验 x86_64；Windows 接入与验收留待后续，不因底层已有 Windows renderer 就声明 UI 已支持该平台。不发布空 renderer。实施批次及定向验证见 [UI 实施记录](../ui/PROGRESS.md)。

静止时停止渲染工作是首期基本要求，见[第 6.1 节](#61-静止时停止渲染工作)。没有待处理更新、绘制失效或需推进的动画时，框架不维持周期性的逻辑遍历、绘制回调或 GPU 提交；该合同同时适用于桌面与未来移动端。

离屏 UI 根、外部 GPU 图像、UI 与 3D 场景的双向合成及视频展示列为后续增强，调研结论见 [UI 设计第 13 节](../ui/DESIGN.md#13-后续增强gpu-互操作3d-与视频)。这些能力尚未实现，不属于本文件的首期合同；进入相应实施阶段时再补充具体 API、线程与资源交付规则。

SkSL runtime effect 同样列为后续 graphics 增强，见[接入设计](../graphics/sksl/DESIGN.md)及 [UI 设计第 14 节](../ui/DESIGN.md#14-后续增强sksl-与组件效果)。组件通过普通 Element 使用效果，Surface Animatable 驱动 uniform；持有 shader 或声明时间参数不产生隐式帧需求，继续遵守第 6.1 节的停帧要求。

弹层／模态、滚动及绘制／实例缓存、虚拟子项、键盘导航、快捷键、路由扩展与公共编辑的后续方案见[基础交互能力设计](../ui/foundations/DESIGN.md)。该方案分批实施；场景与纯绘制协议见第 7.2 节，其他能力进入相应批次时再修订当前合同。

| Cone | 职责 |
| --- | --- |
| `scoop.window` | 原生窗口、事件循环、输入归一、IME 和 native lease |
| `scoop.graphics` | 几何、Canvas、Path、Image、绘图目标、图像滤镜与图层合成 |
| `scoop.graphics.text` | 字体、富文本内容与样式、Paragraph／TextLayout、行内对象排版、文字绘制与查询 |
| `scoop.surface` | SurfaceHost／SurfaceWindow、窗口呈现、帧机会、动画、InputArea 和区域焦点 |
| `scoop.ui` | Component／Element／LazyElement、State／Props／Binding、挂载树、测量与绘制接入、输入／动画的挂载归属；text 子包提供公共段落适配 |
| `scoop.ui.style` | 公共主题绘图／几何、视觉状态及语义参数；通用 Style、外框装饰与内容效果 |
| `scoop.ui.style.macos／material／fluent／shadcn` | 四套公共协议实现与各自 Light／Dark |
| `scoop.ui.component` | Label、Button、Checkbox、Row／Column／Stack、Frame、Absolute、ScrollArea、VirtualList、控件专用样式及交互外观 |

表中是目标分层；字体与 Paragraph／TextLayout 已迁入 graphics.text，富文本／行内对象、独立自绘框架及公共适配 `scoop.ui.text` 已完成。适配直接依赖 graphics.text，只连接文本排版与正常子项，不提供 Label 或默认样式，也不改变 View 的基础形状。

组件、状态及描述使用普通 class／interface／泛型／属性委托。首期不增加 `@Composable`、result builder、属性宏、关联类型、UI 专用 IR 或 runtime 入口。`scoop.ui` 的类型身份查询复用语言规范 9.6 的 `typeInfo<T>()`，不遍历组件字段，不按名称访问私有属性。

主题来源复用 State／Context，见[主题规范](SCOOP-UI-THEME-SPEC.md)。主题更新作为普通 Props 和布局输入进入原节点，不改变框架描述形状；局部主题通过显式参数传递，临时 Context 不成为挂载树环境。主题绘图不读取 State，也不注册输入。

原有独立自绘控件、Counter、条件组件和可重排列表已经交付；当前按基础交互设计增补虚拟列表和公共文本编辑。完整富文本编辑器、手势竞争、原生控件封装、accessibility 树、并发／可中断渲染和自动 memoization 不进入本期 API。

## 2. 组件与描述

组件的公开协议为：

```kotlin
public interface Component {
    fun render(): View
    fun onMount(scope: ComponentScope): Unit = Unit
    fun onUnmount(): Unit = Unit
}
```

`onMount`／`onUnmount` 的默认行为明确为不执行操作；组件只需实现 `render`。使用方式：

```kotlin
import scoop.ui.*
import scoop.ui.component.button

class Counter: Component {
    private var count: Int by state(0)

    public override fun render(): View = button(f"Count: ${this @Counter.count}", { count += 1 })
}

fun main() {
    runUiApplication(title = "Counter", create = ::Counter)
}
```

组件采用持续存在的 class 实例。`render` 可以执行多次，实例及成员 State 不因重建描述而重新构造。用户不为 Component、Element 或使用的控件逐个调用 `use`／`close`。示例中的 button 来自独立控件库。

无状态组合可以写成普通的返回 View 的函数；这种调用不创建独立的挂载身份，其间的 State 读取归调用者当前的 render。需要独立本地状态和生命周期时使用 Component 及第 5 节的 component 入口。

`View` 是 `scoop.ui` 内定义的封闭描述类型，以不可变 class 保存本次输入、子描述或回调。其形状包括空内容、Component、Element、LazyElement 和 Portal 描述。`empty()` 表示没有内容，测量采用约束下限，宽松约束下为零尺寸，不产生绘制或输入区域。一个描述不拥有 InputArea、TextLayout、动画或 GPU 资源；描述本身没有 `close`。

Component 负责组合描述。完全自绘及自定义子项布局通过普通公开接口 Element 实现，外部库不需要继承内部挂载节点或修改 View 的分支：

```kotlin
public interface Element {
    fun measure(context: MeasureContext, constraints: Constraints): LayoutMetrics
    fun scene(context: SceneContext): Unit
    fun onMount(scope: ComponentScope): Unit = Unit
    fun update(): Unit = Unit
    fun onUnmount(): Unit = Unit
}
```

Element 同样采用持久 class。measure／scene 是必需操作；叶子元素的 measure 不访问子项，容器通过公开 context 测量、放置和绘制其子项。update 在首次 onMount 成功后及每次 Props 或子描述更新后同步执行，负责更新输入开关、处理器及动画目标；隐藏或没有 drawable 时也执行。它不执行绘图，也不在其中重入布局。Element 不自行拥有或关闭子组件，子树继续由框架管理。

同一描述值可被使用多次；每个出现位置分别挂载，不能按描述对象引用相等而合并挂载。描述不可变指保存的属性槽及子序列不可修改，不承诺其所引用的业务模型深度不可变。子序列以不可变 Array<View> 保存，允许复用该数组，不因重建描述而强制复制。

`render` 只读取当前输入／状态并产生描述。它不得写 State、操作窗口、取得需要关闭的原生资源、启动任务或登记订阅。measure／scene／draw 允许维护元素自己的测量和绘制缓存，但不得修改 State 或启动动画。State 写入口在活动 render／measure／scene／draw 期间报告状态错误；其他副作用遵循普通 API 的使用约定，不新增编译器纯度检查。Component、Element 与 LazyElement 的构造器只初始化托管状态和保存输入；资源取得放在挂载回调或元素所属的测量缓存中，归属见第 8 节。

## 3. State、变化通知与依赖

### 3.1 状态对象与订阅

`State<T>` 是可变引用类型，提供普通 `value` 属性，并实现属性委托的 `getValue(thisRef: Any): T`／`setValue(thisRef: Any, value: T): Unit`。两个方法均为普通、非泛型、非挂起的 operator。`thisRef` 不用于推断属性名、查找宿主字段或决定状态身份。`state(initial)` 每次调用创建一个新的 State；它没有 remember／hook 槽位语义。

State 的持有者与 UI 订阅者分别确定。一个 State 可以在任何组件挂载前创建，也可以被多个挂载节点读取；不把创建者或属性委托的 thisRef 当成唯一订阅者。组件自有 State、外部 State 的属性委托，以及共享模型中的可观察属性使用同一读取追踪和通知机制。状态值 T 可以是 struct、class 或其他合法类型；把 State 做成 class 不取消读取值类型 T 时的普通值语义。

状态对象绑定创建线程。值的读写及订阅维护在该线程执行；当前 UI 还要求该线程是窗口 owner。后台工作通过既有 owner 投递机制返回结果，不直接修改 UI 状态。相同 owner 上的多个 UiWindow 可以观察同一状态；不同 owner 不共享这套无锁状态对象。

一次 `render` 使用一个库内、同步的依赖收集作用域：

1. State getter 将当前组件节点记入本次读取集合，同一 State 的重复读取只登记一次。
2. 成功执行后，以本次集合替换该组件的旧订阅；不再读取的 State 不再使它失效。
3. setter 先保存新值，再将当前订阅者标记为 dirty；不在 setter 内递归调用 render。
4. 组件卸载时解除所有订阅，并从待更新集合移除；共享 State 本身仍然可用。

首期每次合法 setter 调用都发出变化通知，包括写回相同值。调度器合并同一轮更新，不为任意 T 引入隐式相等、深比较或接口约束。相等策略和专用集合增量通知以后由实际性能需求决定。

同一同步业务操作中的多次写入分别立即生效，UI 工作在既有安全边界合并；这不提供跨挂起点的事务或状态快照。直接订阅粒度是实际读取的 State；父组件生成新的子描述时仍沿第 4 节更新子项，不承诺只执行直接订阅者的 render。

普通 getter／函数调用中的间接 State 读取，同样归当前活动 render。render 之外的读取只是取值，不产生永久订阅。首期不自动建立 update／measure／scene／draw 阶段的独立依赖图；声明式自绘组件在 render 中读取 State，再通过 Element 的 Props 传入本次绘制数据。Element 自己在输入处理中修改的普通交互字段，通过 scope.requestFrame 显式请求呈现。Surface Animatable 的采样走第 7 节的帧通路，不需要先转换成 State。

普通 class 字段不是可观察属性。对 `modelState.value.name` 赋值不会经过 `modelState.value` 的 setter；`name` 应有自己的可观察属性，或调用方显式赋回状态值。默认 setter 不按引用相等跳过通知。对普通可变集合的原地修改也遵循此规则。

### 3.2 自有状态与共享 Store

Store 是应用定义的普通 class，按业务组织一组 State 或可观察模型。框架不要求 Store 基类、字符串 key 注册表或独立 action／reducer 协议。固定业务字段使用普通属性和类型检查；共享一个 State 直接写成 `var count: Int by store.count`，其中 store.count 的类型为 State<Int>。若 Store 自己使用委托属性，组件也可以在 render 中通过普通 getter 读取，或通过第 4 节的 Binding 连接写入。

| 声明 | State 的持有者 | 卸载后再次挂载 |
| --- | --- | --- |
| `var count: Int by state(0)` | 当前组件实例 | 新组件创建新的 State，使用初始值 |
| `var count: Int by store.count` | Store；组件借用同一 State | Store 仍保留该对象时，继续使用已有值 |

两种声明可以出现在同一个组件中。State 本身是托管对象，无需逐个 use；共享 State 不因任一订阅组件卸载而重置或关闭。应用级 Store 可跨页面或窗口保留，文档级 Store 可随文档结束而释放；资源和任务的实际归属遵守第 8 节。

`by` 表达式沿语言规范 9.2.1 只在该属性初始化时求值一次，随后访问保存的 delegate。Store 引用或用于选择 State 的业务输入后来变化，不自动使委托换绑；变化输入应在 render 中经 Props 读取，或按第 5 节更换挂载身份。`val count: Int by store.count` 只禁止通过这个属性赋值，不冻结共享 State 的后续值。普通业务方法可以集中约束写入，框架不要求额外命令语法。

### 3.3 Context 与挂载时绑定

应用可用语言已有的 [Task-local Context](SCOOP-SPEC.md#83-task-local-context) 提供 Store，通过计算属性或普通辅助函数声明依赖。Component／Element 的协议和构造器语法不变；不在 override 上增加与接口不同的 Context requirement。以下示例同时使用共享与组件自有 State：

```kotlin
import scoop.ui.*
import scoop.ui.component.button

class AppStore {
    val count: State<Int> = state(0)
}

context(store: AppStore)
val currentAppStore: AppStore
get() = store

class SharedCounter: Component {
    private var count: Int by currentAppStore.count
    private var localClicks: Int by state(0)

    public override fun render(): View =
        button(
            f"Shared: ${this @SharedCounter.count}; local: ${this @SharedCounter.localClicks}",
            {
                count += 1
                localClicks += 1
            }
        )
}

fun main() {
    val store = AppStore()
    context(store) {
        runUiApplication(title = "Shared state", create = ::SharedCounter)
    }
}
```

此处 Context 在实际创建 SharedCounter 时提供 Store，随后组件直接持有 State。Context 按精确静态类型查找；同类型的多种业务角色使用普通 nominal wrapper，缺失按既有 MissingContextException 处理。Context 绑定快照保留 Store 的引用，不复制其中的状态，也不赋予跨线程读写权限。

应用级绑定覆盖 Application.run／runWithAsyncio 或单窗口便利入口；窗口业务回调沿窗口规范恢复运行时的 Context。普通闭包、函数引用、View 描述与 UiHost.attach 不因创建或调用于某个临时 Context scope 而自动捕获它。仅在 scope 中返回一个延迟 View，不保证稍后挂载时仍能取得 Store；延迟入口应显式捕获所需对象并在执行时安装 Context，或使用覆盖实际执行过程的外层绑定。

不同窗口需要不同依赖时，应用在相应窗口的实际业务／挂载入口安装绑定，或显式传递模型；不把 Context 的调用作用域当成挂载子树的环境继承。重新绑定 Context 不通知 State 订阅者，也不替换已保存的 delegate。首期只提供这种挂载时绑定，不新增 UiLocal、Provider 描述、节点环境快照或自动切换整棵子树 Store 的协议。

## 4. Props 与 Binding

### 4.1 变化输入

组件的构造器参数在实例生命周期内只初始化一次，不能用每次变化的构造器捕获来传递更新。需要变化的父级输入使用 `Props<P>`：它是只读引用容器，公开 `value: P`，由框架在 Component 本轮 render 前或 Element.update 前更新。

```kotlin
import scoop.ui.*
import scoop.ui.component.label
import scoop.ui.component.column

class Greeting(private val props: Props<String>): Component {
    public override fun render(): View = label(props.value)
}

class Screen: Component {
    private var name: String by state("Scoop")

    public override fun render(): View =
        column {
            this.add(component(props = this @Screen.name, create = ::Greeting))
            this.add(component(::Counter))
            this.spacing = 12.0
        }
}
```

`Props<P>` 由组件节点持有，不允许子组件替换父级输入。它是当前输入的入口，不是自动保存旧版本的快照。需要异步操作使用启动时的参数时，应先复制需要的值到局部 `val`；要在调用时读取最新输入的回调，则在回调体内访问 `props.value`。

父组件产生新的子描述时，框架更新 Props 并执行对应的 render 或 update；初期不根据 P 的相等或引用身份跳过。每个节点在同一逻辑更新轮中至多执行一次；与该轮处理同时登记的新失效留到下一轮。只读输入本身不产生第二套可观察模型。

### 4.2 可写绑定

`Binding<T>` 是带 getter／setter 的普通引用对象，通过 `value: T` 提供读写，用于明确允许控件修改外部状态。可以由显式 State 的 `binding()` 取得，也可使用 `binding(get = { count }, set = { count = it })` 连接委托属性。Binding 不复制状态、不保存另一份当前值；getter／setter 继续执行原 State 的追踪与线程规则。绑定到普通不可观察字段不会自动产生变化通知。不增加 `$property` 或取得隐藏 delegate 字段的语法。

随 Props 传入的 Binding 可以变化。控件每轮 render 读取当前绑定，在交互时使用当前 Props 中的写入路径；不能在构造器中用 `by props.value.checked` 固定第一次输入。成功 render 后沿第 3 节替换依赖，新绑定的后续变化生效，旧绑定不再单独使该控件失效；控件自身的交互状态和挂载身份按通常 Props 规则保留。

### 4.3 控件属性与派生输入

公共访问权限与可观察性分别由普通属性声明和 State 决定。实际持有合法组件实例或普通模型时，读取其公开 getter 内部访问的 State，会订阅当前 render 的挂载节点。View／component／控件便利函数返回的是描述，首期不提供获取挂载实例的 ref 协议，也不允许从 View 直接访问某个实例的 checked。

普通只读输入如 Button.enabled 通过 Props 接收值；用户交互可修改的值如 Checkbox.checked 通过 Binding<T> 接入外部状态；hover／pressed 等内部表现使用私有 State。多个公开状态可以组织为普通模型对象，不要求每个控件另建专用状态类型。组件自有状态与外部 Binding 的具体入口由控件库定义，见[控件库规范](SCOOP-UI-COMPONENT-SPEC.md)。

Checkbox 与 Button 联动时，组合组件在 render 中读取 checked 的 State 来计算 enabled，Checkbox 写入同一个 State。直接订阅者是执行该读取的 render；组合组件重建后通过 Props 更新 Button，不需要另存 enabled State。普通赋值 `button.enabled = checkbox.checked` 只赋值一次；以另一个属性初始化 State 也只取得初始值。框架不把任意赋值或初始化转换成永久属性绑定。

## 5. 创建、身份与复用

组件构造发生在挂载时，公开入口的签名形状为：

```text
fun <C : Component> component(
    create: () -> C, key: ViewKey = ViewKey.Position
): View
fun <P, C : Component> component(
    props: P, create: (Props<P>) -> C, key: ViewKey = ViewKey.Position
): View
fun <E : Element> element(
    create: () -> E, children: Array<View> = [],
    key: ViewKey = ViewKey.Position
): View
fun <P, E : Element> element(
    props: P, create: (Props<P>) -> E,
    children: Array<View> = [], key: ViewKey = ViewKey.Position
): View
fun portal(
    open: Binding<Boolean>, content: View, configure: PortalBuilder.() -> Unit = {}
): View
```

`ViewKey` 是包含 Position、Text(String)、Number(Long) 的值类型。Position 表示采用同一父级子序列中的位置；另外两个分支是显式 key。

`component`／`element` 本身只构造描述，不能立即调用 create。首次挂载才调用工厂，并保存其返回实例；之后匹配到同一节点时更新 Props 及子描述，复用实例。无 Props 的入口统一使用 Unit 输入。Element 的 children 由框架按同样规则挂载，空数组表示叶子。

节点形状包含 Component／Element／LazyElement 类别、`typeInfo<C>().type` 或 `typeInfo<E>().type`，以及 `typeInfo<P>().type`。用完整 TypeRef 相等比较，不比较展示名、函数值引用、描述对象引用、源码行号或分配地址。P 是内部泛型承载的组成部分，输入类型改变时重新挂载，不能把旧的 `Props<P>` 强转为另一类型。

Portal 是独立的描述类别，不创建用户 Component／Element 实例。相同位置／key 的 Portal 复用同一层会话，更新 open Binding、options 和唯一子描述；子内容按受控打开状态及 Presence 阶段匹配、挂载和释放。层显示位置不改变逻辑 parent，跨根不共享挂载或输入区域。相同 key 在退出中重开时保留子树；最终移除后的再次打开创建新的内容挂载。

C／E 必须保留 class 类型。库使用既有的一层 TypeShape 描述拒绝接口、object 或值类型作为具体实例类型，不要求新的编译器诊断。相同静态类型下改变工厂闭包或返回子类，并不改变已挂载实例；工厂只负责初次创建。需要选择不同实现的调用方应保留不同的具体类型，或显式更换 key。

同一个 Component、Element 或 LazyElement 实例只能属于一个挂载位置。工厂必须为新挂载创建新实例，不能返回某个已挂载的单例。框架检查当前活动实例的重复挂载，不为已经退休的对象保留全局历史表；为新挂载返回新实例是 create 的使用合同。共享业务状态应通过普通模型／Props／Binding 表达。

匹配规则如下：

| 情况 | 结果 |
| --- | --- |
| 同一父级、同一显式 key、同一组件形状 | 复用原实例；允许在兄弟间移动 |
| 没有显式 key、同一父级及位置、同一组件形状 | 复用原实例 |
| 组件形状或显式 key 改变 | 卸载旧实例，挂载新实例 |
| 离开父级，或本轮描述中消失 | 卸载；之后出现时创建新实例 |
| 仅尺寸、布局位置或绘制属性改变 | 保留实例与状态 |
| 同一父级出现重复的显式 key | 报参数错误，不能任意复用其中一个 |

key 的有效范围仅为直接父级，不是全局 ID；移动到另一父级不保留挂载。同一兄弟序列中显式 key 必须唯一，不因节点类型不同而允许重复。Position 以完整兄弟序列的索引计算；混用 keyed／unkeyed 节点时，插入前方兄弟仍会改变后者的位置。

列表示例应以稳定业务 ID 构造 `ViewKey.Number(item.id)` 或 `ViewKey.Text(item.id)`。普通 `if`／循环没有隐式源码身份：两个分支在同一位置返回相同组件形状及 key 时保留状态；需要区分时显式使用不同 key。容器的插入、删除或替换可能改变父级身份，不能承诺跨该结构变化保留子状态。

## 6. 重建、布局与呈现

State 变化合并到 owner 上的 UI 工作队列。事件／业务回调退出后，经现有 ApplicationProxy 投递处理；不在活动输入派发、render、layout 或 paint 内递归重建。祖先和后代同时失效时，先处理祖先，已被该轮协调实际更新的后代不重复 render；未访问的失效后代仍须完成自身更新，不能因祖先复用 lazy 数据源或同一 View 而丢失通知，也不依赖失效入队顺序。没有排队更新时不投递任务，也不在 AboutToWait 遍历树。

UI 逻辑提交与 surface 的呈现提交有不同的时机：

- 逻辑更新执行 render、匹配节点、Props 替换、Element.update、挂载／卸载及订阅更换。隐藏、遮挡、零尺寸或暂时没有 drawable 不能阻止这一过程及资源清理。
- 需要显示的变化向 SurfaceWindow 请求帧。在真实绘制机会中，Surface 先采样动画，再由 UI 遍历挂载树执行测量／布局、场景装配、纯绘图块执行和 InputArea 放置。
- 只有 surface 成功接受提交后，才按 Surface 规范发布命中几何；失败或 Deferred 不发布新几何，不回滚已经生效的业务状态。

首期在 owner 上同步完成一轮逻辑更新，不提供并发、暂停恢复或丢弃后重放 render 的机制。所有更新使用当前可变状态；不承诺 React 式的不可变状态快照。用户 render／构造／挂载回调失败时走第 9 节的关闭路径，不试图回滚用户修改过的任意对象。

新描述中的事件处理器随逻辑更新替换。尚未提交新的画面时，保留最近已提交的几何与节点身份，派发使用节点最新的处理器。新节点在首次成功放置前不能命中；已卸载节点立即退出交互，不能为了等下一帧而继续接收事件。这里承诺几何对应最近提交的帧，不承诺处理器保留旧帧捕获的输入值。

普通元素在每次实际呈现中测量其直接子项，LazyElement 则按第 7.8 节只准备和测量所需项；昂贵的文本／路径计算由实现它的 Element 按实际输入缓存。这样 Surface 动画可以直接参与本帧尺寸和位置计算，不要求 UI 为每个动画另外建立布局失效订阅。逻辑更新仍按需执行，不因动画每帧采样而重新创建描述或重跑所有 Component.render。

所有自绘元素共享所属窗口的 FrameContext 和渲染目标，不为每个元素创建窗口、renderer 或独立 Surface。窗口 resize／DPI 变化在下一帧进入相同的测量／绘制流程，不重建组件实例。普通元素的子序列使用显式 `Array<View>`，按需元素使用 ItemSource；不把普通 lambda 的未使用表达式冒充 result builder。

### 6.1 静止时停止渲染工作

UI 采用按需调度。一个根在逻辑工作队列为空、没有尚待呈现的失效、没有需采样的运行中动画或待处理的动画完成回调，并且没有新的宿主重绘请求时进入静止。最后的必要帧、已排队通知和实际 GPU 使用正常收尾后，只要没有新的工作来源，就必须保持以下行为：

- 不调用 Component.render、Element.update、measure 或 paint，不执行树匹配、布局遍历、文本重排及输入几何重建。第 6 节的“每次实际呈现都遍历”只规定有帧时的处理，不构成持续生成帧的理由。
- 不为检查状态、Props 或动画是否变化而定时扫描，不投递空的 UI 更新任务，不保留持续运行但每次直接返回的帧回调。State 订阅和挂载树可以保留，变化由实际写入或事件通知。
- 撤销所属 Surface 的帧需求，停止原生帧通知的持续唤醒；不获取新的绘制目标、借用 Canvas、录制绘图命令或调用 submit／present，也不反复呈现同一静态画面。
- UI 不以 Poll、固定周期 timer 或不断重新投递的回调维持空转。没有其他应用工作时，宿主阻塞等待事件；真实 I/O 或未来一次任务的 deadline 可以使用既有等待机制，但其唤醒不自动引发 UI 重绘。

首次挂载、State／Props 失效、显式 requestFrame、动画启动／恢复，以及系统要求重画、窗口尺寸／DPI／主题变化，都可以重新唤醒相应工作。多个请求按已有规则合并，处理中产生的新失效不得丢失；工作完成后再次回到静止。第 3 节的 State 同值写入仍属于显式失效，本节不改变 setter 语义；组件或应用主动持续请求帧属于持续工作来源，框架自身不能凭空产生这种请求。

持有 Animatable 不代表动画正在运行。完成、暂停、取消和卸载应及时结束其持续帧需求；最终值及完成回调引起的必要变化完成呈现后停帧。运行中的显式重复动画可以持续请求帧，已完成动画不能因还保存在 scope 中而继续驱动时钟。

需求按根／窗口维护。一个窗口的动画不能使其他无变化窗口执行 render、布局或提交；没有有效 drawable 时保留失效并等待原生恢复，不通过循环 acquire 或逻辑任务自我投递重试。隐藏窗口的真实 State 更新和卸载仍按需处理，时间轴恢复语义沿 Surface 规范，不需要在隐藏期间持续采样。

未来视频帧到期或 3D 场景的显式持续运动属于外部绘制需求，接入时同样必须能够撤销。暂停视频、静止场景以及外部宿主继续转动物体但 UI 纹理未变，都不能使 UI 自行维持无用帧。首期 Surface 的停帧合同见[第 4.1 节](SCOOP-SURFACE-SPEC.md#41-空闲帧需求与原生等待)。

本要求约束 UI／Surface 引起的周期唤醒和渲染工作。已有 GPU 命令的完成、系统合成与显示扫描、应用独立的音频／网络任务不等同于 UI 持续渲染；不以等待 GPU 空闲来阻塞普通帧，也不以整个进程或桌面的 CPU／GPU 读数绝对为零作为合同。

## 7. 自绘、布局、输入与动画

### 7.1 测量及子项布局

`Constraints` 按轴保存有限非负最小值和可选的有限最大值，最大值存在时不得小于最小值。缺少最大值表达无界，不把 Infinity 传给 graphics 或 graphics.text。`constrain(size: Size): Size` 将一个有限非负期望尺寸收敛到约束。Element.measure 返回 LayoutMetrics，其 size 必须满足收到的约束：

公开字段为 minWidth、maxWidth、minHeight、maxHeight，默认最小值为 0.0，最大值为 None；`Constraints.tight(size)` 同时固定两轴。与 graphics 的几何值一样，在测量或 constrain 操作边界检查值的合法性。

```kotlin
public struct LayoutMetrics(
    val size: Size,
    val alphabeticBaseline: Option<Double> = None,
    val ideographicBaseline: Option<Double> = None
)
```

基线是相对元素顶边的有限有符号偏移，文本采用首行基线；None 表示元素没有该基线。外框裁剪不要求基线落在其高度范围内。空内容采用约束下限尺寸和两项 None。Component 原样透传子描述的尺寸与基线；容器可以根据子项偏移给出自己的基线，未定义时返回 None，不隐式扫描后代。框架不扩展成通用 alignment-line 系统。

MeasureContext 提供本帧 time、当前 layoutDirection、子项数量、`measureChild(index, constraints): LayoutMetrics` 和 `placeChild(index, origin: Point)`。Element 在自己的 measure 中决定子约束、位置和最终 LayoutMetrics。每个直接子项至少成功测量一次，尚未放置时可以使用不同约束重新测量，最后一次结果生效；随后可以放置零次或一次，放置后再测量属于状态错误；没有放置表示此次布局不呈现该子树。每轮测量先清空旧放置，不能把上帧位置遗留给本帧未放置项。结果属于本次测量，不跨回调保存 context、子节点句柄或旧索引。 相邻的同约束请求复用最后成功结果与对应子树布局，不缓存历史结果来配合另一套 placements。重新测量不重跑 render、替换 Props 或改变挂载身份；时间仍为同一帧采样。测量失败清除本次结果，不能凭旧结果继续放置。

子项保持描述中的逻辑顺序，框架根据父级放置累计绘制坐标。未放置的子树保留挂载、Props、State、订阅和资源，继续接收逻辑更新；该子树本帧不执行 scene，也不放置 InputArea。普通 Element 的子树放置资格在 Accepted 帧后更新既有 AnimationGroup 的继承开关；未放置时不请求动画帧，重新放置沿原时间轴恢复，失败帧保持上次资格。没有子项放置时即使省略 children() 也执行此收尾，不新增独立时钟或可见性扫描。祖先未放置时，后代在其内部测量中取得的位置也不会使它们单独呈现。该协议支持行内对象先测量、再根据段落结果选择放置，不要求多轮 intrinsic 协商。

根约束的最小值和最大值均为当前 FrameContext.size，使根填满窗口内容区。Component 将约束原样传给 render 返回的描述，并透明转交绘制；它本身不增加 padding 或对齐策略。子项数量和索引使用 Long，位置为有限 Point，允许显式负偏移。越界、遗漏或重复测量、未测量就放置、重复放置按状态／参数错误报告；测量后不放置是合法结果。

测量允许更新由本 Element 拥有的缓存，例如按宽度替换 TextLayout。创建、替换和失败清理必须由同一个所有者完成；不得把待关闭资源存入临时 View。文字 shaping 与断行由 graphics.text 完成，通用段落缓存及行内子项接入见第 7.5 节；Label 选择自己的输入、外观和裁剪策略。

### 7.2 场景装配与纯绘制

Element.scene 接收 `SceneContext`，提供本帧 time、最终 size、输入区域放置和子项呈现。它不暴露 Canvas。`concat／translate／scale／rotate`、`clipRect／clipPath`、`save／saveLayer／restore` 是影响本场景画面与输入的共同操作，交由 graphics 的同一 Canvas 状态维护，不另建坐标计算实现。原点为元素左上角，DPI 仍只由渲染目标根部施加一次。

`SceneContext.draw(block: (PaintContext) -> Unit)` 提交并同步执行一个直接绘图块；块只负责像素和自身绘制缓存，不登记 InputArea、呈现子项或修改 State。PaintContext 提供 size、time 和隔离借用的 Canvas；块中增加的 transform／clip 仅影响该块的画面，退出后不影响场景中后续区域或子项。场景几何与绘制块的调用分开，省略一个绘图块不会省略已声明的区域。可缓存的有限图层使用第 7.2.1 节的独立入口。

场景操作在同一个实际帧内流式提交和消费。它们不持有跨帧借用，Path／滤镜在同步调用时按 graphics 合同使用；不要求元素为装配另外复制原生资源。真正的跨帧绘制记录／图像缓存另按其拥有关系持有资源。当前还不承诺自动跳过未失效绘图块。

`SceneContext.children()` 按描述顺序呈现本次已测量、放置的直接子项，跳过未放置子树。有已放置子项时必须调用一次；没有已放置子项时可不调用或调用一次。重复调用报告状态错误。元素可在调用前后提交背景和前景 draw；子项拥有独立场景作用域，不能把变换泄漏到兄弟。父子关系不隐式裁剪，输入父关系由挂载作用域确定。

`SceneContext.children(order: Array<Long>)` 指定这一遍的子项呈现顺序，order 必须是全部直接子项索引的完整排列，包含本次未放置的子项；未放置项仍跳过。长度、重复或越界错误在任何子项呈现前报告参数错误，失败后仍可提供有效排列。两个重载共用一次呈现计数及 withState 的借用规则。此顺序只影响场景／绘图和重叠命中，不改变逻辑 children、挂载身份、事件父关系或 FocusScope 的 Tab 顺序。

`SceneContext.withState<T>(block: (SceneContext) -> T): T` 继承 size、time 及同一子项呈现计数，隔离变换／clip／图层状态；子作用域内拒绝使用父 context。draw 回调期间同样不能使用外层 SceneContext，即使通过闭包取得引用也会报状态错误。正常或异常退出均恢复 Canvas 保存层级，结束子 context，未配对的内部 save／saveLayer 不会污染调用方。

`PaintContext.withState<T>(block: (PaintContext) -> T): T` 只隔离纯绘图块内的 Canvas；没有区域或子项接口。SceneContext、PaintContext、Canvas、MeasureContext 都只在对应同步借用内有效，不能跨帧、跨线程或跨挂起保存。用户场景／绘图异常按第 9 节关闭，失败帧不发布几何。输入的变换、裁剪和 layer 顺序均使用实际 graphics 状态，不以视觉滤镜的像素扩展扩大区域。

`SceneContext.onFrameComplete(action: (scoop.surface.FrameCompletion) -> Unit)` 为当前实际场景登记一次性通知，随所属挂载节点保存到本帧结束。UiRoot 在 Surface 的完成回调中按登记顺序交付，回调中的 time／size／submission 来自该帧。此时 render／measure／scene／draw 都已退出，可以写 State、请求焦点或后续帧；这些变更按既有更新边界处理。回调执行前节点已经卸载，或根开始关闭时，取消该节点的通知；回调中关闭根也取消剩余通知。异常按普通业务回调失败关闭根，不继续交付。未进入 scene 的子树没有通知，不产生为兑现通知而轮询的帧。

需要新几何的动作只在 Accepted 中执行；Discarded 可丢弃本次位置结果，不能将其发布为可交互位置。FrameCompletion.size 是窗口内容区尺寸；元素若需要自身尺寸，应在 scene 中按值捕获。通知不能用于必须执行的资源释放，也不能延长 SceneContext／PaintContext／Canvas 的借用。API 不保存永久帧订阅；持续动画仍由 Surface 时间轴驱动。

### 7.2.1 有限绘图层与分块像素缓存

`PaintKey()` 创建普通的稳定绘图标识，由 Element 或公共绘图宿主持有；它没有资源、全局序号或输入归属。同一元素的一次 scene 中，一个 key 最多使用一次，不同元素独立使用。`SceneContext.drawLayer(key, bounds, revision, samplingOutset = 1.0, block)` 声明一个以 bounds 裁剪、以 SrcOver 独立合成的有限绘图层。它复用 PaintContext 的纯度与借用规则；最终场景变换、clip、父级 opacity 和滤镜在合成时应用。

drawLayer 的回调始终在独立局部记录 Canvas 中执行，不继承目标变换、clip 或 DPI；同一内容在缓存开关两种状态下使用同一坐标语义。调用方为影响输出的输入提供 revision；相同 key、revision、size、bounds 和 samplingOutset 表示可复用相同内容。State、时间、动画采样和绘图资源改变须反映在 revision 中，框架不猜测任意可变闭包的依赖。不能提供可靠版本或有限范围，以及需要直接作用于背景的绘图，继续使用 draw。

`PaintCachePolicy.None` 每帧执行、重放并释放图层记录，不保留绘制记录或图像。`Tiles(tileSize = 256, overscan = 1)` 保留未变化的记录，并把可见区域附近的有限像素块光栅化为兼容 Image；命中时复用实际图像，不仅重放 Drawing。tileSize 是正整数设备像素，overscan 是非负块数，与 VirtualList 的逻辑长度 overscan 无关。像素块使用对齐到局部原点设备像素的网格，不从任意字形边界起算；边缘块中 bounds 外的部分保持透明。只有当前 clip 与图层 bounds 相交的范围及相邻块参与，不为整个内容分配 bitmap。

samplingOutset 是有限非负逻辑长度，描述插值及已声明命令需要的采样边缘；每轴至少保留一个设备像素。bounds 必须包含完整输出，不能直接用布局盒子截断字形、阴影或光晕。缓存块先绘制扩展范围，再用 drawImageRegion 合成核心区。无法给出可靠范围时使用 draw；扩大边缘不能把底层不可独立裁剪的效果变成可分块记录。正的轴对齐缩放按实际像素变换选择分辨率；旋转、错切、反射、不可表示的块坐标或尺寸使用不保留像素的图层重放。

缓存也遵守 Graphics 的 Drawing.canRasterizeTiles。记录内部含 ImageFilter 或未声明可独立裁剪的原生命令时，同一回调产生的记录直接隔离重放并释放；不保留记录或图像，不用修改像素容差掩盖裁剪引起的效果差异。可以把滤镜放在 SceneContext.saveLayer 的合成阶段，使正文等输入块保持可缓存，同时在本帧完整内容上执行滤镜。

revision、size、bounds、samplingOutset、像素缩放或缓存参数改变时仅重建对应绘图层；后端 context 不兼容时重建该层的记录和图像。单纯滚动平移、最终 clip、父级合成效果改变不作废仍在缓存范围内的内容块。离开范围的块及时释放；未在本次 scene 使用的 key、不参与帧的子树、None 策略和卸载均释放相应缓存。图像和记录不持有 Component、输入处理器或用户绘图回调。

ScrollViewport.paintCache 在逻辑阶段整体更新，默认 None，改变后请求必要帧。其 present 内容回调把策略传给所呈现的子场景；嵌套视口采用自身策略，Portal 在独立呈现场景中处理。缓存命中仍执行 scene、输入／anchor 登记、子项呈现与帧完成通知，不影响逻辑更新、焦点、IME 或实例生命周期，也不产生持续帧需求。Discarded 的图像只作为对应内容版本的内部缓存，不能发布该帧输入几何或布局状态。

### 7.3 输入与焦点接入

Element 在 onMount 中通过 `scope.createArea(focusable, textInput, parent, handler)` 创建稳定的 Surface InputArea，作用域登记其生命周期。处理器使用 RoutedEventContext／InputEvent；传播和默认行为通过 area.scope 的 handler／captureHandler／defaultAction 设置。框架不将原始事件解释为 Button 的 onClick。

ComponentScope.input 提供该挂载的稳定 InputScope，按需创建并沿逻辑挂载祖先连接；根挂载使用 SurfaceWindow.input。createArea 的 parent: InputArea? 缺省 None，连接本挂载的 input；显式父区域须属于同一挂载，允许通过该挂载自己的区域嵌套，不能穿过另一个挂载的 InputScope。未使用输入的子树不为此预建全部 scope。作用域退休时立即退出路由，资源仍按既有子先父次序关闭。

容器可以用 `scope.routeChildren(area)` 将同一挂载拥有的区域作为直接子挂载的输入父节点；缺省为自身 input。该选择属于挂载逻辑，与当前布局、clip 和显示层无关。更新此选择时重新连接已经存在的直接子 scope，后续创建的子 scope 使用同一选择。createArea、routeChildren、focusTarget 和 Press／Drag 使用同一归属检查：沿区域父链可到本挂载 input，中间不得经过其他挂载作用域，因此不能借此把后代挂载节点变成本挂载子项的共同父节点。

UI 管理的 scope.parent 由挂载和 routeChildren 维护，调用方通过这两个入口改变关系，不在框架外手工重接这些 scope。

SceneContext.place(area, rect或path) 只登记当前几何；children() 只呈现子项，不改变路由。框架不为每个布局节点自动创建 InputArea。显式区域须来自同一窗口，同一区域每帧至多放置一次，绘制顺序可以与逻辑父子顺序不同。

update 在逻辑阶段修改处理器和 enabled 等属性，不等 scene 才应用禁用。保留区域使用最新处理器与最近提交的几何；新区域等待首次成功放置。指针序列、取消、冒泡、局部坐标、hover、逻辑焦点及 IME 路由沿用 Surface，点击／拖动等控件行为由上层实现。

由于行内截断等布局原因不再放置的区域，在该帧成功提交后按 Surface 的区域缺席规则退出命中、取消指针序列并清除焦点；组件本身仍挂载。失败／中止帧保留此前已提交的几何，不能提前使用未提交的可见性。恢复放置不重新创建组件，也不自动恢复已经清除的逻辑焦点。布局可见性不替代逻辑 enabled 或卸载。

移除子树时，框架先使其退出业务交互并注销登记区域，完成必要的取消／离开／失焦收尾，再释放处理器和组件资源。布局尚未提交时不能为新区域强行成功请求焦点。文本编辑的正文、selection、marked range、剪贴板和撤销由第 7.5 节公共文本宿主维护，不进入底层路由或组件匹配机制。

#### 7.3.1 FocusHandle、FocusScope 与 Tab

FocusOptions 增加 `visibility: FocusVisibility = Automatic`，其 builder 同步提供该属性并写入所属 Surface InputArea。FocusHandle 的 `isFocusVisible` 查询同一区域；`requestFocus(origin: FocusOrigin = Programmatic)` 保留来源，延迟显露的 PendingFocus 保存来源直到 Accepted 后兑现。Tab、roving 和 lazy 键盘导航传 Keyboard；Portal 恢复沿目标的既有来源，不把指针触发器恢复成键盘指示。真实焦点与其视觉指示独立，Never 不妨碍键盘输入或 IME。

InteractionState 和 TextEditInteraction 同时包含 focused 与 focusVisible。Press／TextEditHost 订阅 Surface 的统一指示变化，不各自保存最近输入设备。TextEditHost 将 Automatic 解析为 Always，使可编辑及只读正文的指针聚焦均有明确指示；调用者的 Always／Never 原样保留。仅 focusVisible 变化不得重新建立编辑会话、改变选区或结束 composition。

FocusHandle 是 owner 线程上的稳定目标引用，不保存第二份焦点状态。isAttached 表示当前仍登记到活动挂载且区域未关闭；hasFocus／isFocusActive 查询所属 Surface 区域；isPending 表示等待提交的当前请求。requestFocus 返回是否接受请求：已显露目标立即聚焦，滚动内容中的目标先按第 7.7 节显露，再在成功帧聚焦；已登记但等待首次／恢复放置的目标请求一帧并等待成功提交。未登记、已退休或逻辑禁用的目标返回 false。clearFocus 取消该 handle 的待处理请求，并在它是当前焦点时清除焦点。提前关闭 InputArea 使其旧句柄失效；随后可将同一句柄登记到新的活动目标，旧登记的晚到清理不能解除新绑定。

ComponentScope.focusTarget(area, options: FocusOptions = FocusOptions()) 将本挂载直接拥有的可聚焦区域登记为焦点目标，返回随挂载退休的 FocusTarget。同一区域只登记一次，一个 FocusHandle 同时只属于一个活动目标。FocusOptions 包含可选外部 handle、tabStop=true、order=0、visibility=Automatic；没有外部 handle 时目标拥有自己的稳定 handle。FocusTarget.options 在逻辑阶段整体更新，外部 handle 改变时解除旧引用并取消旧请求，区域及其已有焦点不被重建。区域及逻辑祖先的 enabled 是唯一启用状态来源。

`FocusTarget.reveal(rect: Rect): Boolean` 显露该区域局部坐标中的子矩形，不改变键盘焦点或激活 OS 窗口。它用于文本光标等小于整个控件的目标，沿第 7.7 节相同的成功场景及滚动路径逐层处理；矩形必须有限且非负，空矩形返回 false。该请求与普通聚焦共用一个待处理位置意图，handle.isPending 也反映尚未完成的局部显露。更新的显式焦点／显露、已调整视口的新滚动、目标退休或输入域变化取消旧请求；Discarded 不兑现。首次尚无几何时等待一次成功放置，之后仍不可达则结束，不轮询。

每个 ComponentScope 可登记一个 focusScope(options: FocusScopeOptions)，返回 FocusScope。options 包含 loop=false、order=0、defaultFocus 与 fallback。scope 内目标按 order 升序、同值按当前逻辑挂载顺序排列；嵌套 FocusScope 是父序列中按自身 order 排列的一个连续分组，组内独立排序。keyed 子项重排后使用新逻辑顺序，不使用原注册时间或绘制顺序。负 order 只改变排序，tabStop=false 才退出 Tab，仍允许程序化请求。

Tab／Shift+Tab 在内层默认行为未消费后导航。跳过禁用、未放置及不能显露的目标，已放置的滚动内容可以先显露再聚焦。普通 scope 到边界后让父继续，loop scope 在组内循环，窗口根始终在本窗口循环。没有已登记的当前焦点时，从首／末个可用停靠点开始。真实按下和重复按下可导航，合成按键或带 Command／Control／Alt／Function 的组合不作为普通 Tab。键盘导航不激活 OS 窗口，也不修改控件选择状态。

FocusScope.requestFocus 按有效 defaultFocus、首个可用 Tab 目标、有效 fallback 依次选择；fallback 可是 tabStop=false 的区域代理。子项尚未挂载／放置时可等待下一成功帧后选择。域外或失效的引用不作为目标，无可用目标时请求结束。

UiRoot 在 Surface 的 Accepted 完成边界先发布本帧的滚动内容路径，再处理层的移除、模态恢复与首次聚焦，随后兑现当前待处理焦点，最后执行用户登记的 SceneContext.onFrameComplete 动作。请求记录 Surface.focusRevision；UI 接受的待处理请求、已知窗口内被域拒绝的显式请求通过 reserveFocusRequest 推进同一个序号，立即聚焦仍由 InputArea 推进一次。期间另一次显式焦点意图、目标退休、外部 handle 更换或根关闭都会使旧请求失效。未绑定的 handle 没有所属窗口，不能改变其他窗口的请求。Discarded／Deferred 不兑现或轮询；Accepted 后没有可进行的显露且仍不可用的普通目标结束请求。Tab 显露失败时继续当前遍历中尚未尝试的目标，不在同次按键中无限循环；真正的目标退休或新的显式意图取消旧遍历。焦点请求和登记配置变更禁止发生在 render／measure／scene／draw 中。

#### 7.3.2 KeyNavigation 与组内导航

ComponentScope.keyNavigation(options: NavigationOptions) 登记本挂载拥有的无样式导航组，返回随挂载退休的 KeyNavigation；每个挂载至多一个活动组。NavigationOptions 包含不可变 items: Array<NavigationItem>、layout: NavigationLayout = Vertical、direction: LayoutDirection? = None（继承当前挂载方向）、loop=false、roving=true。NavigationItem 包含稳定 FocusHandle 及 row=0、column=0；Horizontal／Vertical 使用数组顺序，Grid 使用显式行列位置。

options 在逻辑阶段整体更新；重复 handle、同一个 handle 属于两个活动导航组、Grid 重复行列坐标均报告配置错误，并保留旧配置。允许先配置尚未挂载的 handle，但它随后只能绑定到该导航组的逻辑子树。关闭组解除成员关系；替换目标的外部 handle 后，旧 handle 不会继续控制新引用。enabled 和可聚焦性仍来自原区域与逻辑祖先，没有另一份成员禁用状态。

roving=true 时仅一个有效成员参与 Tab：优先当前聚焦且符合 Tab 条件的成员，然后选择最近成功聚焦的有效成员，未有历史则选数组中首个。历史使用 Surface 的聚焦请求记录，包含鼠标及直接 InputArea 请求；当前持有焦点不等于必须有 OS 活动焦点。显式 tabStop=false 始终退出 Tab，但仍可通过箭头聚焦。roving=false 恢复成员各自的普通 Tab 条件，组内导航仍有效。已放置的滚动内容可以通过相同显露流程成为目标；未挂载、禁用或不能显露的成员不作为新的导航目标。

组的默认行为仅在当前 focusedArea 就是成员区域时处理方向键和 Home／End，不替代成员内部另一个编辑目标的按键行为。Horizontal 左右按 direction 映射，Vertical 上下不因 RTL 反转；列表 Home／End 到首末有效成员。Grid 的左右只在同一行、上下只在同一列按逻辑坐标查找，越过缺失／禁用项；Home／End 到当前行的逻辑首末。loop 只在当前导航轴循环，不依据像素距离寻找邻居。

仅真实、无修饰键的 key-down（包括 repeat）执行；key-up、synthetic、Command／Control／Alt／Shift／Function 组合不执行。内层默认行为先取得按键；组认领的方向／Home／End 即使到边界也消费，防止祖先误作另一组的移动。组不处理不属于其方向的箭头，也不处理 Enter／Space 或改变选择状态。导航与 Tab 共用 FocusHandle 请求，仍服从成功帧和当前合法域；未挂载条目的导航与显露见第 7.8.2 节。

FocusTarget 与 FocusScope 的待聚焦准入使用 InputScope.isInputAllowed：位于当前 InputBoundary 之外的请求直接失败，不能在模态关闭后意外兑现。逻辑 enabled 状态查询与当前输入域资格分开；已有 pending 仍受新请求、目标退休和成功帧检查约束。

### 7.4 Surface 动画接入

`scope.animation(initial)` 提供与 Surface 相同的 Double／Point／Size／Color 重载，也提供带 AnimationConverter 的泛型重载。它使用本作用域按需创建的 Surface AnimationGroup.value，并登记组随该挂载卸载；返回值就是 Surface Animatable，不包装另一套动画状态或时钟。

元素在 update 或输入处理中调用 animateTo／snapTo／pause 等现有操作，在 measure／scene／draw 中读取已采样的 value。Surface 的统一时间轴负责请求帧、同帧采样、完成回调及隐藏后的时间推进；UI 不另建 timer 或每组件帧循环。框架保证本次测量和绘制使用同一采样时点。需要动画结束后改变组件结构时，由完成回调写 State，排入下一轮逻辑更新。

退出挂载时退休登记动画，停止其后续业务完成回调，并在资源清理阶段关闭。完成回调的挂载有效性遵守与输入相同的规则。Element 普通字段的变化若未经过 State 或 Animatable，应调用 scope.requestFrame；同一轮请求由 Surface 合并。

### 7.5 公共文本接入

`scoop.ui.text` 是本 Cone 的公共子包，依赖 [scoop.graphics.text](SCOOP-GRAPHICS-TEXT-SPEC.md)，提供 InlineView、inlineChildren 和 ParagraphHost。它是自绘 Element 的复用能力，不是控件库或新的 View 分支；第三方控件与 Label 使用相同入口。

InlineView 是不拥有资源的描述，保存 InlineKey、View、自然或固定 Size 的测量策略，以及对齐选择。默认采用子项 alphabetic baseline；调用方可选择 ideographic baseline，所选基线缺失时使用盒子底边，也可显式给出文本 InlineAlignment。固定尺寸不自动按段落宽度缩小。图片可由普通自绘 Element 包装成 View，沿同一协议接入。

公开入口为 `inlineView(key, view, configure: InlineViewBuilder.() -> Unit = {})`，builder 配置 `size: Size? = None` 与 `alignment: InlineViewAlignment = Baseline(TextBaseline.Alphabetic)`。None 尺寸表示自然测量；`InlineViewAlignment.Baseline(kind)` 使用子项相应基线，`Explicit(value)` 直接采用 graphics.text 的 InlineAlignment。两种形式都由文本库参与同一次排版，不在 UI 层另算文字行高。

`inlineChildren(content, inlines): Array<View>` 按 StyledText 中对象的逻辑顺序生成普通 keyed 子描述。它根据 InlineKey 建立对应，不根据文字偏移、视觉顺序或数组下标推断身份；缺失、重复或多余绑定报告参数错误。每个对象用普通 keyed 包装节点保存其出现身份，传入 View 自己的 key 继续作用于包装节点内的子位置。包装只使用普通 Component／Props；Span 不生成挂载节点，同段落的全部对象共享该 Element 的直接父级。

ParagraphHost 是普通 Closeable 辅助对象，由使用它的 Element 在 onMount 中通过 scope.resource 登记。它不拥有子组件，也不自行调用工厂或创建输入区域。创建 Element 描述的 Component 或普通组合函数把 inlineChildren 的结果传入 element.children，并通过 Props 传入同一份内容和绑定；Element 的 measure／scene 分别委托给宿主：

```text
ParagraphHost.measure(context, constraints, fonts, content, settings, inlines): LayoutMetrics
ParagraphHost.scene(context): Unit
```

measure 在零最小值、段落最大宽度及无界高度下测量每个自然尺寸子项；固定尺寸子项使用相应 tight 约束。由 LayoutMetrics 和对齐选择生成完整 InlineMetrics，调用 graphics.text 排版，再按 key 对应的可见 box 放置子项。宽度无界时用 layoutUnwrapped；只有尺寸、基线及约束参与排版，子项不能要求先取得最终所在行的剩余宽度。所有子项仍只测量一次，包括本次最终被截掉的对象。

宿主返回受 Constraints 约束的段落尺寸及相对首行基线，文字原点为元素左上角。scene 通过 draw 使用同一 TextLayout 先画文字，再调用 children 呈现已放置对象；输入父关系取自逻辑挂载作用域，不由场景参数指定。没有可见对象时仍可正常绘制文字。宿主不隐式裁剪外框，调用方需要裁剪时在同一个 SceneContext 作用域内包住文字、子项和输入放置。对象在父段落中不可拆分，其自身超出测量框的绘制遵守普通 Element 的显式 clip 规则。

内容或字体／样式变化重建 Paragraph，宽度和对象 metrics 改变时创建新 TextLayout；实际输入不变时复用缓存。子控件颜色等不改变 metrics 的更新不要求重排；尺寸／基线动画在 Surface 同帧采样后进入测量。关闭或替换文本缓存不卸载行内子项；只有正常的描述匹配、移除和宿主关闭改变挂载。

缓存以不可变 StyledText 的对象身份、FontCollection 的身份及 revision、ParagraphStyle 的字段值识别段落输入；新的 StyledText 描述可以触发重建，不要求 UI 重新解析或比较文本层私有的规范化结果。相同段落的宽度和按逻辑 key 排列的 InlineMetrics 按值比较。只改变约束下限或父级放置不重排，仍按本次 Constraints 返回外框。调用方可复用内容描述；String 便利控件负责在自身输入不变时保留该描述。

`ParagraphHost.layout` 借出最近一次成功测量的 TextLayout，以供自绘控件命中、caret、selection 或位置查询；首次测量前或关闭后访问报告状态错误。调用方不关闭该借用；下次替换或宿主关闭后旧布局资源失效。新 Paragraph／TextLayout 成功取得后才替换缓存；失败时释放新取得的资源并保留原缓存，随后仍遵守框架的回调失败与关闭规则。ParagraphHost 只能在创建它的 owner 线程使用，close 幂等；不关闭传入的字体来源。

所有内容查询、metrics 和 placement 通过文本库的 public API 取得。公共适配不读取 graphics.text 的 internal 缓存，不把无匹配 box 的对象悄悄移除出 children，也不需要基础控件库来实现。

#### 7.5.1 公共纯文本编辑状态

`scoop.ui.text.TextEditorState(text = "", selection: Utf8Selection? = None)` 是 owner 线程上的可观察纯文本模型，可以独立于挂载存在。初始选区缺省位于文档末尾；显式范围沿 Window 的 UTF-8 标量边界规则验证。内部只有一份规范 TextInputState，公开 inputState／text／selection／markedRange／revision／isComposing 和 anchor／cursor（TextPosition）读取同一 State；canUndo／canRedo 也参与普通依赖观察。没有另一份可写字符串 binding 或另建订阅系统。

`setSelection(Utf8Selection)` 或 `setSelection(anchor: TextPosition, cursor: TextPosition)` 保留方向与 affinity；`replace(range, text)` 完成一次可撤销替换。两者先明确结束已有组合并保留其显示文本，再操作当前文档。`replaceAll(text, selection = None)` 是外部整体替换，清除组合和旧历史，缺省把选区收敛到新文档末尾。修改正文／选区／marked range 单调推进 revision；只改变 affinity 不改变原生文档修订。非法参数、错误线程以及 render／measure／scene／draw 中的修改在改变历史之前拒绝。

`applyInput(batch, policy = textEditPolicy(), source = NativeInput)` 复用 Window 的有序纯值操作，检查实际 baseRevision，全部合法并经过策略后一次发布；同批已完成的编辑合为一个 undo 项，末尾仍在进行的 preedit 单独保留其基线。首次非空 preedit 保存替换前基线；更新不逐项入历史，Commit／ClearPreedit 把整个组合作为一次历史操作。空 preedit 和 `cancelComposition()` 恢复基线正文、方向选区与 affinity，使用新的 revision。恢复正文会改变原生临时文档的坐标依据；这时同批后续显式 Range 作为过期范围整批拒绝，接入宿主负责取消并重建连接，不把范围静默解释到恢复后的另一份文本上。CurrentSelection 沿当前临时选区继续应用。

`undo()` 先取消仍在进行的组合，再撤销最近编辑；没有历史时仍可取消组合。`redo()` 在存在可重做项时先取消组合再重做，否则不改变状态。历史只保存必要的 UTF-8 文本差量及前后 TextPosition，不为普通键入保存全文；连续插入／退格／向前删除仅在种类、相邻范围及方向选区相符时合并。粘贴、剪切、显式替换、选择与焦点切换形成边界，`breakUndoGroup()` 可显式结束合并。新编辑丢弃旧 redo，取消组合不丢弃已有 redo。各编辑入口返回是否改变可观察状态；空批次与无效 undo／redo 返回 false。

`applyEdit(range, text, policy, source = Programmatic)` 是显式遵循输入策略的程序编辑入口。普通 replace／replaceAll／setSelection 继续表示明确的模型操作，不读取活动宿主的 policy；宿主发起的粘贴、删除、换行、制表符和结束组合的选择操作统一走策略事务。

`textEditPolicy { … }` 构造不可变 TextEditPolicy，builder 提供 singleLine=false、maxLength: Long?=None、paste(callback)、transform(callback) 和 accepts(callback)。长度非负；来源 TextEditSource 为 NativeInput／Paste／EditCommand／Programmatic。顺序为单行换行处理、仅 Paste 来源的 paste、所有来源的 transform、accepts、grapheme 长度。宿主 multiline=false 强制启用第一个阶段。

paste／transform 收到 TextEditCandidate，含 before／after（TextEditSnapshot 的正文、anchor／cursor TextPosition 及可选 markedRange）、source 和当前候选中本次新增内容的 insertedRanges。回调返回 TextEditDecision.Accept／Reject／Transform；Transform 只接受候选上的有序、不重叠、位于 UTF-8 标量边界的 TextEditReplacement(range, text)，允许同位置按顺序插入。引擎验证这些实际区间并生成位置映射：保留片段保持相对位置，被替换片段内部按 affinity 落到替换边缘，端点仍在合法边界。accepts 逐一检查当前新增片段，不隐式选择 ASCII／Unicode 数字或重写整份既有正文；需要完整候选检查时使用 transform 返回 Reject。

maxLength 使用 scoop.charset 的 Unicode grapheme 分段，既有文本超过限制时本次结果只要求不再增加长度。按候选从后往前删去超出的完整新增 grapheme，保留原有尾部及多操作之间的未改内容；原选择被替换后自然释放容量。若必须切开含保留正文的 grapheme 才能满足限制，则拒绝该次编辑。显式自定义替换的输出属于新增内容，相同内容替换省略。此规则不会截断 UTF-8、组合字符或 emoji ZWJ，也不会因为普通输入而静默裁去旧正文。

原生 batch 先按原始操作顺序完整准备，再将已提交部分交给策略。末尾 preedit 的正文和选区保持原样，单行、字符和长度限制均不应用到临时组合；一个 batch 中先 Commit 后 Preedit 时，前面的提交照常过滤，后面的 preedit 通过区间映射重新接到过滤后的基线上。跨批更新保留同一组合基线；当前 policy 更新不改写活动 preedit，最终 Commit／ClearPreedit 使用当前策略。最终结果一次发布正文、方向选区、affinity、marked range、revision 和历史；一次接受的批次只产生一个最终编辑差量。显式变换及组合边界结束连续键入合并。

拒绝普通编辑保留原文档和历史；拒绝已有组合的最终提交则恢复组合基线并结束 marked 状态。有效原生批次即使被拒绝也推进确认所需的 revision 并向相同 origin 发布真实快照，不依赖正文通知。Undo／Redo 回放已接受的差量，不重新运行策略。回调在现有 UI 纯求值边界执行，不能写 State 或重入编辑器；回调异常、非法区间在发布前失败，保留正文、组合、历史和原生快照，不将策略异常当作过期原生批次而重建连接。

#### 7.5.2 公共纯文本编辑宿主

`TextEditHost(scope, state, appearance, options = TextEditOptions())` 是普通 Closeable，由 Element 在 onMount 中通过 `scope.resource` 登记。宿主拥有输入区域、FocusTarget、ScrollState／ScrollViewport、正文与 placeholder 布局、一次性 caret／拖选 deadline；借用 FontCollection，不拥有子项。`update(state, appearance, options, focus, placeholder, onEdited, onSubmit)` 接收当前配置，`measure(constraints)` 返回文本视口 metrics，`scene(context)` 呈现已测量内容。公开 state、area、focus、scroll、interaction 和最近测量的借用 layout，第三方 Element 使用与内置控件相同的入口。没有编辑内容时也使用真实空段落的行高与 caret，不用 placeholder 的字符作输入文档。

TextEditOptions 包含 multiline=false、wrap=true、readOnly=false、enabled=true、tab=Navigate、submit=Enter、direction: TextDirection?=None、alignment=Start、paintCache=None、caretBlink=500ms、policy=textEditPolicy()、projection=textProjection() 和 copy=TextCopyPolicy.Default。direction 缺省时使用挂载的 LayoutDirection 转为文本方向，显式值保留独立 bidi 行为。TextTabBehavior 为 Navigate／Insert；TextSubmitKeys 为 None／Enter／PrimaryEnter。wrap 只作用于多行。单行模型的外部已提交正文不得含硬换行；粘贴和原生提交把硬换行转为空格，CRLF 作为一个换行。原生批次和活动 preedit 遵守第 7.5.1 节的事务顺序，宿主发布前只检查已提交基线；临时 preedit 可以包含换行。

TextEditAppearance 显式提供 fonts、textStyle、placeholderStyle、selectionColor、inactiveSelectionColor、caretColor、markedColor、caretWidth；字体由调用方保持有效，caretWidth 为有限正值。宿主不读取主题。TextEditInteraction 是可观察的 focused／hovered／focusVisible 投影，实际焦点、指示与命中仍以 InputArea 为准。正文缓存按文本与字体／样式／排版输入复用，选择、标记装饰和 caret 独立绘制；闪烁不重建正文排版或像素缓存。

组合控件在 Component.render 中读取模型 inputState，并在需要交互外观时读取宿主 interaction，使用现有的 State 依赖收集。Element.update、measure 和 scene 的读取不建立订阅；在这些阶段把最新配置传给宿主不能替代 render 中的观察。未聚焦视图也按这一规则响应外部正文变化。

`scrollSnapshot` 返回最近一次 measure 为本帧确定的 ScrollSnapshot；测量前访问拒绝。组合滚动条在 scene 使用它，与正文共享本帧 viewport、content 和收敛后的 offset。持久 scroll 的可观察 snapshot 仍遵守 ScrollState 的发布边界，不用旧的已发布尺寸拼凑本帧滚动条。

同一个 TextEditorState 只允许一个实际活动宿主。聚焦时登记、失焦时按实例解除；未活动视图通过普通 State 观察，不另建通用订阅器。外部模型修改立即使旧 caret 几何失效，并结束／重建旧原生连接，不回放 onEdited。宿主根据 restartTextInput 的实际会话归属决定是否等待旧 Disabled；新区域取得焦点后立即更换文档时，不能把尚未开启的首个连接标成旧会话。用户操作成功后，onEdited 只对正文变化通知；选择和组合仍通过模型读取。回调可以正常更新业务 State，同一个宿主的通知不递归重入。onSubmit 由提交策略触发，Enter 策略只匹配无修饰 Enter，PrimaryEnter 只匹配 Primary+Enter；Shift+Enter 在多行中换行。重复按下不重复提交。单行未匹配的 Enter 被消费，Insert 模式的 Tab 插入制表符，Backtab 和 Navigate 模式的 Tab 继续普通焦点导航。

默认行为使用同一 InputScope 路由。真实 key-down 只执行 window 已产生的 KeyEvent.textCommands 有序命令，UI 不再保存平台编辑键位表；从不根据 LogicalKey.Character 再插入正文。Enter／PrimaryEnter 提交使用 window 的 ShortcutChord 匹配实际组合，额外修饰键不触发提交。`execute(TextEditCommand)` 供当前活动编辑目标执行相同动作；没有实际键事件时，InsertNewline 只按普通 Enter 策略判断。Copy／Cut／Paste 使用所属窗口的 GUI 剪贴板；Cut 先准备并接受策略删除，再写出原选择文本，成功后发布删除；拒绝或剪贴板写入失败均不改文档或历史。Paste 在一个事务内变换并作为一个历史边界，不修改剪贴板。readOnly 仍可聚焦、选择、复制和移动，消费会修改正文的编辑命令；disabled 通过普通 scope.enabled 禁用。Cancel 只在存在组合时消费，其余继续冒泡到弹层等默认动作。

点击、Shift 点击、拖选、双击词和三击逻辑行均通过 TextLayout 命中与 Unicode 导航完成。选区保留方向和 TextPosition affinity；普通左右移动折叠非空选区到相应视觉边缘，逻辑前后移动使用相应字节边界；上下／翻页保留期望 x。删除单个字符使用完整 grapheme，word／行／段落／文档命令使用文本库实际范围，段落末端不删除硬换行。原生标量选区可以落在 grapheme 内部，显示 caret 临时收敛到对应 grapheme 边缘，不改写原生规范选区。

输入区域在 ScrollViewport 内以当前内容坐标放置完整视口，使局部输入与 caret 使用文档坐标。光标变化通过 FocusTarget.reveal 在成功帧后显露，同一次请求沿内外层滚动推进；自动显露不覆盖另一个目标正在等待的焦点请求。宿主保留最后 Accepted 帧的 TextLayout 所有权；失败的新布局不能替换输入所依据的旧几何。文档已更新而尚未呈现时，命中／导航使用新正文与最近成功视口宽度，候选框仍不可用。原生 TextInputState 在操作生效后立即按实际 origin 确认发布，不等待 drawable；成功场景单独发布当前 revision 的 caret。过期／非法原生批次整批拒绝并重建连接，关闭中的旧连接批次不进入恢复或新绑定的编辑器。

caretBlink 为非负时长，零表示保持可见。只在活动焦点、折叠选区、非组合、窗口可见且实际 caret 可见时预约下一次闪烁；输入重置可见阶段，失焦／隐藏／不可见／关闭停止预约。拖选出视口时才预约边缘滚动，按实际可滚范围推进，每次滚动等 Accepted 后再继续；被阻止的释放也通过 onEventFinished 结束拖选。普通滚轮复用残量消费和祖先链，宿主没有永久轮询或第二个场景坐标系统。

#### 7.5.3 编辑显示投影

`textProjection(callback)` 构造不可变 TextProjection，回调收到包含 text、anchor、cursor 和 markedRange 的 TextEditSnapshot，返回原文上有序、不重叠的 TextEditReplacement。缺省回调返回空数组，即恒等投影；可以在空范围插入显示分隔符或将原文范围替换为不同长度的显示单元。复用第 7.5.1 节区间验证与单调映射，禁止原文重排。回调在现有纯求值边界运行，异常或非法范围不发布新的布局／映射组合。

`textMask(glyph = "•")` 对每个原文 grapheme 生成一个显示单元并保留换行；glyph 必须是一个不含换行的 grapheme。`projectText(snapshot, projection)` 返回不可变 TextDisplay，含 source、text、toDisplay(TextPosition)、toDocument(TextPosition) 和 displayRange(Utf8Range)。公共位置使用 UTF-8 标量边界，保留 affinity；被替换片段内部落到对应边缘，不建立逐字节位置表。选区范围排除两端仅用于显示的分隔符，包含内部显示内容。

TextEditOptions 的 projection 缺省恒等投影，copy 为 TextCopyPolicy.Default／Allow／Deny。Default 在内置遮罩时禁止 Copy／Cut，其他投影允许；Allow 明确允许复制原文，Deny 消费命令但不改剪贴板或文档。Paste 与正常编辑不受复制策略影响，允许的 Cut 继续遵守第 7.5.2 节发布原子性。显示开关只更新 projection，不替换 TextEditorState、不追加历史、不重建 IME 连接，也不声称提供平台 secure-input 能力。

TextEditHost.layout 与 display 是最近成功测量的同一借用组合；caret(position)／selectionBounds(range) 把原文范围投影到该布局的内容坐标，供 OTP 等后续控件绘制。Accepted 帧另行保留自己的布局和投影，命中、拖选、视觉移动、选择背景及 marked 装饰必须使用同一快照。原文未变的新字体／宽度／投影失败时继续使用旧 Accepted 组合；原文变化后的即时导航沿既有最后成功视口规则排版新文档。公共逆映射保持标量位置，用户指针选择收敛到原文 grapheme 边界；逻辑逐字移动及逐字删除使用原文 grapheme，纯显示分隔符不会成为不可越过的光标停点。RTL 视觉次序由 TextLayout 决定。

Native surrounding text、selection、marked range、策略过滤和 undo／redo 始终使用真实正文。成功帧只将投影后的 caret 几何反馈原生，不以遮罩文字替换输入法文档；活动 preedit 继续保留一份原文模型。

#### 7.5.4 可选择的 StyledText

`SelectableTextHost(scope, fonts, content, options)` 是独立的只读宿主，拥有 ParagraphHost、一个方向选区、输入区域和 FocusTarget，借用字体并沿 InlineView 协议保留行内对象。它不创建 TextEditorState、undo 历史或可写 IME 连接。update 接收字体、StyledText、SelectableTextOptions、FocusOptions、范围动作／装饰和 onSelectionChanged(anchor, cursor)；measure 接收段落设置及 inlines，scene 呈现同一测量结果。公开 anchor／cursor、selection、layout、selectionBounds、setSelection 和 execute 提供明确的观察及程序入口。

SelectableTextOptions 包含 enabled、copy、clip、非负 dragThreshold 及活动／非活动选择颜色，默认启用选择、复制和裁剪。字符／词／逻辑行选择、Shift 扩展及方向移动复用公共文本导航；键盘只消费 Move、SelectAll、Copy，其他编辑命令留给外层。Copy 使用 Accepted StyledText.toPlainText(range)，包括行内对象 alternateText，不丢失原始逻辑范围。选择活动状态包括自身范围动作的焦点。

`TextRangeAction(key, range, onActivate, enabled, focus)` 按稳定 key 保留嵌套区域、FocusTarget 和 Press；缺省焦点顺序跟随动作数组，显式 FocusOptions 独立。范围可以跨行，由布局 selection 返回的矩形合成一个 Path 命中区域，支持显式激活且不自行调用 URL 或 OS 服务。`TextRangeDecoration(range, background, underline)` 只画范围装饰，不创建交互区域。范围均为原始 StyledText 的合法 UTF-8 标量区间，动作非空且 key 唯一。

文本区域在捕获阶段准备拖选候选，不提前吞掉范围动作的点击；达到阈值后通过同一 Surface 指针序列认领取消 Press，转为文本拖选。失焦、取消、被阻止的释放及卸载均收尾。行内子挂载通过 routeChildren 接到文本区域，继续使用普通输入协议。

宿主保留 Accepted 布局、内容和范围描述；失败的新布局／绘制不让新范围套到旧几何。正文变化时将选区收敛到新文档的合法边界，等待新内容 Accepted 后恢复选择输入；范围变化或移除立即撤销旧动作资格，新范围等待自己的成功几何。相同正文的字体／宽度更新仍沿旧成功布局命中，截断与裁剪使用同一段落可见矩形。只保证本宿主内部的选择，不建立跨组件全局文档或富文本编辑器。

### 7.6 同窗口 portal 与锚点

`portal(open: Binding<Boolean>, content: View, configure: PortalBuilder.() -> Unit = {})` 在逻辑父级保留零内容占位，内容交给窗口层出口。open Binding 是期望打开状态的唯一来源；框架在普通逻辑更新中收集 getter 的 State 依赖，随后在纯求值之外同步呈现阶段。普通 Binding 的非 State 数据仍由调用方以 Props 更新通知。初始 false 不挂载内容、不安装模态屏障；容器只按约束下限测量 Portal 占位，不测量或重复呈现层内容。

PortalOptions 包含 placement、handle=None、anchorVisibility=Close、modal=None、dismiss=OverlayDismiss()、transition=PresenceOptions()、`onCloseRequest(reason): Boolean = { _ -> true }`、`onClosed(reason): Unit = { _ -> Unit }`。配置通过 builder 及 copy 发布，归属在实际绑定时检查。modal 有无是挂载形状，改变时退休旧会话；其他选项可正常更新。OverlayHandle 公开 isAttached、isOpen、lastCloseReason、`requestClose()`；一个 handle 不能同时绑定两个活动挂载，晚到清理不解除其新绑定。OverlayCloseReason 为 Explicit、Escape、OutsidePress、AnchorUnavailable、OwnerClosed。

Escape、outside press 和 requestClose 先调用当前 onCloseRequest；接受后写 open=false，并读取 setter 实际采纳的值。拒绝或 setter 不采纳时保持打开，但不撤回既有输入消费。外部直接写 false 不经过 veto。关闭／退出中的请求合并；请求开始即保存焦点 revision，回调内的更新请求不能被后续退出过程重新记为恢复基线。回调可以同步替换内容或关闭根，回到框架后须检查当前实例。渲染、测量、scene、draw 期间不允许发出关闭请求或修改 Binding。

#### 7.6.1 呈现与最终关闭

transition 使用第 7.12 节的 Presence。进入／呈现阶段正常布局，退出阶段保留同一内容及绘制资源，立即禁用根输入资格、取消内部按下与 IME／焦点活动。退出不能通过提前 deactivate 子树破坏绘制资源。Presence 变为 Absent 后在逻辑更新中卸载内容；确认移除的 Accepted 帧后才通知 onClosed，至多一次。没有动画时直接走同一最终移除边界。

退出中 open 再变为 true，从当前进度转向进入；保留子树、key 和编辑模型。已有区域保持禁用，直到新的 Accepted 场景才恢复，不重放原始 initialFocus。最终移除已确认后再次打开，则创建新的内容及模态会话。普通 Props 更新不能绕过 Binding 另开一份状态。

owner 在窗口仍存活时退休，立即停止动画并收束内容，根保留屏障及完成通知到 Accepted 移除。根关闭先停用输入、停止动画及释放屏障，安全退出当前场景借用并卸载子树后直接通知，不等待未来帧。窗口隐藏／没有 drawable 时复用 Surface 时间轴的 owner deadline 推进，退出可以结束，模态屏障仍等实际画面移除。

#### 7.6.2 定位与锚点

LayoutAnchor 通过 ComponentScope.layoutAnchor(handle) 绑定一个挂载；SceneContext.anchor(handle, rect) 每帧登记一次本元素的局部矩形，使用 graphics 的相同变换／clip 快照生成窗口包围盒及可见性。isAttached 只表示逻辑绑定。旋转采用包围盒，不创建 InputArea；anchor 必须属于同一 UiRoot，更新绑定时只沿实际依赖检查自引用与环，未绑定者在随后绑定／打开时完成检查。

OverlayPlacement 为 Viewport(horizontal=Center, vertical=Center, viewportPadding=8)、Anchored(anchor, preferredSide=Bottom, alignment=Start, gap=4, viewportPadding=8, matchAnchorWidth=false)，以及 `AtPoint(point: Point, preferredSide=Bottom, alignment=Start, gap=0, viewportPadding=8)`。AtPoint 使用窗口逻辑坐标，point 有限，不依赖屏幕物理坐标或虚构的 LayoutAnchor。键盘菜单可用聚焦区域的已提交窗口矩形选择 point。距离参数有限且非负。

OverlayAlignment 为 Start／Center／End；OverlaySide 为 Top／Right／Bottom／Left／Start／End。逻辑 side 和水平 alignment 在碰撞计算前依 LayoutDirection 解析，物理 Left／Right 与垂直对齐不变。定位先扣 viewportPadding，求优先侧／反侧的可用范围，以较大的主轴范围及视口交叉轴范围进行一次有界测量；matchAnchorWidth 给受最大宽度限制的 tight 约束。实际尺寸优先使用可容纳的一侧，否则选较大侧，交叉轴 shift 到视口内。AtPoint 复用同一纯几何计算。

SceneContext.overlayLayout 返回最近层的本帧值：bounds、available、anchor、side 和 `presence: PresenceSnapshot`。side 为翻转后的物理侧；Viewport 的 anchor／side 为 None，AtPoint 的 anchor 是表示该点的零 extent 值矩形。内容可据此画箭头、选择动画原点并采样进度；不保留 Canvas 借用。输入与 Hover 使用 Surface 已提交几何，不能采用本次未确认的位置。

层先按模态域、再按域内打开顺序排列，子 Portal 在父层之后呈现，逻辑 parent／路由保持不变。背景域先绘制，后来打开的背景提示不能覆盖模态。普通场景先登记 anchor，再处理层内 anchor；未参与场景或没有可用位置时不能用旧坐标强行呈现。隐藏的挂载可保留模型与资源，静止时不轮询。

AnchorVisibility 的 Close 在 Accepted 后强制结束不可见锚点的呈现；Hide 保留内容等待当前几何恢复；KeepVisible 使用有效包围盒并把位置限制在视口。没有当前几何时 Hide 等待，其余关闭。解绑／owner 退休使依赖层强制结束。AnchorUnavailable 请求把 Binding 置为 false；setter 未采纳也收尾当前呈现，直到实际 false→true 或重新挂载才重开，不能每帧重复请求。强制关闭不经过 veto，回调仍在纯求值退出后运行。

#### 7.6.3 输入关闭策略与模态

OverlayDismiss 为 escape=false、outsidePress=Ignore；另有 CloseAndConsume、CloseAndContinue。Escape 只匹配无修饰、真实、非重复 key-down，内层默认行为优先，一次只请求关闭当前域最上层允许 Escape 的层。外部按下依 InputScope 逻辑归属判断，子 Portal 仍属于父层内部；未命中模态域根视为外部。CloseAndContinue 继续预处理时冻结的目标，关闭后不重新命中；模态禁止 Continue。

ModalOptions 为 initialFocus=None、restoreFocus=true。一次实际打开建立 InputBoundary、循环 FocusScope 和不可命中的焦点代理，保存原有效 FocusHandle、前驱域和焦点意图序号。新区域按已有首个 Accepted 几何规则取得焦点，先 initialFocus，再首个 Tab stop，再代理；较新的显式请求优先。后台模态不抢当前域焦点，程序请求不激活 OS 窗口。

退出期间保留屏障，背景不能收键盘、抢焦点或被点透；Discarded／Deferred 不提前释放。Accepted 移除时从内到外释放已退休层，只为原当前域尝试恢复，依保存 handle、前驱会话和有效目标查找；restoreFocus=false、域不匹配或新请求已推进 revision 时不恢复。每个完成的会话同时关闭自己的 FocusScope、代理及处理器，允许同一 Portal 随后重新打开。非模态退出停用自身输入，仍遵守 Consume／Continue，不引入整窗屏障。另一个原生窗口不受该同窗口模态隔离。

### 7.7 滚动状态与视口登记

`ScrollState(initialOffset: Point = Point(0, 0))` 是独立于挂载的 owner 线程状态。snapshot 返回最近 Accepted 的 `ScrollSnapshot(offset: Point, viewport: Size, content: Size)`，maximum 为每轴 max(0, content - viewport)；offset／viewport／content／maximum 是对应便利属性，hasSnapshot 区分首个 Accepted 前的零值，isAttached 和 isAnimating 查询当前绑定。render 读取这些属性建立普通 State 依赖，仅结果改变时通知。滚动容器不必在 render 读取 offset，偏移可直接请求场景帧而不重建内容。

`scrollTo(Point, origin = Programmatic)`、`scrollBy(Point, origin = Programmatic)` 和 `reveal(Rect, horizontal = Nearest, vertical = Nearest, origin = Programmatic)` 更新位置意图，origin 的类型为 `ScrollOrigin.User／Programmatic`。请求立即生效并安排帧，但不提前改写可观察的 snapshot。ScrollAlignment 支持 Nearest／Start／Center／End，Rect 使用物理内容局部坐标；Nearest 对已完整可见或已包含视口的超大目标保持原位置。范围未建立前 offset／extent 为零，构造位置、绝对定位和 reveal 延迟到首个成功布局；scrollBy 在当前尚未呈现的位置请求上累加，活动动画则从最后 Accepted 位置接管。它返回本次按最近已知范围收敛的实际请求位移，供 wheel 计算残量；首个范围之前返回零，不把尚未呈现的位置作为画面快照。范围改变时位置收敛；State 脱离视口后保留最后结果供再次挂载。

`observeIntent(handler: (ScrollIntent) -> Unit): Closeable` 登记线程内观察，通常经 `ComponentScope.resource` 管理；独立 close 后不再通知。ScrollIntent 为 `User(requestRevision)`、`Programmatic(requestRevision)`、`LayoutCorrection(requestRevision)`，并提供统一的 requestRevision 读取。前两种在实际请求时交付，即使已经到达边界；布局保持阅读锚点或范围收敛引起的位移在 Accepted 后交付 LayoutCorrection，不推进意图序号。仅尺寸改变、动画采样及一个新定位请求的首次呈现不伪造布局修正。登记和关闭遵守 owner 线程及纯度约束；同一次通知使用订阅快照，关闭立即生效，回调中的新请求依发生顺序继续交付。回调可以关闭挂载或发起更新的请求，旧动作不得随后覆盖它；通知异常不撤销已经生效的位置请求。

`ComponentScope.scrollViewport(state)` 返回挂载拥有的 ScrollViewport。同一 State 至多有一个活动绑定，可整体更新登记的 state；先检查新绑定，失败保持旧状态。登记公开 layout(viewport, content) 与 present(SceneContext)：自绘容器在 measure 取得本帧快照，随后在自己的 scene 登记提交。layout 不发布可观察状态，Accepted 才提交该帧实际呈现的范围和偏移，Discarded 保留旧结果。present 只接受本挂载的 SceneContext，每次 layout 结果只提交一次；关闭和绑定替换取消旧帧回调。布局后的新位置意图优先：仍发布实际 Accepted 帧，保留更新的请求并安排下一帧，不能用尚未绘制的新目标替代画面快照，也不能把旧位置写回更新的请求。

present 可接收可选的 `content: ((SceneContext) -> Unit)?`，以及 `contentOrigin: Point = Point(0, 0)`。提供回调时，框架借出隔离的内容场景，按本次 snapshot 自动施加视口 clip 与 `contentOrigin - offset` 平移，并登记该内容的滚动显露边界；回调中的子项位置相对于 contentOrigin，不重复施加偏移。默认原点为零，普通内容沿用完整内容坐标；超长虚拟内容可以在 Double 中先减去附近原点，再把较小的局部位置交给图形后端，避免巨大正负变换相消时丢失精度。contentOrigin 必须有限，只影响场景坐标表达，不改变 snapshot、完整逻辑范围或显露计算。省略回调只登记原有布局提交，不把任意逻辑后代猜作滚动内容。回调外的滚动条／固定装饰保留原场景路径；普通 withState 和 children 继承边界，Portal 从层出口的新路径开始。

边界几何、snapshot 和内容内区域的所属路径仅在 Accepted 后一起生效；输入区域的位置仍复用 Surface 已提交的几何，不复制输入状态。焦点请求把目标的窗口包围盒映回最近视口的内容坐标，以 Nearest 调整；一次只改变最近一个尚需滚动的视口，再等下一 Accepted 后继续外层。已满足的内层将目标区域限制在自己的视口后交给外层；任意变换使用实际场景几何，已可见时容忍图形后端的浮点误差，不因亚像素差异持续排帧。无法反变换、零视口、未放置或不能通过滚动显露的目标不能靠周期重试变为有效。

显露复用 ScrollState 的同一位置意图序号。由本次显露调整过的视口若收到更新的显式滚动／动画请求，或登记被关闭／换绑，则取消旧显露；不覆盖更新的用户位置意图。焦点请求或等待新位置发布可以要求必要的下一帧，隐藏／遮挡时仍依赖正常恢复，不启动定时轮询。ScrollState 的可观察发布与用户 onFrameComplete 动作保持原登记顺序；焦点依据的是同一 Accepted 帧的几何及 snapshot，不依赖这些用户回调先执行。

`animateTo(Point, AnimationSpec, origin = Programmatic)` 显式平滑滚动，缺省为 300 ms Tween；必须已有绑定，可在首个布局前排队。登记拥有 Surface 同一时间轴的 Animatable<Point>，布局采样、Accepted 发布；isAnimating 包括等待首布局的动画。终点按最新范围收敛，无剩余移动时撤销动画需求，边界请求仍登记意图。cancelAnimation、直接定位、reveal 与用户滚动在最后发布的 offset 停止旧动画，不在消费一份 wheel 残量时另外推进时间轴。cancelAnimation 只终止活动动画，不另生成定位通知。关闭登记立即停止动画并解绑；State 自身不拥有窗口或待关闭资源。

布局容器可向 `ScrollViewport.layout(viewport, content, adjustment)` 提供 `ScrollAdjustment(requestRevision, offset)`，用于可变尺寸内容的阅读锚点或按 key 定位。`ScrollState.beginLayoutRequest(origin = Programmatic): ULong` 为这些尚需布局解析的主动定位登记一次新意图，停止旧动画并保留 Accepted 位置作为基点，返回该请求的序号；调用方在通知返回后检查它仍为最新，再保存待解析目标。ScrollState.requestRevision 是仅供比较的当前定位意图序号，读取不建立 State 订阅；scrollTo／scrollBy／reveal、beginLayoutRequest、动画定位及取消活动动画推进序号，范围发布不推进。hasPositionRequest 查询是否已经登记构造或主动位置请求，同样不建立订阅；默认零构造位置尚未登记显式请求，非零构造位置已登记请求。不得依赖序号的具体数值或增量。adjustment 只在序号仍匹配且没有活动平滑滚动时应用，按本帧范围收敛；布局仍不写 State，Accepted 后才发布。调整准备期间或布局之后的新请求优先，Discarded 不发布旧修正。`ScrollSnapshot.revealedOffset(rect, horizontal, vertical)` 以相同规则纯计算显露位置，供布局复用对齐和收敛算法，不改变 ScrollState。

上述位置写操作遵守 State 的纯度约束，render／measure／scene／draw 中拒绝调用；present 仍只用于当前场景登记。几何和位移须有限，尺寸须非负，派生坐标溢出按普通参数错误报告。ScrollAxes 的 Horizontal／Vertical／Both 提供公开 horizontal／vertical 查询，供容器选择约束和交互轴；具体 ScrollArea 布局、滚动条与默认 lineStep 属于独立控件库。按需子项与实例保留见第 7.8 节，虚拟导航见第 7.8.2 节，绘制缓存按后续批次扩展。

### 7.8 按需子项与 LazyElement

`ItemSource` 提供一个数据快照的 `count: Long`、`keyAt(index): ViewKey`、`indexOfKey(key): Long?` 和 `build(index): View`。count 非负，key 不得为 Position，在同一 source 内唯一；查找函数只读取快照，访问时检查索引与双向映射，不扫描尚未请求的条目。改变数据、索引或条目输入时替换 source 对象；不把可变 source 的原地修改当成通知，也不读取全部内容作摘要。build 只在实际需要的条目节点中执行，遵循 Component.render 的纯度和 State 依赖规则。

`LazyElement` 与 Element 并列，公开 measure(LazyMeasureContext, Constraints)、scene(SceneContext)、onMount(ComponentScope)、update() 和 onUnmount()。`lazyElement(source, create, retention = ItemRetention.Dispose, key)` 及带 props 的重载创建描述，保留具体 class／Props 类型和普通 ViewKey 身份；create 不提前调用。它没有 eager children 数组。普通 Element／MeasureContext 的全部直接子项测量规则保持不变。

LazyMeasureContext 公开 time、layoutDirection、itemCount、`measureItem(index, constraints)`、`placeItem(index, origin)` 和 `pinnedItems: Array<Long>`。每个本轮选中的索引至少成功测量一次，未放置时可重新测量且相邻同约束复用最后结果；最多放置一次，放置后不能再测量；context 只在当前同步测量中有效，子项准备／测量期间拒绝重入同一个 context。scene 的 children() 按当前已实现项的 source 索引顺序呈现已放置项；children(order) 的索引是这个已实现子序列的位置，不是 source 索引。

同一布局中，lazy 只暂存最后一次测量的选中集合与 placements；普通根或 portal 的最终测量结束后才检查 pinned 项、更新帧参与和执行 retention。中间探测准备过的同 key 挂载继续复用，不提前 trim，也不发布滚动修正、scene、锚点或 frame completion。失败布局不以临时测量撤销先前的交互归属。每次新测量都替换该节点待收尾的集合；最终收尾仍沿现有关闭／卸载规则执行。

measureItem 在框架控制的准备边界按稳定 key 取得条目挂载，再运行其 build／render、onMount 和 update，随后恢复普通纯测量。生命周期可以沿既有规则登记资源和写 State，其产生的其他失效留到下一逻辑更新轮，不递归 flush；build／render 和父子 measure 仍禁止 State 写入。每项使用普通挂载子树保存依赖、Props 与资源，滚动复用已实现项时不重跑其 build。真实 source 替换在逻辑更新阶段通过 indexOfKey 更新已实现项、退休删除项，再更新存活项；即使没有 drawable 也完成删除和输入取消。source.build 返回描述的 shape／key 改变时按普通子树规则替换。

缺省 Dispose 下，离开本次选择范围且无活动交互的条目正常卸载。pinnedItems 包含已有逻辑焦点、待处理的 UI 焦点请求、按钮序列、保留滚动路由、文本输入会话或输入边界所属的条目；容器必须测量并放置这些项，可用同一视口 clip 裁掉其像素。此查询复用 Surface 的实际路由归属，不另存输入状态。滚动路由包含接触 Ended 后等待可能的 momentum 的候选；每窗口至多一个，惯性结束／取消、下一段滚动或原路径失效后释放，不因此请求连续帧。真实删除和 owner 关闭优先于固定。

框架在活动归属改变后请求必要的新布局，使失去固定资格的离屏条目恢复淘汰；同一大子树内焦点从条目 A 移到 B 也算归属改变。条目准备期间显式关闭根会终止当前 UI 布局，仍沿原安全关闭路径释放已经取得的资源，不以未完成的子测量继续装配场景。失败仍按第 9 节关闭，不保留半挂载的输入树。可变 extent 与按 key 定位由控件库通过本协议和 ScrollAdjustment 实现；未挂载项按 key 聚焦和键盘导航见第 7.8.1／7.8.2 节，绘制缓存按后续批次扩展。

ItemRetention 另提供 KeepAlive(maxOffscreenItems: Long)，上限非负，零等同 Dispose；它只计算未选中且无活动交互的离屏条目。超出时淘汰最久未测量的条目，同一帧内的先后不作承诺。保留同 key 的普通挂载子树，不跨 key 复用实例；形状变化仍替换实际子内容。策略收紧、数据删除及 Props／State 更新在正常逻辑阶段处理，即使没有 drawable 也不延期清理。

缓存条目继续 mounted、订阅和执行逻辑更新，本帧未选择时不测量或呈现场景。框架对 lazy 容器明确未放置的子树关闭继承的帧资格，不按布局 bounds 推测阴影、文字 overhang 或 Portal 的可见性。局部 State 更新和 scope.requestFrame 不单独唤醒绘制；影响已参与节点的更新仍请求帧。每帧沿 Surface 时间轴采样当前动画，条目测量完成后按实际放置结果恢复资格；只测量而未放置不会短暂重新启用帧。嵌套 lazy 子树保留自身未参与状态。每个作用域通过 Surface AnimationGroup 管理这个开关，保留单个动画的显式 isFrameEnabled 设置；有限离屏动画仍能完成，无限重复不产生离屏周期唤醒。输入边界变化等窗口级交互工作继续使用其必要的全局请求。

待聚焦请求完成、取消或被新请求替换时，即使没有产生实际的 Surface 焦点变化，也撤销其固定需求并请求必要的后续帧；不可见目标的失败请求不能使离屏条目一直留在实现集合中。根关闭时直接清理，不再请求帧。

### 7.8.1 未挂载项的按 key 聚焦

LazyElement 可通过 `ComponentScope.lazyFocus(viewport, options = LazyFocusOptions())` 登记一个由挂载管理的 LazyFocus。viewport 必须属于同一挂载；普通 Element／Component 或第二个同时活动的登记被拒绝。它使用该 lazy 节点已有的 ItemSource 和真实子树，不为全部数据创建 FocusHandle 或 InputArea。准入沿 routeChildren 后的实际输入作用域判断。

LazyFocus 提供 `requestFocus(key, edge = ItemFocusEdge.First)`、`cancelRequest()`、isPending、isClosed 和 `pendingItem: ViewKey?`。edge 的 First／Last 选择该项按逻辑顺序的首／末个可用 Tab 目标；不会把整个条目伪造成另一个区域焦点。key 必须显式。未活动或输入被禁用／模态阻挡时返回 false；true 表示接收异步请求，未知 key、没有可用目标或不能显露时结束，保留原来有效的焦点。cancelRequest 只取消该登记当前的待处理请求，不清除已持有的区域焦点。

请求与普通 FocusHandle 共用 UiRoot 的唯一 pending 及 Surface.focusRevision，并通过该 viewport 的 ScrollState 登记一次新位置意图，取消旧动画。pendingItem 仅在尚需实现／初次定位条目时返回 key；容器据此使用自身 extent 和 ScrollAdjustment 显露整项。选出具体 FocusTarget 后 pendingItem 变为 None，isPending 可以继续为 true，后续目标显露复用第 7.7 节，避免整项定位反复覆盖行内目标的精确显露。

LazyMeasureContext.pinnedItems 此时也包含这个尚未实现的目标索引；容器同样须测量并放置，取得普通挂载子树。只解析本次请求的 key，不预建全部条目。登记本身位于外层 lazy 条目中时，该外层条目也因待聚焦归属而固定，恢复内层布局；不会固定同列表中其他无关条目。已经选择具体目标后，由原有待聚焦归属继续固定其所属项。条目目标只在 Accepted 的实际几何与滚动路径发布后选择并聚焦。Discarded 不兑现请求。

较新的焦点、该 viewport 上较新的滚动、动画、换绑或关闭取消旧请求。source 替换按 key 跟随存活项；目标 key 被删除时在逻辑更新中取消，即使窗口隐藏也不继续保留旧请求。登记关闭和 owner 退休同样取消。隐藏／遮挡沿普通按需帧机制等待恢复，不轮询。按 key 请求本身不改变普通 Tab 顺序，键盘策略由第 7.8.2 节的显式配置控制。

### 7.8.2 虚拟 Tab 与条目导航

LazyFocus.options 使用 `LazyFocusOptions(navigation = ItemNavigation.None, layout = NavigationLayout.Vertical, order = 0)`，在逻辑阶段整体更新；实际配置改变取消旧等待。layout 只接受 Horizontal／Vertical，条目按 ItemSource 的线性顺序导航，Grid 使用第 7.3.2 节的有限成员协议。ItemNavigation 为 None、Sequential 或 `Roving(direction: LayoutDirection? = None, loop = false)`。

None 保留普通已挂载目标的 Tab 行为，仅提供按 key 聚焦。Sequential／Roving 在包含该 lazy 节点的逻辑 Tab 序列中登记一个按 order 排序的条目入口，代替枚举它的已挂载子行；节点自身的独立 FocusTarget 仍由其自身 tabStop 控制。进入条目入口后按需解析 key、测量、显露，再使用行内实际的 Tab 顺序，不创建伪区域或全量句柄。嵌套虚拟列表使用同一流程，普通 FocusScope 的默认目标、Tab 首项和 fallback 也经过这一顺序。

Sequential 的 Tab 先访问本行的其余有效目标，行尾再准备下一项的首目标；Shift+Tab 使用相反方向与末目标。没有可用目标、零几何、被禁用或不能显露的行继续到数据边界，然后恢复父 Tab 序列。窗口根或显式循环 FocusScope 可环绕；每次遍历的节点和项数均有限，失败不会在同一次按键中无限重试。异步期间仍按稳定 key 跟随当前候选，真实删除取消旧请求；新插入或重排不能使一次遍历无限延长。

Roving 使条目入口成为一个 Tab 停靠点，离开任一行后 Tab 继续外层。再次进入时优先当前有效焦点所属 key，其次使用最近实际聚焦的存活 key，尚无历史则从首个有效项开始；行内以首个有效 Tab 目标为该项的主目标。历史从 Surface 的真实焦点和活动通知取得，包含鼠标与程序聚焦，仅保留稳定 key，不成为第二份焦点状态，也不使离屏项常驻。

Roving 的方向键和 Home／End 只在事件仍未消费且当前焦点正是行的主目标时处理。Horizontal 的左右按 direction 映射，Vertical 的上下不受 RTL 影响；Home／End 从数据首／末端寻找有效项。禁用、空项与不能显露的项按需跳过；loop 仅控制方向键越过数据边界后的环绕。接受真实无修饰 key-down 和 repeat，拒绝 synthetic、key-up 及 Command／Control／Alt／Shift／Function 组合。认领的方向即使无下一项也消费，不接管嵌套编辑器已经处理的按键，不处理 Enter／Space 或修改选择状态。

所有导航候选继续使用根上的唯一焦点请求、同一 viewport 定位和成功帧协议；请求完成后释放临时固定。只保存当前遍历的实际挂载路径、有限顺序游标和少量稳定 key，不按 source.count 创建数组，不为遍历等待启动定时器。

### 7.9 布局方向继承

LayoutDirection 为 Ltr／Rtl。`direction(content: View, value: LayoutDirection, key = Position): View` 创建一个透明的单内容方向作用域；根缺省 Ltr，其他节点沿逻辑挂载父级继承，portal 在窗口出口呈现时仍继承逻辑祖先。它不读取系统 locale，不改变子项 key、Tab 顺序或绘制内容自身的变换。

ComponentScope、MeasureContext、LazyMeasureContext 和 SceneContext 的 `layoutDirection` 读取当前有效方向；context getter 沿用同步借用期限，scope getter 沿用挂载检查。`LayoutDirection.textDirection()` 显式转换为 graphics.text.TextDirection。只改变方向时更新继承节点并请求布局，Element／LazyElement 重新 update，Component 可重新 render；不卸载实例、重建输入区域或丢弃编辑状态。方向随普通 Props 协调一同改变时，同样传播到保留的 lazy 条目，即使 ItemSource 身份未变。纯方向传播遇到已有明确覆盖即停止；真实 Props／content 变化仍执行普通协调。该专门机制不改变 Context 的挂载时绑定合同，也不建立任意动态环境框架。

方向只影响逻辑几何：水平布局从逻辑起边排列，纵向布局的交叉轴 Start／End 按方向解析。物理 Left／Right 不变；Canvas、图片、Path 和文字不整体镜像。NavigationOptions／ItemNavigation.Roving 的 None 方向在事件时读取所属挂载的有效值，显式值覆盖；方向改变不替换导航登记或焦点 handle。

ParagraphHost.measure 的 settings 为 `ParagraphStyle? = None`；缺省时以本次 MeasureContext 的方向生成段落设置，明确提供的 ParagraphStyle 保留其文本方向。TextEditOptions 同样缺省继承、显式覆盖，布局方向变化只改变排版，不改变正文、选区、组合基线或文本连接身份。

### 7.10 公共 Press 与交互状态

`InteractionState(hovered=false, pressed=false, focused=false, focusVisible=false)` 是按下行为的只读表现值；选中、展开和校验状态继续归调用方模型。`ComponentScope.press(area, options=PressOptions(), onPress)` 创建并登记公共 Press，借用当前 mount 创建的 InputArea。`PressOptions(enabled=true, acceptsEnter=true)` 配置可用性和 Enter 激活；`Press.interaction` 是会被 render 依赖追踪的当前状态，`update(options, onPress)` 同步最新配置与动作。关闭 Press 注销其处理器并清空临时序列，不关闭 area；scope 销毁自动关闭行为。

Press 通过 Surface 的独立事件／默认动作／完成登记组合，不替换调用方原 handler。鼠标主按钮在已提交 hover 路径内按下建立候选，移出暂停 pressed、移回恢复；同一有效序列在区域内释放后激活。取消、禁用、失焦和卸载立即清理。传播中止或默认行为取消时，完成通知仍清除已经结束的序列，不补做激活。

键盘使用 window 的精确 ShortcutChord：普通 Space 在真实 key-up 激活，Enter 在真实 key-down 激活，acceptsEnter=false 时忽略 Enter；额外修饰键不匹配。重复按下不追加激活，synthetic 不激活，释放必须对应先前物理键。Enter 已触发的按键只等待释放收尾，Space 候选在释放前失焦／禁用即失效。指针与键盘候选互斥。

触发动作前先收束本次激活资格并发布状态，再调用最新动作；动作可同步替换 Props、移除自身或关闭根，返回后不得继续假定原 area 存活。enabled=false 是行为输入，不替调用方决定整个 area 子树的可用性；控件自身仍显式设置 area.scope.enabled。

### 7.11 公共轴向 Drag

`ComponentScope.drag(area, options, accepts={ true }, handler)` 借用当前 mount 的区域并登记 Drag。`DragOptions(axis, threshold=4, enabled=true, button=Left)` 的 axis 为 Horizontal／Vertical，threshold 为有限非负逻辑距离。accepts 接收本次按下的已提交局部坐标，可用于同一条区域内区分 thumb／track。

Drag 在普通传播中记录候选，不消费点击；仅在默认阶段达到阈值且主轴位移不少于交叉轴位移时调用 Surface claim。反向主导移动放弃候选，小幅移动继续保留原点击。threshold=0 可在按下默认阶段开始，例如滚动条 thumb。默认阶段仍按目标到祖先顺序决定谁先接管，不建立新的优先级或 gesture arena。

`DragEvent` 包含 Started／Moved／Ended(DragSample) 和 Cancelled(DragSample, reason)。样本包含局部 start／position、主轴 displacement／velocity 和 InputTime；开始、上一位置和当前位置都由原窗口点通过当前已提交几何转换，避免布局／缩放变化后混用不同坐标系。速度按现有单调输入时间计算，重复或倒退时间为零；无可用几何或无法表示有限结果时取消，不输出无穷速度。不会从未提交布局推算拖动。

`Drag.isDragging` 只表示已经 claim 的阶段。`update` 更换最新 accepts／handler；禁用、改变 axis／threshold／button、关闭或输入取消都会终止当前状态。CancelReason 区分 Input、Disabled、ConfigurationChanged、GeometryUnavailable、Explicit、Closed 和 PreventedRelease；被阻止的 release 通过不可取消的完成通知收尾。未 claim 的候选放弃不发 Started／Cancelled。cancel() 显式取消当前候选／拖动；关闭行为取消自己已经取得的序列，注销登记，保留调用方 area。行为不创建时钟或永久帧需求；需要吸附时调用方驱动现有 Animatable，需要边缘滚动时仅在实际可推进期间预约 owner deadline。

### 7.12 Presence

`ComponentScope.presence(open=false, options=PresenceOptions())` 创建随 scope 关闭的 Presence。PresenceOptions 包含 enter／exit: AnimationSpec?=None，无 repeat 或动画编排图；update(open, options) 从当前 Animatable 值转向新目标，相同目标更新不重新开始。phase 为 Absent／Entering／Present／Exiting，isPresent 表示非 Absent，snapshot 为 PresenceSnapshot(phase, progress)，progress 在 0–1 内；phase 的读取通过普通 State 建立依赖，连续进度在 measure／scene 采样。

Presence.update(open, options=None) 中省略的 options 保留当前配置。

Presence 只管理呈现值，资源、输入和子树由组合调用方管理，Portal 使用第 7.6 节的规则。默认无动画直接变化；动画复用 scope 的 Surface AnimationGroup、帧参与和 deadline，没有第二个时钟。关闭及 retarget 取消旧完成回调；隐藏窗口也能收束阶段，不留下周期定时器。owner 强制收尾幂等，底层时间轴已经关闭时直接结束内部阶段，不再采样。

### 7.13 活动项导航与 Typeahead

NavigationSource 是调用方的有序元数据快照，提供 count、keyAt、indexOfKey、enabledAt=true、searchTextAt=""、rowAt=0、columnAt=0；key 显式且唯一，数据更新替换 source。基础行为不复制项目数组或为搜索创建 View，访问时检查实际索引与双向 key 映射。它可与 ItemSource 共享同一个模型。

`ComponentScope.activeItems(source, active: Binding<ViewKey?>, options=ActiveItemOptions(), reveal={ _ -> Unit })` 返回 ActiveItems。options 为 layout=Vertical、direction=None（继承）、loop=true、enabled=true。activeKey 读取当前绑定，setActive(key) 验证并写入有效 key，move(NamedKey) 与 KeyNavigation 共用下一项查找，跳过 disabled。update 替换元数据及当前回调，options=None／reveal=None 保留当前配置／回调，保留仍有效 key，删除／禁用／过滤后没有该候选则写 None。方向键、Home／End 的默认动作先消费再更新，保持真实焦点不变；Enter／Space、hover 和选中值由控件决定。reveal 可连接 VirtualListHandle.scrollToItem，不能调用 focusItem 来实现活动项导航。公开 handle(context, event) 可从组合 owner 的现有 captureHandler 调用，在输入框自己的编辑默认动作之前处理导航；已经被阻止的默认动作不再认领。编辑组合是否暂交 IME 由拥有编辑模型的控件决定。

`ComponentScope.typeahead(source, currentKey, onMatch, options=TypeaheadOptions())` 返回 Typeahead；options 为 timeout=700ms（正值）、enabled=true、keyboard=true、matches 为精确 Unicode startsWith。query 为短期缓冲，push(text) 追加有效文字，重复同一单字符输入可从活动项之后循环查找，clear／失焦／source 替换取消 deadline。update 中 options=None 保留当前配置。一次搜索最多遍历快照 count 次，匹配策略可显式替换以决定大小写、重音及归一化；默认不声称 Unicode case folding 或 fuzzy 匹配。

keyboard 模式只处理允许的真实 logical 字符，保留 Shift 字符与 window 已识别的 AltGr；应用修饰键不进入缓冲。它用于非编辑列表。Combobox／Command 的输入框直接使用编辑模型提交的查询，关闭该模式，不从 KeyEvent 重建文本，也不使用 IME preedit 作为已提交搜索词。行为只有缓冲活动时才保留一次性 timeout 和窗口失焦观察，scope 关闭后无迟到动作。

### 7.14 Hover

`ComponentScope.hover(trigger, open: Binding<Boolean>, options=HoverOptions())` 借用本挂载的 trigger，返回 Hover。setContent(areas) 接入同一逻辑 owner 后代的 portal 内容区域，形成悬浮组；update 接收当前 Binding 与配置，options=None 保留当前配置。选项为 openDelay=300ms、closeDelay=150ms、transitDelay=350ms、corridorPadding=6、focus=true、enabled=true，时间非负、padding 有限且非负。

组内进入／焦点可以打开，离开安排关闭；触发区和内容间隙使用离开点、当前移动方向及已提交内容矩形构成有限通过区域，仅延迟关闭。通过期限从本次离开起算，移动不能无限延长；离开窗口、失焦、禁用、dismiss 或 owner 退休撤销等待。isPending 表示 owner deadline，isOpen 读取 Binding；关闭行为只清理自己的登记，dismiss 提交关闭状态。

观察通过 Surface.observePointer 和普通区域事件／activity 组合，只在打开或等待期间登记当前窗口观察；没有 OS 全局监听、捕获、常驻 timer 或独立命中树。失去有效内容几何时使用普通短延迟收束，不能根据未确认的 scene 位置继续延迟。

## 8. 挂载与资源生命周期

同步关闭根时，输入退休及对应的内部状态通知在生命周期准备边界运行；普通 render／measure／scene／draw 的 State 修改限制不变。绘制借用结束后才释放实际资源。

`ComponentScope` 是框架为每个 Component／Element／LazyElement 分别创建和关闭的引用对象，提供：

- `resource<R : Closeable>(create: () -> R): R`：成功取得后立即登记为本组件拥有的资源。
- `onDispose(action: () -> Unit)`：登记卸载时同步执行的清理动作，例如解除外部订阅或发出任务取消。
- `isMounted`：当前挂载是否仍可接收业务工作。
- `nativeWindow`：当前挂载所属的 Window，供公共适配接入窗口剪贴板、文本快照和一次性 deadline；不转移窗口所有权，挂载退休后读取报告状态错误。
- `requestFrame()`：请求所属根再次呈现；不立即执行 render、measure 或 paint。
- `createArea(...)`：创建并登记第 7 节的稳定 InputArea。
- `animation(...)`：在所属 Surface 时间轴创建并登记第 7 节的 Animatable。
- `scrollViewport(state)`：登记第 7.7 节的视口范围、位置提交与平滑滚动。

首轮逻辑提交建立节点和订阅后，按父先子后的顺序调用 onMount，后续 render 不再调用。Element 的首次 update 在其 onMount 成功后执行；全轮挂载完成才可能进入测量／绘制，因此元素的资源在首帧可用。onMount 或 update 中的 State 写入排到下一轮，不递归重建。外部订阅和初始资源在此建立；不提供每次 render 自动重跑的 effect API。

卸载先把整棵子树标为不活动并解除依赖／排队工作，然后按子先父后的顺序处理各节点：退休登记输入和动画，调用已成功完成 onMount 的实例的 onUnmount，最后逆登记顺序执行 ComponentScope 清理及元素资源关闭。onUnmount 执行时 scope 已不再允许新增资源或请求帧；除已退休的输入外，其余待清理资源在各自释放前仍属于该实例。没有自定义 onMount 的实例同样完成默认挂载并获得 onUnmount。

原生窗口或 Surface 先进入 Closing 时，仍会向尚未卸载的控件交付失焦／取消通知。UI 在这些通知中继续允许正常的控件状态收尾，但不把新的帧需求送给已经停止呈现的 Surface；根随后按同一关闭路径退休。已经卸载的 ComponentScope 仍拒绝 requestFrame，不改变 SurfaceWindow 自身的关闭后调用规则。

构造器或首轮 render 失败的实例没有完成挂载，不调用 onUnmount；已经建立的节点资源仍须释放。onMount 自身失败时，其 scope 中已登记的资源同样全部清理，不要求失败的回调再自行释放它们。Element 的 update、measure 或 paint 失败时，已完成挂载的实例按通常卸载规则收尾。所有清理分支继续处理剩余资源，错误合并遵守现有 ResourceUseException。

框架管理自己创建的挂载结构及登记资源。控件库和第三方 Element 负责声明自己拥有的 Path、字体或缓存，并通过相同的 ComponentScope 登记。公共 ParagraphHost 管理自己的 Paragraph／TextLayout；其他可变绘图缓存同样可以实现普通 Closeable，在 onMount 时登记一次，由缓存对象替换和关闭旧值，不得每帧追加旧资源直至卸载才释放。测量中创建新缓存值失败时保留或正确关闭原值，成功替换后及时关闭旧值。缓存不拥有行内子组件，截断可见性不触发 scope 清理。

传入的 Image、Path、FontCollection 或外部模型默认借用；需要跨调用保留 native 使用时遵循 graphics／graphics.text 的持有合同，不能擅自关闭调用方拥有的包装对象。作用域不拥有 Surface 的整条时间轴或窗口，只拥有为本挂载登记的对象。关闭动画和区域会从 Surface 的集合中移除，Surface 最终关闭继续作为窗口级收尾。

状态对象与托管引用最终由 GC 回收；卸载、取消输入、关闭 native 资源不能等待 GC。外部继续持有一个卸载后的模型不会使其重新渲染；外部继续持有组件也不延长它的挂载期。

### 8.1 异步任务归属

异步任务属于现有 CoroutineScope／Job，从事件处理或 onMount 启动，不从 render 启动。随组件结束的任务把 Job.cancel 登记到 ComponentScope.onDispose；跨页面或窗口继续的业务任务由应用／文档作用域持有，单个观察组件卸载只解除 State 订阅。State 或 Context 中保存任务引用不改变其结构化归属，也不转移共享 HttpClient 等资源的关闭责任。

ComponentScope 同步发出取消，异步 finally 和实际 I/O 收尾由既有 event loop 完成；宿主按窗口／asyncio 合同持续驱动到收尾结束，不在 UI 线程阻塞等待 Job。Canvas、布局／绘制借用和组件拥有的绘图资源不能跨挂起继续借用。迟到结果在 owner 上确认目标仍有效，或只写仍由业务层持有且已无 UI 订阅的状态。

预期网络失败在业务任务正文内转换为错误状态，取消保留 CancellationException 的传播；未捕获失败继续沿既有任务树与宿主异常路径处理，不增加 UI 错误边界或自动重试。需要显式识别连续任务时，由业务对象保存当前任务身份；旧任务的迟到进度／结果不能覆盖新任务。普通托管状态、任务取消与原生资源释放各自遵守已有合同。

## 9. 宿主、关闭与错误

`UiHost` 包装 SurfaceHost，作为普通 ApplicationHandler 接入 Application.run，也可以接入现有 runWithAsyncio。`UiHost.attach(window, create)` 返回管理根组件的 UiWindow。`runUiApplication(title, create)` 是单窗口便利入口，统一拥有 Application／UiHost／窗口，接受关闭请求并在最后一个窗口关闭后退出；复杂窗口策略使用显式 UiHost。

macOS 的 UiHost.shortcutScope(bindings: Array<surface.ShortcutBinding> = []) 返回其 SurfaceHost 拥有的 HostShortcutScope；采用同一输入路由和生命周期，不复制绑定或匹配算法。每个 UiHost 至多一个活动宿主登记，关闭 UiHost 时停用；局部行为与窗口 ShortcutScope 优先，任何模态输入域都会截断宿主回退。在 Dialog 内有效的保存／确认动作应登记在该域的 InputScope。

底层 scoop.window 已支持同一 Application 的多个独立非模态窗口，见[窗口规范第 4.1 节](SCOOP-WINDOW-SPEC.md#41-多窗口)。一个 UiHost 可对不同 Window 分别 attach，每个窗口拥有独立根、挂载树、布局与区域场景；单窗口便利入口不限制框架只能管理一个窗口。普通关闭只卸载相应窗口的根，显式应用退出或宿主失败继续沿既有整体收尾；同一 owner 上可共享业务 State，不能共享组件实例、InputArea 或跨窗口转移挂载。

原生模态对话框与完整自绘窗框依赖[窗口规范第 4.3 节](SCOOP-WINDOW-SPEC.md#43-模态与自绘窗框的待补能力)的待补接口。窗口归属、窗口模态／应用模态、输入限制、关闭与焦点恢复、系统拖移／缩放属于 window；UI 负责内容、控件和结果交付。当前设计不把置顶或临时屏蔽组件事件当成原生模态，也不因装饰开关已实现就声称已具备完整自绘窗框操作。

资源管理在最外层保留明确的 use 作用域。显式创建 UiHost 的调用方以 `host.use { ... }` 包住事件循环；`runUiApplication` 在内部建立并关闭相同的宿主作用域。所有权由宿主经 SurfaceWindow 的 content 传到 UI 根及各挂载节点，普通组件不再逐个 use。单个窗口关闭时立即收尾它的内容，不等整个应用离开最外层 use；最后的宿主 close 幂等地释放仍然存在的窗口。

UiWindow.close 停止根组件的新工作并请求关闭所附着的 SurfaceWindow，不关闭调用方传入的原生 Window；窗口是否继续存在由调用方决定。根组件的实际资源释放遵循下述 content 的安全关闭时机，不在活动绘制／输入回调中抢先销毁其资源。原生 Closing、Application Exiting、SurfaceHost 失败以及显式 close 都必须走同一幂等资源收尾，不能依赖应用转发某个清理通知。

surface 已提供真实的内容拥有关系：SurfaceContent 具有 paint(FrameContext) 和 Closeable 合同，SurfaceHost.attach 可以接收它。现有 painter 入口继续服务直接绘图，由库适配为无额外持有资源的 content。SurfaceWindow 在当前业务回调和 Canvas 借用结束后、renderer／lease 释放前，关闭其 content 一次。UI 根挂载对象作为 content 被持有，因此异常或原生关闭路径也能释放整个 UI 子树。

SurfaceContent 定义在 scoop.surface，只表达可绘制、需要关闭的内容；它的签名不引用 Component、View 或其他 scoop.ui 类型，不产生低层到 UI 的反向依赖。

SurfaceContent 的 close 必须幂等。通过 owner 线程检查后，attach 接收传入的新 content 的所有权；创建或配置失败也由 surface 关闭 content 并回滚已取得的 renderer／lease，调用方不在返回或抛错后再次自行管理它。错误线程的调用在接收所有权前拒绝。一个 content 不交给多个窗口；UiHost 为每个根创建独立对象。

SurfaceContent 的具体合同见 [Surface 规范第 4／7 节](SCOOP-SURFACE-SPEC.md#4-surface-宿主与帧)，aarch64 实施及原生关闭／异常验证见 [UI 实施记录](../ui/PROGRESS.md#34-surface-内容根的所有权)。UI 根作为其 content 管理完整挂载树，逻辑回调内请求关闭同样延迟到回调返回后释放资源。

参数、重复 key、重复挂载、非法测量／放置或错误线程使用现有参数／状态异常。render、update、measure、paint、输入处理或挂载回调抛错时，停止新 UI 工作，通过宿主关闭其管理的相关根及 surface，按 SurfaceHost 既有异常通路传播。首期不保留一个继续接收输入的半更新树，不增加错误边界或自动重试次数。

### 9.1 异步 I/O、下载进度与跨线程更新

需要异步 I/O 的应用显式组合 UiHost 与 `scoop.window.asyncio` 的 Application.runWithAsyncio，复用附着在窗口 owner 上的 PlatformEventLoop、EventLoopSession 和 currentApplicationScope。普通单窗口便利入口不隐式建立 I/O loop；不在 render 或事件回调内嵌套 PlatformEventLoop.run。asyncio、HTTP client 等是使用者／集成示例的实际依赖，不为此加入 UI 核心的公共签名或生产依赖。

默认在该 EventLoop 的活动应用作用域中创建并复用 HttpClient，下载作为子任务排队执行。同一 client 可由所属 EventLoop 的多个任务并发使用。网络及异步文件等待挂起协程，窗口循环继续处理事件；内核／原生 callback／worker 发布完成，由 asyncio 在 owner 消费并通过 continuation 返回调用者的 executor。默认应用任务的 executor 是 owner，因此其进度回调可直接更新同 owner 的 State；setter 只安排逻辑更新，不同步绘制。launch／async 返回不保证正文已经进入，业务初始化和启动前取消须按实际任务状态处理。

应用作用域中的任意真实挂起都保留其 execution，包括第三方 continuation、Channel 和 blocking；外部完成排队回 owner。计算密集工作通过显式 withExecutor／launch(executor) 交给后台 executor，同步阻塞调用通过 blocking 在独立阻塞池执行；仅写 suspend 不会把计算移出 owner。后台任务在 I/O 完成后仍回自己的 executor，需要更新 UI 时显式 withExecutor(loop.executor)，或向保存了 owner executor 的应用作用域启动子任务。普通线程／native callback 的快照通过 ApplicationProxy.post 或 EventLoopSession.post 交付。Context 只传播 binding，不承担调度。

跨线程投递携带稳定的值／只读 class 快照，发送方在交付期间不再修改快照及其引用的数据；整个 State 读改写与通知在 owner 上执行。不在后台先修改 State 再只投递 render，也不让 State setter 隐式变成异步写入。Context 只传递依赖对象；proxy／session 投递执行于接收方的运行 Context，不继承外线程投递时的临时绑定。业务 post 的 false 表示拒绝，true 仍可能在关闭时被丢弃；取消和实际资源清理继续使用下层已有的收尾通路。

下载复用 HttpDownloadOptions.onProgress 及 HttpDownloadProgress 的 bytesWritten、可选 totalBytes 和 resumedBytes。进度快照是普通 class，不要求应用把整个 Store 改成值类型或不可变快照。回调只做轻量状态发布；下载等待该 suspend 回调完成，不能通过等待 UI 绘制来限速回调。总长度未知时保持不定进度，成功完成以 download 返回、目标文件提交及必要清理结束为准，不以字节百分比替代完成结果。

高频普通进度可以在业务交付层按时间合并，只保留最新快照；跨线程实现用同步保护的 latest 值和至多一个待执行投递，入队失败及取消时释放该交付状态。完成、失败、取消及时交付，停止接受该任务的迟到进度；任务被替换后在应用结果时检查业务身份。State 的同轮合并不代替跨线程消息合并，进度交付策略留在业务层。隐藏／无 drawable 不能阻止业务状态、任务结局和清理，不依赖可见帧处理这些工作。

对应下层合同见[窗口集成](SCOOP-WINDOW-SPEC.md#8-asyncio-集成)、[asyncio 宿主接入](SCOOP-ASYNCIO-SPEC.md#113-宿主接入合同)与[HTTP 下载](SCOOP-HTTP-CLIENT-SPEC.md#95-文件下载)；使用示例与取舍见[UI 设计](../ui/DESIGN.md#92-异步下载与-ui-线程)。

具体控制器和显示节流策略归独立下载示例及其业务模型，不加入框架生产依赖或公共接口。组件可在 ComponentScope 中登记普通 Closeable 控制器：关闭时停止接受新发布并取消自己的 Job／后台计算，异步 finally 继续由现有宿主驱动。示例以真实 Thread 执行已下载文件的 SHA-256，线程内创建并关闭文件、缓冲区和 Digest，终态在计算及资源清理之后投递。应用持有活动线程集合，正常终态在 owner 回收句柄并移除任务，事件循环退出后取消并 join 尚未回收的线程；不能依赖可能被拒绝或丢弃的业务 post 完成最终清理。组件卸载不 join 任意后台工作。共享 State、HttpClient、外部字体等借用对象仍由应用所有者关闭；组件卸载不改变其他窗口对共享模型的正常订阅。

## 10. 验收要求

框架的普通库行为归 `scoop.ui` 的 scoop.test。框架测试及自绘示例不依赖 `scoop.ui.component`；控件行为归控件库自身。验收覆盖公开行为，至少包括：

1. 只依赖框架的自绘组件连续更新复用实例和自有 State；两个挂载的自有状态各自独立，卸载后重新出现从初始值开始；同一组件同时委托外部 State，重挂载时保留 Store 中的值。
2. 多组件共享 State、条件读取移除旧依赖、经普通模型 getter 间接读取、只读 val 委托、同轮合并，以及 render／measure／scene／draw 内写入的错误路径；卸载一个订阅者不影响其余订阅或共享值。
3. Component／Element 的 Props 更新不重跑构造器；update 在隐藏时也更新输入；Binding 修改同一原状态、切换绑定后使用新写入路径并退订旧 State；普通 class 内部变更不会被伪装为自动可观察。
4. keyed 列表插入／删除／重排、重复 key、位置身份、组件／Props 类型替换、分支与跨父级移动。
5. 资源取得失败、首轮 render／onMount／update 失败、measure／scene／draw 失败、缓存替换、正常卸载、回调内关闭及清理异常下的剩余资源释放。
6. 隐藏／零尺寸／Deferred 期间仍执行逻辑卸载；成功提交前新区域不命中，失败帧不发布几何，已卸载区域立即退出交互。
7. 外部自绘 Element 的约束、子项测量／放置、背景／子项／前景顺序、变换与 clip、Rect／Path 输入命中和真实窗口关闭。
8. Surface 动画直接驱动测量和绘制、同帧采样、重定目标、隐藏恢复、卸载后取消完成回调，动画不重跑未失效的 Component.render。
9. 普通 Application.run 与现有 runWithAsyncio 的宿主组合、同 owner 的异步进度更新、外线程直接读写 State 的拒绝、proxy 交付、任务取消及关闭时投递拒绝；迟到结果不访问已释放绘图资源。
10. LayoutMetrics 基线、Component 透传、容器偏移；未放置子树保持挂载、继续逻辑更新且不绘制，下一轮不复用旧放置。
11. 不依赖控件库的公共 ParagraphHost 自绘组合：多样式文字、图片和交互子项，一次测量、RTL 对应、截断后退出命中与焦点、恢复后保留 State、失败帧及尺寸动画重排。
12. 同一 Application／UiHost 下两个独立窗口根，共享模型时分别更新，输入与组件身份隔离，普通关闭一个根不卸载另一根；模态与自绘窗框操作在 window 对应能力实现后单独验收。
13. 应用 Context 在延迟组件构造和后续业务入口有效；短暂绑定不被 View／工厂隐式捕获；Context 重绑定不替换已初始化的 State 委托，缺失异常沿正常挂载失败路径清理。
14. 首次画面及已有通知收尾后的稳定区间内，render／update／measure／scene／draw、目标获取、GPU 提交和呈现次数不再增加，UI 不产生周期唤醒或空任务；单次状态变化／合并请求能唤醒必要更新，随后再次停止。
15. 动画自然完成、暂停、取消、零时长和卸载后停帧；完成回调产生的新失效不丢失。一窗动画不带动其他静止根，隐藏／Deferred 不忙重试，恢复后正确呈现再停帧。普通 run 与 runWithAsyncio 均覆盖静止和唤醒。

独立的自绘示例 Cone 通过已编译的公开 UI 接口实现交互和动画，不引入控件库作为间接依赖。随后以独立控件库的 Label／Button／Checkbox／容器实现及 Counter 实例验证同一接口足以构建常用控件；不能通过向框架增加控件类型分支补足缺失能力。

单独的集成示例／测试 Cone 显式依赖 UI、window.asyncio、HTTP client 及实际使用的下层库，以本地 HTTP 服务验证下载进度驱动自绘界面、未知总长度、成功／失败／取消、任务替换及高频进度合并；覆盖窗口隐藏和关闭时的任务／资源收尾。该组合不重复 HTTP 协议、重试或文件 ABI 的全部测试，也不把 HTTP 依赖引入 UI 核心测试。

native 呈现和窗口行为使用现有真实后端，不为测试单独搭一套生产 renderer 工厂。编译／链接合同使用少量必要的独立 Cone 或 fixture，不按每种组件重复普通类型错误与缓存检查。

静止验收用测试内计数及真实后端的调用记录或分析工具，同时检查逻辑／绘制入口、原生帧通知和 GPU 提交，不能仅凭画面不变或平均帧率低判定通过。先完成初始化及有限的在途通知，再用一次性等待观察覆盖多个显示刷新周期的稳定区间；测试自身不以逐帧 timer 唤醒应用。CPU／GPU 活动与能耗观察用于辅助定位，不把固定硬件百分比当成可移植门槛，也不向生产路径加入持续运行的监测循环。


### 接收者配置与行内装配

上述描述 API 遵守 [库 API 风格](SCOOP-API-STYLE.md)。`portal(open: Binding<Boolean>, content: View, configure: PortalBuilder.() -> Unit = {}): View` 以受控打开绑定及完整内容为必需参数，PortalBuilder 继承 PortalOptionsBuilder 的 placement=Viewport()、handle=None、anchorVisibility=Close、modal=None、dismiss=OverlayDismiss()、transition=PresenceOptions()、onCloseRequest={ _ -> true }、onClosed={ _ -> Unit }，增加 key=Position。`options { ... }` 在当前配置上继续修改，`options(value: PortalOptions)` 完整替换上述选项字段而保留 key。`portalOptions(configure: PortalOptionsBuilder.() -> Unit = {})` 和 PortalOptions.copy 产生可独立使用的配置；PortalOptions 的长参数构造器内部化。

PortalOptionsBuilder 的 viewport 块配置 horizontal／vertical=Center、viewportPadding=8；anchored(anchor) 块配置 preferredSide=Bottom、alignment=Start、gap=4、viewportPadding=8、matchAnchorWidth=false。重复相同 placement 块基于当前值，切换种类从该种类默认值开始；anchored 的 anchor 始终取必需参数。atPoint(point) 块配置 preferredSide=Bottom、alignment=Start、gap=0、viewportPadding=8，point 始终取必需参数。dismiss 块配置 escape=false、outsidePress=Ignore；modal 块从当前 Some 配置或 ModalOptions() 开始，None 赋值显式取消。所有嵌套成员返回 Unit。校验在 PortalOptions 产生时沿用原合同，配置不打开弹层或调用 onCloseRequest／onClosed。

`focusOptions(configure: FocusOptionsBuilder.() -> Unit = {})`、`focusScopeOptions(configure: FocusScopeOptionsBuilder.() -> Unit = {})` 与相应值的 copy 提供原有全部属性和默认值。FocusOptions／FocusScopeOptions 保留小型完整值构造；这不增加第二种挂载流程。`navigationOptions(configure: NavigationOptionsBuilder.() -> Unit = {})` 的 layout=Vertical、direction=None、loop=false、roving=true，使用 add(NavigationItem)、addAll(Array<NavigationItem>) 或 item(handle, row=0, column=0) 收集稳定 handle。结果 items 是独立数组；NavigationItem 保留小型值构造。注册 focusTarget／keyNavigation 和修改其当前 options 仍是明确的挂载操作。

`inlineView(key: InlineKey, view: View, configure: InlineViewBuilder.() -> Unit = {}): InlineView` 配置 size=None 与 alignment=Baseline(Alphabetic)，替换原公开长参数构造。`inlineText(configure: InlineTextBuilder.() -> Unit = {}): InlineText` 同时发布 content: StyledText 与 inlines: Array<InlineView>，便于传给 Label／Button 或 ParagraphHost。InlineTextBuilder 的 style 从 textStyle() 开始，style 块按当前值修改；text(String)、add(TextSpan)、span(TextSpanBuilder.() -> Unit) 收集文本；inline(key, view, alternateText, configure: InlineViewBuilder.() -> Unit = {}) 同时追加文本占位和相应 View 描述。以已有 content／inlines 程序化组合仍沿原有验证入口，inlineText 在发布时检查 key 对应与重复，不能发布缺少 View 的占位。构造不测量、挂载、打开字体或读取 View 内容；发布后追加不改变已发布数组。graphics.text 只负责纯文本，不反向依赖 UI。
