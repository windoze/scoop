# Safepoint Baseline

`T5000j4` 的目标不是引入新的优化，而是把当前主线在“减少调用边界之后，LLVM statepoint 数量与 `gc-live` roots 压力如何变化”这件事上，固化成一套可重复执行、可持续追加 workload 的观测口径。

## 重跑方法

```bash
cargo run -p scoop_tools -- safepoint-baseline
```

该命令会自动：

- 选取仓库内置 workload；
- 分别以 `-O0` / `-O2` 生成 LLVM IR；
- 统计实际发射的 `llvm.experimental.gc.statepoint` 调用点，以及每个调用点 `"gc-live"(...)` metadata 中的 roots 数量；
- 输出一份可直接粘贴到 issue / 计划文档中的 Markdown 报告。

## 指标口径

- `statepoints`：LLVM IR 中实际发射的 `llvm.experimental.gc.statepoint` 调用点数量，不包含 declaration。
- `rooted statepoints`：带非空 `"gc-live"(...)` roots metadata 的 statepoint 数量。
- `total gc-live roots`：所有 statepoint 的 `gc-live` roots 总数，用作当前 root-pressure 的最小可复验代理指标。
- `max gc-live roots`：单个 statepoint 上观测到的最大 live-root 数量，用于识别“是否仍存在少数极重的 safepoint 边界”。

当前 workload 刻意覆盖三类不同问题面：

- `inline_wrapper_string`
  普通 direct-call 边界，经 `summary-driven inlining + DirectCallOnly provenance` 摊平后，statepoint/roots 是否明显下降。
- `non_escaping_closure`
  局部 non-escaping closure 在 `O1+` closure simplification 之后，closure alloc / closure-call 边界是否消失。
- `task_handoff_gc_stress`
  task/effect/thread handoff 压力路径在当前主线下仍保留多少 safepoint 与 roots 压力，用于判断后续是否已经进入值得优先做 `mem2reg` / register-root 的窗口。

## 当前快照（2026-04-29）

通过 `cargo run -p scoop_tools -- safepoint-baseline` 得到：

| workload | opt | statepoints | rooted statepoints | total gc-live roots | max gc-live roots |
| --- | --- | ---: | ---: | ---: | ---: |
| `inline_wrapper_string` | `O0` | 12 | 9 | 13 | 2 |
| `inline_wrapper_string` | `O2` | 5 | 0 | 0 | 0 |
| `non_escaping_closure` | `O0` | 6 | 3 | 3 | 1 |
| `non_escaping_closure` | `O2` | 1 | 0 | 0 | 0 |
| `task_handoff_gc_stress` | `O0` | 255 | 233 | 435 | 5 |
| `task_handoff_gc_stress` | `O2` | 290 | 241 | 380 | 4 |

对应 delta：

- `inline_wrapper_string`：`statepoints 12 -> 5`，`total gc-live roots 13 -> 0`
- `non_escaping_closure`：`statepoints 6 -> 1`，`total gc-live roots 3 -> 0`
- `task_handoff_gc_stress`：`statepoints 255 -> 290`，但 `total gc-live roots 435 -> 380`，`max gc-live roots 5 -> 4`

## 当前结论

1. 当前中端主线已经能在“小 direct-call wrapper”和“non-escaping local closure”这两类结构上，直接减少 safepoint 数量，并把对应 `gc-live` roots 压力降到 0。后续若新增类似结构回归，这两条 workload 应保持敏感。
2. `task_handoff_gc_stress` 说明当前更复杂的 task/effect/runtime 路径，仍然是 safepoint 与 roots 压力的主要集中区。`O2` 虽然已经把总 roots 与单点峰值压低，但绝对量仍高，且 statepoint 数量不一定同步下降。
3. 因此当前更值得继续优先做的是：
   - 继续减少 task/effect/runtime 路径中的高层调用边界；
   - 继续压缩每个 safepoint 的 live-root 集；
   - 而不是立即把 `mem2reg` / register-root 提到主线优先级之前。

换句话说，这份 baseline 当前支持的判断是：`mem2reg` 研究窗口还没有消失，但从现有 workload 看，近期收益仍更可能来自“继续减少调用边界与 roots 压力”，而不是先转向 register-root 改造。
