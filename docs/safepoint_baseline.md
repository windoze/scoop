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
- `root_pressure_loop`
  普通 loop 中多个 live `String` root 跨调用边界时，默认 explicit root frame 模式是否仍避免发射 LLVM statepoint / `gc-live` metadata。

历史 workload `task_handoff_gc_stress` 已随 async/task surface 删除而退役；当前用 `root_pressure_loop` 覆盖同一类“多个 live root 跨调用边界”的局部压力观测。

## 当前快照（2026-05-26）

通过 `cargo run -p scoop_tools -- safepoint-baseline` 得到：

| workload | opt | statepoints | rooted statepoints | total gc-live roots | max gc-live roots |
| --- | --- | ---: | ---: | ---: | ---: |
| `inline_wrapper_string` | `O0` | 0 | 0 | 0 | 0 |
| `inline_wrapper_string` | `O2` | 0 | 0 | 0 | 0 |
| `non_escaping_closure` | `O0` | 0 | 0 | 0 | 0 |
| `non_escaping_closure` | `O2` | 0 | 0 | 0 | 0 |
| `root_pressure_loop` | `O0` | 0 | 0 | 0 | 0 |
| `root_pressure_loop` | `O2` | 0 | 0 | 0 | 0 |

对应 delta：

- `inline_wrapper_string`：`statepoints 0 -> 0`，`total gc-live roots 0 -> 0`
- `non_escaping_closure`：`statepoints 0 -> 0`，`total gc-live roots 0 -> 0`
- `root_pressure_loop`：`statepoints 0 -> 0`，`total gc-live roots 0 -> 0`

## 当前结论

1. 当前默认 explicit root frame 模式下，这三条 workload 在 `-O0` / `-O2` 都不再发射 LLVM statepoint，也不再产生 `gc-live` metadata roots。
2. 这份 baseline 当前主要用于防止普通调用边界、non-escaping closure 与局部 loop root-pressure 场景意外退回 stackmap/statepoint 路径。
3. 如后续重新引入显式 stackmap/statepoint workload，应在本工具中单独新增 opt-in workload，而不是复用已经退役的 async/task handoff fixture。
