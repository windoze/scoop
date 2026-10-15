# M48 验收对应表

状态：实现与最终验收完成。M46 已完成；M48 本批提交完成后才推进 M38。本机集中完成全部修改、清理、完整功能复验与性能测量，再统一完成 Linux Rust、GNU／musl 正式 CLI 及最终外部调用观察。详细范围与补验来源见[结果概要](validation/README.md)。

## 功能证据

下表逐项对应 [设计第 10 节](DESIGN.md#10-验收)。已核对每份实际 fixture 的操作与断言，以及三平台完整验收的状态；目标不适用的用例单列，不能计为通过。执行范围与补验说明见[结果概要](validation/README.md)。

| 要求 | 实际 fixture |
| --- | --- |
| 库命中、输出发布和缺失结果补齐 | [m48-build-record-library](../../tests/fixtures/m48-build-record/library/fixture.toml)、[m48-artifact-summary](../../tests/fixtures/m48-artifact-summary/fixture.toml) |
| 选中源码变化、未选中源码与真实缺失输入 | [m48-external-source-file](../../tests/fixtures/m48-source-inputs/file/fixture.toml)、[m48-external-test-sources](../../tests/fixtures/m48-source-inputs/testing/fixture.toml)、[m33-sources-cache](../../tests/fixtures/m33-sources/cache/fixture.toml) |
| manifest 的生产／测试配置、实际文件选择 | [m48-manifest-record](../../tests/fixtures/m48-manifest-record/fixture.toml)、[m43-source-selection](../../tests/fixtures/m43-source-selection/fixture.toml)、[m43-test-selection](../../tests/fixtures/m43-test-selection/fixture.toml) |
| 依赖实现与程序变化、profile 切换、run 复用 | [m48-executable-record](../../tests/fixtures/m48-executable-record/fixture.toml) |
| C／C++ 头文件、include 选择与 native 对象复用 | [m48-native-objects-c](../../tests/fixtures/m48-native-objects/c/fixture.toml)、[m48-native-objects-cxx](../../tests/fixtures/m48-native-objects/cxx/fixture.toml) |
| runtime、链接库、库搜索候选与工具更新 | [m48-runtime-record](../../tests/fixtures/m48-runtime-record/fixture.toml)、[m48-executable-native](../../tests/fixtures/m48-executable-native/fixture.toml)、[m48-executable-tools](../../tests/fixtures/m48-executable-tools/fixture.toml)（Darwin 不适用，GNU 已通过） |
| 编译器／native／最终工具发现复用 | [m48-compiler-cache-library](../../tests/fixtures/m48-compiler-cache/library/fixture.toml)、[m48-compiler-cache-native](../../tests/fixtures/m48-compiler-cache/native/fixture.toml)、[m48-native-tool-record](../../tests/fixtures/m48-native-tool-record/fixture.toml)（Darwin 不适用，GNU 已通过）、[m48-final-tools](../../tests/fixtures/m48-final-tools/fixture.toml) |
| 稳定对象与显式产物消费 | [m48-object-files](../../tests/fixtures/m48-object-files/fixture.toml)、[m43-test-artifact-dependencies](../../tests/fixtures/m43-test-artifact-dependencies/fixture.toml) |
| 测试列表、过滤、no-run、用途与运行期失败 | [m43-test-list](../../tests/fixtures/m43-test-list/fixture.toml)、[m43-test-process](../../tests/fixtures/m43-test-process/fixture.toml)、[m43-test-purpose-failures](../../tests/fixtures/m43-test-purpose/failures/fixture.toml) |
| dump 逐阶段补算、已有观察复用与机器结果不变 | [m48-stage-observations](../../tests/fixtures/m48-stage-observations/fixture.toml)、[cli-process-source-observations](../../tests/fixtures/m23-cli-cache/process-observations/fixture.toml) |

run／test 的 [命令入口](../../compiler/scoop/src/cli/commands.rs)先取得本次成功的 `BuildOutcome`，构建错误直接返回，再进入实际执行；没有从失败分支取旧程序的路径。[链接冲突用例](../../tests/fixtures/m23-cli-program-link/unused-extern-conflict/fixture.toml)验证失败保留此前输出，完整本机报告已通过。

普通并发、取得锁后的复查和失败不发布由 [cache/store 测试](../../compiler/scoop/src/cache/store/tests.rs) 的实际发布与并发用例覆盖，两机完整 Rust 验收均通过。类型、格式、typed 引用、ABI、GC、FFI 与源码诊断继续由完整 Rust 和正式 CLI 套件验证。

## 清理与格式

- 历史 1,628 份源码隔离候选及后来补充的包装已迁移，逐项落点与保留断言见 [清理记录](CLEANUP.md)、[迁移验收](validation/source-isolation.md)和 [SOURCE-ISOLATION.tsv](SOURCE-ISOLATION.tsv)。当前集中批次删除旧 Compile 流水线、消费者语义／布局重放及专用工厂；名称级退役与保留职责见 [集中清理记录](validation/consolidated-cleanup.md)。
- 文件产物输入通过同一 [ArtifactFile](../../compiler/slib/src/artifact_file.rs) 延迟读取并持有首次需要的 member；缓存已命中时只使用普通结果记录。[最终外部观察](validation/final-io.md)已确认暖构建无编译／链接和受检内容读取，下游依赖一次打开且无重叠读取。
- 机器协议为 11，layout ABI 为 16；本批保存内嵌 typed layout key 与完整存储，不要求值都有独立 layout symbol，不恢复 MIR 重建。runtime ABI 保持。
- M47 设计已采用来源保留与实际调用观察；M38 不要求无源码消费。M45 本地未跟踪草稿的一处旧要求已同步，整份草稿不混入本次提交。M47 generator 实现不属于 M48 前置条件。

## 当前整体验收

| 范围 | 已取得的证据 |
| --- | --- |
| Darwin Rust | 5,302 项全部通过；完整首轮后，仅补验两条同步旧期望的测试。 |
| Darwin 正式 CLI | 4,089 项通过、18 项不适用；完整首轮后，仅补验三个修正源码的新增 consumer，无未闭合失败。 |
| Linux Rust | 5,338 项一次完整通过，零失败、零忽略。 |
| Linux GNU 正式 CLI | 4,078 项通过、29 项不适用，零失败，无需补验。 |
| Linux musl 正式 CLI | 4,074 项通过、33 项不适用，零失败，无需补验。 |
| 规则与 lint | 两机整仓 fmt／Clippy 及各 50 项 runner 规则通过；本机 Python 使用 Ruff 0.16.10。 |
| 性能 | [最终代码同机对照](PERFORMANCE.md)完成：原版本与最终实现各 55 次构建，每场景五组冷／暖，native 另五次头文件更新；两端均在 M4 Pro。 |
| 最终调用／I/O | [GNU 55 次、musl 5 次实际观察](validation/final-io.md)通过；所有暖构建零编译／链接、零受检内容打开、零 rename。真实下游只打开依赖一次，4,207 个机器 member 未读。 |

本机完整 CLI 首轮用时 28.1 分钟；用例累计耗时与进程数相对上一批分别减少 48.1% 和 684 次。累计耗时是并行用例各自耗时之和，不是墙钟加速比例；4,613 variants 和 88 项 stage／plan golden 保持。

本批已清理本机结束的 38.3 GiB fixture 工作目录，之前约 70 GiB 的旧工作清理另有记录。Linux GNU／musl 工作目录均在完整通过后清理，另清理已结束的旧轮次约 39.6 GiB。两机近期增量编译缓存继续保留。

追加清理前的 Darwin 汇总、Linux Rust 汇总和旧 Linux 中断报告只作为历史证据，不替代本批验证。旧 Linux CLI 为 825 通过、29 不适用、3,253 中断，该轮 musl 未启动。
