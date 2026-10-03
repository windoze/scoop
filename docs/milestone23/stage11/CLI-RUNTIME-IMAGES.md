# M23-11 多 image 运行与初始化迁移

`m23-runtime-images` 的原 Scoop 源码通过普通 Cone manifest、正式 `.slib` 生产和 program-link 执行。原临时 C main 中的 trace 函数改为显式 native companion；源码中的 `m23_trace` 声明补充 `lib = "m23_runtime_images"`，按现有 native requirement 规则选择库。trace 的数值、调用顺序和失败行为不变，每次输出后立即刷新，以保留 abort 前的完整观测。

每层保存完整四阶段输出，在消费前删除已编译的源码；最终通过独立 `scoop link` 重新链接，在普通和 moving GC 模式核对 stdout、stderr 和退出／signal。链接使用正式 startup 和 runtime index，不再为正常运行拼装另一套 main 或 image 列表。

迁移结果和旧入口清理逐批记录于下表；未列出的 runtime 生命周期边界检查仍待处理。

| 原功能 | CLI 用例 | 进程／golden | 保留的断言 |
| --- | --- | --- | --- |
| `multi_image_startup_runs_empty_ordinary_and_nogc_roots` | `empty`、`ordinary`、`nogc` | 20／27 | empty／ordinary／NoGC root；完整三 image 图，原输出和两种 GC 模式 |
| `multi_image_startup_uses_the_current_ready_set_and_complete_roots` | `schedule-root` | 11／21 | base → next 与 peer → join → root；实际输出严格为 10、20、30、40、50 |
| `multi_image_startup_reports_managed_root_and_eager_failures` | `root-failure`、`eager-failure` | 14／18 | root／eager 异常均 SIGABRT，完整 stderr 和 abort 前 11 的输出 |
| `early_ensure_runs_each_eager_unit_once_before_its_scheduled_turn` | `early-root` | 8／9 | 6 个初始化 unit；持久 ID 顺序 v2、v4、v5、v0、v3、v1，执行 v5 → v0 各一次，main 最后输出 7 |
| `lazy_initialization_retains_failed_roots_and_catches_cycles` | `unused-lazy-root`、`lazy-failure-root`、`lazy-cycle-root`、`cycle-recovery-root` | 32／36 | 未使用 lazy 不执行；失败对象与 payload 保活且重复读取相同；cycle message 身份稳定；可捕获 cycle 并恢复 |
