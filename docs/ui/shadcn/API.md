# 公共 API、主题与组合合同

日期：2026-11-04。状态：分批实施，见 [实施记录](PROGRESS.md)。尚未闭合批次的新增签名表达拟议 API；实施前须把完整字段、默认值和错误合同写入所属 spec。现有接口是否可用以各库现行规范为准。

## 1. 沿用 Scoop 的声明与状态模型

所有入口继续返回普通 View，使用持久 Component／Element 和更新后的 Props。必需的模型、绑定、内容、动作在 configure 之前；配置块同步执行一次，只构建描述。多子项使用现有 ChildrenBuilder 的真实成员或 `add(View)`，不引入第二种 DSL、隐式收集表达式、React hooks 或挂载树 Provider。

代表签名如下；configure 的具体 builder 按控件定义，不用一个万能配置对象承载所有字段：

```scoop
button(content: View, onClick: () -> Unit, configure: ContentButtonBuilder.() -> Unit = {}): View
checkbox(checked: Binding<Boolean>, configure: CheckboxBuilder.() -> Unit = {}): View
checkbox(checked: Binding<CheckState>, configure: CheckboxBuilder.() -> Unit = {}): View
input(state: TextEditorState, configure: InputBuilder.() -> Unit = {}): View
inputOtp(state: TextEditorState, length: Long, configure: InputOtpBuilder.() -> Unit = {}): View
slider(values: Binding<Array<Double>>, range: RangeSpec, configure: SliderBuilder.() -> Unit = {}): View
fieldLabel(text: String, target: FieldTarget, configure: FieldLabelBuilder.() -> Unit = {}): View
collapsible(open: Binding<Boolean>, content: View, configure: CollapsibleBuilder.() -> Unit = {}): View
dialog(open: Binding<Boolean>, content: View, configure: DialogBuilder.() -> Unit = {}): View
popover(open: Binding<Boolean>, anchor: LayoutAnchor, content: View, configure: PopoverBuilder.() -> Unit = {}): View
tooltip(trigger: View, content: View, configure: TooltipBuilder.() -> Unit = {}): View
toaster(state: ToastState, configure: ToasterBuilder.() -> Unit = {}): View
messageScroller(source: TranscriptSource, state: MessageScrollState, configure: MessageScrollerBuilder.() -> Unit = {}): View
questionnaire(state: QuestionnaireState, onSubmit: (QuestionnaireResult) -> Unit, configure: QuestionnaireBuilder.() -> Unit = {}): View
```

这里 Button、Boolean Checkbox、Input 等沿用既有工厂；新增 builder 成员直接扩展原入口。Checkbox 的 Boolean／CheckState 重载仅做同一绑定的值映射，共用一个组件、一份 Press 和一条写入路径，不建立同步用的第二份 State。无参自有 Checkbox 继续使用既有 Boolean 默认入口，不增加另一条同样可无参调用的重载。

状态分工固定如下：

| 数据 | 归属与规则 |
| --- | --- |
| checked、selected、open、当前页、展开项、thumb 值 | 调用方的 Binding；事件读取／写入最新 Props，不缓存首次 binding |
| 正文、选区、composition、撤销历史 | 唯一的 TextEditorState；Input、Textarea、OTP、可编辑 Combobox 均显式接收 |
| hover、按下候选、活动候选项、拖动起点、弹层 Presence | 控件作用域的瞬态状态；不冒充已选择的业务值 |
| Toast 队列、表格查询、问卷进度、消息阅读策略 | 有具体职责的普通模型；简单 Boolean 控件不为统一外观另外造 State class |
| 行数据、消息内容、上传／请求／校验结果 | 应用模型；控件只消费结果和明确动作 |

简单自有状态入口只在有真实便利用途时提供，以 `initial...` 初始化一次；受控和自有模式用不同完整描述，不能同时出现 `value`／`defaultValue`／内部镜像值。构造不读写 Binding，不运行动作、不创建字体或 painter。State／模型均限 owner 线程；异步任务沿现有 executor 和窗口投递合同交付。

外部更新是正常状态变化，不回放用户通知。用户事件先计算候选、写 Binding，再按实际采纳的值通知；setter 未采纳时不伪造已改变外观。拖动的 `onValueCommit` 仅在正常完成时交付；取消有明确取消结果。关闭请求沿 Portal 的 `onCloseRequest`，最终关闭沿 Accepted 移除后的 `onClosed`，不把动画开始当作最终关闭。

预期的组合用法如下。这里的 FieldTarget、ButtonVariant、Dialog 及相关 builder 成员是本设计新增 API；应用启动与 ThemeSource 绑定仍沿现有入口：

```scoop
import scoop.ui.*
import scoop.ui.component.*
import scoop.ui.text.TextEditorState

class Profile: Component {
    private val open = state(false)
    private val name = TextEditorState()
    private val nameTarget = FieldTarget()

    public override fun render(): View {
        val body = column {
            this.fieldLabel("Name", this @Profile.nameTarget)
            this.input(this @Profile.name) {
                this.labelTarget = Some(this @Profile.nameTarget)
                this.placeholder = "Display name"
            }
            this.spacing = 8.0
        }
        return column {
            this.button("Edit profile", { this @Profile.open.value = true }) {
                this.variant = ButtonVariant.Outline
            }
            this.dialog(this @Profile.open.binding(), body) {
                this.title("Edit profile")
                this.description("Update the name shown in your profile.")
            }
        }
    }
}
```

编辑正文在 Profile 中持有，关闭／再次打开 Dialog 不丢失草稿；Dialog 按自身内容保留策略决定挂载生命周期。两个调用方共享业务 State 时也遵循相同规则，不靠 UI 构造器捕获第一次的值。

## 2. Theme 与样式

新增主题入口与三套现有主题一致：

```scoop
shadcnTheme(mode: ThemeMode = ThemeMode.Light, configure: ShapeThemeBuilder.() -> Unit = {}): ShapeTheme
```

应用仍持有单一 `ThemeSource`，在实际 run／mount 入口绑定；切换只更新 `source.current.value`。控件保存来源并在 render 读取本轮主题，局部覆盖使用现有 StyleOverride／StyleValue；不在构造闭包中冻结解析后的颜色。不同窗口／子组合通过显式 theme 来源接入，不把短暂 Context 当作子树继承。

### 2.1 语义颜色的直接演进

现有 ThemePalette 不足以表达 shadcn 的 primary／secondary／accent、card／popover、input、sidebar 与 chart。它应演进为以下完整的类型化数据；四套主题及现有消费者同步迁移，不保留旧字段别名。

| 新语义组 | 对应 shadcn token | 现有调用迁移 |
| --- | --- | --- |
| background: ColorPair | background／foreground | canvas／text |
| card: ColorPair、popover: ColorPair | card／card-foreground、popover／popover-foreground | surface／elevated 配合各自前景色 |
| primary: ColorPair | primary／primary-foreground | 现有 accent／onAccent 表达的是强调操作，迁到 primary |
| secondary: ColorPair、muted: ColorPair | 对应两组背景／前景色 | secondaryText 迁到 muted.foreground；弱操作和弱背景分开 |
| accent: ColorPair | accent／accent-foreground | 新含义是候选／悬浮表面，不能误用旧的品牌强调色 |
| destructive: ColorPair | destructive 及控件所需配对前景色 | error 迁到 destructive 的对应颜色 |
| border、input、ring | border、input、ring | focus 迁到 ring；普通边框与输入边框不合并 |
| chart: ChartPalette | chart-1～chart-5 | 五个完整颜色槽，按系列顺序映射，显式系列颜色可覆盖 |
| sidebar: SidebarPalette | sidebar 的背景／前景、primary、accent、border、ring | 独立完整数据，不以普通 card 近似全部侧栏状态 |
| disabledText、shadow | 本地绘图需要的语义颜色 | 保留实际已有用途 |

ColorPair 是普通不可变的 background／foreground 值对；SidebarPalette、ChartPalette 也是完整值，不是字符串字典。新增字段由各主题显式给出完整结果，不以 None 加运行时猜测补齐。旧主题只做公共合同迁移和必要回归，不在本轮重做三套主题的全部美术设计。

共同的部件尺寸、文字角色和动画参数按真实使用扩充 ThemeTypography、部件 Options 与相应 metrics；不向 Theme 增加 64 个 `draw<Component>` 方法。Button／菜单项／输入等把变体映射为公共 Style、文字和尺寸覆盖，再使用同一 geometry／painter。Slider、Switch、Chart 等专用布局由自己的 Element 计算，通用表面、标记和轮廓仍复用现有公共能力。

### 2.2 视觉基线与覆盖顺序

一 CSS px 对应一 Scoop 逻辑单位；rem 以基线 16 单位换算，DPI 只在既有目标接入处作用一次。颜色按上游 OKLCH／alpha 转为明确的 graphics 颜色值，保留来源 token，不能凭近似截图手选一组灰色。

| 项目 | Base Nova 默认基线 |
| --- | --- |
| Button 高度 | Xs 24、Sm 28、Default 32、Lg 36；Icon 四尺寸对应同边长正方形 |
| 常规正文／控件文字 | 正文通常 14／20，操作文字 medium；特殊层级从实际 typography 定义取得 |
| 基准 radius | 10；sm／md／lg／xl 等按当前上游的倍率派生，不套旧版减 2／4 的公式 |
| 边框与焦点 | 常规边框 1；Base Nova 键盘焦点 ring 为 3，颜色透明度与 border 状态按来源保留 |
| 图标 | 默认 Lucide 风格及对应图标尺寸；方向性箭头按逻辑方向选取，其他图形不整体镜像 |
| Destructive | 当前 Nova Button 是带 tint 的破坏性表面，不能用旧版满底红按钮代替默认值 |

上述是预设值，不以固定像素覆盖调用方字体和实际父约束。字号／density 改变时，建议高度、padding、文字基线和命中区域共同重算。外框绘制与内容测量使用最终同一几何；阴影／滤镜不暗中扩大点击区。

样式解析顺序为：当前主题 → variant／size 与选择／展开／invalid 状态 → 明确的局部基本覆盖 → 当前适用的 InteractionStyles。disabled 时不叠加 hover／pressed；焦点指示最后绘制，disabled 不自动清除业务选择。视觉优先级在每个控件的 spec 中固定，不能在 painter 和 measure 各自解析一次不同结果。

图标以现有 PathView／Canvas 和静态路径数据接入，只引入本轮实际用到的图标并保存许可证。应用自定义图标继续是 View／借用 Path。没有引入 SVG 解析器或整套图标平台的必要。字体默认提供可替换的 sans／mono family fallback；视觉基准测试显式使用固定字体集合。

动画参数沿现有 Surface AnimationSpec，提供统一的完整／减少动态模式。减少动态时省略位移、pulse、shimmer，保留必要的状态变化；它是显式主题配置，本轮不声称已接入 OS 的 reduced-motion 设置。窗口状态与真实动画参与仍由既有 owner 管理。

## 3. 内容插槽、关联与触发器

Card／Dialog／Field／Message 等的 body 是必需 View，title、description、media、header、footer、actions 是有明确类型的可选插槽。标题可提供 String 便利成员，内部构建真实文本 View；不使用按字符串命名的 slot map。可选插槽为空是正常结构，不用空 Component 占位。

Dialog、菜单和折叠面板的激活触发器接收**非交互的内容 View**，自己创建一个 Press／焦点目标及需要的 LayoutAnchor。需要复用已存在的 Button 时，由该 Button 显式接收 anchor／FocusHandle，并在动作中写同一 open Binding；Popup 使用这个外部 anchor。不能克隆 View、递归找某个 Button 或合并其隐藏 Props。

Tooltip／HoverCard 的包装只建立无激活动作的区域、anchor 和 Hover，借现有 routeChildren／scope.activity 观察子内容的 hover／focus；可以直接包住已有 Button，Button 仍保有唯一 Press 和原动作。装饰包装不额外加入 Tab。纯图标需要键盘触发时，调用方明确给出可聚焦的内容；不能为了提示文字偷偷再包一层按钮。

Dialog、Collapsible 等 builder 的 `trigger(content) { ... }` 收集 TriggerSpec，由所属持久组件创建实际触发器；有程序化入口的根允许不提供 trigger。菜单行、TraceLine、BubbleContent、Item 等采用同样的原始内容加行为组合。链接由普通激活动作调用应用导航；库不自己打开 URL。

FieldTarget 是为 Label 关联新增的窄句柄：包含稳定的焦点目标关联和所属控件当前的标签激活动作，不暴露组件实例。输入控件的 `labelTarget` Props 接入它，`fieldLabel(text, target)` 使用同一目标。文本输入／Slider／Select 的标签请求聚焦；Checkbox／Radio／Switch 的标签执行与自身相同的一次选择动作并聚焦。未挂载、禁用、隐藏、域外和旧绑定均不得被标签激活；替换 Props 更新当前动作。它不成为通用“按名字调用控件”的接口。

ButtonGroup 和 InputGroup 不能通过检查不透明 View 来修改子组件样式：

- ButtonGroup 的 button／separator／text 等成员收集明确条目，render 按位置生成各条目的边角与接缝覆盖。外部自定义条目可以接收 `GroupPosition` 值构建内容；plain `add(View)` 保留调用方已经完成的外观。没有为每个子项偷偷加一份激活行为。
- InputGroup 拥有一个外框和一个实际编辑宿主；input／textarea 槽采用共享文本控件的无外框内容模式，addon 和独立按钮正常参与布局／路由。自定义输入必须显式提供自己的 FocusHandle／FieldTarget，不靠遍历后代查找编辑器。
- Item／Attachment 等“整行可打开，右侧又有按钮”的结构让行 Press 在 bubble/default 阶段处理；子动作消费激活，拖选／Drag 接管取消行点击。动作区不会因覆盖整行透明按钮而不可用。

## 4. 键盘、焦点与隐藏内容

公共焦点指示判定的数据来自同一焦点入口的 `FocusOrigin.Pointer／Keyboard／Programmatic` 来源：指针聚焦默认不显示 focus-visible ring，键盘导航和显式程序聚焦默认显示；文本输入按明确的输入控件策略显示。`FocusOptions.visibility` 使用 `FocusVisibility.Automatic／Always／Never` 显式覆盖。FocusHandle、roving 导航、延迟显露和 Portal 恢复保留同一来源语义；样式只消费 `focusVisible`，不保存自己的最后输入设备。仅指示变化不重建实际焦点或 IME 会话。

候选活动项、真实焦点、已提交选择三者分开：

| 控件 | 焦点／按键合同 |
| --- | --- |
| RadioGroup | 一个 roving 停靠点；方向键移动并选择；Space 选择，disabled 项跳过 |
| ToggleGroup | 一个 roving 停靠点；方向键只移动；Space／Enter 切换，单选允许取消 |
| Tabs | 默认 Auto，方向键移动并选择；Manual 只移动，Space／Enter 提交；复杂／有延迟面板可选 Manual |
| Accordion | header 焦点导航与展开分开；方向／Home／End 导航，Space／Enter 切换 |
| 菜单／Select | roving 真实焦点；typeahead 不创建输入模型；Enter／Space 执行动作或提交选择 |
| Combobox／Command | 焦点始终留在输入；ActiveItems 处理 Up／Down／Home／End，Enter 提交当前候选；composition 活跃时先交给 IME |
| Slider／Resizable／Carousel | 主轴方向键、Home／End 和规定的 Page 操作；垂直轴方向明确；不会消费文本子控件已有默认动作 |

快捷键统一使用 window 的 ShortcutChord，显示与匹配共用同一值，Primary 不写死为 Command 或 Control。Escape 交给最内层可处理者；文本 composition 活跃时先结束组合，本次按键不继续关闭整个 Dialog。禁止从原始 key event 合成输入文字。

折叠内容使用 `ContentRetention.Dispose／KeepMounted`：默认关闭完成后卸载；KeepMounted 保留同一个子树和状态，但不布局呈现、不接收输入、不参加 Tab。两种模式在开始关闭时都立即撤销内部激活、焦点／IME 和可见动画资格。再次打开按正常 Accepted 几何恢复。保留挂载不等于让不可见 Spinner 一直占用帧。

## 5. 弹层合同

把 Portal 的焦点会话与指针屏障分开，采用完整的 `OverlayMode.NonModal／FocusContained／Modal`。三种模式共享同一打开／退出／移除流程；既有 ModalOptions 的初始／恢复焦点迁入独立的 OverlayFocusOptions，旧入口同步迁移。

| 模式 | 焦点与背景输入 |
| --- | --- |
| NonModal | 无全窗屏障；可明确初始聚焦和关闭恢复；Tab 按普通作用域离开 |
| FocusContained | 同窗口焦点被约束在层内并循环 Tab；背景 pointer／wheel 仍可执行，背景默认聚焦不突破当前约束 |
| Modal | 焦点约束加既有 InputBoundary；背景不能接收本窗口输入 |

非模态层也使用现有 focusRevision 的保存／恢复条件；新的显式焦点意图优先，不能在 Escape 或外部点击后抢回刚刚取得的新焦点。FocusContained 只是焦点约束，不伪造一个会吞掉 pointer 的模态边界。嵌套关闭按从内到外收尾；窗口失活不强行激活 OS 窗口。

| 家族 | 默认关闭／焦点策略 |
| --- | --- |
| Tooltip | 不取焦点；hover／focus 丢失或 Escape 关闭，不把提示内容加入 Tab |
| HoverCard | 非模态；hover corridor 保持；点击进入内容后的焦点按普通非模态规则处理 |
| Popover | 非模态；可聚焦内容，Escape／外部按下请求关闭；外部继续处理时沿 Surface 已冻结目标，不重新命中 |
| Dialog／Sheet／Drawer | 默认 Modal；Escape／外部按下请求关闭，外部按下消费；有明确的 onCloseRequest 业务 veto |
| AlertDialog | 默认 Modal；外部按下忽略；Escape 等价取消；默认初始焦点落在取消项，确认动作不强制立即关闭 |
| Menu／Select | 采用明确的模态／非模态配置；默认键盘进入候选列表，关闭恢复 trigger；动作选中与退出逻辑分开 |

Drawer 复用同一个 Portal，增加实际布局得到的拖动位置与 snap。Sheet 的侧边位移不等于 Drawer：后者有拖动阈值、释放速度、吸附、滚动接管与取消回弹。snap 使用明确的 `Fraction`／`Length` 值，不用一个 Double 同时猜比例和像素，更不解析 CSS 长度字符串。

Select 的选中项对齐采用公共内容锚点：Popup 根布局在测量中提交选中行的局部矩形，Portal 按该矩形与外部 LayoutAnchor 的目标位置求平移，之后统一执行视口约束／碰撞。普通 popup 模式仍沿 Anchored。选中项可能虚拟化时只按需实现该项及可见范围，准备完成后再开放输入；没有选中项或该项不可用时使用普通侧边定位。两条定位方式共用最终 Popup 几何，不在 Select 里保存第二份窗口坐标或访问 UI internal 节点。

关闭中复用 Presence：立即取消内部输入，保留绘图资源，Modal／FocusContained 的相应约束保留到成功移除；中途重开从当前进度返回。首次焦点、锚点、碰撞、拖动命中和最终恢复都不能先于成功场景。Backdrop 只需颜色／透明度；不把尚未交付的 backdrop blur 设为前置。

## 6. OTP 与消息列表所需的局部适配

OTP 的专用槽布局属于 component，但编辑命中、光标／选择、视觉导航与原生 caret 快照必须消费同一份排版结果。拟在 TextEditHost 的排版接入处允许实际布局提供上述几何操作：默认适配现有 TextLayout，OTP 适配固定槽。几何以同一 TextDisplay、版本、约束和方向构造，提供完整的 caret、selection、hit-test 与导航结果；保留到 Accepted 后再替换输入使用结果。正文事务、过滤、撤销、IME／clipboard 继续由原宿主执行。

这只服务已有段落和 OTP 两个实际布局，不公开 renderer 工厂、编辑插件链或第二个文本事件入口。槽间隙映射到最近合法插入点，空槽映射到当前正文末端，不能出现不存在的 UTF-8 位置；组成分隔符不进入正文或复制文本。隐藏一个真正输入框再画一排无法对应 caret 的假输入框，不满足交付条件。

VirtualList 增加沿逻辑主轴的显式末端留白，并为按 key 对齐提供定位 margin；它们是普通长度参数，参与同一 content extent、滚动范围与最终定位。留白不是伪造的消息，不占数据 key、不计入可见条目。MessageScroller 用它实现回合顶部定位和 previous-item peek；具体跟随、保持与跳转状态仍留在 MessageScrollState，见 [MODELS.md](MODELS.md)。

Sidebar 的宽／窄宿主改变、Tabs 的 Dispose 模式等遵守既有挂载规则。同 key／同父级／同类型的更新保留状态；跨父级或不同组件类型不承诺保存私有 State。需要跨这些切换保留的正文、选择和问卷结果显式提升到外部模型，不借任务之机增加任意子树迁移协议。
