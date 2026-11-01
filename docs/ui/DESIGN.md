# scoop.ui：自绘框架与独立控件库设计

日期：2026-11-02。状态：`scoop.ui` 的独立自绘框架和公共文本适配已在 macOS aarch64 交付，公共 `scoop.ui.style`、主题绘图／几何和三套风格也已交付，基础控件与组合验证继续推进，进展见 [UI 实施记录](PROGRESS.md)。框架公开行为见 [UI 规范](../specs/SCOOP-UI-SPEC.md)，公共文本能力见 [文本排版规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)与[文本排版设计](../graphics/text/DESIGN.md)，基础控件行为见 [控件库规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)。现有 `scoop.gui` 已直接改名为 `scoop.surface`；底层实现与验证沿用 [Surface 规范](../specs/SCOOP-SURFACE-SPEC.md)及[实施记录](../surface/PROGRESS.md)。

## 1. 分层目标

UI 框架提供构建完全自绘控件的能力，应用可以只使用框架和 Surface 绘图／动画完成自己的界面。基础控件作为另一普通库构建：

| 层 | 职责 |
| --- | --- |
| `scoop.graphics` | Canvas、Path、Image、几何、离屏绘图、图像滤镜与图层合成 |
| `scoop.graphics.text` | 字体、富文本描述、Paragraph／TextLayout、行内对象、文字绘制与查询 |
| `scoop.window` | 原生窗口、事件循环、输入和 native lease |
| `scoop.surface` | 窗口呈现、帧调度、统一动画时间轴、InputArea 和焦点 |
| `scoop.ui` | Component／Element、状态、身份、挂载、测量／绘制接入、资源生命周期；text 子包提供公共段落适配 |
| `scoop.ui.style` | 主题绘图协议／几何、视觉状态与语义参数；通用 Style、外框装饰、有序内容效果及自绘适配 |
| `scoop.ui.style.macos／material／fluent` | 同一公共协议的三套自绘主题，各自支持 Light／Dark |
| `scoop.ui.component` | Label、Button、Checkbox、Row／Column／Stack、Frame 及控件专用样式和交互外观 |

主要依赖方向为 `scoop.ui.component → scoop.ui.style → scoop.ui → scoop.surface → scoop.window + scoop.graphics`；三套主题实现分别依赖 style，component 不依赖具体主题，文本支路为 `scoop.ui → scoop.graphics.text → scoop.graphics`。使用下层类型的 Cone 同时声明实际直接依赖。框架不依赖或转导出 style／控件库，不根据 Label／Button 等具体类型做分派，不提供只有该库可以访问的控件入口。

这是目标分层；基础字体、布局、StyledText／SpanStyle 和行内对象已在 graphics.text 实现，UI 接入按实施批次继续推进。`scoop.ui.text` 是 UI Cone 中的公共适配包，不是额外的库或控件集合。它复用 graphics.text 的段落算法，连接子项测量与放置，第三方自绘元素可以直接使用。

Row／Column／Stack 的布局策略也放在控件库。框架提供约束、基线、子项测量和放置协议；文字换行由 graphics.text 处理，点击语义、默认字体、背景、边框和主题不进入框架。`empty` 只表示结构上的空内容。

## 2. 保留 class 组件的使用方式

```kotlin
import scoop.ui.*
import scoop.ui.component.button

class Counter: Component {
    private var count: Int by state(0)

    public override fun render(): View = button(text = f"Count: ${count}", onClick = { count += 1 })
}
```

Component 是普通接口，Counter 是有持久身份的 class。挂载期间只初始化一次成员，状态变化后再次调用同一对象的 render。button 是控件库中的普通函数，最终通过公开框架能力实现。

这里分别看挂载实例、状态、描述与内部节点：

| 对象 | 生命周期与职责 |
| --- | --- |
| Component／Element 实例 | 在一次挂载期间复用；保存本地状态引用、交互信息或绘制缓存，卸载结束其 UI 生命周期 |
| State 与普通业务模型 | 由组件、应用或文档等持有；共享 State 不随某个观察组件卸载而结束 |
| View 描述 | 每次 render 可以重新生成；保存此次输入、工厂、key 和子描述 |
| 框架内部挂载节点 | 保存父子关系、具体实例、布局结果及资源作用域，执行更新和关闭 |

View 采用不可变 class；Point、Size、Constraints 等小型数据继续采用值类型。描述不拥有 native 资源，也不要求应用 close。子序列为不可变 Array<View>，可以复用，无须因为重建描述而强制复制。

## 3. 自绘通过公开 Element 协议接入

Component 负责声明式组合，返回 View。Element 负责实际测量、子项放置、绘制和输入接入。两者都由框架按挂载位置持有，并通过 ComponentScope 管理资源。

Element 的必需操作是 `measure(context, constraints): LayoutMetrics` 与 `paint(context): Unit`，生命周期包括 onMount、update 和 onUnmount。LayoutMetrics 保存尺寸及可选的首行 alphabetic／ideographic baseline。update 在首次挂载后及 Props／子描述变化后执行，即使窗口隐藏也会执行，可以及时改变输入 enabled 或动画目标。

框架的 View 仍可以是封闭描述类型，仅保存空内容、Component 和 Element 三种形状。扩展能力由公开 Element 协议提供：控件实现者不继承内部节点，也不向 View 增加一个 Label 或 Button 分支。

例如，应用自己的颜色块完全不依赖控件库：

```kotlin
import scoop.graphics.*
import scoop.ui.*

class ColorTile(private val props: Props<Color>): Element {
    public override fun measure(context: MeasureContext, constraints: Constraints): LayoutMetrics =
        LayoutMetrics(constraints.constrain(Size(48.0, 48.0)))

    public override fun paint(context: PaintContext) {
        context.canvas.drawRect(
            Rect(0.0, 0.0, context.size.width, context.size.height),
            Paint(color = props.value)
        )
    }
}

class Indicator: Component {
    private var active: Boolean by state(false)

    fun setActive(value: Boolean) {
        active = value
    }

    public override fun render(): View =
        element(
            props =
                if (active)
                    Color(0.2, 0.8, 0.3)
                else
                    Color(0.5, 0.5, 0.5),
            create = ::ColorTile
        )
}
```

State 在 Indicator.render 中读取，新的颜色经 Props 更新到原 ColorTile。paint 直接使用 Surface 帧借用的 graphics Canvas，框架保留 ColorTile 的实例、布局和生命周期。创建闭包只在首次挂载时调用。

Element 可以单独成为自绘叶子，也可以接收显式 children 实现容器。普通无状态组合函数仍返回 View；需要独立状态时使用 Component，需要测量和绘制时使用 Element，两者按实际需要组合。

每个 Element 的绘制使用 FrameContext.withState 的独立子 Canvas，坐标与 clip 随同一份帧状态进入输入收集；子级结束后画布失效，并在正常及异常退出时恢复父级状态。底层 Canvas.withState 已接入真实 Skia／Metal，自绘框架无需访问 graphics 的内部句柄或维护第二套变换。

## 4. 普通语言能力、Props 与身份

state 就是普通 State class。成员属性委托的 getValue／setValue 执行读追踪和写失效，不扫描私有字段，不依赖属性名，也不按 hook 调用序号保存状态。闭包捕获组件 this 时保存 class 引用，符合现有语言规则。

状态依赖通过现有同步 Context 收集，只围绕 Component.render 建立和替换订阅。首期写入即通知，包括同值写回；依靠同轮合并控制重建次数。render／measure／paint 不写 State；元素可以在输入处理和 update 中改变状态，更新排入下一轮。

Element 的 Props 由框架更新。measure／paint 的读取不另外建立依赖图；自绘组件在 render 中读取业务 State 并将数据传入 Props。元素自己的普通交互字段可以在输入处理中修改，随后显式 requestFrame。Surface 动画使用现有采样通路。

component 和 element 均为延迟工厂入口。变化输入使用 Props，双向修改使用 Binding，工厂捕获只适合一次性依赖。父级产生新的描述后，框架复用实例、替换 Props，执行 Component.render 或 Element.update。

匹配采用直接父级下的 key／位置，再比较节点类别、C／E 与 P 的完整 TypeRef。工厂函数引用、描述对象引用、FQN 和地址都不作为身份。具体实例类型须保留 class，使用现有 TypeShape 在库边界检查，不增加 compiler 特判。类型或 key 改变时卸载并重建。

同一实例只能占据一个活动挂载位置，工厂为新挂载创建新实例。只检查当前活动的重复实例，不保留退休对象历史。key 在兄弟范围内生效，跨父级移动会重新挂载；条件退出即卸载，不增加全局 key 或 keep-alive 池。

不引入关联类型、不透明返回类型或 result builder。所有协议方法都是普通固定签名方法；泛型工厂、Props 和具体 ComponentNode／ElementNode class 通过普通泛型实现跨 Cone 承载。

### 4.1 集中状态与 Context

组件自有 `by state(0)` 与共享 `by store.count` 使用相同的 State<T> 委托。前者随新组件创建新的状态，后者取得 Store 已有的 State 引用；同一个组件可以同时采用两种形式。State 保存多个挂载节点的订阅，持有它的对象不必是 Component，也不以属性 delegate 的 thisRef 判断谁需要更新。

业务 Store 是普通 class，可按会话、文档等组合子模型。固定字段由普通属性表达，初始化和业务方法集中在所属模型中；不为此建立字符串 key 字典、StateStore 基类、action/reducer 层或私有字段反射。多个组件委托同一 State，无须复制值、创建镜像状态或逐项 use。间接读取模型 getter 继续由当前 render 收集依赖。

Store 的访问优先复用语言 Context。普通计算属性或辅助函数声明 `context(store: AppStore)`，组件初始化时通过它取得外部 State，之后直接访问保存的 delegate。完整示例及自有／共享状态的生命周期见[框架规范第 3 节](../specs/SCOOP-UI-SPEC.md#3-state变化通知与依赖)。

Context 绑定覆盖实际运行／挂载入口；整个 Application.run／runWithAsyncio 可以在应用 Store 的绑定中运行。仅在临时 scope 中生成 View 不会捕获 Context，函数引用和普通工厂也不会因此保存绑定。不同窗口的专用依赖由应用在实际入口显式安装或传递，不给每个节点保存通用 Context 快照。

State 委托只在初始化时绑定一次，Context 后续重绑定不会使其换绑，也不会产生响应式失效。需要更换依赖时使用当前 Props 或新的挂载身份。首期保留 View 的三个基础形状，不增加 UiLocal、Provider 或挂载树环境继承；Context 提供依赖，State 提供通知，挂载树继续管理组件和资源。

### 4.2 控件属性与派生输入

控件的普通输入通过 Props 更新，可交互的公开值通过 Binding 接入外部状态，内部 hover／pressed 等使用私有 State。Binding.value 只执行源 getter／setter，本身不保存另一份值。当前 Props 中的 Binding 可以替换，控件的 render 和输入路径必须使用最新绑定，不能在构造器中永久捕获第一次输入。

Checkbox.checked 与 Button.enabled 的联动由一个共同 State 完成：Checkbox 通过 Binding 修改它，组合组件在 render 中读取并计算 enabled，再通过 Props 交给 Button。依赖属于读取发生的 render；普通赋值或 State 初始值表达式不会自动成为永久绑定。具体例子见[控件库的 Checkbox](../specs/SCOOP-UI-COMPONENT-SPEC.md#5-checkbox)。

公开的可观察属性本身也能被其他 render 读取，但便利函数返回 View 描述，不能据此取得挂载实例。首期用 State／Binding／普通模型表达跨控件关系，不新增组件 ref 的挂载、缺失和替换协议。

## 5. 一棵挂载树与公开布局能力

内部保留一棵持久树。Component 节点连接 render 返回的子描述；Element 节点持有自绘实例、子项、布局结果和资源作用域。该树同时表达身份与资源归属，不并行维护组件树、布局树、渲染树三套结构。

Constraints 明确区分有限最大值与无界，最终尺寸是有限非负逻辑值。MeasureContext 让 Element 测量每个直接子项、取得尺寸和基线、给出子项位置，并返回自身 LayoutMetrics。Component 透传结果；单子项装饰和 Button 将子项基线加上实际偏移，没有基线时明确返回 None。

首期每个子项每帧只测量一次，可以放置零次或一次。每轮清除旧放置；未放置子树保持挂载，但不绘制、不放置输入区域。这样行内对象可以先测量再参与断行，被省略号隐藏时仍保留状态；不需要为这一需求引入多轮 intrinsic 协商或 keep-alive 池。

PaintContext 提供本帧 Canvas、最终尺寸和统一时间。框架在元素局部坐标中调用 paint，并隔离各元素的 Canvas 状态。元素通过 paintChildren 在自己的背景和前景之间呈现已放置子项，使用 Canvas 的 transform／clip／组透明度控制效果。没有放置的子树不进入 paint。父子关系不自动裁剪，也不自动建立输入区域。

文本内容、样式继承、shaping、TextLayout 和行内对象几何属于 graphics.text。公共 ParagraphHost 复用 Element 的协议管理段落缓存及行内子项接入，UI 核心不需要识别文本类型或提供特殊 TextNode。Label 每帧可以收到 measure，但内容、字体、宽度和行内 metrics 不变时复用布局。

### 5.1 公共文本宿主与行内对象

StyledText 由普通文字、带局部样式的 Span 和带 InlineKey 的行内对象组成。Span 共同参与段落排版，不分别生成 Label；行内图片和控件作为不可拆分盒子参与断行和行高。文本层只接收对象尺寸、基线与对齐，不持有组件。

`scoop.ui.text` 的 InlineView 将对象 key 关联到普通 View，inlineChildren 生成正常 keyed 子描述。ParagraphHost 是 Element 登记的 Closeable 辅助对象，不是额外控件或挂载树。它先通过 measureChild 测量对象，再调用 Paragraph.layout，最后按同一 TextLayout 的 key／box 放置可见对象。

自然尺寸子项在段落最大宽度及无界高度内测量；固定尺寸使用显式 tight 约束。不根据最终行剩余宽度反复协商。默认基线取子项 alphabetic baseline，缺失时使用底边，调用方可选择 ideographic 或明确的其他对齐。

TextLayout 按逻辑顺序保留每个对象的结果，包括 box 为 None 的截断项，不能把可见矩形数组与所有 children 直接 zip。插字、重排及隐藏不改变 key；图片尺寸或按钮文字变化引起 metrics 改变时重排，纯外观变化可以复用布局。paint 先画文字，再画已放置对象，输入几何与其实际变换／clip 一致。

Label 与第三方自绘 Element 使用相同宿主。文字光标、selection 和替代文字的公共语义由 graphics.text 提供，具体编辑交互、样式和剪贴板接入仍属于使用者。详见 [文本排版设计](../graphics/text/DESIGN.md)。

## 6. 输入和动画使用 Surface 的现有能力

Element 在 onMount 中调用 scope.createArea，取得稳定的 Surface InputArea；scope 自动登记其卸载。事件继续使用 AreaEvent／InputResult，焦点、caret、指针捕获序列及取消语义沿用 Surface。

paint 时使用 context.place 放置 Rect／Path 命中区域，记录 Canvas 当时的变换和 clip。容器可将自己的已放置区域传给 paintChildren，成为子区域的输入父级。布局树本身不等于输入区域树，没有交互的 Label 和 Row 可以不创建区域。

点击是否发生、拖出后的视觉状态、键盘激活及 disabled 外观是 Button 的实现职责。Element.update 在逻辑阶段更新 enabled 和处理器，避免把输入禁用推迟到下一次可见绘制。

因段落截断而没有放置的子区域，只在该帧成功提交后按 Surface 规则退出命中、取消序列并清除焦点；失败帧继续使用旧几何。恢复可见性保留组件状态，但不自动恢复已清除的焦点。逻辑卸载／禁用仍立即生效，不与这种呈现可见性混为一谈。

scope.animation 将所属 SurfaceWindow.animations.value 创建的 Animatable 登记为当前挂载的资源。元素调用 Surface 已有的 animateTo／snapTo／pause 等方法，在 measure／paint 中读 value。UI 不提供平行的动画类型、timer 或帧时钟。

每个真实帧由 Surface 先统一采样动画，UI 再测量、放置和绘制当前挂载树。首期每帧遍历布局，昂贵计算由具体元素缓存，因此尺寸或位置动画自然进入同帧测量，无须额外的动画依赖图。只改变绘制值的动画不重跑未失效的 Component.render。

## 7. 逻辑更新独立于 drawable

现有 SurfaceWindow 取得真实 RenderFrame 后才调用 painter。UI 的逻辑队列通过现有事件循环的安全边界独立推进，使隐藏、零尺寸或 Deferred 期间也能移除组件、解除订阅和释放资源。

| 变化 | 生效时机 |
| --- | --- |
| State.value 写入 | setter 返回前保存，逻辑更新另行合并 |
| Props、Component.render、Element.update、事件处理器和 enabled | owner 上的逻辑更新 |
| 子树卸载、任务取消请求、区域和动画退休 | owner 上的逻辑卸载 |
| 动画 value、本帧测量与绘制 | Surface 的实际帧机会 |
| 位置、变换、clip 和命中顺序 | Surface 成功接受该帧后 |

保留节点在旧几何上使用最新处理器；新节点等待首次放置提交才命中，移除的节点立即退出交互。失败帧不发布半帧几何，也不回滚任意业务 class。更新中产生的新 dirty 留到下一轮，不能在结尾统一清零而丢失。

### 7.1 静止、唤醒与能耗

静止时停止渲染是桌面首版的设计要求，未来移动端沿用，见 [UI 规范第 6.1 节](../specs/SCOOP-UI-SPEC.md#61-静止时停止渲染工作)。逻辑更新和绘制分别按实际需求推进：前者负责 dirty 节点，后者沿 Surface 的帧机会工作；两者都没有工作时，不保留持续 tick 的驱动器。

逻辑队列只在实际入队时安排至多一个待执行的唤醒，在安全边界消费本轮工作并保留处理中产生的新失效。队列为空的 AboutToWait 不遍历组件树，不重新检查 State 或 Props；一次处理完成且没有剩余工作时不再次 post 自身。首次挂载、外部写入或节点移除仍能从等待状态唤醒，不为降低能耗推迟真实的资源收尾。

绘制需求通过 SurfaceWindow 合并。最后的必要帧完成且无运行中动画后，向平台撤销需求；保留已挂载组件、缓存和最后画面即可，不需要重放绘图维持显示。动画完成／暂停／取消后，其对象即使仍被 scope 持有也不继续驱动帧。原生通知或 GPU 工作的有限收尾不形成新的周期请求。

| 场景 | 调度行为 |
| --- | --- |
| 稳定画面，无更新和动画 | UI 队列不自我投递，树不遍历，原生帧通知停止，没有新的目标获取或 GPU 提交 |
| 一次状态／输入导致的失效 | 合并逻辑工作及所需绘制，完成后回到等待；输入事件本身不无条件重画全部 UI |
| 活动动画或完成回调 | 仅相关根请求所需帧，最终值和回调产生的变化呈现后停止 |
| 隐藏、零尺寸或 Deferred | 逻辑工作按事件推进，停止无效绘制，等待实际恢复；不靠 timer 反复取帧 |
| 一个窗口活动，其他窗口静止 | 各根独立处理失效，静止窗口不跟随提交 |
| 独立 I/O 或音频继续 | 主循环按业务任务等待／唤醒，只有真正影响画面的通知才产生 UI 需求 |

验证同时覆盖 render／update／measure／paint、原生帧唤醒、目标获取和 submit／present；仅跳过 Component.render 仍可能留下布局或 GPU 空转，仅跳过 GPU 提交也可能留下 CPU 周期唤醒。用真实后端观察收尾后的稳定区间，再触发一次变化并验证重新停帧。细节见[Surface 设计第 6.3 节](../surface/DESIGN.md#63-停帧机制与验证)。这些检查随首期批次交付，不推迟到移动端或局部重绘优化阶段。

## 8. 公共样式、主题绘图与控件

主题的核心调整见[公共主题规范](../specs/SCOOP-UI-THEME-SPEC.md)。本轮把公共主题合同、默认语义参数与基础绘图部件从控件层移到 style；取代控件自己硬编码背景／标记和 ComponentDefaults 固定视觉值的方案。控件保留持久 State、布局组合与交互，只提交类型化视觉状态，消费主题返回的同一份几何与绘图。三套实现可以表达字体、尺寸、内缩、边框、圆角、阴影、焦点及各状态差异，组件不按风格分支。

公共部件首批包含表面／外框、Checkbox 三态标记、Radio 标记和横／纵 Scrollbar。metrics 查询提供建议／最小尺寸与内容内缩；geometry 只接收普通数据，返回对应部件的区域、轮廓及滑块行程，不需要 Canvas。Theme.createPainter 提供可复用 Closeable 绘图对象，draw 只接收 Canvas、同一几何和视觉数据；Element 登记一次 ThemePainterCache，主题变更时替换并关闭资源。离屏和第三方自绘采用同一入口，完整 Radio 分组和滚动行为不因绘图部件交付而扩入本批。

应用通过真实运行入口的 Context 提供稳定 ThemeSource，控件挂载保存来源、render 读取其 State<Theme>，把当前主题传入持久 Element。一次更新切换 macOS／Material／Fluent 和 Light／Dark，保持实例、State、焦点及业务绑定，按新 metrics 重排。局部样式用 StyleOverride／SpanStyle 只覆盖指定属性，其他继续跟随当前主题；局部主题显式通过参数／Props 传递，首期不引入自动子树 Provider。系统跟随使用 window 的 ThemeChanged 通知。


公共外观独立为 `scoop.ui.style`，具体合同见[公共样式规范](../specs/SCOOP-UI-STYLE-SPEC.md)和[图像滤镜规范](../specs/SCOOP-GRAPHICS-EFFECTS-SPEC.md)。Style 是经 Props 更新的普通描述；styled 始终保留稳定包装和内容位置，空样式、添加或移除效果不会重建内容 State。StylePainter 是 Element 登记一次的可替换资源缓存，第三方自绘也可直接使用。UI 增加通用 PaintContext.withState 隔离嵌套绘制，子项绘制次数在同一元素的子作用域间共享；不增加专用效果节点。

顺序为外框阴影、背景、经过有序效果的完整内容、边框。外框阴影使用圆角外框；内容阴影／glow 使用真实 alpha，文字无需转 Path。Glow 是零 offset 的着色 alpha 模糊与原内容 SrcOver 组合。内容效果数组区分先模糊再投影和投影后整体模糊；本节点装饰不进入其自身内容效果，嵌套装饰则属于子树内容。显式 clipContent 裁最终内容输出和子输入，不裁自己的外框阴影。

样式原样传递约束、尺寸和基线，不产生点击区域或隐式 padding。图层不以布局框裁掉向外扩展像素；sigma／offset 使用局部逻辑单位，由根 DPI 变换一次。首批仅缓存当前滤镜与轮廓，不缓存子树像素；替换及时关闭旧资源，文字、图片和子动画持续使用当帧内容。参数动画由 Surface Animatable 采样，静态样式及缓存没有时钟、空投递或帧需求。Backdrop blur 采样背后内容，留为独立后续增强。

`scoop.ui.component` 的公开 Label／Button／Checkbox 等是普通 Component class。便利函数 label／button／checkbox 等保存变化输入并延迟构造这些组件；对应 render 使用公开 Element 实现及子组件组合。

Label 的文字元素委托公共 ParagraphHost，接受 String 或 StyledText 和行内 View；Button 的元素负责输入区域、背景、标签放置、基线透传和焦点前景；Checkbox 以外部 Binding 或自有 State 驱动布尔勾选与切换；Row／Column／Stack 的元素负责布局；Frame 的元素负责单子项装饰。具体 class 可以放在控件库 internal 范围内，其与 UI 的调用仍只经过 public 协议。

字体与文本模型／排版测试归 graphics.text；段落适配、挂载和可见性测试归 UI；控件样式、点击及布局策略的测试归控件库。框架测试不依赖控件库，使用自绘示例验证图文与行内交互能力；控件库本身就是公开扩展协议的实际跨 Cone 使用者。

这一边界通过正常依赖和可见性实现，不需要控件注册表、特殊 provider、动态插件装载或私有友元接口。缺少公共能力时补足具体需要的 Element/context 操作，不能在 UI 中按 Button／Label 类型开分支。

## 9. 生命周期与宿主关闭

Component 与 Element 共用 ComponentScope 的资源登记方式：resource、onDispose、isMounted、requestFrame，以及直接复用 Surface 的 createArea／animation。每个挂载有自己的 scope，不给每个节点另建 Surface、窗口或时间轴。

长寿命资源在 onMount 中创建并登记。ParagraphHost 或路径缓存是普通 Closeable 对象，登记一次，在 measure 中更新并及时关闭旧值；临时 View／TextSpan 不取得 native 资源。外部 Image／Path／字体默认借用，不因传给控件而自动转移所有权。文本缓存不拥有行内组件，关闭旧布局不改变子组件挂载。

卸载先取消业务交互、订阅和排队工作，再按子先父顺序退休区域／动画、调用已成功挂载实例的 onUnmount，并逆序释放登记资源。首次 onMount 失败仍清理已取得资源；清理失败继续处理剩余对象并按 ResourceUseException 汇总。

最外层继续用 use 管理宿主：显式入口采用 `host.use { ... }`，runUiApplication 在内部完成。宿主拥有窗口的 UI 根，根拥有各挂载节点；控件使用位置不写 use。单个窗口关闭即清理相应子树，不等整个应用退出。

`scoop.surface` 已提供最小的 `SurfaceContent : Closeable`，提供 paint(FrameContext)，签名不依赖 UI。SurfaceWindow 明确持有 content，在当前业务回调和 Canvas 借用结束后、renderer／lease 释放前关闭它，覆盖 Closing、Exiting、显式关闭与异常退出。

通过 owner 线程检查后，attach 接收新 content 的所有权；此后的配置失败也负责关闭 content 并回滚 renderer／lease。错误线程调用在转移前拒绝。UiWindow.close 先停止新工作并请求 Surface 安全关闭，不在活跃回调中提前释放子树资源。

SurfaceContent 已在 aarch64 验证正常、原生和异常关闭，并接入现有 Surface 组合示例；合同见 Surface 规范第 4／7 节。UI 挂载树及其子节点清理继续按下述批次实施。

异步继续复用 runWithAsyncio、CoroutineScope 和 Job。随组件结束的任务在 onDispose 发出取消；应用／文档拥有的下载可以跨页面继续，观察组件卸载只解除订阅。同步卸载不阻塞等待自己的回调或任意异步 finally，宿主继续驱动实际收尾。后台任务不跨挂起借用 Canvas 或组件资源，迟到结果在 owner 上检查挂载或当前业务任务身份；不另建 UI 调度器。

### 9.1 多窗口与窗口层前置

scoop.window 已实现多个独立的非模态顶层窗口，共享一个 Application 和原生主循环，按 WindowId 区分事件，默认 LastWindowPolicy.KeepRunning。拟实施的 UiHost 对每个 Window 分别 attach 独立 UI 根；普通关闭一个窗口只卸载该根，同一 owner 上共享的业务 State 可以继续供其他根使用。组件、输入区域和挂载身份不跨窗口共用。

窗口装饰／缩放、按钮和层级已有属性入口；macOS 另有 TitlebarOptions。Win32 在关闭 decorations 但保留 resizable 时仍使用 WS_THICKFRAME，完全自绘窗框的系统拖移／边缘缩放接口尚未实现。

模态窗口的 owner 关系、窗口模态／应用模态的输入范围、关闭及焦点恢复需要先在 window 层补齐；现有原生模态循环测试只是 asyncio／事件循环接入验证。UI 层负责对话框内容与结果交付，不以置顶或屏蔽个别组件事件模拟原生窗口模态。具体现状与待补范围见[窗口规范第 4.1～4.3 节](../specs/SCOOP-WINDOW-SPEC.md#41-多窗口)。

先用现有多窗口基础完成独立根和生命周期组合；模态对话框、自绘窗框系统操作随对应 window 能力单独接入和验收，不延后普通自绘组件的交付。

### 9.2 异步下载与 UI 线程

默认采用一个 UI owner 加附着式 asyncio：应用显式以 Application.runWithAsyncio 驱动 UiHost，在其 EventLoop 和活动应用作用域中创建并复用 HttpClient。下载协程提交网络或文件 I/O 后挂起，窗口继续处理输入和绘制，asyncio 的完成在 owner 恢复。HTTP client 不知道 State 或 Component；应用通过既有进度回调把结果写入模型。

下面的设计示例表示单个活动下载：client 已在该 EventLoop 中创建，Context 提供 DownloadModel，startDownload 从 UI 事件或 onMount 调用。DownloadPhase 是应用自己的业务状态；UI State 类型仍待框架实施，HTTP 下载和 window.asyncio 接口已经存在。

```kotlin
import scoop.http.client.*
import scoop.platform.fs.Path
import scoop.ui.*
import scoop.window.asyncio.currentApplicationScope

enum DownloadPhase {
    Idle, Running, Completed, Cancelled, Failed
}

class DownloadModel {
    val progress: State<Option<HttpDownloadProgress>> = state<Option<HttpDownloadProgress>>(None)
    val phase: State<DownloadPhase> = state(DownloadPhase.Idle)
    val error: State<Option<Throwable>> = state<Option<Throwable>>(None)
}

context(model: DownloadModel)
fun startDownload(client: HttpClient, request: HttpRequest, destination: Path): Job =
    currentApplicationScope().launch suspend {
        model.progress.value = None
        model.error.value = None
        model.phase.value = DownloadPhase.Running
        try {
            client.download(
                request,
                destination,
                HttpDownloadOptions(
                    onProgress = suspend { progress -> model.progress.value = Some(progress) }
                )
            )
            model.phase.value = DownloadPhase.Completed
        } catch (cancelled: CancellationException) {
            model.phase.value = DownloadPhase.Cancelled
            throw cancelled
        } catch (error: Throwable) {
            model.error.value = Some(error)
            model.phase.value = DownloadPhase.Failed
        }
    }
```

返回的 Job 由发起方按组件或业务作用域保存。预期失败在该任务内变为可显示的错误，取消继续传播；共享 client 由其应用所有者在结构化作用域结束前关闭，不随每次下载或某个进度控件卸载而关闭。重复启动时先结束旧任务，或由模型记录任务身份并过滤旧结果，不把本例的一份状态同时交给多个下载竞争写入。

HttpDownloadProgress 是包含 bytesWritten、可选 totalBytes 和 resumedBytes 的只读 class；未知总长度使用不定进度。onProgress 是被下载顺序等待的 suspend 回调，只做轻量发布；成功返回 download 才表示文件提交与必要清理完成。UI 在 render 中读取模型，State setter 只安排更新，不在网络回调中立即绘制。

真实工作线程或独立网络 EventLoop 通过 ApplicationProxy.post／EventLoopSession.post 向 owner 交付快照，State 的整个读改写留在接收方。发送方不再修改快照及其引用的数据，不传仍在复用的网络缓冲区。独立 loop 使用属于该 loop 的 client；CPU 密集计算不会因 suspend 或 launch 自动移出主线程。

高频进度在业务层按时间保留最新值；跨线程用受同步保护的 latest 快照和一个已排队标志合并投递，关闭拒绝后停止发布。完成、失败和取消及时交付，并终止旧进度对结果的覆盖。这里合并的是显示进度，不合并任务结局或清理通知；隐藏窗口和没有 drawable 时，逻辑状态与收尾仍继续执行。

Context 传播 Store 引用，不改变线程和任务归属。只有 asyncio 自己的 I/O 保证排队恢复到 owner，任意第三方挂起点仍可能在别的线程恢复；外部回调通过投递返回 UI。业务 post 使用接收方的运行 Context，不能依赖它取得发送方的临时绑定。完整合同见[框架规范第 9.1 节](../specs/SCOOP-UI-SPEC.md#91-异步-io下载进度与跨线程更新)。

## 10. 失败与退出

构造、render、update、measure、paint、动画业务完成回调和输入处理均可能抛错。首期沿 SurfaceHost 的现有异常通路关闭所管理的根和 Surface；不继续使用半更新树，不提供自动重试或错误边界。

框架恢复绘制状态并中止失败帧，丢弃未提交 placement；新建但未完成挂载的节点也须清理。控件自己的缓存遵守创建失败回滚及替换时及时关闭旧值的合同，错误不能留下未登记 native 资源。

## 11. 实施批次

本期先在 macOS aarch64 完成实施与定向验证，功能完成后统一补验 x86_64；Windows 留待后续。先修复文本、Surface 以及实际遇到的编译器／核心库缺口，再由框架和简单组件消费这些公开能力，不在 UI 中绕过底层问题。测试使用所属 Cone 的 scoop.test 与必要的真实窗口组合，不运行全量测试；只有涉及编译器或 core／std／test 自身时才考虑相应定向 fixture。每批验证后提交、推送并合并 origin/main，清理不再需要的临时文件，详见实施记录。

| 批次 | 交付 | 验收 |
| --- | --- | --- |
| 0：命名整理，已完成 | scoop.surface 及公开类型／native 符号／调用方／文档直接改名 | 既有 Surface 验证见实施记录 |
| 1：文本公共能力 | scoop.graphics.text、现有文本迁移、StyledText／SpanStyle、行内 metrics／结果及 Canvas 接入 | 只依赖 graphics.text／graphics 的真实图文排版；混排、基线、bidi、截断和位置语义 |
| 2：独立自绘闭环 | scoop.ui、Component／Element／State／Props／Binding／View／ComponentScope、LayoutMetrics、UiHost 与 SurfaceContent | 自有与共享 State、Context 挂载时绑定；独立自绘、静止停帧／失效唤醒、Surface 动画、基线／可选放置、关闭与异常释放 |
| 3a：公共文本接入，已完成 | scoop.ui.text 的 ParagraphHost／InlineView | 自绘行内交互、基线、截断、缓存与身份 |
| 3b：graphics 基础效果 | ImageFilter、图层滤镜／混合与共用圆角 Path | 真实离屏／Metal 像素、alpha 阴影、顺序、外扩、clip、DPI、借用与资源 |
| 3c：公共样式 | scoop.ui.style、Style／StyleOverride／styled／StylePainter，消费已交付的 PaintContext.withState | 背景／边框／圆角、外框阴影、内容 drop shadow／glow／blur，身份、输入、资源与最终停帧 |
| 3d：公共主题绘图 | Theme／ThemePainter、四类部件的 metrics／geometry／draw、macOS／Material／Fluent 与 Light／Dark | 离屏输出、几何一致、尺寸／DPI、主题参数与资源替换 |
| 3e：基础控件与主题展示 | Label／Button／Checkbox 消费主题绘图，ThemeSource／Context 及明确局部覆盖 | 图文与 Button 混排；Counter、checked 与 enabled 联动、交互外观、截断和焦点 |
| 4：布局与组合 | Row／Column／Stack／Frame、keyed 子项及资源组合 | 动态列表、基线透传、嵌套布局和动画重排 |
| 5：宿主与生命周期组合 | 隐藏更新、现有 asyncio／HTTP 下载接入及多窗口 | 主线程进度、跨线程合并投递、任务取消／替换、Deferred 无空转、两种 run 的停帧／唤醒、清理失败及多根需求隔离 |

每批完成能运行的纵向能力，不先导出空组件类、空 renderer 或调用后报尚未支持的 API。Frame 和容器属于控件库，其所需的通用子项协议在框架批次中以实际自绘组合验证。

普通行为测试放在所属 Cone。graphics.text 验证文本及行内排版；框架以实际 graphics.text／graphics 离屏能力及 Surface／Window 后端验证子项适配、几何、动画、输入和关闭；style 与主题实现库验证公共绘图／几何和三风格明暗；控件库验证组合、点击、布局、局部覆盖、切换保留身份与焦点。额外的自绘示例 Cone 只消费框架及所需下层公开接口，不依赖控件库。新语言行为不属于本设计，不新增 UI 特判 fixture。

下载集成的独立示例／测试 Cone 通过真实本地 HTTP 服务和现有 window.asyncio 接入验证 UI 组合，显式声明 HTTP、asyncio 与窗口依赖；不扩大框架生产依赖，不重复网络库已有的全部协议测试。Context、属性委托和普通类型错误仍使用语言已有规则，UI 只验证实际挂载与任务组合。

图文绕排／分栏、离屏 UI、GPU 互操作、3D 与视频属于上述首期批次之后的增强方向，分别见[文本排版设计第 8 节](../graphics/text/DESIGN.md#8-后续增强图文绕排分栏与区域续排)和[本文第 13 节](#13-后续增强gpu-互操作3d-与视频)。这些调研结论不增加首期验收前置；相应阶段从实际场景出发补充规范、实现和验证。

基础阴影、glow、内容 blur 及其动画组合已纳入本期 3b／3c；先补 graphics 再由 style 消费。公共主题和三套绘图风格纳入 3d／3e。SkSL 自定义填充／滤镜仍是[第 14 节](#14-后续增强sksl-与组件效果)的后续增强，复用已有图层合同，不阻塞基础效果，也不依赖 3D／媒体库。

## 12. 现有底层的命名迁移

| 原名 | 新名 |
| --- | --- |
| Cone／package `scoop.gui` | `scoop.surface` |
| GuiHost | SurfaceHost |
| GuiWindow／GuiWindowState | SurfaceWindow／SurfaceWindowState |
| `scoop_gui_*` C 符号 | `scoop_surface_*` |
| `docs/gui` | `docs/surface` |
| `SCOOP-GUI-SPEC.md` | `SCOOP-SURFACE-SPEC.md` |

示例、测试 Cone、package 和 native helper 已一起迁移，不保留兼容别名。Windows 的 subsystem 值 gui、通用 GUI 概念和历史 Git 分支名不属于库名。准备脚本按普通 Cone manifest 发现系统库；graphics.text、ui 及后续控件库均按普通依赖构建，不增加特殊解析或框架内置加载步骤。文本从 graphics 的独立迁移已完成，见文本设计与实施记录。

## 13. 后续增强：GPU 互操作、3D 与视频

调研日期：2026-11-01。本节记录把 UI 贴到 3D 物体、在自绘组件内显示 3D 场景，以及视频播放的演进方向。已检查现有 graphics／surface 接口，尚未运行这些场景的原型或选定 3D／媒体依赖；新增 API、GPU 交付及宿主合同在相应后续阶段具体设计，不进入第 11 节的首期范围。

### 13.1 当前基础与缺口

| 能力 | 当前依据 | 后续需要补充 |
| --- | --- | --- |
| 向外部 GPU 目标绘制 Canvas | graphics.native 的 MetalContext.target／D3DContext.target 返回 RenderTarget；draw 借出 Canvas，submit 提交 GPU 工作 | 按实际应用补充外部目标与采样方的交付规则；GL／GLES／Vulkan 需要各自的后端接入 |
| 把外部 GPU 结果画入 Canvas | Image 目前从 RGBA 像素或 PNG／JPEG 创建；RenderTarget.snapshot 对 GPU 目标进行显式读回 | 原生纹理／图像导入、格式转换、使用完成通知及资源释放规则 |
| 完整 UI 离屏运行 | 当前拟实施的 UiHost.attach 绑定 Window，InputArea 与动画属于 SurfaceWindow | 可由外部宿主持有的 UI 根，以及目标、尺寸、时间、输入和关闭的适配 |
| 3D 与媒体处理 | Canvas 提供 2D 绘图；组件可以使用普通库并管理其资源 | 3D 渲染和媒体解码／音频由独立库或应用接入，不进入 UI 核心 |

源码依据为 [MetalContext](../../sysroot/src/scoop.graphics/src/platform/darwin/metal.scoop)、[D3DContext](../../sysroot/src/scoop.graphics/src/platform/windows/d3d.scoop)、[RenderTarget](../../sysroot/src/scoop.graphics/src/native/target.scoop)及 [Image](../../sysroot/src/scoop.graphics/src/drawing/image.scoop)。现有 [Metal 测试](../../sysroot/src/scoop.graphics/tests/metal.scoop)使用的[原生目标](../../sysroot/src/scoop.graphics/test-native/native/metal.m)是同时可绘制和采样的离屏纹理；这证明目标包装已有基础，不代表完整 3D 合成或视频路径已经验证。

当前后端为 macOS 的 Metal 与 Windows 的 D3D12。GL／GLES 可以围绕 FBO 与纹理接入，Vulkan 可以围绕具有相应用途的 image 接入，但仍需实现与部署对应后端；不能因为使用 Skia 就宣称这些路径已经开放。这里的 GPU 渲染目标与负责窗口、帧和输入的 scoop.surface 是不同层次。

### 13.2 把 UI 绘制到 3D 物体

采用 `UI 组件树 → Canvas → GPU 纹理 → 3D 材质 → 物体表面`。UI 仍按二维逻辑尺寸布局，宿主根据显示质量选择纹理像素尺寸和 scale；透视、光照、深度与物体遮挡由 3D 渲染器处理。UI 纹理只在状态、尺寸或自身动画需要时更新，物体的旋转可以以不同频率使用同一张纹理。

3D 宿主自身持续渲染时，也不能逐帧调用未失效 UI 根的布局或绘图。离屏根通过实际失效通知请求更新，静止纹理继续被采样；验证时分别观察宿主帧和 UI 纹理更新次数。

完整 UI 需要离屏宿主接入：宿主提供绘制目标、帧机会、统一时间映射和输入，明确持有并关闭 UI 根；根继续拥有组件、输入区域、动画和缓存。沿现有 owner 串行模型组织托管操作，不因离屏就默认允许跨线程 render。不能只把 UiHost.attach 的窗口参数换成纹理，就认为当前 SurfaceWindow 依赖已经消除。

输入路径为 `窗口位置 → 3D 射线拾取 → 命中面的 UV → UI 逻辑坐标 → InputArea`。3D 一侧处理可见面的选择和遮挡，适配层处理纹理原点、尺寸与坐标映射，然后复用 UI 的普通命中与事件处理。pointer capture 必须覆盖拖出物体表面的后续事件；键盘焦点和 IME 候选框需要连接真实窗口，并把 caret 几何投影回窗口坐标。纹理与对应的输入几何按同一成功提交的 UI 帧交付，不使用尚未提交的布局结果。

### 13.3 在自绘组件内显示 3D 场景

采用 `3D 场景 → 离屏颜色纹理 → graphics 图像接入 → Element.paint`。场景使用自己的深度等渲染资源，UI 合成其颜色结果；Canvas 继续完成组件位置、裁剪、透明度及前景 UI 叠加。这种接入不使普通 UI 子项自动参与 3D 场景的深度测试。

Element 的尺寸和 DPI 决定 viewport 及目标像素尺寸，InputArea 把局部指针、滚轮和键盘事件交给场景控制器，用于相机旋转、缩放和物体选择。场景变化请求绘制；它不要求每次更新都重建组件描述。当前 UI 的实际帧仍执行既有布局与绘制流程，这不是仅重绘一个矩形区域的承诺。

场景静止后撤销持续帧需求，不能因组件内存在 3D renderer 就永久运行绘制循环。时间驱动的运动由场景控制器显式表达需求，结束后停帧；不可见时的处理沿宿主和应用明确的策略，不以绘制空帧维持逻辑时间。

场景拥有的纹理、缓存和控制器在挂载时登记到 ComponentScope；尺寸变化通过已登记的缓存对象替换并释放旧资源。外部共享 renderer／device 的所有权继续由其创建者维护，组件只释放自己的资源和借用。3D pass 与 UI 合成按明确的 GPU 顺序协调，不把 Canvas 保存到下一帧或交给异步任务。

### 13.4 GPU 接入的共同边界

- 优先在同一后端、兼容的 device／context 中共享资源。跨 Metal、D3D、GL 或 Vulkan 不能直接互传纹理句柄，需要实际平台支持的互操作或转换；不承诺任意后端之间零拷贝。
- 区分可渲染目标与可采样图像，明确格式、用途、采样数、颜色空间、alpha、坐标原点及 DPI。纹理能被绘制不代表它已经具备所需采样条件；GPU 图像的导入形式也不必等同于 RenderTarget。
- 生产方写入先于消费方采样执行，使用相应队列顺序、资源状态转换和 GPU 同步。不同队列需要相应同步机制，不能把每帧 CPU 读回或等待 GPU 完成作为普通合成路径。
- 导入时明确拥有或借用关系，资源在 GPU 使用完成前不得销毁、回收或覆盖。同一帧图像使用期间的内容要稳定；不能把持续原地修改的纹理直接冒充现有不可变 Image 的快照。具体导入类型和帧有效期在实施时规定。
- 托管 graphics 包装保持明确的创建线程与关闭规则；原生资源的保留及 GPU 完成后释放由接入层协调。关闭、异常、目标重建和未呈现帧也必须结束实际使用，普通组件不逐帧手写 use。

graphics 负责目标、图像与后端互操作，surface 负责宿主、帧及输入衔接，ui 负责挂载、布局和生命周期；3D 渲染仍由应用或独立库负责。普通组合优先使用纹理合成。直接向 Canvas 当前目标插入原生 3D 命令还涉及 Skia 状态缓存、render pass、深度／模板与裁剪边界，留待存在具体需求时单独设计，不为此提前增加通用渲染插件或 render graph。

### 13.5 视频播放

视频复用外部图像接入，路径为 `文件／网络 → 解复用／解码 → 带呈现时间戳的视频帧 → GPU 图像 → 视频 Element`，音频由媒体播放链路输出。读取、解码、缓冲、播放／暂停、seek、倍速及音画同步属于媒体库；UI 适配显示画面并组合播放按钮、进度条和字幕。后续选型复用成熟媒体库或平台能力，UI 核心和基础控件库不因此强制依赖解码器。这里尚未确定媒体 Cone 或展示组件的公开命名。

播放状态与视频帧采用不同交付方式。播放／暂停、缓冲、错误、时长和适当频率的位置更新可由 UI 适配层映射到 State；视频帧通过具有明确有效期的帧接口交给展示 Element，到应更新画面时请求绘制。30 fps 视频不要求父组件的 render 每秒执行 30 次，也不把每个原生帧资源作为业务 Store 的普通状态值。进度显示通知可以合并，播放结局与资源释放不能被丢弃。

解码完成不等于立即呈现：解码器可能提前产生多帧，视频帧率也不同于显示刷新率。播放器按播放时钟与呈现时间戳选择当前帧，有音频时通常参考音频播放进度；过早的帧等待，过期的帧按策略丢弃，同一视频帧可以用于多个显示帧。待播帧按播放需要保留和释放，不能把尚未到期的帧一律压缩成最新解码结果。

Surface 提供绘制机会，媒体适配协调下一帧到期时间与重绘需求。现有 FrameContext 的采样时间不等于媒体位置或已经发生的屏幕呈现时间；暂停、seek 和倍速需要明确的时间映射，不能直接用 UI 动画时间替代播放器时钟。seek 或更换媒体后，旧位置已排队的帧不得覆盖新的播放结果。

视频播放是明确的外部绘制需求，暂停／停播且控制 UI 无失效时必须撤销；仅音频继续不能自动刷新视频画面。下一视频帧尚未到期且没有其他视觉工作时，允许依靠到期通知或一次性 deadline 再请求帧，不为检查是否有新帧而维持空的高频 tick。恢复播放或新帧到期能够重新唤醒展示。

硬件解码可能返回原生缓冲或 GPU 资源，常见格式包括 NV12／P010。后续图像接入需要处理平面、格式、颜色空间与亮度范围，兼容时直接使用原生资源，否则按需要在 GPU 转换或复制；软件解码的 CPU 像素上传仍可作为可用路径。当前输出合同是 sRGB，首个播放闭环先明确 SDR 转换；HDR 输入需要映射到 SDR，或在独立增强中扩展输出链路，不能仅凭纹理导入就宣称正确支持 HDR。

解码线程通过媒体库的线程交付合同和既有 owner 投递机制通知 UI，不直接调用 Canvas 或写 UI State。原生帧交付需保留其实际资源，不能把在解码线程创建的 graphics 包装直接当成可跨线程对象；接入层在正确 owner 上导入，并在 GPU 不再采样后归还或释放原生帧。

播放器由页面独占时，其关闭登记到页面作用域；应用持有共享播放器时，展示组件卸载只解除订阅并释放自己的帧及绘图资源。异步停止和解码收尾遵守媒体库及宿主的关闭顺序，不在 paint 中阻塞等待。隐藏画面后是否继续音频、预解码或暂停播放由应用策略决定，不能以没有 drawable 代替播放器的逻辑状态。此路径也允许同一视频画面继续作为 3D 材质使用。

### 13.6 后续实施与验证方向

| 后续能力 | 依赖与交付 | 代表性验证 |
| --- | --- | --- |
| GPU 目标／图像互操作 | 先在一个现有后端完成真实纹理的写入、导入与采样；其他 API 按实际需求接入 | 连续更新、格式／alpha／原点、尺寸变化、异步使用完成及关闭；普通帧不经 CPU 读回 |
| 离屏 UI 宿主 | 使用 GPU 目标并补齐 UI 根的帧、输入和所有权适配 | 在立方体上操作按钮／拖动，验证 UV、capture、焦点、纹理与几何一致性及异常收尾；物体旋转而 UI 未变时不重绘纹理 |
| 3D 自绘元素 | 依赖 GPU 图像导入和明确的场景 pass 顺序，可独立于离屏 UI 宿主推进 | 可交互模型查看器，叠加普通 UI；静止停帧、缩放、裁剪、隐藏／恢复和卸载期间资源正确 |
| 视频展示 | 依赖媒体播放链路及帧图像接入，不以 3D 渲染器或离屏 UI 根为前置 | 帧率与屏幕刷新率不同、音画同步、缓冲、暂停／seek、迟到帧、隐藏音频策略、暂停后停止画面驱动及快速卸载 |

以上验证随各自后续能力实施，先完成实际端到端场景，再按具体需求扩展；不把全部 GPU API、HDR 或多媒体平台组合加入首期 UI 验收。高级文本排版按[文本设计第 8 节](../graphics/text/DESIGN.md#8-后续增强图文绕排分栏与区域续排)独立推进。新增后端及媒体依赖遵循普通 Cone、FFI、native 资源与发行流程，不增加 UI 专用编译器或 runtime 通道。

## 14. 后续增强：SkSL 与组件效果

SkSL 的职责、参数与资源模型、扫光示例及分批验证见 [graphics 接入设计](../graphics/sksl/DESIGN.md)。固定 Skia 已有 SkRuntimeEffect，Scoop 尚未暴露相关接口。后续在 graphics 提供 effect、uniform／输入 shader 和 Paint 接入；普通 Element 在 paint 中使用，基础 UI 不增加专用 Shader 节点或编译器机制。

组件在挂载或加载时准备程序和绑定，使用 Props 传入业务参数，在实际帧读取 Surface Animatable 的采样值更新 uniform。程序复用，组件参数独立；自有资源由 ComponentScope 收尾，共享程序由长期所有者维护。有限动画完成后停帧，声明 time uniform 或持有 shader 不产生隐式时钟，遵守[第 7.1 节](#71-静止唤醒与能耗)。

本期先完成无须 SkSL 的 graphics 滤镜与公共 style；后续 SkSL 先支持形状填充和已有图像，再扩展自定义子树滤镜并复用图层接入。背景 Paint 的 shader 不自动处理文字／子项；像素扭曲不自动改变布局、命中或 IME 几何，交互映射必须明确。验证包括多实例参数隔离、颜色／alpha／坐标、连续动画不重复编译、暂停／完成后停止提交以及卸载释放。
