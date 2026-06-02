# TODO 总索引（Fact 体系统一重构）

> 计划基线：[`PLAN.md`](./PLAN.md)（设计依据 [`EFFECT_INFER.md`](./EFFECT_INFER.md) + [`FACT_GAPS.md`](./FACT_GAPS.md)）
> 本文件是**唯一任务单入口**。具体任务分布在分批子文件中，按批次顺序执行。

## 给执行 agent 的导航规则

1. 本文件不直接含可执行任务，只做索引与排序。
2. 按下表**从上到下**找到第一个状态不是 `[DONE]` 的子计划文件，打开它，执行其中第一个标题未带 `[DONE]` 的任务（含 review 任务），完成后回写该子文件的状态与完成记录，然后停止。
3. 批次之间是**硬依赖**：后一批的任何任务都不得在前一批全部 `[DONE]` 之前开始。
4. 每完成/更新一个子计划，请同步更新本索引中该子计划的"状态"列（`TODO` / `进行中` / `DONE`）。

## 子计划索引（按执行顺序）

| 顺序 | 子计划 | 主题 | 覆盖 FACT_GAPS / EFFECT_INFER | 状态 |
| --- | --- | --- | --- | --- |
| 1 | [`TODO-1.md`](./TODO-1.md) | 基线清理（含 bypass 失败 delegate fixture）+ 稳定 identity key/`EffectRowTemplate` + 上游 identity 贯穿 | FG-01/02/03/04/05/14（表示+上游）；EFFECT_INFER §2.2 | DONE |
| 2 | [`TODO-2.md`](./TODO-2.md) | 完整 fact 发布 + self-contained artifact（HIR 分层 source facts / 统一 expression inference / MIR instance·effect-event·provenance·boundary facts / backend contracts 收口 / P4 纯消费） | FG-06/08/09(发布)/10/11(必发)/12/13/15；EFFECT_INFER §3/§4 | DONE |
| 3 | [`TODO-3.md`](./TODO-3.md) | 下游纯消费 + 删 fallback / fail-fast（P4 env 收口 / P5 LIR stable key / P6 LLVM 纯消费 / verifier） | FG-07/09(删 fallback)/11(fail-fast)/14/16/17/18；cross-cutting #3/#4 | 进行中 |
| 4 | [`TODO-4.md`](./TODO-4.md) | effect 语义收口（分层 row 契约 / dispatch ABI / 边界 / 递归 / inference 放宽）+ owner-eff 委托端到端 + 恢复 bypass + 全量回归 | EFFECT_INFER §5/§6/§7；承接 P5-T02B0/B/T03 | TODO |

**当前活跃任务**：`TODO-3.md` → `T3-04C`（收口 T3-04R 三次审查发现的 intrinsic/root/declaration ABI/reflection/verifier/gate 残余缺口）。

## 临时 bypass 登记

为避免前置任务期间每次 CI build 失败，`T1-00` 暂时 bypass 两类用例（fixture 头 `// IGNORE-UNTIL-FIX:`，Rust 单测 `#[ignore]`）：

1. **owner-eff/delegate 未完成导致的失败** —— 必须在 `TODO-4.md` 的 `T4-04` 全部移除并恢复通过，不得遗留永久跳过。
2. **并发 GC 偶发超时 fixture**（timeout 55000/59000ms：`runtime_gc/gc_language_parallel_alloc_shared_roots`、`std_sync_backend_parity_immix_major`、`gc_language_cross_thread_ref_handoff`、`gc_language_repeated_collect_shared_chain`）—— 与本轮无关的既有不稳定问题，**本轮 Fact 重构完成后另行安排修复**，不在 `T4-04` 恢复范围内。

具体清单由 `T1-00` 写入 `./memory/claude_plan.md`。

## 归档

- [`docs/archive/PLAN-2.md`](./docs/archive/PLAN-2.md) / [`docs/archive/TODO-2.md`](./docs/archive/TODO-2.md)：`@ReleaseHook` + `@NoGC` + `scoop.sync`/委托库化计划（P0–P5-T02A + P5-T02B00 已完成）。其余 P5 委托库化目标由本计划批 4 承接。
