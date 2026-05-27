# Spec Fix + Overload Resolution TODO 索引

> 生成时间：2026-05-27  
> 设计基线：[`SPEC_FIX.md`](./SPEC_FIX.md)、[`OVERLOAD_RESOLUTION.md`](./OVERLOAD_RESOLUTION.md)  
> 计划基线：[`PLAN.md`](./PLAN.md)  
> 格式参考：[`docs/archive/plans/TODO-pipeline-refactor.md`](./docs/archive/plans/TODO-pipeline-refactor.md)  
> 当前状态：任务已拆分为 5 个任务包；所有任务均为 `[TODO]`。每个实现任务后都紧跟一个独立 review 任务，编号为原任务 ID + `R`。

## 总原则

- `PLAN.md` 是当前执行计划基线；如果实现时发现阶段边界、语言决议或验收条件需要改变，必须先回写 `SPEC_FIX.md` / `OVERLOAD_RESOLUTION.md`，再调整 TODO。
- 所有任务按 `TODO-1.md` 到 `TODO-5.md` 顺序推进；除非对应文件明确允许，不跨包并行实现。
- 每个实现任务后必须紧跟一个独立 review 任务，复审该任务的完整变更、阶段目标和约束遵守情况。
- review 任务不是形式检查；如果发现前一任务没有真正完成目标，review 任务必须直接修正或阻塞下一任务。
- 任务完成后必须同时更新本索引和对应 `TODO-[1-5].md` 中的任务状态与完成记录；不得只更新其中一边。
- 不保留无明确需求的兼容层。旧 `perform`、handler `with`、tuple `._0`、旧 f-string `{...}` 插值、`@Inline`、`AnyRef` / `AnyValue` marker 等旧 surface 在对应任务完成后应成为前端错误或彻底删除。
- 所有用户可见 reject 必须发生在 parser/typecheck 侧；overload 相关错误不得泄露 `backend`、`LLVM`、`UnsupportedMainBody`、`codegen` 等内部术语。
- overload resolution 的 source of truth 必须是 typecheck 选出的唯一 callable binding / overload identity；HIR/MIR/codegen 不得再用 bare FQN 或同名函数 map 重新猜目标。
- `!!`、`as`、refutable `val` pattern、enum `with` variant mismatch 的断言失败路径统一走 `panic(...)`；不得通过 `Raise.raise(RuntimeError.*)` 间接表达。
- 默认 visibility 改为 `internal` 时，必须与 sysroot / fixture / `.cone` export 同步处理；不能先改默认值再留下大量隐式 public API 断裂。

## 任务包划分

| 包 | 文件 | 覆盖 PLAN 阶段 | 目标 | 当前细化状态 |
| --- | --- | --- | --- | --- |
| 1 | [`TODO-1.md`](./TODO-1.md) | P0-P1 | 冻结迁移清单、overload bug 基线；落纯 spec 与 `@Inline` 删除 | 已细化 |
| 2 | [`TODO-2.md`](./TODO-2.md) | P2 | 收敛 parser / AST 语法 surface | 已细化 |
| 3 | [`TODO-3.md`](./TODO-3.md) | P3 | 落地 SPEC_FIX type/effect/lowering/sysroot/cone 语义 | 已细化 |
| 4 | [`TODO-4.md`](./TODO-4.md) | P4 | 落地 overload definition-time 规则 | 已细化 |
| 5 | [`TODO-5.md`](./TODO-5.md) | P5-P6 | 落地 call-site resolution、callable identity 贯通与最终收尾 | 已细化 |

## 具体任务索引

| 任务 | 状态 | 文件 | 目标 |
| --- | --- | --- | --- |
| P0-T01 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01建立旧-surface--sysroot--fixture-迁移清单) | [DONE] 建立旧 surface / sysroot / fixture 迁移清单 |
| P0-T01R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01rreview-旧-surface--sysroot--fixture-迁移清单) | [DONE] Review P0-T01 迁移清单质量 |
| P0-T02 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02建立-overload-bug-与-diagnostics-基线样例) | [DONE] 建立 overload bug 与 diagnostics 基线样例 |
| P0-T02R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02rreview-overload-bug-与-diagnostics-基线) | [DONE] Review P0-T02 overload 基线质量 |
| P1-T01 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t01更新纯-spec-决议nothingconepackagevalue-type-with) | [DONE] 更新纯 spec 决议：`Nothing`、cone/package、value type `with` |
| P1-T01R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t01rreview-纯-spec-决议更新) | [DONE] Review P1-T01 spec 更新质量 |
| P1-T02 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t02删除-inline-annotation-surface) | [DONE] 删除 `@Inline` annotation surface |
| P1-T02R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t02rreview-inline-删除结果) | [DONE] Review P1-T02 `@Inline` 删除结果 |
| P2-T01 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t01删除-perform-prefix并迁移-effect-op-调用语法) | [DONE] 删除 `perform` prefix，并迁移 effect op 调用语法 |
| P2-T01R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t01rreview-perform-删除结果) | [DONE] Review P2-T01 `perform` 删除结果 |
| P2-T02 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t02将-handler-keyword-从-with-改为-on) | [DONE] 将 handler keyword 从 `with` 改为 `on` |
| P2-T02R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t02rreview-handler-on-切换结果) | [DONE] Review P2-T02 handler `on` 切换结果 |
| P2-T03 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t03实现-tuple-field-0--1-语法并移除-_0-正例) | [DONE] 实现 tuple field `.0` / `.1` 语法并移除 `._0` 正例 |
| P2-T03R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t03rreview-tuple-field-语法切换结果) | [DONE] Review P2-T03 tuple field 语法切换结果 |
| P2-T04 | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t04将-f-string-插值从--改为-) | [DONE] 将 f-string 插值从 `{...}` 改为 `${...}` |
| P2-T04R | [DONE] | [`TODO-2.md`](./TODO-2.md#done-p2-t04rreview-f-string-插值切换结果) | [DONE] Review P2-T04 f-string 插值切换结果 |
| P2-T05 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t05新增-operator-modifier-的-lexerparserast-surface) | 新增 `operator` modifier 的 lexer/parser/AST surface |
| P2-T05R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t05rreview-operator-modifier-surface) | Review P2-T05 `operator` modifier surface |
| P2-T06 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t06解析-inline-generic-bounds-与-ref--value-bound-keywords) | 解析 inline generic bounds 与 `ref` / `value` bound keywords |
| P2-T06R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t06rreview-generic-bound-parser-surface) | Review P2-T06 generic bound parser surface |
| P3-T01 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t01operator-positioned-calls-必须要求-operator-modifier) | operator-positioned calls 必须要求 `operator` modifier |
| P3-T01R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t01rreview-operator-gate-语义) | Review P3-T01 operator gate 语义 |
| P3-T02 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t02将--与-as-failure-从-raiseruntimeerror-改为-panic) | 将 `!!` 与 `as` failure 从 `Raise<RuntimeError>` 改为 `panic` |
| P3-T02R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t02rreview--与-as-panic-语义) | Review P3-T02 `!!` / `as` panic 语义 |
| P3-T03 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t03enum-with-mismatched-variant-改为-panic) | enum `with` mismatched variant 改为 panic |
| P3-T03R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t03rreview-enum-with-mismatch-panic) | Review P3-T03 enum `with` mismatch panic |
| P3-T04 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t04允许-refutable-val-pattern-并在-mismatch-时-panic) | 允许 refutable `val` pattern 并在 mismatch 时 panic |
| P3-T04R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t04rreview-refutable-val-pattern) | Review P3-T04 refutable `val` pattern |
| P3-T05 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t05禁止-closure-捕获外层-var) | 禁止 closure 捕获外层 `var` |
| P3-T05R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t05rreview-closure-var-capture-诊断) | Review P3-T05 closure `var` capture 诊断 |
| P3-T06 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t06用-ref--value-bound-constraint-kind-替换-anyref--anyvalue-sealed-marker) | 用 `ref` / `value` bound constraint kind 替换 `AnyRef` / `AnyValue` |
| P3-T06R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t06rreview-ref--value-bound-kind-替换结果) | Review P3-T06 bound kind 替换结果 |
| P3-T07 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t07默认-visibility-改为-internal-并同步-sysroot--cone-export) | 默认 visibility 改为 `internal` 并同步 sysroot / cone export |
| P3-T07R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t07rreview-default-internal-visibility) | Review P3-T07 default internal visibility |
| P4-T01 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t01实现-overload-effective-signature-与-signature-equivalence-helper) | 实现 overload effective signature 与 signature equivalence helper |
| P4-T01R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t01rreview-effective-signature-helper) | Review P4-T01 effective signature helper |
| P4-T02 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t02实现-generic-overload-shape-规则) | 实现 generic overload shape 规则 |
| P4-T02R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t02rreview-generic-overload-shape-规则) | Review P4-T02 generic overload shape 规则 |
| P4-T03 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t03实现-vararg-与非-vararg-overlap-的定义点-reject) | 实现 vararg 与非 vararg overlap 的定义点 reject |
| P4-T03R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t03rreview-vararg-overlap-reject) | Review P4-T03 vararg overlap reject |
| P4-T04 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t04实现-override--overload-边界与虚方法-generic-禁止) | 实现 override / overload 边界与虚方法 generic 禁止 |
| P4-T04R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t04rreview-override--overload-边界) | Review P4-T04 override / overload 边界 |
| P4-T05 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t05把-constructor-overload-纳入-definition-time-规则与-diagnostics) | 把 constructor overload 纳入 definition-time 规则与 diagnostics |
| P4-T05R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p4-t05rreview-constructor-overload-definition-time-规则) | Review P4-T05 constructor overload 规则 |
| P5-T01 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t01实现-phase-a-c候选收集visibilityapplicability) | 实现 Phase A-C：候选收集、visibility、applicability |
| P5-T01R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t01rreview-phase-a-c-resolution) | Review P5-T01 Phase A-C resolution |
| P5-T02 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t02实现-phase-d-e-specificity-与-ambiguity-diagnostics) | 实现 Phase D-E specificity 与 ambiguity diagnostics |
| P5-T02R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t02rreview-specificity-与-ambiguity-diagnostics) | Review P5-T02 specificity 与 ambiguity diagnostics |
| P5-T03 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t03整合-member--constructor--operator--effect-after-selection-路径) | 整合 member / constructor / operator / effect-after-selection 路径 |
| P5-T03R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t03rreview-call-surface-整合结果) | Review P5-T03 call surface 整合结果 |
| P5-T04 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t04贯通-selected-callable-identity修复-concrete--arity--generic-concrete-codegen-bug) | 贯通 selected callable identity，修复 overload codegen bug |
| P5-T04R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t04rreview-selected-callable-identity-贯通) | Review P5-T04 callable identity 贯通 |
| P5-T05 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t05审计-overload-diagnostics-与-user-visible-failure-policy) | 审计 overload diagnostics 与 user-visible failure policy |
| P5-T05R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p5-t05rreview-overload-diagnostics-审计) | Review P5-T05 overload diagnostics 审计 |
| P6-T01 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t01回写-scoop_full_specmd-与-split-spec-的全部语言变更) | 回写 `SCOOP_FULL_SPEC.md` 与 split spec 的全部语言变更 |
| P6-T01R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t01rreview-spec-回写完整性) | Review P6-T01 spec 回写完整性 |
| P6-T02 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t02同步-spec-doctests-与-handwritten-fixtures-到新-surface) | 同步 spec doctests 与 handwritten fixtures 到新 surface |
| P6-T02R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t02rreview-fixture-同步结果) | Review P6-T02 fixture 同步结果 |
| P6-T03 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t03执行旧-surface-与-overloadcodegen-回归审计) | 执行旧 surface 与 overload/codegen 回归审计 |
| P6-T03R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t03rreview-旧-surface-与回归审计) | Review P6-T03 audit 结果 |
| P6-T04 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t04全量格式化测试矩阵与最终收口记录) | 全量格式化、测试矩阵与最终收口记录 |
| P6-T04R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p6-t04rreview-最终收口质量) | Review P6-T04 最终收口质量 |

## 包间验收门禁

- 进入 `TODO-2.md` 前，旧 surface inventory、overload bug baseline、纯 spec 决议和 `@Inline` 删除必须已完成并通过 review。
- 进入 `TODO-3.md` 前，parser / AST 必须已经能表达所有目标语法，旧语法正例已迁移或转为 negative fixture。
- 进入 `TODO-4.md` 前，SPEC_FIX 的非 overload 语义、sysroot marker 替换、默认 internal visibility 与 cone export 合同必须已完成并通过 review。
- 进入 `TODO-5.md` 前，overload definition-time 规则必须已完成；P5 不应再处理“无论怎么调用都不合法”的 overload set。
- 完成 `TODO-5.md` 后，`SPEC_FIX.md` 与 `OVERLOAD_RESOLUTION.md` 的目标行为应成为活跃 spec 和 compiler 的实际 contract；旧 surface 只允许存在于 archive/history/design baseline 或明确 negative fixtures。
