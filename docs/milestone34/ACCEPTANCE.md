# M34 实施与验收

完成日期：2026-10-10。

基线为 M33 的 `e97b40f5c9c408e2ff785e5d09c6bf9222c78979`，工作分支为 `codex/m34`。功能按批次完成并分别提交，规范、实现与实际测试见 [设计](DESIGN.md)、[实施记录](PROGRESS.md)和[性能记录](PERFORMANCE.md)。本记录按真实执行范围区分普通快照比较、审阅后的快照更新与定向复验。

## 已交付能力

| 批次 | 交付与关键验证 |
| --- | --- |
| M34-1 | 三份规范先行；保存两机 M33 工具、源码和性能基线，完成 LLVM 双分量根与目标 C ABI 实验。 |
| M34-2 | 16-byte interface，object／itab 分量与 slot receiver 分开；转换、identity、niche、AtomicRef、default／variance／boxing adapter、协程、native 和跨 Cone 均走普通 ABI／GC 路径。 |
| M34-3 | 实际 receiver 类型传播与唯一目标去虚拟化；按调用点选择小函数内联，保留实参顺序、独立值 place、异常、Context 和 moving roots。构造期间不提前使用最终动态类型。 |
| M34-4 | 函数入口复用线程状态地址，poll 正常路径执行原子条件检查，慢路径保留完整停稳／relocation；线程局部分配计数在 detach 时精确归并。 |
| M34-5 | GC-free、size ≤ 16、alignment ≤ 8 的小值使用 DirectParts；固定参数、无 errno 捕获的 C-layout aggregate 直接使用目标 C ABI。含引用值、较大值、errno 和 callback 保留各自既有边界。 |
| M34-6 | 16 MiB 普通 region、独立 large mapping、四层稀疏 page map 与 region cards；实际存活堆超过旧 1 GiB 上限的用例已在两机运行。 |
| M34-7 | 选择稀疏且未 pin 的 source，复用旧代空洞并完整预留目标，失败保全原图；移动与引用回写完成后 discard 空页／unmap 空 region，保留一个空 region。 |
| M34-8 | STW 并行首次标记、分块数组任务、全局终止和 worker-local 汇总；复用存活集合更新引用。1／2／4／8 worker、创建失败回退、连续 minor／full、多 mutator 和两机 TSan 已验证。 |
| M34-9 | `MaybeUninit<T>` 与 T 等大、等对齐、等 scan，支持全零构造、合法值包装和 unsafe `assumeInit`；ArrayList 使用普通 `MutableArray<MaybeUninit<T>>`，StringBuilder 复用列表并适配原生拼接。 |

独立／组合／negative fixture 保存实际 HIR、MIR、LIR；跨 Cone 用例删除源码后重新链接，debug／release 使用同一 ABI，并分别执行普通、full-moving、minor-stress。C aggregate 分类由独立 C compiler 互调检查，GC correctness 由真实移动和线程交错检查；没有以 LLVM verifier 代替这些运行。

## 总回归中的修复

| 问题 | 最终处理与证据 |
| --- | --- |
| companion 延迟登记沿用上一文件上下文 | 恢复实际声明的 `file` 后登记注解和成员；两机真实 core／用户源码回归通过，保留 intrinsic 声明规则。 |
| 原生对象 debug path 不确定 | 先取得编译临时目录物理路径，使已有 prefix map 覆盖真实路径；完整对象字节比较通过。 |
| 小值 coercion 产生 LLVM `memset` relocation | 复用普通 target-support registry 登记 `memcpy`／`memset`；三个 target、两个 profile 的真实机器对象和跨 Cone 产物消费通过。 |
| byval 复制中的 managed pointer 被 LLVM 整数化 | 在 module data layout 声明 `ni:1`，保持 typed pointer load；实际协程与最小 LLVM 回归通过，不放宽 provenance 或精确根检查。 |
| LLVM 机器布局复制条件 poll 后的不返回调用 | 关闭布局阶段的 tail duplication，保留共享尾块与唯一 SafepointId；三个 target 的 debug／release 对象回归及真实退出用例严格通过。 |
| String default adapter 使用普通 Class 表示 | 使用类声明已有 `physical_type`，同时覆盖 Any；六组修改 core 后从产物继承成员的用例及协程在三个 target 通过。 |
| 索引复合更新错误复用临时实参 | 保存 getter 已物化的真实 typed local，setter 复用显式索引；跨 Cone、缺省参数、vararg、嵌套更新与异常／GC 的求值次序通过。 |
| core 原生 Option 结果仍使用旧 sret | Char／Float／Double 按新 DirectParts 返回；24-byte slice 与含引用结果保持间接返回，三个 target 的 Unicode／浮点／JSON 与 Linux 原生 frame 验证通过。 |
| 旧 native fixture 要求每次普通 GC 都搬迁 | 普通模式继续检查内容和根回写，moving-stress 模式明确要求地址变化；24-byte 含引用值的 Scoop byval／sret 约定保持原状。 |
| 产物损坏用例保存旧截断长度 | 按实际新产物大小删除最后一个字节，保留完整诊断与旧可执行文件字节不变检查；Darwin 严格复验通过。 |

旧 Rust 预期同时迁移到真实 ABI、MaybeUninit core、初始化单元及完整 wire／fingerprint。原 sret 用例改为 24-byte 值，继续覆盖间接结果；没有删除 ABI、引用、诊断位置、精确根或产物字节断言。

## Rust 与公共 runner

Darwin 执行一次 `cargo test --release --workspace --no-fail-fast`，共 5,404 项：初跑 5,343 通过、61 失败、0 ignored。保留初跑并按失败清单补验，61 项已在 [Darwin](validation/rust-failure-closure-darwin.json) 和 [GNU](validation/rust-failure-closure-linux-gnu.json) 逐项闭合；五项 capability 测试随真实格式版本改名，记录保留旧名与新名的对应关系。

后续 `memset`、non-integral 与机器布局修复各增加一个 codegen 回归。最终生产实现 `db40db07a35896c040166fe454ff5d32c9ba3555` 下，Darwin 的 363 项、Linux 的 378 项 codegen 测试全部通过，数量差异来自平台条件测试；两机各 20 项 MIR 引用类型测试通过。没有把这些分组相加声称执行了另一轮全工作区，也没有重复未受影响的测试。公共 Python fixture runner 的 44 项规则测试通过。

每批实现先运行 `cargo fmt --all` 与 `cargo clippy --workspace --all-targets`，再测试，部分批次另用 release profile。Python 使用固定 `ruff==0.16.10`，C 使用 clang-format 与严格 warning 检查。源码拆分与 target 清理见下文。

## 正式 CLI 覆盖口径

正式 CLI 使用冻结的同一批配套工具，完整执行源码构建、产物消费、链接、运行、moving GC 和诊断断言。快照更新只迁移审阅过的预期；普通比较、更新执行与定向复验分别记录，合并时按 fixture 名去重。

Darwin 的一次 `--all` 初跑发现 2,920 项，其中 1,593 通过、1,311 失败、16 不适用。边界修复后的七组严格复验、1,304 组预期迁移、11 组完整诊断／native／损坏产物严格复验，以及 M34 其余 53 组严格复验，共闭合全部 2,904 个适用 fixture；按每项最后成功执行去重为 3,125 variants、14,240 processes、12,201 次 stage／plan／symbols 快照检查。完整名称与执行来源见 [Darwin 覆盖记录](validation/cli-coverage-darwin.json)。这不是另一次无更新的 `--all`。

机器布局修复后，Darwin 再严格运行全部 40 个 M34 正例与退出用例。旧 fixture 的公共预期原本会被 release 覆盖，现按实际 profile 拆分；九个代表用例共 19 variants、106 processes、89 次快照检查严格通过。离线逐份核对先前保存的实际文件输出，11,442 次观测／10,904 个不同文件全部一致；对没有落盘的共享 native link plan，仅重放原 artifact-link 步骤，44 次精确快照比较通过，不把它们算成再次完整执行 fixture。

M34 自身有 54 个 fixture，覆盖接口 6、去虚拟化 3、内联 5、poll 2、线程分配 2、小值 ABI 5、DirectC 3、多 region 2、reclamation 2、并行 mark 2、MaybeUninit 18、容器 4；MaybeUninit 的 18 项包含 14 项编译错误规则。Musl 的这 54 项及六组 String receiver 边界已严格通过，共 100 variants、502 processes、304 次快照检查，原始精简报告见 [musl 功能验收](validation/features-strict-musl-report.json)。

GNU 对 Darwin 受影响用例和 Linux 专用用例累计覆盖 1,347 项，其中 1,317 项适用用例通过、30 项不适用，没有未闭合失败；去重为 1,521 variants、10,685 processes、11,593 次快照检查。63 项 core 重建与既有超时用例以两路并发全部通过，保持原有时限。最终生产实现再严格运行 47 项 M34 正例、退出与 native 组合，93 variants、539 processes、384 次快照检查全部通过。完整来源见 [GNU 覆盖记录](validation/cli-coverage-gnu.json)与[最终严格复验](validation/final-strict-gnu-report.json)。

两机共有预期中，633 份 LIR 因真实 carrier 差异拆为按 target 选择，涉及 416 个 fixture；共同 HIR／MIR 没有新增平台差异。GNU 的 10,854 次保存文件观测／10,399 个不同文件逐份吻合，共享 native plan 另以 34 次原 artifact-link 进程严格复验。profile 拆分、目标拆分与精确文件核对见[快照迁移记录](validation/golden-migration.json)。

musl 对实际不同的原生符号、链接计划与平台专用用例生成自己的预期；相同 SysV ABI 的 805 份预期更新单独记录，不计作 musl 执行成功。随后 442 项旧回归用最终生产实现完整执行，11 项以两路并发、431 项以四路并发全部通过，未修改时限。与 M34／String 和退出用例按名称合并，累计 503 项全部通过，共 583 variants、5,589 processes、6,018 次快照检查，完整来源见 [musl 覆盖记录](validation/cli-coverage-musl.json)。GNU 与 musl 均按受影响范围选择，没有声称执行全部 2,920 项。

最终生产实现下，musl 的全部 40 个 M34 正例再次关闭快照更新通过，共 80 variants、436 processes、288 次快照检查，见[最终严格复验](validation/final-strict-musl-report.json)。编译前端未变化的 14 个 MaybeUninit 负例复用已通过的严格结果。

musl 保存文件的 5,279 次观测／5,103 个不同文件全部一致，34 次共享 native link plan 精确比较通过。取得实际输出后，5,769 份快照中 4,932 份与本机字节相同，合入另外 837 份目标专用文件，没有共享输出冲突；15 份已无引用的旧 musl profile 文件删除。487 份 fixture 声明经解析后与提交前比较，变化仅有 800 处 snapshot 路径，命令、超时、环境、运行断言与归一化规则保持原值。三个 target 最后的非更新模式配置发现均为 2,920 项。

## 实际 ABI 与产物版本

以下来自最终代码中的 capability registry 与 runtime ABI 编码；实施记录保留各中间批次当时的版本，不能当作当前版本清单。

| 合同／section | 版本 |
| --- | ---: |
| runtime ABI contract | 12 |
| runtime metadata ABI | 8 |
| HIR／MIR／LIR identity-foundation | 8／6／8 |
| manifest single-cone-production | 6 |
| HIR core-bootstrap-interface | 14 |
| HIR cross-cone-interface | 69 |
| HIR cross-cone-type-semantics | 27 |
| MIR cross-cone-param-free-bridge | 2 |
| MIR cross-cone-type-bridge | 19 |
| LIR cross-cone-param-free-bridge | 3 |
| LIR cross-cone-layout-abi | 15 |
| LIR cross-cone-layout-link-closure | 8 |
| LIR strong-production | 21 |
| LIR cone-production | 11 |
| LIR link-identity-closure | 15 |
| Scoop／C ABI classifier | 3／3 |

`MaybeUninit` nominal／callable role 为 14／25，三种操作为 1／2／3，默认值 tag 为 73，MIR／LIR 表示 tag 为 13／9。LLVM memory target-support 中 `memcpy`／`memset` 的稳定 tag 为 1／8。`ni:1` 只约束 LLVM 的指针转换，不改变目标指针大小、alignment 或原生 ABI。

两机均把真实 M33 `.slib`、其 core 依赖与原 runtime index 交给当前 linker：明确拒绝 runtime ABI fingerprint，exit 1，不产生输出文件；原始结果见 [Darwin](validation/old-abi-darwin.json) 和 [GNU](validation/old-abi-linux-gnu.json)。当前期望值为 `54f8b11101c6fd80f93492fe8ccd2b2bcd0e917cf1a697273de5d8c379376586`，旧值为 `8d0878fbaa8a430839c184f53feba9db6305368881034179e1bb13e5e0b8d583`。新 ABI 的混合 debug／release 与 artifact-only 用例正常运行。

## 默认策略与范围

默认 GC worker 数为 `min(处理器数, 4)`；未显式设置时，minor 或 committed heap 小于 4 MiB 使用单 worker。`SCOOP_GC_WORKERS=1..8` 用于显式比较；worker 创建失败回退到单线程。复制、release hook 和引用回写仍由 coordinator 执行，不把并行 mark 的收益等同于整个 pause 的收益。

接口每次动态调用直接取已有 slot；未知 Any 转接口仍按真实目标表查找，反复转换仍有成本。ArrayList 的 GC-free 元素明显缩小；引用／接口仍按完整 capacity 扫描，`MaybeUninit` 不携带初始化状态 tag，也不新增按 size 扫描。内联采用局部编译启发式，不决定源码是否合法。

并发 collector、并行 evacuation、survivor、多级代龄、LTO／PGO、普通外部正文导入、任意含引用 aggregate 的寄存器 ABI、callback／errno DirectC 扩展与按有效前缀扫描均不属于 M34。完整性能样本同时保留收益、代码增长及部分容器／GC 等待回退；未把全面快于其他语言或预设加速比作为完成条件。

## 代码长度与 target 管理

新增源码分为实际优化、ABI、marker、队列、region 与回收子模块。优化实现最长 260 行，新增 runtime 实现最长 183 行；Scoop ABI 验证从原大文件按 metadata／call site／destination 拆分，最长 473 行。对实现差异检查没有新增 `TODO`／`todo!`／`unimplemented!` 占位。

按批次清理已链接的中间对象和 inactive incremental，保留有效库、CLI、测试 executable 和用于比较的 M33 工具。已记录 Darwin 16 次、GNU 17 次清理，逻辑大小分别为 114,293,748,135 和 35,331,927,274 bytes；这些是清理项的逻辑大小，部分项为目录。最近 Darwin 一次清理为 1,509,504,489 bytes。GNU 最后一次清理为 1,412,833,095 bytes；逐批原始指标，见 [target 清理记录](validation/target-cleanups.json)。

## 功能提交

每个功能完成相应验证后分别提交。主要实现如下，边界修复与预期迁移保留为独立提交；性能最终重测记录为 `05d25c849`。

| 功能 | 提交 |
| --- | --- |
| 规范、基线与 ABI 实验 | `8c65d6c38` |
| 双字接口 | `d83371042` |
| receiver 传播与去虚拟化、调用点内联 | `0fbd84911`、`444d2bd52` |
| 条件 poll、线程分配统计 | `56343447b`、`430a69c71` |
| 小值 DirectParts、C aggregate DirectC | `dfc609f83`、`6f6950575` |
| 多 region 与稀疏索引 | `3cc277490` |
| 集中搬迁与内存归还 | `66abed0c8` |
| 并行 mark | `39127c897` |
| MaybeUninit、core 原生结果与容器 | `a4aeac060`、`9176404ba`、`759664a8d` |
| 索引更新的真实实参复用 | `253f93c00` |
| 源文件归属与原生对象确定性 | `e534687d5` |
| LLVM 内存 helper、pointer provenance 与唯一 statepoint | `96323e2b0`、`098c2523e`、`db40db07a` |
| intrinsic class 的 receiver adapter | `fbd98ad0d` |
| 旧 fixture 的 profile／target 迁移 | `a8b2bddef` |
