# Scoop 声明式 UI 库规范

日期：2026-11-01。状态：设计稿，`scoop.ui` 尚未实现。本文件规定拟实施的库合同；取舍、内部组织及实施顺序见 [UI 设计](../ui/DESIGN.md)。现有底层库以 `scoop.surface` 命名，已实现的绘图、窗口呈现及输入合同见 [Surface 规范](SCOOP-SURFACE-SPEC.md)。

## 1. 定位与依赖

`scoop.ui` 是普通 Scoop Cone，提供声明式组件、可观察状态、组件身份、布局及组件资源生命周期。它依赖实际使用的 core／std／platform、`scoop.graphics`、`scoop.surface` 和 `scoop.window`。窗口事件循环、呈现后端、文本 shaping 和 GC 仍由原有层负责。

首期目标跟随 surface 的 Darwin／Windows 后端；不因提供声明式 API 就扩大到底层尚未实现的平台，也不发布空 renderer。

| Cone | 职责 |
| --- | --- |
| `scoop.window` | 原生窗口、事件循环、输入归一、IME 和 native lease |
| `scoop.graphics` | 几何、Canvas、Path、Image、字体、Paragraph 和 TextLayout |
| `scoop.surface` | SurfaceHost／SurfaceWindow、窗口呈现、帧机会、动画、InputArea 和区域焦点 |
| `scoop.ui` | class 组件、State／Props／Binding、描述与挂载树、布局、控件和资源归属 |

组件、状态及描述使用普通 class／interface／泛型／属性委托。首期不增加 `@Composable`、result builder、属性宏、关联类型、UI 专用 IR 或 runtime 入口。`scoop.ui` 的类型身份查询复用语言规范 9.6 的 `typeInfo<T>()`，不遍历组件字段，不按名称访问私有属性。

本期交付目标是 Counter、条件组件、可重排列表以及资源随组件卸载的完整闭环。完整文本编辑器、虚拟列表、手势竞争、原生控件封装、accessibility 树、并发／可中断渲染和自动 memoization 不进入首期 API。

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

class Counter : Component {
    private var count: Int by state(0)

    override fun render(): View =
        button(
            text = f"Count: {count}",
            onClick = { count += 1 }
        )
}

fun main() {
    runUiApplication(title = "Counter", create = ::Counter)
}
```

组件采用持续存在的 class 实例。`render` 可以执行多次，实例及成员 State 不因重建描述而重新构造。用户不为 Component、text、button 或容器逐个调用 `use`／`close`。

无状态组合可以写成普通的返回 View 的函数；这种调用不创建独立的挂载身份，其间的 State 读取归调用者当前的 render。需要独立本地状态和生命周期时使用 Component 及第 5 节的 component 入口。

`View` 是 `scoop.ui` 内定义的封闭描述类型，以不可变 class 保存本次输入、子描述或回调。一个描述不拥有 InputArea、TextLayout、动画或 GPU 资源；描述本身没有 `close`。应用通过 Component 组合库提供的描述，不在首期实现任意自定义底层节点协议。

同一描述值可被使用多次；每个出现位置分别挂载，不能按描述对象引用相等而合并挂载。描述不可变指保存的属性槽及子序列不可修改，不承诺其所引用的业务模型深度不可变。子序列以不可变 Array<View> 保存，允许复用该数组，不因重建描述而强制复制。

`render` 只读取当前输入／状态并产生描述。它不得写 State、操作窗口、取得需要关闭的原生资源、启动任务或登记订阅。State 写入口在活动 render 期间报告状态错误；其他副作用遵循普通 API 的使用约定，不新增编译器纯度检查。组件构造器只初始化托管状态和保存输入；资源取得放在挂载回调。

## 3. State、变化通知与依赖

`State<T>` 是可变引用类型，提供普通 `value` 属性，并实现属性委托的 `getValue(thisRef: Any): T`／`setValue(thisRef: Any, value: T): Unit`。两个方法均为普通、非泛型、非挂起的 operator。`thisRef` 不用于推断属性名、查找宿主字段或决定状态身份。`state(initial)` 每次调用创建一个新的 State；它没有 remember／hook 槽位语义。

组件成员初始化时创建 State，组件被保留时自然保留该对象。共享模型也可以声明委托属性，多个组件读取同一个 State 时各自建立依赖。状态值 T 可以是 struct、class 或其他合法类型；把 State 做成 class 不取消读取值类型 T 时的普通值语义。

状态对象绑定创建线程。值的读写及订阅维护在该线程执行；当前 UI 还要求该线程是窗口 owner。后台工作通过既有 owner 投递机制返回结果，不直接修改 UI 状态。相同 owner 上的多个 UiWindow 可以观察同一状态；不同 owner 不共享这套无锁状态对象。

一次 `render` 使用一个库内、同步的依赖收集作用域：

1. State getter 将当前组件节点记入本次读取集合，同一 State 的重复读取只登记一次。
2. 成功执行后，以本次集合替换该组件的旧订阅；不再读取的 State 不再使它失效。
3. setter 先保存新值，再将当前订阅者标记为 dirty；不在 setter 内递归调用 render。
4. 组件卸载时解除所有订阅，并从待更新集合移除；共享 State 本身仍然可用。

首期每次合法 setter 调用都发出变化通知，包括写回相同值。调度器合并同一轮更新，不为任意 T 引入隐式相等、深比较或接口约束。相等策略和专用集合增量通知以后由实际性能需求决定。

普通 getter／函数调用中的间接 State 读取，同样归当前活动 render。render 之外的读取只是取值，不产生永久订阅。首期不自动建立 layout／paint 阶段的独立依赖图；自定义绘图回调需要的数据应先在 render 中读取，再捕获该次值。

普通 class 字段不是可观察属性。对 `modelState.value.name` 赋值不会经过 `modelState.value` 的 setter；`name` 应有自己的可观察属性，或调用方显式赋回状态值。默认 setter 不按引用相等跳过通知。对普通可变集合的原地修改也遵循此规则。

## 4. Props 与 Binding

组件的构造器参数在实例生命周期内只初始化一次，不能用每次变化的构造器捕获来传递更新。需要变化的父级输入使用 `Props<P>`：它是只读引用容器，公开 `value: P`，由框架在该组件本轮 render 前更新。

```kotlin
class Greeting(private val props: Props<String>) : Component {
    override fun render(): View = text(props.value)
}

class Screen : Component {
    private var name: String by state("Scoop")

    override fun render(): View =
        column(
            arrayOf(
                component(props = name, create = ::Greeting),
                component(::Counter)
            ),
            spacing = 12.0
        )
}
```

`Props<P>` 由组件节点持有，不允许子组件替换父级输入。它是当前输入的入口，不是自动保存旧版本的快照。需要异步操作使用启动时的参数时，应先复制需要的值到局部 `val`；要在调用时读取最新输入的回调，则在回调体内访问 `props.value`。

父组件产生新的子组件描述时，框架更新其 Props 并请求子组件 render；初期不根据 P 的相等或引用身份跳过。每个组件在同一重建轮中至多执行一次；与该轮并行登记的新失效留到下一轮。只读输入本身不产生第二套可观察模型。

`Binding<T>` 是带 getter／setter 的普通引用对象，用于明确允许子组件修改父级状态。可以由显式 State 的 `binding()` 取得，也可使用 `binding(get = { count }, set = { count = it })` 连接委托属性。绑定的 getter／setter 继续执行原 State 的追踪与线程规则；不增加 `$property` 或取得隐藏 delegate 字段的语法。

## 5. 创建、身份与复用

组件构造发生在挂载时，公开入口的签名形状为：

```text
fun <C : Component> component(
    create: () -> C, key: ViewKey = ViewKey.Position
): View
fun <P, C : Component> component(
    props: P, create: (Props<P>) -> C, key: ViewKey = ViewKey.Position
): View
```

`ViewKey` 是包含 Position、Text(String)、Number(Long) 的值类型。Position 表示采用同一父级子序列中的位置；另外两个分支是显式 key。

`component` 本身只构造描述，不能立即调用 create。首次挂载才调用工厂，并保存其返回实例；之后匹配到同一节点时更新 Props，复用实例。无 Props 的入口统一使用 Unit 输入。

组件形状包含 `typeInfo<C>().type` 和 `typeInfo<P>().type`。用完整 TypeRef 相等比较，不比较展示名、函数值引用、描述对象引用、源码行号或分配地址。P 是内部泛型承载的组成部分，输入类型改变时重新挂载，不能把旧的 `Props<P>` 强转为另一类型。

C 必须保留 class 类型。库使用既有的一层 TypeShape 描述拒绝接口、object 或值类型作为 C，不要求新的编译器诊断。相同静态 C 下改变工厂闭包或返回子类，并不改变已挂载组件；工厂只负责初次创建。需要选择不同组件的调用方应保留不同的具体 C，或显式更换 key。

同一个 Component 实例只能属于一个挂载位置。工厂必须为新挂载创建新实例，不能返回某个已挂载的单例。框架检查当前活动实例的重复挂载，不为已经退休的对象保留全局历史表；为新挂载返回新实例是 create 的使用合同。共享业务状态应通过普通模型／Props／Binding 表达。

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

State 变化合并到 owner 上的 UI 工作队列。事件／业务回调退出后，经现有 ApplicationProxy 或 AboutToWait 的安全边界处理；不在活动输入派发、render、layout 或 paint 内递归重建。祖先和后代同时失效时，由祖先处理同轮子描述，并避免重复执行后代 render。

UI 逻辑提交与 surface 的呈现提交有不同的时机：

- 逻辑更新执行 render、匹配节点、更新输入、挂载／卸载及订阅更换。隐藏、遮挡、零尺寸或暂时没有 drawable 不能阻止这一过程及资源清理。
- 需要显示的变化向 SurfaceWindow 请求帧。在真实绘制机会中执行所需测量／布局、绘制和 InputArea 放置。
- 只有 surface 成功接受提交后，才按 Surface 规范发布命中几何；失败或 Deferred 不发布新几何，不回滚已经生效的业务状态。

首期在 owner 上同步完成一轮逻辑更新，不提供并发、暂停恢复或丢弃后重放 render 的机制。所有更新使用当前可变状态；不承诺 React 式的不可变状态快照。用户 render／构造／挂载回调失败时走第 9 节的关闭路径，不试图回滚用户修改过的任意对象。

新描述中的事件处理器随逻辑更新替换。尚未提交新的画面时，保留最近已提交的几何与节点身份，派发使用节点最新的处理器。新节点在首次成功放置前不能命中；已卸载节点立即退出交互，不能为了等下一帧而继续接收事件。这里承诺几何对应最近提交的帧，不承诺处理器保留旧帧捕获的输入值。

首期内建描述包括 empty、text、button、row、column、stack，以及 padding／固定尺寸／对齐／背景／clip 组合。容器接收显式 `Array<View>` 子序列；不把普通 lambda 的未使用表达式冒充 result builder。函数式组合以后可以在有实际需要时增加纯库 receiver DSL。

测量使用有限非负逻辑尺寸。约束按轴包含最小值和可选最大值，缺少最大值表达无界，不把 Infinity 传给 graphics。Text 使用最大宽度换行，无界时使用已有无换行布局入口。row／column 初期按子项自然尺寸、交叉轴约束及 spacing 排列，不包含 flex／weight 算法；容器自身尺寸按约束收敛，超出部分只有显式 clip 才裁剪。stack 共享约束并按对齐放置子项。

文本节点持有 Paragraph／TextLayout，按文字、样式和约束的实际变化重建；纯位置、变换或透明度变化复用布局。所有原生资源由挂载节点持有，绘制使用 FrameContext 借用的 Canvas。窗口 resize／DPI 变化使布局或绘制失效，不重建组件实例。

## 7. 输入、焦点与按钮

可交互节点各自持有稳定的 InputArea，更新描述时修改处理器和必要属性，不每帧创建新区域。绘制与区域放置共享 Canvas 的 transform／clip，继续采用 surface 的命中、冒泡、指针序列和逻辑焦点规则。

button 保存自己的 hover／pressed／focus 交互状态。主按钮在区域内按下、随后在有效区域内释放时触发一次 onClick；取消、禁用、卸载或失焦清除按下状态，不合成点击。获得键盘焦点时支持 Enter／Space 的按下与释放激活，重复按键不产生多次激活；激活序列途中失焦或禁用同样取消。组件状态和按钮内部交互状态分别属于各自挂载节点。

移除子树必须注销 InputArea，结束相关拖动／hover／焦点／IME 接收，再释放其处理器引用。布局尚未提交时不能为新区域强行成功请求焦点。完整文本编辑控件在后续单独规定文档、selection、marked range、剪贴板和撤销行为；首期不以拼接键盘字符代替现有 TextInputBatch。

## 8. 挂载与资源生命周期

`ComponentScope` 是框架创建和关闭的引用对象，提供：

- `resource<R : Closeable>(create: () -> R): R`：成功取得后立即登记为本组件拥有的资源。
- `onDispose(action: () -> Unit)`：登记卸载时同步执行的清理动作，例如解除外部订阅或发出任务取消。
- `isMounted`：当前挂载是否仍可接收业务工作。

首轮逻辑提交建立节点和订阅后，按父先子后的顺序调用 onMount，后续 render 不再调用。onMount 中的 State 写入排到下一轮，不递归重建。外部订阅和资源在此建立；不提供每次 render 自动重跑的 effect API。

卸载先把整棵子树标为不活动并解除依赖／排队工作，然后按子先父后的顺序处理各节点：退休输入和内建动画，调用已成功完成 onMount 的组件的 onUnmount，最后逆登记顺序执行 ComponentScope 清理及节点资源关闭。onUnmount 执行时 scope 已不再允许新增资源，但原有资源尚未被该 scope 释放。没有自定义 onMount 的组件同样完成默认挂载并获得 onUnmount。

构造器或首轮 render 失败的实例没有完成挂载，不调用 onUnmount；已经建立的库内节点资源仍须释放。onMount 自身失败时，其 scope 中已登记的资源同样全部清理，不要求失败的回调再自行释放它们。所有清理分支继续处理剩余资源，错误合并遵守现有 ResourceUseException。

库自动管理自己的 InputArea、动画、字体／段落／布局等资源。传入的 Image、Path、FontCollection 或外部模型默认借用；库需要跨调用保留 native 使用时遵循 graphics 的持有合同，不能擅自关闭调用方拥有的包装对象。组件要转移资源所有权时，通过自己的 ComponentScope 创建并登记。

状态对象与托管引用最终由 GC 回收；卸载、取消输入、关闭 native 资源不能等待 GC。外部继续持有一个卸载后的模型不会使其重新渲染；外部继续持有组件也不延长它的挂载期。

与 asyncio 组合时，异步任务仍属于现有 CoroutineScope／Job；可在 onDispose 同步调用现有取消入口，异步 finally 由既有 event loop 完成。ComponentScope 的同步清理不承诺等待任意异步任务完成，不新增 UI 任务调度器。Canvas、布局／绘制借用和组件拥有的 GUI 资源不能跨挂起继续借用；迟到结果需检查挂载是否仍活动，或只写已无 UI 订阅的业务状态。需要专用 launch／effect API 时再规定独立的取消与结果交付合同。

## 9. 宿主、关闭与错误

`UiHost` 包装 SurfaceHost，作为普通 ApplicationHandler 接入 Application.run，也可以接入现有 runWithAsyncio。`UiHost.attach(window, create)` 返回管理根组件的 UiWindow。`runUiApplication(title, create)` 是单窗口便利入口，统一拥有 Application／UiHost／窗口，接受关闭请求并在最后一个窗口关闭后退出；复杂窗口策略使用显式 UiHost。

资源管理在最外层保留明确的 use 作用域。显式创建 UiHost 的调用方以 `host.use { ... }` 包住事件循环；`runUiApplication` 在内部建立并关闭相同的宿主作用域。所有权由宿主经 SurfaceWindow 的 content 传到 UI 根及各挂载节点，普通组件不再逐个 use。单个窗口关闭时立即收尾它的内容，不等整个应用离开最外层 use；最后的宿主 close 幂等地释放仍然存在的窗口。

UiWindow.close 停止根组件的新工作并请求关闭所附着的 SurfaceWindow，不关闭调用方传入的原生 Window；窗口是否继续存在由调用方决定。根组件的实际资源释放遵循下述 content 的安全关闭时机，不在活动绘制／输入回调中抢先销毁其资源。原生 Closing、Application Exiting、SurfaceHost 失败以及显式 close 都必须走同一幂等资源收尾，不能依赖应用转发某个清理通知。

UI 接入阶段需要给 surface 增加一个真实的拥有关系：SurfaceContent 具有 paint(FrameContext) 和 Closeable 合同，SurfaceHost.attach 可以接收它。现有 painter 入口继续服务直接绘图，由库适配为无额外持有资源的 content。SurfaceWindow 在当前业务回调和 Canvas 借用结束后、renderer／lease 释放前，关闭其 content 一次。UI 根挂载对象作为 content 被持有，因此异常或原生关闭路径也能释放整个 UI 子树。

SurfaceContent 定义在 scoop.surface，只表达可绘制、需要关闭的内容；它的签名不引用 Component、View 或其他 scoop.ui 类型，不产生低层到 UI 的反向依赖。

SurfaceContent 的 close 必须幂等。通过 owner 线程检查后，attach 接收传入的新 content 的所有权；创建或配置失败也由 surface 关闭 content 并回滚已取得的 renderer／lease，调用方不在返回或抛错后再次自行管理它。错误线程的调用在接收所有权前拒绝。一个 content 不交给多个窗口；UiHost 为每个根创建独立对象。

上述 SurfaceContent 是待实施的 UI 接入增量，尚不属于已实现的 surface API；实施前须先同步 Surface 规范第 4／7 节。单纯给现有 painter 闭包增加一个 GC 引用不能满足此关闭合同。

参数、重复 key、重复挂载或错误线程使用现有参数／状态异常。render、layout、paint、输入处理或挂载回调抛错时，停止新 UI 工作，通过宿主关闭其管理的相关根及 surface，按 SurfaceHost 既有异常通路传播。首期不保留一个继续接收输入的半更新树，不增加错误边界或自动重试次数。

## 10. 验收要求

普通库行为归 `scoop.ui` 的 scoop.test。验收覆盖公开行为，至少包括：

1. Counter 连续更新复用组件和 State；两个 Counter 各自独立；卸载后重新出现从初始状态开始。
2. 多组件共享 State、条件读取移除旧依赖、间接读取、同轮合并，以及 render 内写入的错误路径。
3. Props 更新不重跑构造器；回调按约定读最新输入；显式 Binding 修改原状态；普通 class 内部变更不会被伪装为自动可观察。
4. keyed 列表插入／删除／重排、重复 key、位置身份、组件／Props 类型替换、分支与跨父级移动。
5. 资源取得失败、首轮 render／onMount 失败、正常卸载、回调内关闭及清理异常下的剩余资源释放。
6. 隐藏／零尺寸／Deferred 期间仍执行逻辑卸载；成功提交前新区域不命中，失败帧不发布几何，已卸载区域立即退出交互。
7. 实际 graphics 文本测量、约束／DPI 变化、button 指针／键盘取消及真实 SurfaceHost 窗口关闭。
8. 普通 Application.run 与现有 runWithAsyncio 的宿主组合，及取消后迟到结果不访问已释放 GUI 资源。

native 呈现和窗口行为使用现有真实后端，不为测试单独搭一套生产 renderer 工厂。编译／链接合同使用少量必要的独立 Cone 或 fixture，不按每种组件重复普通类型错误与缓存检查。
