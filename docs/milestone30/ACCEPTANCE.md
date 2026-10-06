# M30 验收记录

日期：2026-10-07。状态：实现、正式验收与指纹测试清理已完成。Darwin 覆盖全部适用 fixture；Linux 覆盖 M30 专项及下述受影响组合和补充样本。

设计、分批提交和工具链调研分别见 [DESIGN.md](DESIGN.md)、[PROGRESS.md](PROGRESS.md) 与 [INVESTIGATION.md](INVESTIGATION.md)。本记录区分初次完整运行、快照迁移和普通模式复验；更新快照本身不计作验收通过。

## 1. 交付范围

Float／Double 是实际的 binary32／binary64 值类型，Float32／Float64 为透明别名。十进制 literal、const、默认值、annotation 和 JSON 直接按目标精度舍入；动态算术保持 IEEE 比较、subnormal、正负零和逐次舍入，不使用 fast-math 或隐式融合。普通关系不经过 compareTo，显式 totalOrder 区分 NaN sign／quiet／payload；浮点没有 Hash。

八种整数与两种浮点之间的转换显式进行，浮点转整数按目标宽度饱和，NaN 为零。值存储、泛型、递归 pattern、派生相等、boxing、容器、闭包与协程使用既有 IR、布局和 GC 通道。C／Scoop ABI 覆盖 scalar、混合 aggregate、global／TLS、指针和双向 callback；线程首次 attach 建立规定的浮点环境。

ToString 使用 Ryu 的目标精度最短转换；单值协议和两个 companion codec 已实现。JSON 保留整数精度与原始 number 文本，浮点编码只接受有限值，解析保留负零、合法下溢和直接 Float 舍入，并按实际 path 报错。跨 Cone 的 alias、重导出、const、annotation、默认表达式、派生 codec 与泛型 ODR 均已贯通；移走源码并隐藏 LLVM 后可以只凭产物和 runtime index 链接运行。

Int128／UInt128 与 Float128 完成调研，未增加类型、IR 占位或运行库依赖。调研明确区分 binary128 与目标 C long double，并记录 compiler-rt／libgcc、libquadmath、SoftFloat 的能力及未验证边界。

## 2. 环境与产物版本

| 环境 | 工具链与运行方式 |
| --- | --- |
| Darwin/AArch64 | 本机 LLVM 22.1.8，`LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22`；正式 CLI、实际链接和两种 GC |
| Linux/amd64 glibc | `ssh nuc12`，`~/repos/scoop`，LLVM 22.1，prefix `/usr/lib/llvm-22`；动态链接 |
| Linux/amd64 musl | 同一主机的 musl 工具链与 `/usr/bin/musl-gcc`；静态链接及独立动态 artifact 变体 |

三个配套命令为 `scoop`、`scoopc` 与 `scoop-link`，后者来自 `scoop-linker` package。本轮最后的生产实现提交是 `4833e4010`，之后的测试／文档清理复用这些 release 工具，没有为了测试快照重复构建编译器。

HIR 当前 core-bootstrap-interface 为 `/11`、cross-cone-interface 为 `/60`、cross-cone-type-semantics 为 `/22`；MIR cross-cone-type-bridge 为 `/15`；LIR identity-foundation 为 `/5`、cross-cone-layout-abi 为 `/9`、strong-production 为 `/19`、cone-production 为 `/8`、link-identity-closure 为 `/14`。具体能力名、浮点 tag 与兼容规则以[实现规范 2.18](../specs/SCOOP-IMPL-SPEC.md)为准；旧产物和缓存重建。runtime metadata ABI 仍为 4。

编译期使用 `rustc_apfloat 0.2.3+llvm-462a31f5a5ab`。Ryu 固定在 `4c0618b0e44f7ef027ebae05d2cc7812048f7c8f`，仅引入 f2s／d2s 及必要头文件，版权见 [runtime 说明](../../runtime/third_party/ryu/README.scoop.md)。

## 3. 验证方式与当前结果

### Rust 与公共 runner

Darwin 执行一次 `cargo test --workspace --no-fail-fast`，保留其中已成功的结果，并逐项修复和定向复验原来的失败项。原有 5352 项最终全部覆盖通过，没有忽略项；并非初次全量运行即全部通过。定向结果包括 codegen 317、HIR 877、HIR lowering 1375、slib 602，以及 driver 原先失败的 4 项；最终 GC 参数调整后，其 11 项 collector 测试再次通过。Linux/glibc 也通过同一组 11 项 collector 测试。

指纹清理新增 3 项独立 Rust 测试，并在现有完整 reader 测试中增加两种调度下的 8 个缺项组合。4 项定向测试和之后清理固定机器码摘要的端到端测试均通过，因此当前 5355 项 Rust 测试均有有效的通过结果。每批 Rust 变更先执行格式化和 workspace clippy，再运行测试。dev／test 使用 `opt-level=1` 缩短编译时间，保留 debug assertions 和溢出检查。

Python 变更按规定执行 Ruff 0.16.10 format／check。公共 runner 原有 38 项，加上 4 项 native 摘要和 2 项依赖摘要测试后共 44 项，在 Darwin 与 Linux 均通过。归一化测试特别验证错接引用、不同成员摘要、成员范围／选择、符号、动态绑定、错误依赖身份、记录值与实际值的错误关系和额外诊断仍会失败。

### M30 专项与 Linux 受影响组合

M30 共 55 个文件 fixture，包含 38 个 negative 与 17 个正例／平台变体。负例按完整 code、来源位置、message 和 notes 验证；正例保留阶段 dump、实际运行及相关 ABI／GC 断言。

| 范围 | 普通运行或有效验证结果 | 证据目录 |
| --- | --- | --- |
| Darwin M30 专项 | 初次正式全量中的 54 项全部通过，1 项仅适用于 musl | `/tmp/scoop-m30-final-darwin` |
| GNU M30 集中 | 54 项通过、1 项不适用；55 变体、119 进程、67 golden；后续普通模式复验覆盖所有适用正例 | nuc12 `/tmp/scoop-m30-final-gnu` 及 GNU 受影响复验 |
| musl M30 集中 | 55 项通过；56 变体、129 进程、71 golden；后续普通模式复验覆盖全部正例 | nuc12 `/tmp/scoop-m30-final-musl` 及 musl 受影响复验 |
| GNU 原有受影响范围 | 85 项普通模式通过、1 项不适用；分两次新工作目录完成，保留此前成功结果 | nuc12 `/tmp/scoop-m30-affected-gnu`、`/tmp/scoop-m30-gnu-remaining-verify` |
| GNU 新增组合 | 5 项全部通过；6 变体、58 进程、44 golden | nuc12 `/tmp/scoop-m30-extra-gnu-verify` |
| musl 受影响组合 | 46 项全部通过；48 变体、277 进程、192 golden | nuc12 `/tmp/scoop-m30-affected-musl-verify` |
| GNU 收尾补充样本 | 11 项普通模式全部通过；11 变体、143 进程、188 golden | nuc12 `/tmp/scoop-m30-last-samples-gnu-verify` |
| musl 收尾补充样本 | 11 项普通模式全部通过；11 变体、143 进程、188 golden | nuc12 `/tmp/scoop-m30-last-samples-musl-verify` |

Linux 集中轮对目标快照作了更新，最终正例使用新的工作目录在普通模式复验；没有把更新动作当作通过。旧 GNU 复验曾因复用目录中的只读产物导致 25 项准备失败，这 25 项已在新目录全部通过，没有重新执行其余成功项。最后两套 libc 的 11 项普通复验使用独立目录并行完成；核对 342 个同步输入及期望文件，内容与已审阅版本一致。此次 Linux 没有再跑无关的全仓库 CLI 全量，表中明确记录实际范围。

### Darwin 完整发现与定向收尾

初次 `--all` 发现 2551 个 fixture，结果为 1277 通过、1266 失败、1 个配置错误、7 个按平台不适用；执行 1278 个变体、6527 个进程、3411 份阶段／计划 golden。失败主要来自新增 core 声明、codec 方法和构造器推导修复之后的旧 HIR／MIR／LIR、符号、计划及无关固定摘要，随后逐项审阅迁移。

后续只选择此前失败的项目。B 轮 707 项中 181 项通过；C 轮 1085 项中 369 项通过；D 轮 708 项中 516 项通过；E 轮 192 项中 171 项通过；F 轮 21 项中 18 项通过；H 轮最后 3 项全部通过。另有四个独立的清理与 core 可见性回归轮；这些普通模式结果与最初完整轮合并复用。

退役重复的冻结 optional-members fixture 后，当前完整发现为 2550 项。按当前 fixture 名称逐项核对报告，Darwin 适用的 **2543 项全部通过**，另 **7 项不适用**，无待处理的失败或配置错误。最后 H 轮为 3 变体、58 进程、70 份阶段／计划 golden；其前置更新轮仅改动三个 ImportedIdentityId 编号，未计入通过并集。汇总证据为 `/tmp/scoop-m30-darwin-acceptance-union.json`，包含每项 fixture 的普通模式通过报告路径。

阶段快照保留真实 typed ID、符号、布局、顺序和引用。只涉及 ImportedIdentityId 的迁移先确认除此之外逐字节一致；构造器推导带来的闭包路径、局部编号及相关 ODR 身份另行审阅。多 Cone 的 consumer dump 按完整 HIR 精确对应实际产物；三个 M29 编码 consumer 的 LIR 与原始基线比较，完整指令与控制流保持，变化仅为 struct、external function 和 external type descriptor 索引。1957 份完整阶段快照单独提交。

Darwin 符号清单读取本轮实际程序，不为收集 nm 输出重新编译。Linux 先在 namespace、继承／ZST 和 core library 三个实际程序中核对共同新增符号，再为 GNU／musl 的 1372 份标准 nm 清单补齐相同 core/runtime 集合；保留已有符号、完整名称和顺序。四个闭包用例的作用域变化及目标专属 byte-string、layout、stackmap 名称另从 GNU／musl 实际程序更新，符号类别与数量保持；完整 nm 的 helper 用例只有新增符号。

core 的浮点声明增加 40 个对象，runtime 增加 5 个对象。Linux 计划另增加 7 个实际系统导入；只同步明确的计数字段，其他对象、库、原生输入、选择和动态绑定结构保持。剩余的 1650 份 Linux 计划依照已核对的相同运行库闭包迁移；收尾补充样本覆盖不同 core 配置、闭包、helper 及依赖失效的实际编译、链接和运行。删除两个不再由 Darwin-only fixture 引用的旧 GNU 计划文件。Linux 的这些基线迁移不表述为重新运行了全部 Linux fixture。

## 4. 指纹测试清理

- 删除 345 个普通 fixture 的独立固定产物指纹步骤及三个目标的 1035 份无关摘要 golden；其他输入、诊断、IR、符号与运行步骤保持。
- 对 560 个 fixture 的明确 native 内容摘要字段使用 `native-digests`，按首次出现编号保留相等与引用关系。迁移 563 个计划检查、21 个诊断步骤及 1673 份期望；普通 typed identity、requirement、符号和结构不归一化。
- 删除 13 个整份文件 SHA-256 检查及失用 runner 分支。将冻结 manifest／归档拼接损坏输入迁到现有 Rust reader／目录校验测试，删除 73 个失用文件；保留泛型委托的 27 个正常 CLI 步骤以及通用 program-link 的产物消费、运行和原子失败覆盖。
- 删除 Rust 泛型机器码端到端测试中 13 个额外固定哈希字面量，保留当次数据相等／变化、ODR 引用和补丁字节检查。
- 过期依赖诊断只对 recorded／actual 的六个明确 HIR／MIR／LIR 内容摘要使用 `dependency-digests`，保持相等／变化关系及完整依赖身份、来源、阶段和错误信息。三个 fixture 的四个诊断步骤不再固定某次构建摘要；12 份三平台期望经归一化后逐字节确认相同，合并为 4 份共同 JSON。
- 缓存命中／失效、构建确定性、读写、源码与 artifact-only 一致性继续比较本次实际值。直接构造受控字节或 canonical 编码的专门指纹向量继续保留。生产指纹语义与实际编译工具选择未因快照稳定性而改变。

五个清理提交为 `2f7bb9f24`、`4ea796f6c`、`d2d5f1aab`、`865e33eeb` 与 `91d182035`，分别对应固定产物摘要、native 摘要字段、动态损坏输入、Rust 端到端固定哈希表及依赖失效诊断。阶段快照提交为 `a901ae220`，平台计划与完整符号提交为 `77ff17843`。新写源码文件最长 265 行（第三方原文不计）；工作期间分批清理闲置 target incremental 产物，最后又从 Darwin／Linux 清理约 1.6／6.5 GiB，保留正在使用的 release 命令与可复用依赖。

## 5. 复现入口

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
cargo fmt --all
cargo clippy --workspace --all-targets
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
uv tool run --from ruff==0.16.10 ruff format tests/fixture_runner tests/run_fixtures.py
uv tool run --from ruff==0.16.10 ruff check tests/fixture_runner tests/run_fixtures.py
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all --work-dir /tmp/scoop-m30-clean-verification \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --jobs 12
```

定向开发验证使用 `--suite tests/fixtures --filter '<name-or-tag>'` 替换 `--all`；musl 另加 `--target x86_64-unknown-linux-musl --cc /usr/bin/musl-gcc`。每轮使用新的工作目录；可复制普通 cache 目录复用产物，不能把 cache 根替换成符号链接。快照更新必须审阅，再以普通模式完成相关断言。
