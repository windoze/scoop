# 内置库测试迁移

111 个 API 场景迁入 `scoop.test`：cli 4、json 19、std 78、独立 core 消费者 10。既有 std 10 个测试继续保留，scoop.test 新增 4 个自身断言测试，无自身依赖或 core/test 环。

下表中的原目录均相对于 `tests/fixtures/`，同名场景移到目标测试目录的相应子目录，以 `ApiTests::behavior` 执行。保留各场景的正常路径、错误路径、边界值和组合断言；原成功标记由测试报告替代，集合的逐行 stdout 断言改成等价的 `assertEquals`/`assertTrue`，core 的错误输出和提前返回改成 `fail`。辅助 `check`/`equal` 使用 scoop.test 并传递调用点。

| 原功能目录 | 场景 | 目标测试目录 |
| --- | --- | --- |
| `m38-cli` | `basic`, `configuration`, `numbers`, `stable-decoder` | `sysroot/src/scoop.cli/tests/cli/` |
| `m42-algorithms` | `grouping`, `join`, `sorting`, `terminals`, `transforms`, `windows` | `sysroot/src/scoop.std/tests/algorithms/` |
| `m42-bytes` | `bounds`, `floating`, `integers`, `snapshots` | `sysroot/src/scoop.std/tests/bytes/` |
| `m42-datetime` | `calendar`, `date-arithmetic`, `date-text`, `date-time`, `instant`, `local-time`, `offset` | `sysroot/src/scoop.std/tests/datetime/` |
| `m42-duration` | `arithmetic`, `construction`, `text`, `units` | `sysroot/src/scoop.std/tests/duration/` |
| `m42-encoding` | `base64`, `base64-errors`, `hex`, `sources` | `sysroot/src/scoop.std/tests/encoding/` |
| `m42-foundation` | `collection`, `comparisons`, `lists` | `sysroot/src/scoop.std/tests/foundation/` |
| `m42-hash` | `basic`, `callbacks`, `iteration`, `model` | `sysroot/src/scoop.std/tests/hash/` |
| `m42-linked` | `basic`, `bulk`, `callbacks`, `factories`, `model` | `sysroot/src/scoop.std/tests/linked/` |
| `m42-queues` | `deque-basic`, `deque-model`, `heap-basic`, `heap-model` | `sysroot/src/scoop.std/tests/queues/` |
| `m42-random` | `bounded`, `collections`, `invalid`, `primitives`, `seeded`, `sequences` | `sysroot/src/scoop.std/tests/random/` |
| `m42-results` | `basic`, `exceptions`, `option` | `sysroot/src/scoop.std/tests/results/` |
| `m42-sequences` | `destinations`, `failures`, `foundation`, `lifecycle`, `pipeline`, `sorting`, `terminals`, `windows` | `sysroot/src/scoop.std/tests/sequences/` |
| `m42-text` | `float-grammar`, `float-long`, `float32`, `float64`, `integers`, `lines`, `parse-errors`, `radices`, `replace-split`, `search`, `whitespace-padding` | `sysroot/src/scoop.std/tests/text/` |
| `m42-time` | `marks` | `sysroot/src/scoop.std/tests/time/` |
| `m42-tree` | `basic`, `callbacks`, `floating`, `model`, `structure` | `sysroot/src/scoop.std/tests/tree/` |
| `m42-uuid` | `parsing`, `random`, `values` | `sysroot/src/scoop.std/tests/uuid/` |
| `m46-decoding` | `composition`, `records`, `scalars`, `sequences` | `sysroot/src/scoop.json/tests/decoding/` |
| `m46-encoding` | `committed`, `composition`, `errors`, `keyed`, `unkeyed` | `sysroot/src/scoop.json/tests/encoding/` |
| `m46-serialize` | `codecs`, `defaults`, `fields`, `generics`, `option`, `shapes`, `three-state` | `sysroot/src/scoop.json/tests/serialize/` |
| `m46-values` | `errors`, `numbers`, `values` | `sysroot/src/scoop.json/tests/values/` |
| `m26-characters` | `basic`, `values` | `sysroot/src/scoop.core/consumer-tests/tests/characters/` |
| `m26-lists` | `basic`, `iteration` | `sysroot/src/scoop.core/consumer-tests/tests/lists/` |
| `m26-string-builder` | `basic`, `reentry`, `snapshot` | `sysroot/src/scoop.core/consumer-tests/tests/string-builder/` |
| `m26-strings` | `basic`, `custom-list`, `snapshots` | `sysroot/src/scoop.core/consumer-tests/tests/strings/` |

B-tree 结构场景原本把 probe 加入临时生产库，现在 probe 位于 std 的 tests 中，继续验证节点容量、父子关系、顺序、深度和插入/删除复杂度。配置场景保留独立的 shadow 子包，验证同名注解不会被误认。

原 111 个 fixture 在对应 release 测试通过后删除；其输入未被剩余 fixture 共享。首轮曾保留普通 negative 和 production consumer，后按下面的补充清理继续合并、迁移；消费预编译库本身不构成保留 fixture 的理由。对四个实际读取 gcStats 验证回收的容器场景（hash、linked、queue、tree）不做同 Cone 迁移，归入 GC 专测，既有基线 51 项因此扩展为 55 项。

测试框架的计数、过滤、生命周期和生产/测试产物隔离使用固定的十用例 selection Cone；不绑定 std 总用例数。内置库的五个正式入口只检查运行完整且成功，新增库测试无需修改统计常量。

Darwin 的 Rust 格式化和 workspace clippy 通过；五个内置/消费者 Cone 共 125 个 release 测试通过，固定 selection Cone 的构建、运行、列表和过滤共 8 个正式进程步骤通过。Ruff/runner 和整体跨平台验收在后续运行模式与目录迁移完成后统一执行。

## 补充清理：普通编译错误合并

首轮“生产消费者、语言负例继续保留”的范围过宽，现按 [设计](DESIGN.md) 的补充规则修正。77 个借用库 API 触发普通编译器检查的 negative 已退役；这些库不再各自维护一组同类错误。原源码和诊断可从 Git 追溯，逐项清单在任务工作目录，仓库只保留规则对应。

| 普通编译器规则 | 合并掉的库场景 | 既有 expected-error 代表 |
| --- | --- | --- |
| 类型参数边界、接口符合性 | algorithms、hash、queue、sequences、tree、results 及 container/tuple 的普通边界错误 | `access-reject-component-conformance`、`add-reject-missing-conformance` |
| 泛型不变性与普通参数类型 | foundation、list、sequences、serialization、string-builder、text-writer、type-model | `generic-member-consumption-interface-invariant`、`add-reject-missing-conformance` |
| 缺少实参、缺少推断信息 | bytes、codecs、list-encoding、uuid、list | `arguments-errors-missing-required`、`aliases-unresolved-constructor` |
| 不可变属性、缺少可写操作 | bytes、datetime、time、linked、list、strings、tree | `classes-errors-assign-val-property`、`arrays-errors-write-immutable-array` |
| 构造器、成员及公开签名的可见性 | datetime、time、random、values、annotation-query、derived 编解码 | `generic-member-consumption-access-constructor-error`、`visibility/semantics/errors/access-domains` |
| override、缺少接口成员、继承默认冲突 | decoding、serialization、debug、derived 编解码 | `classes-errors-spurious-override`、`add-reject-missing-operator`、`interface-defaults/errors/default-rules` |
| 注解重复、参数类型、目标和未知注解 | cli、serialize 中的普通注解错误 | `annotation-repeated`、`annotations-error-scalar-for-array`、`annotation-rules` |
| 缺少依赖、unsafe 上下文、注解不是值类型 | foundation、strings、type-query | `core-dependencies-missing`、`cstrings-reject-unsafe`、`annotation-value-type` |

自动派生、声明查询、字符词法等编译器内建语义的 113 个独有拒绝条件已从库分组迁出：112 个归入 `language` 下的相应规则，1 个跨产物派生诊断归入 `compiler-correctness/derived-inspection`。各组统一使用 `expected-error` 标签；源码、诊断和模式保持原有内容，Darwin 的 113 个用例、118 个进程步骤全部通过。API 正常/错误路径及普通 consumer 的迁移另见下文。

## 专测边界归位

`native-requirements`、`native-weak`、`characters-ffi` 分别验证 native helper 的跨产物链接、ELF 弱引用和字符的 C ABI，归入 `compiler-correctness`；`lists-values-gc` 实际断言 gcStats 回收，归入 `gc-correctness/lists`。这些用例复用共享 cache，保留原有 target 与断言。Darwin 三项适用用例、九个进程步骤通过；四项在 GNU/musl 也已通过，包括 ELF 弱引用。

## 补充清理：普通 consumer 迁入 Cone 测试

154 个 consumer 场景已迁入 16 个所属 Cone，形成 162 个测试方法；原来以参数选择的模式分别作为测试方法执行。文件放在各 Cone 的 `tests/consumer/<原 fixture 名>/`，保留场景名称和辅助源码分组。core 继续使用独立测试 Cone。

| 所属 Cone | 原 fixture 场景 | 新增测试方法 | release 测试总数（三目标各自通过） |
| --- | ---: | ---: | ---: |
| `scoop.asyncio` | 10 | 10 | 133 |
| `scoop.charset` | 1 | 1 | 15 |
| `scoop.cli` | 1 | 1 | 5 |
| `scoop.compression` | 5 | 5 | 49 |
| `scoop.core/consumer-tests` | 22 | 22 | 32 |
| `scoop.crypto` | 2 | 2 | 25 |
| `scoop.http` | 22 | 23 | 269 |
| `scoop.http.asyncio` | 13 | 13 | 129 |
| `scoop.http.client` | 9 | 14 | 178 |
| `scoop.idna` | 1 | 1 | 11 |
| `scoop.json` | 44 | 46 | 65 |
| `scoop.platform` | 4 | 4 | 115 |
| `scoop.std` | 16 | 16 | 104 |
| `scoop.terminal` | 1 | 1 | 63 |
| `scoop.tls` | 1 | 1 | 28 |
| `scoop.url` | 2 | 2 | 14 |

保留原消费者的成功、错误、边界、异步清理和并发断言。固定成功输出由测试报告替代，stdout 值改成等价断言；文件和目录由测试创建并在 finally 中清理。普通 provider/re-export 示例改成同 Cone 辅助包，语言层的跨 Cone 可见性、同名类型身份和重导出继续由相应编译器专测覆盖。JSON 派生场景中的两个同名类型在同一测试 Cone 内使用不同声明名，保留编解码行为检查。

四个终端重复 fixture 直接合并到已存在并通过的 Cone 测试：`terminal-cursor` → `tests/output/cursor.scoop`；`terminal-screen` → `tests/output/screen.scoop`；`terminal-output` → `tests/output/commands.scoop`；`terminal-events` → `tests/input/modes.scoop`、`legacy_keys.scoop` 和 `protocol.scoop`。

在上述测试通过后删除 158 个旧载体；先计算剩余 fixture 的 inputs/support 归属，只删除无其他使用者的原文件。新测试新增的依赖只进入 test-dependencies，生产依赖不变。每个旧场景到新目录/方法的明细保存在任务工作目录，Git 可追溯原输入。

剩余 libraries 组有 71 项：5 个 `scoop test` 命令入口，66 个独立协议对端、native/C wire peer、进程环境、PTY 或标准流控制边界。其保留理由是被测进程外部的协议/系统行为，不能仅凭 consumer、artifacts、预编译库或缓存命中保留；普通功能断言继续优先进入 Cone tests。

16 个受影响 Cone 的 1,235 个 release 测试在 Darwin、Linux GNU 和 Linux musl 各自全部通过；其余三个未修改入口沿用各目标原有的 29 项结果，每个目标共 1,264 项。没有未解决失败。
