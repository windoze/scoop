# scoop.ui：class 组件与声明式界面设计

日期：2026-11-01。状态：设计完成，尚未实现 `scoop.ui`。公开行为见 [UI 规范](../specs/SCOOP-UI-SPEC.md)。本次同时将现有 `scoop.gui` 直接改名为 `scoop.surface`；已有实现和验证沿用 [Surface 规范](../specs/SCOOP-SURFACE-SPEC.md)与[实施记录](../surface/PROGRESS.md)，不保留旧 Cone、package、类型或 C 符号别名。

## 1. 采用的使用方式

```kotlin
class Counter : Component {
    private var count: Int by state(0)

    override fun render(): View =
        button(
            text = f"Count: {count}",
            onClick = { count += 1 }
        )
}
```

Component 是普通接口，Counter 是有持久身份的 class。组件存活期间只初始化一次成员；状态变化后调用同一个对象的 render。应用描述当前界面，框架负责复用、输入区域、布局和资源关闭。用户既不逐组件写 use，也不按 hook 调用序号保存状态。

这里的三个对象必须分开：

| 对象 | 生命周期与职责 |
| --- | --- |
| Component 实例和 State | 挂载期间持续存在；保存业务状态和响应式依赖 |
| View 描述 | 每次 render 可以重新生成；保存此次属性、子描述和事件处理器 |
| 内部挂载节点 | 按父级／形状／key 复用；拥有组件或绘制资源并执行卸载 |

View 初期采用不可变 class，避免把接口承载和频繁复制复杂描述作为前置优化问题；Point、Size、Insets、约束等小型数据继续使用值类型。该选择不承诺 class 一定更快：持久状态只需分配一次，短命描述的分配量在实现后用实际界面测量。

## 2. 底层改名与分层

`scoop.surface` 表达“将绘图呈现到窗口，并管理这个表面的帧与交互”。它仍能独立用于游戏、自绘工具或无组件的画布程序。`scoop.ui` 则专门表达组件、状态、布局和控件。

依赖方向为：`scoop.ui → scoop.surface → scoop.window + scoop.graphics`。UI 直接使用 graphics 的几何／文本 API 和 window 的应用入口时，也声明实际直接依赖；低层库不反向引用 UI。

本次重命名覆盖：

| 原名 | 新名 |
| --- | --- |
| Cone／package `scoop.gui` | `scoop.surface` |
| GuiHost | SurfaceHost |
| GuiWindow／GuiWindowState | SurfaceWindow／SurfaceWindowState |
| `scoop_gui_*` C 符号 | `scoop_surface_*` |
| `docs/gui` | `docs/surface` |
| `SCOOP-GUI-SPEC.md` | `SCOOP-SURFACE-SPEC.md` |

示例、测试 Cone、测试 package 和 native helper 一起迁移。Windows 的 `subsystem = "gui"`、通用“GUI 应用”概念及历史 Git 分支名不属于库名，不随之改动。准备脚本按 Cone manifest 自动发现系统库，使用新 manifest 重建，不增加兼容解析或重定向。

## 3. 为什么这能作为普通库实现

状态不再寄存在重复执行的函数局部位置中，因此不需要 compiler 为每次 state 调用生成槽位。`state(0)` 就是构造一个 State<Int>；宿主 class 的成员持有它。getValue／setValue 实现读追踪和写失效，不需要 private 字段扫描或属性名元数据。

闭包捕获 Counter.this 时复制 class 引用，符合语言规范 3.3／8.1.3。`count += 1` 通过对象的委托访问器修改同一个 State，不捕获外层词法局部 var。

组件身份使用语言已有的 TypeRef。`component(::Counter)` 保留 C=Counter，构造器引用即使每次产生不同函数值也不影响匹配。使用接口返回类型包装所有工厂会丢掉这个调用点信息，因此入口明确要求保留 class 类型，并在库边界检查 TypeShape。

不引入 Swift 的关联类型、不透明返回类型或 result builder。容器首先使用显式子数组；如果以后增加 receiver DSL，发出子描述的动作必须由普通方法完成，不能假设普通 lambda 会自动收集表达式或保留 if 的源码分组。

## 4. Props 与工厂调用

直接在父 render 中调用 `Counter()`，然后把对象当子节点，容易把对象创建和挂载混在一起。本设计统一使用延迟工厂：

```kotlin
component(::Counter)
component(props = title, create = ::Greeting)
```

第二种形式对应 `Greeting(Props<String>)`。框架首次挂载时创建 Props 和 Greeting，后续只更新 Props.value。工厂捕获不是更新通道；父组件每次换一个 lambda，不会重新执行已经挂载的工厂。

内部组件描述可以是普通 `ComponentView<P, C>`，对应持久 `ComponentNode<P, C>`。匹配信息包含 C、P 的完整 TypeRef；匹配成功后使用普通泛型类型转换访问正确节点。库内非泛型节点接口只提供固定签名操作，具体泛型适配保留在这些普通 class 中，不设计泛型虚方法，不把输入存在不带类型关系的任意属性字典里。

同一静态 C 下更换工厂实现仍复用第一次产生的组件。应用需要切换不同类型或重置构造状态时，应使用具体类型不同的 component 调用或改变 key。构造参数中的一次性依赖与每次变化的 Props 必须在示例里区分。

## 5. 一棵持久挂载树

首期使用一棵内部节点树同时承担身份、父子关系、布局结果、组件实例和原生资源的归属。组件节点连接 render 产生的子树；文本／按钮／容器节点各自保存必要数据。不同时建立彼此平行的组件树、布局树、渲染树和通用协调协议。

描述与节点按直接父级下的 key 或位置匹配，再比较节点形状。key 只在兄弟之间有效。移动 keyed 项保留该节点及其子树；移到另一个父级即卸载并重建。显式 key 的重复是错误；同一描述对象在两个位置出现仍产生两个挂载。

对 keyed 列表，复用的是 ComponentNode 及其实际 Component。输入区域、按钮按下状态和文本布局缓存也随节点复用。不能把“重用绘图对象”当作“保留业务组件状态”的替代。

初期没有全局 key、跨父级搬迁、keep-alive 缓存或节点池。条件退出就卸载；需要在页面退出后继续保留的业务状态，由页面之外的 class 模型明确拥有。

## 6. 响应式更新

库内使用现有同步 Context 机制表示当前读取者，在执行每个 Component.render 前建立作用域，返回或抛错时恢复。依赖集合由读取到的 State 对象构成；订阅者是挂载节点，不是临时 View。

State setter 只保存值、标 dirty 并请求一次 owner 工作。首期写入即通知，不试图为任意 T 自动选择 equals、深比较或对象版本。需要减少同值写入时，业务可以在具体类型上判断；框架先依靠同轮合并控制重建次数。

更新队列保留树关系，祖先失效时在更新其子描述的过程中覆盖同轮后代工作。每次开始处理时消费本轮 dirty 标记，处理期间新产生的标记留下，不能用结尾的统一清零丢掉 onMount 或其他回调触发的下一轮变化。

外部共享状态可以被多个根读取，失效分别进入相应 UiWindow。卸载必须主动解除订阅，否则外部模型会继续保有已退休的节点；不能把可达性问题交给 GC 猜测组件是否应当卸载。

首期以组件 render 为更新粒度。内建布局／绘制节点根据描述属性判断是否需要重排或重画，但不建设 snapshot 系统、派生状态缓存、自动 memo 或独立的 phase 依赖框架。

## 7. 逻辑更新不能依赖 drawable

现有 SurfaceWindow 仅在取得真实 RenderFrame 后调用 painter。这适合直接绘制，却不能成为声明式组件唯一的更新入口：隐藏窗口没有 drawable 时，条件退出的组件也必须取消输入和释放资源。

因此 UiHost 的逻辑队列在现有事件循环的安全回调边界推进，SurfaceWindow 只承担按需显示。逻辑更新完成后请求帧；帧到来时使用当前挂载树测量、绘制和放置区域。

两个提交的具体约定是：

| 变化 | 生效时机 |
| --- | --- |
| State.value 的写入 | setter 返回前保存，渲染另外合并调度 |
| 组件输入、订阅、处理器更新 | owner 上的逻辑更新 |
| 子树卸载、任务取消请求、区域注销 | owner 上的逻辑卸载，即使窗口不可绘制 |
| 节点新位置、变换、clip 和命中顺序 | surface 成功接受对应帧后 |

保留节点在旧几何上接收事件时使用最新处理器；不保留一套旧组件状态与旧闭包来模拟画面快照。被移除节点立即不可交互，新节点要等首次放置提交才可命中。失败帧沿 surface 的原有规则丢弃 placement，不回滚任意业务对象。

## 8. 布局与内建节点

首期完成 text／button／row／column／stack 与有限的样式组合，足以验证类组件、列表和重排。约束明确区分有限最大值与无界；每个最终 Size 都是有限非负逻辑尺寸。初期不做 flex、grid、intrinsic 多轮协商或虚拟化。

文本节点根据文字和样式创建 Paragraph，再按宽度持有 TextLayout；变化时替换并关闭旧资源。位置、变换或 alpha 变化复用布局。绘制和输入区域位置都从同一布局及 Canvas 状态得出。

按钮的交互状态归自己的持久节点。它复用 InputArea，按 Surface 的固定指针序列处理拖出／返回／取消，并将一次完整的激活转换为 onClick。库负责其资源，用户的 Counter 只拥有 count 和行为。

外部 Image／Path／字体默认借用；传入一个 Closeable 不是自动转移所有权。对应用自己创建的资源，ComponentScope 提供一次登记及统一释放；对库自己创建的字体、段落和区域，节点直接拥有。

## 9. 生命周期与现有 surface 的接入点

组件协议只要求 render，onMount／onUnmount 有明确无操作的默认行为。onMount 的 ComponentScope 提供 resource 与 onDispose；onUnmount 子先父后执行，随后逆序关闭登记资源。状态容器本身没有 UI 卸载义务，可以被外部继续持有。

最外层仍采用 use 管理宿主：显式入口由调用方以 `host.use { ... }` 包住事件循环，便利入口 runUiApplication 在内部完成。宿主拥有各窗口的内容，UI 根拥有子节点，节点拥有实际资源；用户不逐组件 use。单窗关闭即释放对应子树，最外层 use 负责应用退出时剩余内容的兜底收尾。

现有 SurfaceHost.attach 只接收 painter，SurfaceWindow 并不知道闭包捕获的组件树需要关闭。UI 接入时增加一个真实使用者所需的最小接口 SurfaceContent，包含 paint 与 Closeable。UI 根对象实现它；直接绘图的 painter 由无额外资源的适配对象承载。

SurfaceContent 的签名只使用 surface 的 FrameContext 和 core 的 Closeable。surface 不认识 UI 根的具体类型，也不依赖 scoop.ui；这里增加的是普通资源拥有关系。

SurfaceWindow 负责 content 的确定性关闭。关闭发生在当前输入／绘制回调与 Canvas 借用退出后、renderer 和 NativeWindowLease 释放前；renderer 异常、原生 Closing、Exiting 和手动 close 都覆盖。UiHost 不另外复制一套 native Window 状态机。

这个接口在本次仅作为待实施接入点写入设计，重命名后的 surface 行为保持现状。实施 UI 宿主闭环时先更新 Surface 规范，再增加接口和必要的真实窗口清理测试。

异步继续复用 runWithAsyncio、CoroutineScope 和 Job。onDispose 可发出取消；同步 close 不等待自己的回调，也不等待任意异步 finally。后台任务只持有其自行管理的 I/O／托管数据，不跨挂起借用组件的 Canvas 或 GUI 资源；返回时确认挂载仍有效。首期不增加另一套 launch／effect／取消树。

## 10. 失败与退出

组件构造、render、原生布局、paint 和回调都可能抛异常。首期沿已有 SurfaceHost 的异常政策停止并关闭所管理的 surface；UI 根的确定性拥有关系保证组件也收尾。不建立失败后继续使用半更新树的路径，也不假装能回滚用户 class 的任意原地修改。

一轮中新建但尚未完成挂载的节点必须在失败路径清理。未成功完成 onMount 的组件不接收 onUnmount，但其 ComponentScope 已登记资源仍全部关闭。清理错误不阻断后续资源释放，使用现有 ResourceUseException 汇总。

关闭先让子树不再接受业务事件和 State 订阅通知，再执行清理；清理回调产生的其他仍活动根的状态更新可正常排队，但不能使关闭中的节点复活。退出不能以再次投递 ApplicationProxy 作为唯一清理步骤。

## 11. 实施批次

| 批次 | 交付 | 验收 |
| --- | --- | --- |
| 0：命名整理，本次执行 | scoop.surface 及公开类型／native 符号／调用方／文档直接改名 | 格式化、类型检查、真实现有 Cone 测试和示例构建 |
| 1：Counter 完整闭环 | scoop.ui、Component／State／View／ComponentScope、最小 text／button、UiHost、SurfaceContent 和根卸载 | 一个可点击窗口；父更新保留实例；关闭与异常确定性释放资源 |
| 2：组合、输入与布局 | Props／Binding、row／column／stack、位置／key 匹配、子树资源及按钮焦点 | 独立计数器、动态输入、keyed 列表重排、条件卸载和真实文本测量 |
| 3：生命周期组合 | 共享模型、隐藏窗口逻辑更新、既有 asyncio 接入示例 | 订阅／任务取消、迟到结果、Deferred、回调内关闭、多窗口及清理失败 |

每批先实现能运行的纵向功能，不先提交空组件类、空 renderer 或只供测试的 backend。规范所列其他 API 随所属批次实现；不提前导出调用后报“尚未支持”的符号。

测试优先放在所属 Cone。状态、匹配、Props 和作用域测试围绕可观察结果；布局使用实际 graphics 离屏能力；窗口呈现、焦点和关闭使用真实 surface／window 后端。新语言行为不属于本设计，因此不新增 UI 特判的编译器 fixture。必要的跨 Cone 链接和平台构建复用已有测试方式。

## 12. 实施前已确定的边界

- Component／State 优先采用 class；不为这一 API 修改 struct 的不可变规则。
- State 由成员持有，没有调用序号、隐式 remember 或自动字段发现。
- create 只在挂载时调用；变化输入走 Props，双向修改走 Binding。
- key 是直接父级内的稳定业务身份；class 引用和描述引用不代替位置匹配。
- 逻辑卸载独立于帧机会；区域新几何继续只在成功提交后发布。
- 普通组件不使用 use；框架内部仍用现有确定性资源管理来实现所有权。
- 本次提供设计及底层重命名，尚未建立 scoop.ui 源码 Cone，避免把设计状态误报为可用 API。
