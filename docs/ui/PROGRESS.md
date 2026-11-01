# UI 实施记录

日期：2026-11-01。实施依据为 [UI 设计](DESIGN.md)、[框架规范](../specs/SCOOP-UI-SPEC.md)、[文本排版规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)及[基础控件规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)。本期先在 macOS aarch64 完成全部实施批次，再统一补验 x86_64，Windows 后续接入；图文绕排、GPU／3D／媒体互操作和 SkSL 仍属于设计中列出的后续增强。

## 1. 批次状态

| 批次 | 状态 | 交付范围 |
| --- | --- | --- |
| 0. 已确认设计与平台范围 | 已记录 | 自绘分层、State／Context／Binding、资源所有权、静止停帧、macOS 首期与后续增强 |
| 1. 文本公共能力 | 基础拆分完成，富文本待实施 | 已独立 graphics.text 与公开 Canvas 借用；下一步实现 StyledText／SpanStyle、行内 metrics 与布局结果 |
| 2. 独立自绘闭环 | 待实施 | SurfaceContent 所有权、Component／Element／State／Props／Binding、挂载、布局、输入及 UiHost |
| 3. 公共文本适配与简单控件 | 待实施 | ParagraphHost／InlineView、Label／Button／Checkbox |
| 4. 布局与组合 | 待实施 | Row／Column／Stack／Frame、keyed 子项、基线与资源组合 |
| 5. 宿主与生命周期组合 | 待实施 | 空闲停帧、动画收尾、隐藏更新、asyncio／HTTP 进度、取消和多窗口 |

已有 scoop.surface 的窗口、动画和 InputArea 实现作为基础；当前尚未交付 UI 框架，已完成的底层能力及其实际验证记录如下。源码与结果随各批完成更新，不能把设计示例和文档检查当成已交付功能。

## 2. 验证与提交方式

普通 API、资源和控件行为使用所属 Cone 的 scoop.test；真实 GPU／窗口组合使用现有 macOS 后端。默认 release，定向验证改变的能力和必要消费者，不执行全量测试。遇到编译器、runtime 或其他系统库缺口时先修订其规范并修复该层，再补相应定向回归，不在 UI 中实现规避分支。

每批先完成 aarch64 验证并提交，不把 x86_64 补验作为继续实现下一批的前置。已提前准备的部分 x86_64 资源和缓存保留，尚未进行本轮运行验收；本期 aarch64 功能完成后再更新资源并统一补验。

每项完整变更通过格式化、检查和定向测试后提交并推送，再 fetch／merge origin/main；合并改变相关代码时补充必要验证后继续。每轮清理不再使用的临时日志、探测文件和构建输出，仍需复用的正常构建缓存及本期目标资源继续保留。记录只保存验证范围、结果和实际限制。

## 3. 已完成变更

### 3.1 独立文本库与画布借用

字体、TextStyle／TextRun、Paragraph／TextLayout 及其 native 实现、固定测试字体迁入 scoop.graphics.text。该 Cone 自己释放字体与段落资源，Canvas.drawText 通过 graphics 的公开 Unsafe `Canvas.withSkiaCanvas` 同步绘制；正常返回和异常退出都恢复原生坐标、clip 和保存层级。文本库不包含 graphics 的私有 native 头文件，graphics 不反向引用文本类型。FontCollection 新增只在成功登记后递增的 revision。

Surface 主体仍只接收 Rect caret；使用文本的示例和焦点测试已改为直接依赖 graphics.text。已有 DirectWrite 工厂随文件归属一起移动，没有新增 Windows 行为或进行 Windows 验收。StyledText、样式继承及行内对象仍属下一步。

本轮在 macOS aarch64 使用当前源码的工具链及显式准备的开发 sysroot，先完成 Scoop／C++ 格式检查、C++ 编译告警检查和两库测试源码的类型检查，再运行以下 release scoop.test：

| 范围 | 结果 | 必要覆盖 |
| --- | ---: | --- |
| scoop.graphics | 13／13 | 原绘图／Path／Metal，加原生借用、异常恢复、过期与异线程拒绝 |
| scoop.graphics.text | 10／10 | 固定字体、CJK／bidi／emoji、独立布局、字体 revision、资源关闭与真实 Metal 文本 |
| scoop.surface 的 InputFocusTest | 5／5 | 文本 caret 与区域变换、IME／焦点及关闭 |
| Surface basics 示例 | 4／4 | 文本编辑、布局缓存和拖动收尾 |

画布恢复测试初次对 SkPath 的恰好边界点作了半开矩形的假设；改为检查明确位于 clip 外的点后，几何和实际像素断言均通过。本轮未修改编译器、core／std／test，未新增 fixture 或运行全量测试。x86_64 留待本期功能完成后补验。
