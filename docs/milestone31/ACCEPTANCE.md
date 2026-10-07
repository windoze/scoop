# M31 验收记录

日期：2026-10-07。状态：主要功能与三个 target 的功能验收已完成，性能测量进行中。分批实现见 [PROGRESS.md](PROGRESS.md)，范围见 [DESIGN.md](DESIGN.md)。本记录只把普通模式的实际通过结果计入最终验收；快照生成与迁移另行记录。

## 1. 交付与版本

debug/release 贯通正式 CLI、child 请求、producer 和缓存；Scoop machine 与 generated-C 分别使用 O0/O2，runtime 保持 O2。局部整数/复制/已知 variant 清理位于根计划之前，LLVM 函数内优化之后形成实际 GC/EH 发射计划，再运行 RS4GC；真实根发布与回写继续保留。

ODR 在普通依赖语义一致时按 typed key 与 ABI 选择实现，正文与自身 GC/EH/Context/登记数据同选。候选 image 退出最终链接，linker 根据实际所选登记生成各逻辑 Cone 的最终 image。专用 definition fingerprint 及其独占编码、补丁和重复校验已退役。

普通对象使用跨 line 的连续分配，引用写入按完整范围原子标卡；1 MiB nursery 在实际 refill 时触发 minor，第一次存活即晋升旧代。大对象与带 hook 对象直接进入旧代，pin block 原地晋升，空间预留失败在原图完整时转 full。显式 collect 保持 full，normal minor 不遍历完整旧代。

| 合同 | M31 当前值 |
| --- | --- |
| child protocol / optimization field | 4 / request field 9；Debug=1、Release=2 |
| metadata ABI / runtime ABI | 5 / 9 |
| stackmap | LLVM v3，managed ref、对象头和 TD 表示保持 |
| LIR identity-foundation | 6 |
| LIR strong-production / cone-production | 21 / 10 |
| LIR cross-cone-layout-abi / link-identity-closure | 10 / 15 |
| cross-cone-layout-link-closure | 5 |
| manifest single-cone-production | 5；mode field 12，definition 表 field 5 退役 |
| backend GC/EH contract | 必需 field 28=1 |
| 编译 / 链接 / LIR semantic / RuntimeImage 域 | compile-cache-v2 / resolved-link-plan-v3 / lir-semantic-v2 / runtime-image-v2 |

具体字段、tag 与兼容范围见[实现规范 2.19](../specs/SCOOP-IMPL-SPEC.md)。旧产物与缓存需要重建。

## 2. 环境

| target | 实际环境 |
| --- | --- |
| aarch64-apple-darwin | Apple M3 Ultra，Mac15,14，32 核、512 GiB；外部 LLVM 22.1.8 |
| x86_64-unknown-linux-gnu | OrbStack 的独立 Debian 13/amd64 容器，amd64 模拟执行；GCC 14、LLVM 22.1.8 |
| x86_64-unknown-linux-musl | 同一独立容器的 musl-gcc，覆盖 static 与 PIE |

容器为 `scoop-m31-linux`，CPU 配额为 8 核、内存限制为 16 GiB；仓库只通过独立 `/build` 保存 Linux 构建与报告。用户的 `nuc12:~/repos/scoop` 保持原状；SSH 后续持续超时，因此本批 Linux 收尾使用上述容器，不称作 NUC 原生运行。此前可连通时完成的 Linux 批次仍在实施记录中保留。

三个配套命令为 `scoop`、`scoopc`、`scoop-link`，最后的生产提交为 `c3ea0d7d2`。收尾工具包含优化后稀疏 safepoint ordinal、NoGC invoke 的 `nomerge`、InstCombine 的 `no-verify-fixpoint` 和 AArch64 局部跳转表修正；runtime 同时记录实际复制字节数。每轮在启动前构建配套工具，不在活跃 fixture 中替换它们；生产代码未变化时复用已通过结果。

## 3. 已收集的结果

- Darwin Rust workspace 完整运行共 5325 项：5321 项通过，4 项旧固定向量/分区数量断言失败。已保留成功结果，4 项修正后全部定向复测通过；局部常量池另增 3 项测试，当前 5328 项均有通过结果，没有忽略测试。
- 独立链接的 36 个组合已通过，57 个变体、272 个正式进程、280 份阶段快照，见 [M31-DARWIN-LINK.json](M31-DARWIN-LINK.json)。
- GNU/musl 的 M31、浮点、Context、Linux ABI/动态库及受影响 native/IR 组合各选择 127 个用例。GNU 的 112 个适用项全部普通通过，15 项不适用；musl 的 115 个适用项全部普通通过，12 项不适用。两组均包含九个 M31 用例，覆盖 debug/release、minor/full moving 和 artifact-only。合并后的 GNU 覆盖为 153 个变体、630 个正式进程、641 次快照检查；musl 为 156 个变体、646 个进程、645 次快照检查。逐项结果见 [GNU](M31-LINUX-GNU-ACCEPTANCE.json) 与 [musl](M31-LINUX-MUSL-ACCEPTANCE.json)，生成快照的实际执行结果不计入通过。
- Linux codegen 额外完整运行 351 项：350 项通过，栈增长测试遗漏浮点环境入口的链接依赖；修正后只复测该测试，GNU PIE、musl static/PIE × O0/O2 六种组合均通过；同时以有效窄缓存强制四条入口刷新，覆盖预先映射整个主栈的环境。当前 351 项验收闭合。
- Darwin/Linux 公共 fixture runner 各 44 项通过。每批 Rust 先 fmt 和 workspace clippy，Python 按 Ruff 0.16.10 格式化与 lint，C 使用严格告警。
- 优化边界的定向验证另有 foundation 关系 12 项、EH 19 项、普通优化与实际根计划 6 项；实际复制统计通过 5 项 nursery 回归。Context/协程的 debug/release 与 moving 组合已在三个 target 普通模式通过。
- AArch64 局部常量池新增 3 项测试，另有 9 项符号/重定位编码回归通过；Darwin 真实 release 迭代与 moving fixture 通过，浮点两种 profile 也再次通过。

Darwin 正式 suite 完整发现为 2559 项，其中 7 项按平台不适用。完整轮先保存结果；发现旧计划/符号/LIR 快照后，仅续跑未通过项。普通语言 fixture 保留 actual build/artifact-only link 一致性、Cone/image/root、符号、阶段结构和运行，不重复冻结整个 core 的物理对象数量与 startup C。专门的 native 选择测试保留完整计划与动态绑定断言。

第一轮保留 26 个通过项；续跑其余 2526 项得到 2026 个通过项、500 个失败项，其中 490 项停在旧快照。修正后仅选择尚未通过的 492 项复验，454 项通过；后续 38 项通过 10 项，再复验剩余 28 项及一个受影响的浮点用例，最后 29 项全部通过。多步 fixture 在先前失败处之后尚未执行的断言逐项补齐，没有重新执行完整 suite。

按当前 fixture 名称逐项核对，Darwin **2552 项适用 fixture 全部通过，7 项不适用**。合并后的实际覆盖为 2663 个变体、12646 个正式进程、11359 次阶段/计划快照检查；每个 fixture 的普通模式证据与各轮计数见 [M31-DARWIN-ACCEPTANCE.json](M31-DARWIN-ACCEPTANCE.json)。该表只统计每项最终采用的通过轮次，不把重试累加为额外覆盖。

Linux 保留此前 71 项功能组合的有效通过结果，再补受影响的目标输出与 native 边界。54 项补充选择中 12 项按 target 不适用，其余 42 项分为 19、14、9 三组，在 GNU 和 musl 都以普通模式全部通过。快照生成时发现完整源码图 dump 的实际构建超过旧 120 秒限制，相关构建步骤改为 300 秒，运行、阶段结构与链接断言保持；复验使用已发布的真实缓存目录，未重复运行 Linux 全量。报告中的 `complete=false` 保留“没有使用完整 suite”的含义，`selection_complete=true` 表示本次所选范围全部闭合；未变化的平台不适用分类直接复用。

快照迁移保留 typed identity、引用、符号名称和完整阶段结构。45 份 HIR 变化逐项核对局部 ImportedIdentityId 序号，AST/MIR 没有本批语义变更；LIR 保留实际常量传播、分支删除和根计划。Darwin 符号取自已生成程序的真实 nm 输出；Linux 的共同 core/runtime 差异从实际 GNU/musl 程序建立，匹配的局部 atom 删除依据实际 Darwin 输出迁移，同时保留 Linux 实测仍存在的目标专属 stackmap。Linux 受影响的 native、指针、序列化和优化边界用例另外生成目标快照并普通复验，不将此表述为重跑了全部 Linux fixture。

## 4. 复现

```sh
export LLVM_SYS_221_PREFIX=/opt/homebrew/opt/llvm@22
cargo fmt --all
cargo clippy --workspace --all-targets
cargo build --release -p scoop -p scoopc -p scoop-linker --bins
python3 -m unittest discover -s tests/fixture_runner/tests
python3 tests/run_fixtures.py --all --work-dir target/m31-verify \
  --scoop target/release/scoop --scoopc target/release/scoopc \
  --scoop-link target/release/scoop-link --llc "$LLVM_SYS_221_PREFIX/bin/llc" --jobs 12
```

定向验收用 `--filter` 选择实际受影响名称或 tag；Linux 指定 target 与实际 LLVM 22.1 工具。每次使用新工作目录，可复制普通 cache 目录复用产物，不能用符号链接替换 cache 根。debug/test 构建使用 `opt-level=1`、关闭增量以控制时间和 target 空间，保留 debug assertions 与溢出检查。性能工作负载及四组对照入口见 [benchmarks](../../tests/benchmarks/README.md)。
