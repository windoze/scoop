# M41 验收记录

状态：已完成并验收（2026-10-13）。实施基线为 `9e715a3c`；范围见[设计](DESIGN.md)，逐功能过程见[实施记录](PROGRESS.md)。

## 交付内容

- 普通名义运算符接口覆盖算术、一元、自增减、比较、区间、包含、索引、调用、component 与复合赋值。内建类型显式实现对应能力；泛型正文按定义处的 bound 绑定成员，扩展不产生名义 conformance。
- `Equals<T>` / `operator equals` 统一默认相等；删除 core `Equality` / `equalTo`。Unit 使用其声明的普通成员，结构派生不自动补齐接口实现。
- Float/Double 默认比较采用等价类全序：正负零相等，全部 NaN 相等并排在正无穷之后。`Compare` / `compareTo`、常量、普通比较、when 值匹配和结构派生保持一致。
- `IeeeComparison` / `ieeeCompare` 提供显式 IEEE 四态比较；普通 NoGC core body 组合分类与默认比较。删除旧 `isTotallyOrdered` 和 totalOrder 实现，保留算术、转换、舍入、存储与 FFI 的既有规则。

具体化后的已知内建操作继续直接使用 intrinsic；实际接口调用通过既有装箱、slot 和 GC effect 适配运行。
HIR intrinsic 保存完整物理参数，MIR 复用已有 callable materialization。接口与产物消费使用真实 typed 身份，
没有新增 runtime ABI、依赖、授权、预算或通用框架。新增编译器代码文件最长 158 行，按实际职责分模块。

旧浮点 binary wire tags 6～12 退役，新默认比较与 compareTo 使用 13～19，旧语义明确拒绝；依赖与 core 按现有缓存／产物规则重建。
Float/Double 仍不提供 Hash，函数值不自动实现 Invoke；本里程碑未实现 Map 容器或参数包／泛型虚方法。

## 功能提交

| 提交 | 内容 |
| --- | --- |
| `94c02bed` | 规范、设计与 Add 方案 |
| `45339fb3` | Add、内建实现与 intrinsic 接口入口 |
| `fc116838` | 算术、一元、比较、区间与递归上界 |
| `625a1696` | 索引、调用、component 和复合赋值 |
| `4ea6e76e` | Equals 统一与 Unit 普通成员 |
| `6de0d23b` | 浮点默认全序与显式 IEEE 四态 |
| `bf30c869`、`d83d0ca0`、`6aa495bd` | 审阅后的 Darwin、GNU、musl 优化与 ABI 期望 |
| `1a99cfbc` | Linux child 环境测试按 libc 检查 REALGCC |
| `459be2fe`、`524603ee` | Unit 迁移后的 default callable 与派生导出测试 |
| `a9b07246` | 补齐 IntRange 带入的四个实际形状依赖 |
| `442518c0` | 五项优化专测的十份 MIR 能力元数据期望 |
| `d35522aa` | 修正导入默认值中按类型生成的派生调用来源 |
| `a82432ca` | Linux 原生链接的 fmod/fmodf 系统导入期望 |
| `2608794d` | 结构相等中浮点字段与非自反用户 equals 的规范澄清 |
| `a507cdf4` | M33 generic/接口 NaN 合约断言迁移 |
| `dd8a5622` | 异常返回路径只锁定六个用户函数的完整 CFG |
| `68393b09` | 三目标 native storage 归档成员期望与复验 |
| `83db88e1` | 浮点求值顺序组合按默认全序更新 NaN 关系期望 |

元数据对象发射还修正了一项实际冷构建开销（`e5ba66a9`）：按既有物理对象序列逐层投影较小模块，
避免逐对象复制全份 LLVM 元数据。本机一次 debug core 冷构建由约 62 秒降至 12.96 秒，新旧 `.slib` 的
SHA-256 完全一致；[记录](validation/metadata-projection.json)说明计时范围。新增投影模块 74 行，未改变对象或 ABI 合同。

## 验证环境与结果

| 宿主 | 环境 |
| --- | --- |
| 本机 | Darwin 27.0.0 arm64，Rust 1.99.0，LLVM 22.1.8，Python 3.15.0 |
| nuc12 | Linux 7.0.0-38-generic x86_64，Rust 1.99.0，LLVM 22.1.2，GCC 15.2.0、musl-gcc，Python 3.14.4 |

所有 scratch、日志和临时目录均在 `target/m41/`，命令设置 `TMPDIR`，未使用 `/tmp`。
Linux native unwind 安装保留复用。正式 CLI 使用 release 构建；测试源码中的 debug/release profile 均由 fixture 声明覆盖。

| 范围 | 通过 | 不适用／忽略 | 覆盖来源 |
| --- | ---: | ---: | --- |
| Linux Rust workspace 完整覆盖 | 5,584 | 0 | [汇总](validation/rust-linux.json)、[逐项结果](validation/rust-linux-coverage.json.gz) |
| GNU 正式 CLI 完整覆盖（全量＋定向补验） | 3,785 | 30 不适用 | [报告](validation/linux-gnu.json)、[日志](validation/linux-gnu.log.gz) |
| musl 正式 CLI 完整覆盖（全量＋定向补验） | 3,783 | 32 不适用 | [报告](validation/linux-musl.json)、[日志](validation/linux-musl.log.gz) |
| Darwin 分功能定向 CLI，最终同名结果去重 | 177 | 1 不适用 | 230 变体、825 进程、24 次 golden；[覆盖](validation/darwin-coverage.json)、[原始报告](validation/darwin-reports.json.gz) |
| 公共 fixture runner unittest | 每台宿主 47 | 0 | 本机与 Linux 各通过 |

GNU 最终同名结果去重覆盖 4,271 个变体、15,963 次进程、70 次 golden；29 项初次失败均以定向复验闭合。musl 对应为 4,269 个变体、15,959 次进程、70 次 golden；5 项旧结果均以定向复验闭合。两份全量原报告均 complete，选中全部 3,815 项，没有未选中或中断项。

M41 专项共 46 个正式 fixture，其中 2 项从 M30 迁移，净增 44 项。
本机专项全部通过：70 个变体、238 次进程；GNU/musl 全量中的 46 项也全部通过。独立与组合用例覆盖三层 Cone 再次发布、删除源码后的链接、
普通／moving／minor GC、显式接口与泛型调用、alias、闭包、求值顺序、写回副作用及准确负例。
浮点矩阵覆盖两种精度的 18 种特殊位型两两组合，检查六种默认关系、compareTo、IEEE 四态和 signaling NaN 位型保持。

来源修复后，本机完整 HIR 906 项及 HIR lowering 1,429 项通过；
115 个实际 intrinsic 接口入口的参数、GC effect 和 MIR 输出结构检查通过。
两台宿主的 fmt、workspace clippy 与 release CLI 构建通过。本机定向覆盖不计为 Darwin 全量。

## 差异审阅与失败闭合

三目标共更新 20 份专用 LIR 期望（本机 8、GNU 6、musl 6）。变化为新增接口 TD/layout、Equals 身份、
可达类型支持及引用编号；逐份比较确认，归一化 external-fn/external-td/td 编号后函数指令和签名不变。
另外十份优化专测 MIR 已逐函数审阅：八份函数正文完全不变，M31 两份仅外部引用编号一致平移；debug/release、跨库消费及 GC 覆盖保持。
另更新三份 native storage 物理成员清单：66 → 65；实际 MIR 仍含 Wide/指针相等及六个 native callback bridge，三目标普通比较复验通过。
本机普通比较复验通过；Linux 完整验收使用普通比较模式，不在全量验收中更新快照。

Linux Rust 原有四项旧断言按已交付语义修正：GNU 不注入 musl 专用 REALGCC；Unit 不再作为派生比较引用和导出；IntRange 新增能力带入 Contains/Iterable/Iterator/Option 的具体形状。
保留实际字段、环境、产物读写、全部 callable 分类及结构派生数量检查。
Linux 原生链接 19 项旧计划另有系统导入数变化：Rem 接口入口使既有 fmodf/fmod 依赖可达。40 份 GNU/musl 计划仅调整这一计数，其余原生输入、归档成员与绑定保持；GNU 普通比较补验全部通过（34 变体、195 进程、35 次计划检查），musl 完整运行中的对应 19 项也全部通过。
GNU CLI 另暴露四项真实编译失败：导入默认值中的 tuple 派生 helper 调用普通 Unit.equals 时，依赖调用来源检查遗漏了现有按类型生成函数的来源规则。按 spec 补齐 typed DerivedEquality/NoSubstitution 识别，所有位置与上下文投影仍检查；新增 Rust 回归及全部 20 个源码默认值正式 fixture 在本机通过。GNU、musl 同范围补验各 20 项全部通过；Linux 补验使用独立构建目录，未替换仍在执行的全量二进制。最终标准 target/release 的三个工具与补验工具逐字节相同，见[工具记录](validation/linux-final-tools.json)。
GNU CLI 另发现旧 M33 generic/接口合约程序中的四个 NaN 不相等断言，按新默认语义修正；用户自定义非自反 equals 仍受测试。
GNU CLI 另发现旧异常返回路径整模块 MIR 快照。六个用户函数 CFG 不变；现逐字节检查完整函数片段，删除无关 core 元数据和打印函数正文快照。
两项其他旧期望分别为 `NaN >= 0.0` 的结果和 native-storage 归档成员数；
二者经实际语义／产物审阅后修正，并以普通比较模式定向通过。最终两个 Linux 汇总均取每个用例的最新结果，原始全量及补验报告分别保留。
scoop 包与两个 driver 用例先分别补验；元数据投影修正后重新覆盖全部 codegen 与 driver；另定向补齐 main、实际二进制能力和 doctest 入口，其余未变 crate 复用已完成的 workspace 覆盖。来源修复后完整重测 HIR 及 HIR lowering 两个 crate，最终共覆盖 5,584 个不同 Rust 测试。

GNU ABI 首两轮与大型 Rust 测试并行时，冷构建触及原有 300 秒限制。保留日志，独立完成正常 core 构建、
复用构建缓存并调整 CPU 调度后，两个 ABI fixture 通过；未修改测试时限。
后续私有缓存用例也触及时限，因此修正上述重复 LLVM 模块复制。旧 GNU 运行保留 102 项通过、15 项超时、3,668 项中断与 30 项不适用，作为诊断历史；最终验收使用新二进制重跑完整 CLI。

## 清理与复跑

最后一批清理的 `target/m41` 占用（`du -sk`）：本机 314.9 MiB → 8.3 MiB，Linux 13.14 GiB → 6.1 MiB。已删除结束用例、重复 core／工具构建、缓存、私有 sysroot 和临时目录；此前多轮清理单独记录。保留日志、报告、可复用 Cargo 依赖、当前 release 工具及 `sysroot/native` unwind 安装。数字仅表示最后一批目录占用，不能作为整个里程碑的物理回收总量；详见[清理记录](validation/cleanup.json)。

从仓库根目录执行，先建立 `target/m41/tmp` 并设置 `TMPDIR="$PWD/target/m41/tmp"`。
LLVM 前缀：本机 `/opt/homebrew/opt/llvm@22`；Linux `/usr/lib/llvm-22`，并将 `$HOME/.cargo/bin` 加入 PATH。
本轮使用 `CARGO_INCREMENTAL=0`、Linux `CARGO_BUILD_JOBS=6`。

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets
cargo test --workspace
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-gnu --jobs 8 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m41/linux-gnu
python3 tests/run_fixtures.py --all --target x86_64-unknown-linux-musl --jobs 8 \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --work-dir target/m41/linux-musl
```

本次 Rust 按已完成覆盖续跑：`cargo test -p scoop`、`cargo test --workspace --exclude scoop --no-fail-fast`，
然后定向补验 driver 用例；元数据投影修复后重跑 `cargo test -p scoop-codegen` 与 `cargo test -p scoopc -- --test-threads=6`，来源修复后执行 `cargo test -p scoop-hir -p scoop-hir-lower -- --test-threads=6`。正式 CLI 前预构建 GNU/musl 的 debug/release core 并复用正常 cache；
针对 core 重建的用例仍实际执行其源码变更与构建，不以普通 core 缓存替代这些检查。
