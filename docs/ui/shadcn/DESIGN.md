# shadcn 全套自绘控件设计

日期：2026-11-04。状态：实施中，实际交付与验证范围见 [实施记录](PROGRESS.md)。承接已经完成 macOS 验收的 [UI 框架](../DESIGN.md)、[基础交互](../foundations/DESIGN.md)、[接收者 DSL](../DSL-MIGRATION.md)和[控件基础补齐](../component-foundations/DESIGN.md)。尚未在实施记录中闭合的类型、签名和批次仍是拟议合同，不表示仓库已经提供这些 API。

## 1. 交付目标与复刻基线

在现有 `scoop.ui.component` 中交付完整控件集，新增独立的 `scoop.ui.style.shadcn` 主题。控件沿用同一份状态、布局、输入和文本实现；shadcn 的颜色、尺寸、部件外观与动效由主题和类型化控件样式表达。已有 Button、Checkbox、Input／Textarea、ScrollArea 等直接演进，不建立第二套同名控件库。

范围以 2026-11-04 的 [shadcn Components](https://ui.shadcn.com/docs/components) 为准：66 项，明确排除 **Code Block、Native Select**，交付 **64 项**。自绘 Select、Combobox、Typography 的行内 code，以及目录中的 Attachment、Bubble、Marker、Message、Message Scroller、Questionnaire、Trace 均包含在内。完整映射与逐项完成条件见 [CATALOG.md](CATALOG.md)。

复刻包含默认外观、公开变体、可组合部件及核心交互。单个静态截图、只有占位外观的类、用通用容器替代复杂控件，都不算该项完成。Scoop 使用自身的 View、Binding、模型与接收者 DSL；React props、DOM 属性、CSS selector、`render`／`asChild`、浏览器表单与 JS 包管理不逐字移植。

参考风格选 **Base / Nova + Neutral，Light／Dark 两种模式**。以官方 [base-nova registry](https://ui.shadcn.com/r/styles/base-nova/button.json) 的实现和[主题 token](https://ui.shadcn.com/docs/theming)为外观基线；部分新增组件文档展示 base-rhea，使用对应 base-nova 源码统一视觉。控件目录完整性与皮肤数量分开：本轮实现这一套可定制皮肤，不把所有 shadcn preset、Blocks 和独立 AI Elements 示例另算一批控件。当前 Drawer 已改用 Base UI，Toast 也是当前 Base 版本；不按旧 Vaul／Sonner 教程推定现行行为。

沿用上一轮已确定的交付边界：

- 键盘、焦点、IME、选择、复制、禁用及正常交互属于本轮。accessibility 语义树和 OS 辅助技术桥接另期实施，不能宣称已经达到上游的屏幕阅读器支持程度。
- 上层全部为平台无关 Scoop 源码。先在 macOS aarch64 验证，收尾补 x86_64 Rosetta；Windows／Linux 由各自基础后端满足公共合同后验收。当前 window 没有已交付的 Touch 接口，Drawer／Carousel／Toast 的拖动先通过现有指针输入完成，不把桌面拖动测试记为触屏验收。
- 数据获取、路由跳转、上传／下载、Markdown 解析、自然语言日期解析和持久化由应用或相应库提供。控件给出具体组合入口与示例；不因某个 Web 示例使用了它们而引入网络框架、表单引擎或 AI 会话运行时。
- Calendar、Chart、Data Table 的范围按本设计明确交付；不以“已提供扩展槽”代替其中规定的算法、交互和演示，也不承诺复制其全部第三方依赖 API。

## 2. 文档与职责

| 文档 | 内容 |
| --- | --- |
| [CATALOG.md](CATALOG.md) | 64 项对应关系、组成部件、变体、关键行为和所属批次 |
| [API.md](API.md) | DSL、状态归属、主题、组合与触发器、弹层／焦点及必要的局部扩展 |
| [MODELS.md](MODELS.md) | 选择、编辑、日期、表格、图表、Toast、消息与问卷的具体模型和转换规则 |

| 层 | 本轮职责 |
| --- | --- |
| `scoop.window`／`scoop.window.asyncio` | 继续提供窗口、平台键位／标签、文本服务和宿主；本轮不增加控件 native 实现 |
| `scoop.graphics`／`scoop.graphics.text`／`scoop.surface` | 复用绘图、字体／Unicode、几何、成功帧输入、指针序列、deadline 和动画 |
| `scoop.ui` | 复用 Component、Props／Binding、Focus、Press／Drag、ActiveItems、Hover、Presence、Portal、TextEditHost；按实际消费者补少量公共合同 |
| `scoop.ui.style` | 完整语义 token、通用 Style／StyleOverride、公共部件几何与绘图；不按具体控件名建立注册表 |
| `scoop.ui.style.shadcn` | `shadcnTheme`，完整 Light／Dark 参数和对应公共部件外观；不依赖 component |
| `scoop.ui.component` | 64 项控件、特定控件模型／布局／状态转换，以及真实成员 DSL；只用下层 public 接口 |
| 应用／示例 Cone | 业务 State、IO 与异步请求、会话数据、格式化／解析、页面组合和窗口宿主 |

依赖主线仍为 `component → style → ui → surface`，shadcn 主题依赖 style 和实际使用的绘图库；component 不依赖具体主题实现。每个 Cone 显式声明使用的直接依赖。四套主题均提供完整的公共 token，控件代码不判断主题类型或名称。

源码按实际功能拆为 `actions/`、`forms/`、`disclosure/`、`overlays/`、`menus/`、`selection/`、`navigation/`、`data_table/`、`chart/`、`conversation/` 等目录；公共工厂保留 `scoop.ui.component` 包。复杂模型和算法在所属目录内拆分，不把每个控件变成一个 Cone，也不先搭建通用控件生成器。

## 3. 已有能力与剩余工作

现有 [组件规范](../../specs/SCOOP-UI-COMPONENT-SPEC.md)已经覆盖 Flex／Wrap／Grid、共享固定列宽、方向与图像、RangeSpec／resizePair／snapTarget；[UI 规范](../../specs/SCOOP-UI-SPEC.md)已有活动项导航、悬浮走廊、受控退出、文本策略／投影、滚动意图及阅读恢复。它们是直接实施控件的基础，不再开一轮笼统的“补齐基础设施”。

以下差距必须随实际使用批次闭合，不能把现有近似能力当作完整支持：

| 差距 | 具体原因与落点 | 使用批次 |
| --- | --- | --- |
| 焦点可见性（P0b 已交付） | 同一 Surface 聚焦入口记录来源，FocusOptions 提供指示策略；InteractionState 保留实际 focused，样式消费 focusVisible，延迟显露与模态恢复保留来源 | P0／P1，UI 焦点与样式视觉输入 |
| 弹层焦点合同独立于屏障 | 现有 Portal 的初始／恢复焦点依附 ModalOptions；非模态 Popover 的恢复和 Drawer 的 FocusContained 模式需要同一焦点会话可独立使用 | P4，UI Portal／FocusScope |
| Select 内容锚点定位 | 现有 Anchored 只围绕外框定位；选中项对齐 trigger 需要内容测量产生的定位点及统一碰撞处理 | P5，UI 定位协议与 Select 布局 |
| OTP 固定槽几何 | 现有文本投影能映射正文，不能单凭插入空格保证任意字体下的等宽槽、间隙命中和 IME caret；补共享编辑宿主的实际几何适配 | P2，UI 文本宿主与 OTP Element |
| 回合起点与末尾留白 | 现有 FollowEnd／restore 不包含新回合顶部定位、上一项 peek 和可收缩尾部空间；补 VirtualList 的逻辑末端留白及定位 margin | P8，component 虚拟布局／消息模型 |

具体接口边界见 [API.md](API.md)。这些工作不增加另一套焦点状态、挂载树、编辑器、窗口循环或动画时钟。发现真实缺陷时在所属层修复并补对应回归；不在每个控件里绕过。

## 4. 实施顺序

每批同时交付对应 spec、实现、公共 DSL、示例及必要测试。目录项目数只统计首次完成的组件；后续组合仍可补充验收。较大批次可按表内家族拆分提交，不把全目录一次性合入。

| 批次 | 项数 | 工作与完成条件 | 前置 |
| --- | ---: | --- | --- |
| P0 主题与真实纵向样例 | 0 | 完整 token 与 shadcn 主题；现有 Button／Input／Checkbox 的普通、focus、invalid、disabled 和明暗展示；统一焦点可见性；建立 Gallery 的实际窗口入口 | 已有基础库 |
| P1 基本内容与操作 | 16 | Button 到 Item 的基础展示／操作家族，详见目录第一组；含内容插槽、FieldLabel 的真实关联、分组边框、Avatar 回退 | P0 |
| P2 表单控件 | 11 | 三态勾选、Radio／Switch／Toggle、Slider、文本输入、InputGroup、OTP 和 Field；完成“带校验的设置表单” | P1 |
| P3 折叠与基本导航 | 7 | Direction、ScrollArea、Collapsible、Accordion、Tabs、Breadcrumb、Pagination；键盘、保留内容和 RTL 组合 | P1 |
| P4 弹层与焦点 | 6 | Tooltip、HoverCard、Popover、Dialog、AlertDialog、Sheet；受控关闭、非模态恢复、焦点模式、退出重开；表单进入对话框 | P2、P3 |
| P5 菜单与选择 | 7 | DropdownMenu、ContextMenu、Menubar、Select、Combobox、Command、NavigationMenu；共享菜单行为、选中项对齐、输入法与候选导航 | P4 |
| P6 拖动布局与通知 | 5 | Drawer、Resizable、Carousel、Sidebar、Toast；真实拖动／取消／吸附，侧栏的窄宽布局和通知生命周期 | P4、P5；RangeSpec 等已存在 |
| P7 日期与数据展示 | 5 | Calendar／DatePicker；Table／DataTable；六种 Chart。三个子批分别交付，最终形成数据管理界面 | P2、P5、P6 |
| P8 会话与问卷 | 7 | Attachment、Bubble、Marker、Message、MessageScroller、Questionnaire、Trace；流式内容、阅读保持、回合定位、问卷校验／恢复 | P2～P7 中实际依赖 |
| P9 完整目录与组合验收 | 0 | 64 项逐项闭合；四个真实应用组合、明暗／RTL、系统 IME、资源收尾与停帧；补 x86_64 验证 | P1～P8 |
| **合计** | **64** | 排除项恰为 Code Block、Native Select | |

P2 的 OTP、P4 的弹层焦点、P5 的 Select／Combobox、P7 的图表以及 P8 的消息滚动是主要技术风险，应在各批开始时先做真实窗口纵向样例。样例成功后再扩齐相同家族的外观与部件，不能先画完一批静态控件再补交互。

## 5. 验收与完成标准

每项至少有独立用法、所有公开视觉变体及一项必要组合；交互控件验证正常路径、取消／禁用／卸载、外部状态更新，以及相关的键盘操作。完整结果在 `scoop.ui.component/examples/shadcn` Gallery 中可浏览、可操作、可切换主题／方向；图库本身使用这些控件，搜索、分类和实例状态是普通应用逻辑。

四个组合承担跨家族验收：

1. **设置表单**：Field、InputGroup、OTP、Checkbox／Radio／Switch、Slider、DatePicker，中文 IME、错误说明、键盘显露与提交。
2. **工作区**：Sidebar、Resizable、Tabs、Menubar、Command、嵌套菜单、Dialog／Sheet／Drawer，缩放窗口、关闭层级及焦点恢复。
3. **数据管理**：带筛选／排序／分页／选择的 DataTable、行菜单、删除确认、Chart 与 Toast，替换数据保留稳定 key。
4. **会话与问卷**：长消息、图片增高、Attachment、Trace 嵌套展开、MessageScroller 历史 prepend／新回合／阅读中断，以及 Questionnaire 分支／错误／恢复。

验证分工：

- 模型、范围、日期、排序、列宽、图表几何、问卷状态转换使用所属 Cone 的 `scoop.test`；基于输入与结果断言，不记录全树快照。
- 真实窗口交互复用现有 native 测试辅助，通过公开 API 驱动鼠标、键盘和真实文本服务；一般库行为不建 fixture。UI 行为测试不等同于 OS accessibility 验收。
- Raster 截图覆盖代表部件和 Gallery 页面。固定字体、窗口逻辑尺寸、主题与输入内容，核对边框／间距／圆角／token；字体边缘和跨平台光栅允许合理容差，不要求 Web 与 Skia 每个像素相等。真实窗口同时检查焦点、裁剪和弹层位置，截图不能代替交互验证。
- 每批先按修改语言格式化、lint／check，再运行受影响的定向测试；只在新改动或未解决问题需要时扩大范围。普通 UI 使用 release，无全量 GC stress 或每控件独立冷构建。
- Spinner、Skeleton、Toast、流式文字等只在确有可见动画时请求帧；停止、暂停、隐藏、离屏和卸载后验证逻辑更新、帧需求及 GPU 提交停止。大量表格行／消息只创建当前需要的 View，滚动和流式更新不能重建全量历史。
- 原始截图、日志和逐次报告在仓库外；实施记录只保留范围、结果概要、问题闭合与实际平台限制。

## 6. Spec 落点

设计中的待实施 API 不记为“已实现”。开始相应批次时，先更新下列合同，再同步源码和实际调用方；实际完成范围记录在 PROGRESS.md：

| 规范 | 需要演进的内容 |
| --- | --- |
| [SCOOP-UI-COMPONENT-SPEC](../../specs/SCOOP-UI-COMPONENT-SPEC.md) | 第 2 节入口与 builder；Button 变体、Checkbox 三态、Input／Textarea 组合；虚拟列表留白与定位；新增控件及具体模型章节 |
| [SCOOP-UI-THEME-SPEC](../../specs/SCOOP-UI-THEME-SPEC.md) | 第 3 节完整语义颜色／尺寸／文字／动画；新增 shadcn 主题；按实际需求扩展共享几何和视觉状态 |
| [SCOOP-UI-STYLE-SPEC](../../specs/SCOOP-UI-STYLE-SPEC.md) | token／主题引用及焦点视觉输入；继续复用 StyleOverride、同一几何与 painter |
| [SCOOP-UI-SPEC](../../specs/SCOOP-UI-SPEC.md) | 第 7.3 节焦点指示／约束，第 7.5 节文本几何适配，第 7.6 节独立焦点会话与内容锚点 |

语言、HIR／MIR／LIR、`.slib`、构建缓存与 runtime ABI 没有新增职责。外部图标／源码实际引入时记录对应来源与许可证；shadcn 源码采用 [MIT License](https://github.com/shadcn-ui/ui/blob/main/LICENSE.md)。不把来源记录变成编译或产物验证机制。
