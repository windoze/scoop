# shadcn 64 项控件覆盖表

日期：2026-11-04。状态：分批实施，实际完成项与验证范围见 [实施记录](PROGRESS.md)。范围与架构见 [DESIGN.md](DESIGN.md)，公共使用合同见 [API.md](API.md)，复杂模型见 [MODELS.md](MODELS.md)。

逐行对应当日官方 Components 目录，以下 64 项均为交付要求。Code Block、Native Select 是仅有的目录排除项。官方示例中的独立业务服务和第三方引擎不自动扩大为库依赖；对应 UI 和调用方接入必须实际可用。表中的组成部件是需要可组合的结构，不要求为无行为的每一层 Web `div` 另造一个持久 Component。

## 1. 基本内容与操作：16 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Button](https://ui.shadcn.com/docs/components/button) | 演进 `button`；文字、StyledText、任意 View 内容 | Default／Outline／Secondary／Ghost／Destructive／Link；Xs／Sm／Default／Lg 及四种 Icon 尺寸；图标／Spinner、invalid、disabled、键盘焦点与最新回调；Link 是外观，跳转由动作负责 | P1 |
| [Button Group](https://ui.shadcn.com/docs/components/button-group) | `buttonGroup`、text／separator／item | 横／纵、不同尺寸、嵌套、split action；Button／Input／Select／菜单组合共享边框，焦点环在接缝可见；本身不附带 ToggleGroup 选择语义 | P1 |
| [Badge](https://ui.shadcn.com/docs/components/badge) | `badge(content)`；图标和动作内容 | 六种 Button 同名色调变体、图标／Spinner、可点击／链接式组合；纯展示不加入 Tab，显式动作只执行一次 | P1 |
| [Card](https://ui.shadcn.com/docs/components/card) | `card(content)`；header／title／description／action／footer | 内容、头部动作和脚部可单独组合；长文、窄宽、分隔线与内边距；空的可选区域不占位 | P1 |
| [Separator](https://ui.shadcn.com/docs/components/separator) | `separator` | 横／纵、明确逻辑长度与细线；装饰不参与输入，DPI 下边框清晰 | P1 |
| [Aspect Ratio](https://ui.shadcn.com/docs/components/aspect-ratio) | `aspectRatio(content, ratio)` | 有限正比例、父约束冲突、空尺寸、图片 Cover／Contain；测量与裁剪一致，不预乘 DPI | P1 |
| [Typography](https://ui.shadcn.com/docs/components/typography) | `typography` 及引用／列表／行内 code 组合；复用 Label／SelectableStyledText | H1～H4、P、Blockquote、Table、List、InlineCode、Lead／Large／Small／Muted；真实字体排版、选择和行内内容；无需 Markdown 解析器，排除 Code Block 不排除 InlineCode | P1 |
| [Label](https://ui.shadcn.com/docs/components/label) | `fieldLabel(text, target)`；现有 `label` 保持纯文本用途 | 点击文字聚焦关联输入，勾选／开关／单选关联执行同一控件动作；disabled／卸载／模态时不越过目标资格；不把已有纯文本 Label 暗改成按钮 | P1 |
| [Kbd](https://ui.shadcn.com/docs/components/kbd) | `kbd(chord)`、`kbdGroup`；文字提示内容 | 直接消费 window 的 ShortcutChord／显示标签，与实际快捷键共用一个描述；不是快捷键注册器 | P1 |
| [Avatar](https://ui.shadcn.com/docs/components/avatar) | `avatar`、fallback／badge／group／groupCount | Sm／Default／Lg（24／32／40）、图片与 initials、loading／failed 回退、堆叠／计数；图片由外部装载，替换不关闭借用资源 | P1 |
| [Skeleton](https://ui.shadcn.com/docs/components/skeleton) | `skeleton` | 矩形／圆形和内容尺寸占位，脉冲或静态模式；主题切换，离屏及停止后的停帧 | P1 |
| [Spinner](https://ui.shadcn.com/docs/components/spinner) | `spinner` | 独立、按钮、Badge、进度中的旋转图标；尺寸／颜色继承，有限 owner 动画，停止后无唤醒 | P1 |
| [Progress](https://ui.shadcn.com/docs/components/progress) | `progress(value)`；label／value／track | 确定值、未知进度、标签／数值插槽、RTL；范围合法性明确，外部更新无需内置任务或轮询 | P1 |
| [Alert](https://ui.shadcn.com/docs/components/alert) | `alert(content)`；icon／title／description／action | Default／Destructive；长描述、多行动作、图标基线；静态提示不自动弹窗或关闭 | P1 |
| [Empty](https://ui.shadcn.com/docs/components/empty) | `emptyState(content)`；media／header／title／description | 边框／背景、图标／Avatar、输入组和操作区；保留 UI 的 `empty()` 作为零内容，不混淆两者 | P1 |
| [Item](https://ui.shadcn.com/docs/components/item) | `item`、group／separator；media／content／actions／header／footer | 默认／边框／muted、尺寸、图标／图片／Avatar；整行操作与独立动作互不重复；窄宽布局和可选择正文 | P1 |

## 2. 表单：11 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Checkbox](https://ui.shadcn.com/docs/components/checkbox) | 演进 `checkbox`；Boolean 与 CheckState 两种绑定入口共享实现 | Unchecked／Checked／Indeterminate；Mixed 激活变 Checked，组／表头全选与 invalid；没有另一套三态 Press | P2 |
| [Radio Group](https://ui.shadcn.com/docs/components/radio-group) | `radioGroup`、item／indicator | 单选、空选、disabled 项、说明和 choice card；一个 roving Tab 停靠点，方向键移动并选择，Space 选择 | P2 |
| [Switch](https://ui.shadcn.com/docs/components/switch) | `switchControl`；track／thumb | Boolean 绑定、尺寸、disabled／invalid、标签关联；Space 切换，颜色和位移动画来自同一值 | P2 |
| [Toggle](https://ui.shadcn.com/docs/components/toggle) | `toggle(content, pressed)` | Default／Outline、尺寸、图标／文字、disabled；pressed 与短暂按下外观分开 | P2 |
| [Toggle Group](https://ui.shadcn.com/docs/components/toggle-group) | `toggleGroup`；单／多选绑定与 item | 单选可清空、多选、横／纵、间距／连边；方向键只移动焦点，Space／Enter 改选择；不要套用 RadioGroup 的选择策略 | P2 |
| [Slider](https://ui.shadcn.com/docs/components/slider) | `slider(values, range)`；track／range／thumb | 单值、双端范围、多 thumb、横／纵、step／minimumGap、disabled／invalid；方向／Home／End／Page 键、轨道点击、拖动与提交／取消；thumb 不交换身份 | P2 |
| [Input](https://ui.shadcn.com/docs/components/input) | 演进 `input(TextEditorState)` | placeholder、readonly、disabled、invalid、密码显隐和输入策略；真实 IME／撤销／复制；文件输入示例接应用选择动作，不嵌入 Native Select 或文件对话框 | P2 |
| [Textarea](https://ui.shadcn.com/docs/components/textarea) | 演进 `textarea(TextEditorState)` | 多行、行数、wrap、滚动、readonly／invalid、Tab／提交键和 group 组合；复用既有编辑宿主 | P2 |
| [Input Group](https://ui.shadcn.com/docs/components/input-group) | `inputGroup`；input／textarea 与 addon／text／button | InlineStart／InlineEnd／BlockStart／BlockEnd；图标、Kbd、菜单、Spinner、自定义输入；一份正文、一个 IME 连接和统一外框，不创建嵌套编辑器 | P2 |
| [Input OTP](https://ui.shadcn.com/docs/components/input-otp) | `inputOtp(state, length)`；group／slot／separator | 固定长度、数字／字母数字／自定义策略、分组、粘贴、删除、选择、readonly／disabled／invalid；一份 TextEditorState；空槽／间隙命中和 IME caret 与可见槽一致 | P2 |
| [Field](https://ui.shadcn.com/docs/components/field) | `field(control)`、fieldGroup／fieldSet；legend／label／title／description／error | 横／纵／按实际宽度布局；各种输入和 choice card；required 说明、错误数组、标签关联；校验结果由业务提供，不另造通用 Form engine | P2 |

## 3. 展开与导航布局：11 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Direction](https://ui.shadcn.com/docs/components/direction) | 复用 `scoop.ui.direction` 与 ChildrenBuilder.direction | LTR／RTL 子树继承与热切换；逻辑边、箭头导航、菜单定位、文本方向默认值一致，不镜像图像／正文或反转数据 | P3 |
| [Scroll Area](https://ui.shadcn.com/docs/components/scroll-area) | 演进 `scrollArea`，复用 ScrollState／Scrollbar | 单／双轴、横向、主题滚动条、嵌套残量、RTL、键盘／显露；不把普通 ScrollArea 冒充虚拟化 | P3 |
| [Collapsible](https://ui.shadcn.com/docs/components/collapsible) | `collapsible(open, content)`；trigger／content | 受控展开、disabled、默认卸载或显式 KeepMounted；退出期间立即停用内容输入，结束后停帧 | P3 |
| [Accordion](https://ui.shadcn.com/docs/components/accordion) | `accordion`；item／heading／trigger／content | Single／Multiple、Single 是否允许全关、disabled、边框／Card；方向／Home／End 导航不自行切换面板，Space／Enter 切换 | P3 |
| [Tabs](https://ui.shadcn.com/docs/components/tabs) | `tabs(selected)`；list／trigger／content | Default／Line、横／纵、图标、disabled；Auto／Manual 激活；明确内容卸载／保留；切换到 disabled／缺失项不暗写应用值 | P3 |
| [Breadcrumb](https://ui.shadcn.com/docs/components/breadcrumb) | `breadcrumb`；item／separator／ellipsis／current | 分层路径、长项省略、Dropdown 组合、RTL；当前项无跳转，导航回调由应用处理 | P3 |
| [Pagination](https://ui.shadcn.com/docs/components/pagination) | `pagination(page, pageCount)`；previous／next／item／ellipsis | 首尾边界、空数据、页码范围和省略号；用户改变页码只写绑定，控件不取数据；数值以 Long 表达 | P3 |
| [Resizable](https://ui.shadcn.com/docs/components/resizable) | `resizable(sizes)`；panel／handle | 横／纵、嵌套、min／max、可折叠面板与恢复；拖动／键盘／取消保持总尺寸，视口变化重分配，不要求重建内容 | P6 |
| [Carousel](https://ui.shadcn.com/docs/components/carousel) | `carousel(source)`；viewport／item／previous／next | 横／纵、多个可见项、间距、真实测量 snap、循环模式、受控索引／通知；拖动与子项点击仲裁；自动播放为显式 owner deadline 配置 | P6 |
| [Sidebar](https://ui.shadcn.com/docs/components/sidebar) | `sidebar(open, content)`；header／footer／group／menu／action／badge／rail | Sidebar／Floating／Inset、Start／End、Offcanvas／Icon／None 折叠；宽布局与窄布局 Drawer、快捷键、submenu／skeleton；业务状态由外部保留，跨宿主切换遵守正常挂载规则 | P6 |
| [Navigation Menu](https://ui.shadcn.com/docs/components/navigation-menu) | `navigationMenu`；list／item／trigger／link／content／viewport | 一级链接与内容面板、横／纵、延迟打开／跨间隙移动、键盘／Escape、方向热切换；链接与命令菜单语义分开 | P5 |

## 4. 弹层与通知：11 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Tooltip](https://ui.shadcn.com/docs/components/tooltip) | `tooltip(content)`；trigger／popup／arrow | hover 和键盘焦点打开、延迟、Escape／失焦关闭、碰撞翻转；提示不夺焦点、不放交互内容 | P4 |
| [Hover Card](https://ui.shadcn.com/docs/components/hover-card) | `hoverCard(content)`；trigger／popup／arrow | 延迟、悬浮走廊、键盘触发、可交互内容与关闭；内容进入不误关，锚点退出视口按既有策略处理 | P4 |
| [Popover](https://ui.shadcn.com/docs/components/popover) | `popover(open, content)`；trigger／anchor／close | 锚定、对齐／gap／箭头、碰撞、宽度跟随、外部按下和关闭恢复；默认非模态，嵌套输入／日期控件可用 | P4 |
| [Dialog](https://ui.shadcn.com/docs/components/dialog) | `dialog(open, content)`；trigger／header／title／description／footer／close | 模态屏障、初始／循环／恢复焦点、滚动内容／固定 footer、可替换／隐藏关闭按钮；退出重开与失败帧无穿透 | P4 |
| [Alert Dialog](https://ui.shadcn.com/docs/components/alert-dialog) | `alertDialog(open, content)`；action／cancel／media | 响应式尺寸、媒体、破坏性确认；默认聚焦取消，外部点击不关闭，Escape 走取消；提交中保持打开由应用决定 | P4 |
| [Sheet](https://ui.shadcn.com/docs/components/sheet) | `sheet(open, content)`；header／footer／close | Top／Bottom／Start／End 侧边面板、不同宽高、滚动、模态与退出；RTL 下逻辑侧正确，区别于有拖动／snap 的 Drawer | P4 |
| [Drawer](https://ui.shadcn.com/docs/components/drawer) | `drawer(open, content)`；swipeHandle／viewport／popup | 四方向拖动、垂直 snap points、嵌套、受控 snap、Modal／NonModal／FocusContained；滚动内容与拖动接管、取消回弹、窗口缩放重新定位 | P6 |
| [Dropdown Menu](https://ui.shadcn.com/docs/components/dropdown-menu) | `dropdownMenu`；trigger／item／checkboxItem／radioGroup／sub／label／separator／shortcut | 动作、disabled、destructive、勾选／单选、子菜单；typeahead、roving 焦点、关闭策略、层间键盘与悬浮通路 | P5 |
| [Context Menu](https://ui.shadcn.com/docs/components/context-menu) | `contextMenu(content, menu)` | 共享菜单模型，以右键坐标或键盘菜单键／Shift+F10 打开；无可用几何不猜坐标；右键不触发内容主动作 | P5 |
| [Menubar](https://ui.shadcn.com/docs/components/menubar) | `menubar`；menu／trigger／content | 多个共享菜单根、横向切换、纵向菜单、子菜单、checkbox／radio／快捷键；一次 Escape 关闭正确层，随后焦点回到正确入口 | P5 |
| [Toast](https://ui.shadcn.com/docs/components/toast) | `toaster(state)`；ToastState.show／update／dismiss；title／description／action | Default／Success／Info／Warning／Error／Loading、六位置、堆叠／排队、动作／滑动关闭、定时暂停及异步状态更新；每窗口显式宿主，无全局总线 | P6 |

## 5. 选择、日期与数据：8 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Select](https://ui.shadcn.com/docs/components/select) | `select(value, choices)`；trigger／value／content／group／item／indicator／scrollButton | 自绘单选、placeholder、分组、disabled／invalid、长列表、typeahead；默认选中项对齐 trigger，可切普通 popup 定位；开关与选择各有独立状态 | P5 |
| [Combobox](https://ui.shadcn.com/docs/components/combobox) | `combobox(value, choices, query)`；input／trigger／clear／list／item／chips | 单／多选、过滤、自定义项、分组、autoHighlight、清空、chips、popup 内输入、InputGroup；查询与选择分开，焦点留输入，IME preedit 不触发筛选或选项提交 | P5 |
| [Command](https://ui.shadcn.com/docs/components/command) | `command(source, query)`；input／list／group／item／empty／shortcut；Dialog 组合 | 命令搜索、分组、disabled、active item、空结果与快捷键标签；支持调用方排序／过滤；不充当应用全局命令注册中心 | P5 |
| [Calendar](https://ui.shadcn.com/docs/components/calendar) | `calendar(selection)`；header／monthGrid／day／weekNumber | Single／Multiple／Range、单／多月、翻月／年与下拉、禁用日期、周起点／周号／locale、键盘日期导航、范围预览；默认 LocalDate 公历，其他历法使用明确的月页数据接入 | P7 |
| [Date Picker](https://ui.shadcn.com/docs/components/date-picker) | `datePicker`、`dateRangePicker`，复用 Popover／Calendar／Input | 日期／范围、生日、preset、可编辑日期、时间输入与自然语言输入组合；文本草稿、解析失败、提交／取消明确，外部解析器不能在 render 中运行 | P7 |
| [Table](https://ui.shadcn.com/docs/components/table) | `table`；caption／header／body／footer／row／head／cell | 同一份列轨道、对齐、横向溢出、footer／actions、RTL；数据表仅负责呈现，不隐式排序和虚拟化 | P7 |
| [Data Table](https://ui.shadcn.com/docs/components/data-table) | `dataTable(source, columns, state)`，复用 Table 布局／VirtualList | 本地及外部结果模式、稳定排序／筛选／分页／列可见性／行选择、行操作与空结果、共用列宽、虚拟正文；不用 Any 值包或通用插件框架 | P7 |
| [Chart](https://ui.shadcn.com/docs/components/chart) | `chart(model)`；container／axis／grid／series／tooltip／legend | Area、Bar、Line、Pie／Donut、Radar、Radial 六类；实际数据、轴／刻度／图例／tooltip、堆叠及规定的组合；明暗／缩放／空值处理，交付真实绘图而非图片 | P7 |

## 6. 会话与问卷：7 项

| shadcn 项目 | Scoop 入口与组成 | 必须交付及代表验收 | 批次 |
| --- | --- | --- | --- |
| [Attachment](https://ui.shadcn.com/docs/components/attachment) | `attachment`、group；media／content／title／description／actions／trigger | Default／Sm／Xs、横／纵、图标／图片；Idle／Uploading／Processing／Error／Done；整卡打开与独立取消／移除不双触发，group 横向滚动／吸附／边缘淡出；不执行上传 | P8 |
| [Bubble](https://ui.shadcn.com/docs/components/bubble) | `bubble(content)`、group／reactions | Default／Secondary／Muted／Tinted／Outline／Ghost／Destructive；Start／End，普通最大宽度 80%，Ghost 可占满；反应锚点、可折叠／可操作内容 | P8 |
| [Marker](https://ui.shadcn.com/docs/components/marker) | `marker(content)`；icon | 会话内状态行、Border 行、带文字 Separator；Spinner／shimmer 与显式动作；Marker 不是文本高亮编辑工具或地图标记 | P8 |
| [Message](https://ui.shadcn.com/docs/components/message) | `message(content)`、group；avatar／header／footer | 起终侧排列、连续发送者分组、底部 avatar 和操作 footer；正文组合 Bubble／Attachment／Trace／任意 View；不解析或拥有会话协议 | P8 |
| [Message Scroller](https://ui.shadcn.com/docs/components/message-scroller) | `messageScroller(source, state)`；viewport／item／controls | 初始 End／Start／LastAnchor、回合锚定、peek、可选自动跟随、prepend／异步增高不跳、按 key 跳转／恢复、已提交可见项与当前回合；用户选择／滚动等中断后不自动拉回 | P8 |
| [Questionnaire](https://ui.shadcn.com/docs/components/questionnaire) | `questionnaire(state, onSubmit)`；item／title／description／choice／input／error／progress／actions | 单选、多选、freeform、明确 Skip、required、disabled 条件题、快捷键、前后导航／完整提交校验、外部错误、恢复／reset；取消 Dialog 不等于 Skip | P8 |
| [Trace](https://ui.shadcn.com/docs/components/trace) | `trace`、line／icon／title／description；Collapsible 组合 | 完成／Running／Error／Pending、嵌套步骤、说明与图标、展开／链接／Dialog／Sheet 触发；运行态 shimmer 可停；纯 line 不加入 Tab，不额外实现 agent/tool 调度 | P8 |

## 7. 覆盖统计与上游更新

分组为 16 + 11 + 11 + 11 + 8 + 7 = **64**。Direction 复用框架能力，Label 对应 FieldLabel，Typography 和 DatePicker 保留组合性质；它们仍各有实际使用与验收，不按新增 class 数量计数。

上述表格是固定的本轮范围。实施时记录所取 registry 源码的版本／日期及必要许可证，确认颜色与 variant 值；若上游增加项目，单独评估，不默默扩大本轮。若发现当日已有的部件遗漏，补入所属行及验收，不以“原表没有列子部件”缩减承诺。全部 64 行闭合后才可称本轮控件集完成。
