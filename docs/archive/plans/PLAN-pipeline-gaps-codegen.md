# Scoop：Pipeline Gaps Codegen 收口计划

> 生成时间：2026-05-06  
> 差距基线：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md)  
> 执行清单：[`TODO-pipeline-gaps-codegen.md`](./TODO-pipeline-gaps-codegen.md)  
> 格式参考：[`PLAN.md`](./PLAN.md)  
> 本轮主题：在 HIR/MIR handoff 完整之后，收口 refactor LLVM / runtime codegen gaps，让默认 refactor 主线不再依赖 legacy fallback 或 late unsupported。

## 0. 工作原则

- 本阶段只修 effect-refactor / default codegen path；legacy backend 只作为 compare 入口，不作为正确性兜底。
- Codegen 只能消费 refactor HIR/MIR/materialized MIR、effect facts、late-lowered handoff、ABI query 和 target/session config；不得回 AST/HIR 私有 side table 恢复语义。
- 如果缺的是 MIR contract，必须回到 [`TODO.md`](./TODO.md) 的 MIR-facing owner 补 handoff；不得在 LLVM emitter 现场猜 shape。
- ABI routing 必须由 callable 的 actual outward effect set 决定：outward 空集发布 plain ABI，非空才发布 EffectStep body 或 effect boundary/adapter。
- raw MIR route 要么实现对应 construct，要么在 route verifier 阶段明确拒绝；不得让 unsupported terminator/call kind 进入 body emission 才失败。
- 验证以定向 unit/build/run-pass/runtime_gc fixture 为主；只有 CG8 才恢复 full regression。

## 1. 顺序总览

1. CG0：codegen gap inventory 与 backend gate 冻结。
2. CG1：raw MIR effect/control route 与 unsupported call kind 收口。
3. CG2：runtime type/value primitive codegen 收口。
4. CG3：call/ctor/function-ref/intrinsic/default/interface lowering 收口。
5. CG4：aggregate/enum/array/closure/boxing transport lowering 收口。
6. CG5：effect-typed function-value adapter 与 plain/Step ABI 收口。
7. CG6：effect-refactor source classification、unwind、thread boundary 收口。
8. CG7：extern global 与 GC pin/handle runtime surface 收口。
9. CG8：默认 refactor regression 与阶段退出审计。

## 2. 分阶段计划

### CG0. codegen gap inventory 与 backend gate 冻结

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3、§4、§5、§6、§7.2、§7.6、§9。

目标：建立 codegen gap owner map，区分 raw MIR LLVM、effect-refactor LLVM、runtime C、fixture/regression 和 upstream MIR contract 缺口。

阶段输出：backend route verifier、unsupported inventory、每个 gap 的 owner task 和验证样本。

### CG1. raw MIR effect/control route 与 unsupported call kind 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.1、§3.2、§3.3、§3.6。

目标：禁止 unsupported effect/control MIR 误入 raw MIR codegen，或为 raw route 实现完整语义。

阶段输出：raw route capability check、`PerformResult` resume payload binding guard、virtual/interface/resume call kind route policy。

### CG2. runtime type/value primitive codegen 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.4、§3.5、§3.8、§6.1、§6.2、§7.2。

目标：实现 `is` / `as` / `as?` / `!!` / pattern type test 的 refactor LLVM lowering，或在 frontend/MIR 阶段明确拒绝 unsupported function type cast。

阶段输出：runtime type descriptor / itable matching lowering、cast failure ordinary effect boundary、`Option<T>` construction、not-null assertion raise path。

### CG3. call/ctor/function-ref/intrinsic/default/interface lowering 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.7、§3.9、§3.10、§6.3、§6.5。

目标：让 codegen 只消费 typed call/ctor/intrinsic contract，不再补 named/default args 或猜 top-level function refs/interface defaults。

阶段输出：complete-args ctor lowering、function-ref normalization lowering、runtime reflection/platform intrinsic lowering、interface default dispatch verification。

### CG4. aggregate/enum/array/closure/boxing transport lowering 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.11、§4.1、§4.2、§4.3、§4.4、§4.5、§5.5。

目标：统一 composite value transport，覆盖 closure env、value boxing、enum payload、array element 和 cross-thread resume payload。

阶段输出：traceable heap/value boxing layout、boxed enum payload layout、composite array get/set/copy、closure capture box、cross-thread payload root/copy ABI。

### CG5. effect-typed function-value adapter 与 plain/Step ABI 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.12、§5.1、§5.4。

目标：保证 outward-empty callable 公开 plain ABI，同时为 actual-outward 非空或 effect-typed adapter surface 生成正确 EffectStep adapter。

阶段输出：hidden-sret aggregate adapter、plain closure/function value adapter、plain `main(args)` argv ABI、outward-empty no-Step verifier。

### CG6. effect-refactor source classification、unwind、thread boundary 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §3.13、§5.2、§5.3、§5.6。

目标：让 late-lowered source classification、cleanup/unwind、continuation storage route、thread resume non-complete boundary 在 verifier 或 codegen 中有完整语义。

阶段输出：unsupported source verifier、unwind payload carrier、cleanup continuation lowering、unique continuation storage route consumption、cross-thread non-complete policy。

### CG7. extern global 与 GC pin/handle runtime surface 收口

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §6.4、§7.6。

目标：实现 `@Extern` global storage/linkage lowering，并为 GC pin/handle intrinsic surface 建立 codegen/runtime contract 或 frontend reject。

阶段输出：extern symbol/TLS/global access lowering、unsafe access guard、pin/unpin lifetime lowering、root/handle runtime integration。

### CG8. 默认 refactor regression 与阶段退出审计

参考：[`PIPELINE_GAPS.md`](./PIPELINE_GAPS.md) §5.7、§9。

目标：恢复默认 refactor full regression，关闭 P7 剩余 blocker，并把 codegen gaps 重分类为已修复、frontend reject 或后续非本阶段 runtime work。

阶段输出：full `cargo test --all`、`cargo run -p scoop -- test`、GC env regression 记录和阶段退出审计。

## 3. 阶段切换门槛

- CG0 未完成前，不允许新增 backend workaround 绕过 inventory。
- CG1 未完成前，raw MIR path 不得宣称支持 effect/control body。
- CG2 未完成前，runtime cast/typecheck/not-null 相关 run-pass 不作为 default complete 依据。
- CG4 未完成前，composite payload/array/closure/env 相关失败不得归咎于 runtime fixture 本身。
- CG5 未完成前，不允许重新引入 complete-only `Step_F` 或 Step argv ABI 作为 outward-empty workaround。
- CG8 未完成前，codegen pipeline gaps 阶段不算完成。

## 4. 完成标准

本阶段完成时，必须能够明确陈述以下结论全部成立：

1. refactor codegen 不依赖 legacy HIR lowering、legacy handler stack 或 old callable wrapper 作为 correctness 兜底。
2. raw MIR route 对 unsupported effect/control/call kind 有 verifier 或完整 lowering。
3. runtime type/value primitives、call/ctor/intrinsic/default/interface dispatch、aggregate transport、enum/array/closure/boxing 都有 refactor codegen 路径或明确 frontend reject。
4. outward-empty callable 在 LLVM 层公开 plain ABI，actual-outward 非空或 adapter surface 才使用 EffectStep。
5. effect-refactor source classification、unwind、thread boundary 不再晚到 body emission 或 runtime fatal 才暴露缺 contract。
6. `PIPELINE_GAPS.md` 中 codegen-stage scope 的 gap 已关闭或有明确非本阶段 owner。
