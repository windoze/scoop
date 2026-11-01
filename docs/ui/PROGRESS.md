# UI 实施记录

日期：2026-11-01。实施依据为 [UI 设计](DESIGN.md)、[框架规范](../specs/SCOOP-UI-SPEC.md)、[文本排版规范](../specs/SCOOP-GRAPHICS-TEXT-SPEC.md)及[基础控件规范](../specs/SCOOP-UI-COMPONENT-SPEC.md)。本期只实现和验证 macOS，Windows 后续接入；图文绕排、GPU／3D／媒体互操作和 SkSL 仍属于设计中列出的后续增强。

## 1. 批次状态

| 批次 | 状态 | 交付范围 |
| --- | --- | --- |
| 0. 已确认设计与平台范围 | 已记录 | 自绘分层、State／Context／Binding、资源所有权、静止停帧、macOS 首期与后续增强 |
| 1. 文本公共能力 | 开始实施 | 先独立 graphics.text 与公开 Canvas 借用，再实现 StyledText／SpanStyle、行内 metrics 与布局结果 |
| 2. 独立自绘闭环 | 待实施 | SurfaceContent 所有权、Component／Element／State／Props／Binding、挂载、布局、输入及 UiHost |
| 3. 公共文本适配与简单控件 | 待实施 | ParagraphHost／InlineView、Label／Button／Checkbox |
| 4. 布局与组合 | 待实施 | Row／Column／Stack／Frame、keyed 子项、基线与资源组合 |
| 5. 宿主与生命周期组合 | 待实施 | 空闲停帧、动画收尾、隐藏更新、asyncio／HTTP 进度、取消和多窗口 |

已有 scoop.surface 的窗口、动画和 InputArea 实现作为基础；当前没有 UI 实现或上述组合的运行验收。源码与结果随各批完成更新本记录，不能把设计示例和文档检查当成已交付功能。

## 2. 验证与提交方式

普通 API、资源和控件行为使用所属 Cone 的 scoop.test；真实 GPU／窗口组合使用现有 macOS 后端。默认 release，定向验证改变的能力和必要消费者，不执行全量测试。遇到编译器、runtime 或其他系统库缺口时先修订其规范并修复该层，再补相应定向回归，不在 UI 中实现规避分支。

每项完整变更通过格式化、检查和定向测试后提交并推送，再 fetch／merge origin/main；合并改变相关代码时补充必要验证后继续。每轮清理不再使用的临时日志、探测文件和构建输出，仍需复用的正常构建缓存及本期目标资源继续保留。记录只保存验证范围、结果和实际限制。
