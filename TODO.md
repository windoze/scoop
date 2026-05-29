# GC Pacing + Immortal Objects TODO 索引

> 生成时间：2026-05-29
> 设计基线：[`GC_PACING.md`](./GC_PACING.md)、[`GC_IMMORTAL_FIX.md`](./GC_IMMORTAL_FIX.md)
> 计划基线：[`PLAN.md`](./PLAN.md)
> 格式参考：[`docs/archive/plans/TODO-spec-fix-overload.md`](./docs/archive/plans/TODO-spec-fix-overload.md)
> 当前状态：任务已拆分为 5 个任务包；`P0-T01` / `P0-T01R` / `P0-T02` / `P0-T02R` / `P0-T03` / `P0-T03R` / `P1-T01` / `P1-T01R` / `P1-T02` / `P1-T02R` 已完成，其余待执行（`[TODO]`）。每个实现任务后都紧跟一个独立 review 任务，编号为原任务 ID + `R`。

## 总原则

- `PLAN.md` 是当前执行计划基线；如果实现时发现阶段边界、运行期决议或语言决议需要改变，必须先回写 `GC_PACING.md` / `GC_IMMORTAL_FIX.md`，再调整 TODO。
- 所有任务按 `TODO-1.md` 到 `TODO-5.md` 顺序推进；除非对应文件明确允许，不跨包并行实现。Pacing 线（TODO-1/2）优先于 Immortal 线（TODO-3/4），因为它决定长程序能否运行。
- 每个实现任务后必须紧跟一个独立 review 任务，复审该任务的完整变更、阶段目标和约束遵守情况。
- review 任务不是形式检查；如果发现前一任务没有真正完成目标，review 任务必须直接修正或阻塞下一任务。
- 任务完成后必须同时更新本索引和对应 `TODO-[1-5].md` 中的任务状态与完成记录；不得只更新其中一边。
- **Pacing 必须 on-by-default**。现状是无条件无界增长；`SCOOP_GC_PACING=off` 只保留给需要确定性堆计数的测试，且每个用到它的测试必须注明 why。
- **Immortal 不变式必须守干净**：immortal 对象永不被写、永不被 trace。任何可写或可能需要 trace 的对象（`.data` 静态、含可变托管引用的全局）一律不进 immortal 轨道。
- “是否常量化”是由类型**传递不可变性**决定的通用决策，不得退回逐类型特判或类型白名单；“是否 dedup”是正交的、仅对 String 开的内容池。
- 所有 runtime 改动必须保持 `immix` / `hosted` / `minimal` 三 backend 可编译可回归。
- 不接受把当前无界增长或 per-use wrapper 分配记成最终期望；目标行为由两份设计文档定义。

## 任务包划分

| 包 | 文件 | 覆盖 PLAN 阶段 | 目标 | 当前细化状态 |
| --- | --- | --- | --- | --- |
| 1 | [`TODO-1.md`](./TODO-1.md) | P0-P1 | 冻结 pacing/immortal 当前行为基线与度量；落地 pacing 核心触发 | 已细化 |
| 2 | [`TODO-2.md`](./TODO-2.md) | P2 | pacing 分代触发、OOM 防御、hard cap 与 backend parity | 已细化 |
| 3 | [`TODO-3.md`](./TODO-3.md) | P3-P4 | `@InteriorMutable` + `__AtomicInt` struct；immortal 运行期与 content-hash 键 | 已细化 |
| 4 | [`TODO-4.md`](./TODO-4.md) | P5-P6 | 通用 `is_immutable` 谓词 + 折叠器 + String immortal；Platform 折叠与审计 | 已细化 |
| 5 | [`TODO-5.md`](./TODO-5.md) | P7 | spec / 文档 / fixtures 收尾与全量回归矩阵 | 已细化 |

## 具体任务索引

| 任务 | 状态 | 文件 | 目标 |
| --- | --- | --- | --- |
| P0-T01 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01核对并冻结-pacing-当前行为基线) | 核对并冻结 pacing 当前行为基线 |
| P0-T01R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t01rreview-pacing-行为基线) | Review P0-T01 pacing 行为基线 |
| P0-T02 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02核对并冻结-immortal-当前行为基线) | 核对并冻结 immortal 当前行为基线（含 `__AtomicInt` 擦除点） |
| P0-T02R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t02rreview-immortal-行为基线) | Review P0-T02 immortal 行为基线 |
| P0-T03 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t03建立堆增长与字面量分配计数度量) | 建立堆增长与字面量分配计数度量 |
| P0-T03R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p0-t03rreview-度量基线) | Review P0-T03 度量基线 |
| P1-T01 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t01实现-pacing-核心-next_gc--request_collect--safepoint--阈值) | 实现 pacing 核心：`next_gc` + `request_collect` + safepoint + 阈值 |
| P1-T01R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t01rreview-pacing-核心) | Review P1-T01 pacing 核心 |
| P1-T02 | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t02接入-pacing-env-旋钮与默认-on并加长程序有界回归) | 接入 pacing env 旋钮与默认 on，并加长程序有界回归 |
| P1-T02R | [DONE] | [`TODO-1.md`](./TODO-1.md#done-p1-t02rreview-pacing-env-旋钮与有界回归) | Review P1-T02 env 旋钮与有界回归 |
| P2-T01 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t01nursery-满触发-minor-gc-再重试) | nursery 满触发 minor GC 再重试 |
| P2-T01R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t01rreview-nursery-full-minor-gc) | Review P2-T01 nursery-full minor GC |
| P2-T02 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t02block-pool-耗尽先-full-gc-再增长) | block pool 耗尽先 full GC 再增长 |
| P2-T02R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t02rreview-block-pool-回退) | Review P2-T02 block pool 回退 |
| P2-T03 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t03接入-hard-cap-与-oom-返回) | 接入 `SCOOP_GC_MAX_HEAP_BYTES` hard cap 与 OOM 返回 |
| P2-T03R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t03rreview-hard-cap) | Review P2-T03 hard cap |
| P2-T04 | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t04hostedminimal-backend-pacing-parity) | hosted/minimal backend pacing parity |
| P2-T04R | [TODO] | [`TODO-2.md`](./TODO-2.md#todo-p2-t04rreview-backend-parity) | Review P2-T04 backend parity |
| P3-T01 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t01新增-interiormutable-注解) | 新增 `@InteriorMutable` 注解 |
| P3-T01R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t01rreview-interiormutable-注解) | Review P3-T01 `@InteriorMutable` 注解 |
| P3-T02 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t02__atomicint-升为-interiormutable-struct) | `__AtomicInt` 升为 `@InteriorMutable struct` |
| P3-T02R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p3-t02rreview-__atomicint-struct-化) | Review P3-T02 `__AtomicInt` struct 化 |
| P4-T01 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p4-t01运行期-immortal-flag-与-marker-短路) | 运行期 `SCOOP_GC_FLAG_IMMORTAL` 与 marker 短路 |
| P4-T01R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p4-t01rreview-immortal-运行期短路) | Review P4-T01 immortal 运行期短路 |
| P4-T02 | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p4-t02byte-数组-content-hash-键与-unnamed_addr) | byte 数组 content-hash 键与 `unnamed_addr` |
| P4-T02R | [TODO] | [`TODO-3.md`](./TODO-3.md#todo-p4-t02rreview-content-hash-键) | Review P4-T02 content-hash 键 |
| P5-T01 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t01实现-is_immutable-谓词) | 实现 `is_immutable(T)` 谓词 |
| P5-T01R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t01rreview-is_immutable-谓词) | Review P5-T01 `is_immutable` 谓词 |
| P5-T02 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t02实现-try_emit_immortal-折叠器并路由-string-literal) | 实现 `try_emit_immortal` 折叠器并路由 String literal |
| P5-T02R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t02rreview-折叠器与-string-immortal) | Review P5-T02 折叠器与 String immortal |
| P5-T03 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t03string-内容池-dedup-与其它-ref-类型-per-site) | String 内容池 dedup 与其它 ref 类型 per-site |
| P5-T03R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p5-t03rreview-dedup-策略) | Review P5-T03 dedup 策略 |
| P6-T01 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p6-t01platform-lower-成-structlit-并删除专用-codegen) | Platform lower 成 StructLit 并删除专用 codegen |
| P6-T01R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p6-t01rreview-platform-折叠) | Review P6-T01 Platform 折叠 |
| P6-T02 | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p6-t02typemetadataliteral-审计与指针相等断言) | `TypeMetadataLiteral` 审计与指针相等断言 |
| P6-T02R | [TODO] | [`TODO-4.md`](./TODO-4.md#todo-p6-t02rreview-typemetadata-审计) | Review P6-T02 TypeMetadata 审计 |
| P7-T01 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t01回写-runtimespec-文档pacing--immortal) | 回写 runtime/spec 文档（pacing + immortal） |
| P7-T01R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t01rreview-文档回写) | Review P7-T01 文档回写 |
| P7-T02 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t02审计需要-pacingoff-的测试并注明原因) | 审计需要 `PACING=off` 的测试并注明原因 |
| P7-T02R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t02rreview-pacingoff-审计) | Review P7-T02 `PACING=off` 审计 |
| P7-T03 | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t03全量测试矩阵out-of-scope-归位与收口) | 全量测试矩阵、out-of-scope 归位与收口 |
| P7-T03R | [TODO] | [`TODO-5.md`](./TODO-5.md#todo-p7-t03rreview-最终收口质量) | Review P7-T03 最终收口质量 |

## 包间验收门禁

- 进入 `TODO-2.md` 前，pacing 行为基线、immortal 行为基线、堆增长/分配计数度量必须已建立，且 pacing 核心触发已让长程序在默认配置下有界并通过 review。
- 进入 `TODO-3.md` 前，pacing 的分代触发、block-pool 回退、hard cap 与 backend parity 必须已完成并通过 review（pacing 线收口）。
- `TODO-3.md` 内部：P4（immortal 运行期）必须先于 `TODO-4.md` 的 P5 codegen 发射；P3（`@InteriorMutable` + `__AtomicInt`）必须先于 P5 的 `is_immutable` 谓词。
- 进入 `TODO-4.md` 前，`@InteriorMutable` 注解、`__AtomicInt` struct 化、`SCOOP_GC_FLAG_IMMORTAL` 运行期短路、content-hash byte 键必须已完成并通过 review。
- 进入 `TODO-5.md` 前，String / `Platform` / `__type_name` 已走通用常量化路径且零 `scoop_alloc_typed`，dedup 策略已就位。
- 完成 `TODO-5.md` 后，`GC_PACING.md` 与 `GC_IMMORTAL_FIX.md` 的目标行为应成为运行期与编译期的实际 contract；旧的无界增长与 per-use wrapper 分配只允许存在于 `PACING=off` 对照与 design history 中。
