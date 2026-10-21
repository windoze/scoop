# 实施与验收记录

基线：`4de3ae9e8ac3f18bc63eba4947afd8bb87ea34db`。设计和边界见 [DESIGN.md](DESIGN.md)，分步计划见 [PLAN.md](PLAN.md)。功能与测试清理已实现并分批提交，Darwin ARM64、Linux GNU 和 Linux musl 的验收均已闭合。

## 实现结果

| 范围 | 最终行为 | 主要提交 |
| --- | --- | --- |
| package/group | manifest 要求 group 本身或其子包；仅 core/std 豁免。单文件合法 package 警告后忽略，始终使用 RootPackage | `912841170` |
| 新建 Cone | executable/library 固定模板，支持 group/name/version/package 和 `--no-tests`，已有目标不覆盖 | `339eac72e` |
| shebang/runner | 首字节 `#!` 作为普通注释，保留位置；单文件支持无分隔符的位置参数，并保留 `--` 和默认 Cone 语义 | `3d824fab`、`058e6c5f` |
| 单文件依赖 | `//! dep:` 只在头部生效，始终直接依赖 core/std，显式依赖只追加；贯通图发现、scoopc、缓存与链接 | `00769dc0d` |
| GC stress | `--enable-gc-stress` 编入能力，再由环境变量选择；默认产物不包含 stress 支持及对应环境读取 | `9417c771` |
| 测试职责与模式 | 普通测试使用 release 和正常 GC；普通库行为/consumer 使用 Cone tests，编译器与 GC 专测保留必要组合 | `3eb63e241`、`e5a397876`、`3ceedf9db` |
| 目录与去重 | 按功能、编译器规则和回归组织；合并普通 negative，分离 ABI/GC 边界 | `5e5f5c414`、`e38ebbe35`、`49c103f17`、`0b539af1d` |
| 资源复用 | 直接使用准备好的 sysroot；cache 独立于 work-dir，跨 fixture/运行共享；内置测试读取仓库当前源码 | `ee637febb`、`e06840a93`、`3fc2ebce0` |

规范先行，新增 CLI、package、单文件及 runtime 合同均已同步到三份权威 spec。产品默认 debug 行为保持。准备脚本显式提供普通/stress runtime，普通构建不自动重建系统资源。

## 覆盖变化

- fixture 从基线 4,532 项，加 14 项新覆盖，迁出首轮 111 个 API 场景，合并 77 个普通 negative，再迁出 154 个 consumer 和 4 个重复终端用例，最终 4,200 项。目录重新分类不改变总数，所有 Scoop 输入均通过正式 discovery 归属检查。
- 当前分类：language 3,415、libraries 71、driver 297、compiler-correctness 345、gc-correctness 59、regressions 13。libraries 中保留 5 个测试命令入口与 66 个外部协议/native/进程边界；消费生产库本身不是保留理由。
- 154 个 consumer 场景形成 162 个测试方法，覆盖 16 个 Cone；库测试总数由首轮 1,102 项增至 1,264 项。保留原断言、并发与清理行为；临时输入由测试自身管理，新增依赖仅进入 test-dependencies。详细对应见 [LIBRARY-TESTS.md](LIBRARY-TESTS.md)。
- 清理过程中累计去掉 334 个 fixture 中的 369 个重复系统库 build 步骤及 132 个无关 warm build 步骤；GC、ABI 和回归也默认共享缓存。21 个修改 core 的专测保留 std 重建步骤并写明原因；它们继续共享缓存。修改输入、发布、工具选择或缓存本身的专测按被测事实保留隔离资源。
- 两个 HTTP 用例在不同 work-dir 复用同一缓存，耗时分别从约 70.1 秒降至 1.8 秒、24.8 秒降至 0.7 秒。源码、工具和预编译资源始终存在；数据仅代表这两个本机用例，不能推算全套耗时。随后它们的普通行为也迁入 Cone 测试。

## 验证范围

Rust workspace 的 5,136 项完成首轮及失败修复后的定向验证；路径搬迁相关 MIR 另有 152 项通过。Rustfmt、workspace clippy、Python Ruff 与 55 个 runner 单测通过。本轮后续改动限于测试源码、fixture 声明和文档，未重复无关 Rust 全量检查。

当前 4,200 个 fixture 的有效覆盖按名称合并既有结果与后续复验，退役项不计入，重复执行不重复计数：

| 目标 | 通过 | 平台不适用 | 未解决失败 |
| --- | ---: | ---: | ---: |
| Darwin ARM64 | 4,183 | 17 | 0 |
| Linux GNU | 4,167 | 33 | 0 |
| Linux musl | 4,164 | 36 | 0 |

首轮 Darwin 的 4,435 项 fixture 闭合为 4,418 项通过、17 项不适用。77 个失败和 1 个配置错误主要涉及迁移后的 import、身份/位置期望、模式归类和平台工具路径，均经对应修复与定向复验闭合；没有全量重录快照。按当前 4,200 个用例名称剔除退役项并覆盖后续定向复验，Darwin 最终为 4,183 项通过、17 项不适用。三目标原有 19 个内置/消费者测试入口各 1,102 项已通过；后续新增库测试的结果另列，不把历史数量当作当前集合结果。

迁移后的 16 个 Cone 在三个目标各有 1,235 项 release 测试全部通过，包括 HTTP client 的 178 项；其余三个未改入口沿用各目标原有的 29 项通过结果，每个目标共 1,264 项。Darwin 另完成 113 个内建 expected-error 用例、118 个进程步骤，以及 9 个共享缓存的 HTTP2/GC/回归用例、13 个变体、38 个进程步骤。资源复用改动影响的 323 个本机 fixture 经定向修复后闭合为 321 项通过、2 项不适用；21 项修改 core 的专测已在三个目标全部通过。

GC 开关正式用例的 13 个步骤已在三个目标通过；首轮 Darwin 的 56 个 GC fixture 通过 131 个必要变体、445 个进程步骤，Linux 深栈与 musl PIE 定向场景通过。普通测试未启用 stress，也未重复 debug/release。

Linux 使用 nuc12 的隔离 checkout，保留原仓库。开发 sysroot 的 19 个生产 Cone、两类 runtime 及 native 资源已准备；musl 使用独立的 Linux UAPI include 包装脚本。Linux 工具保持冻结，验收复用每个目标的共享缓存。51 个库边界 fixture 的源码、协议对端和有效行为断言未变，只删除多余构建及冷启动计数检查，沿用既有两目标通过结果；HTTP 认证、缓存生命周期和网络交互代表场景另在共享缓存上通过复验。其余 fixture 增量和此前未覆盖项均已验证，不重复无关的全量检查。

原始日志和 JSON 报告位于任务工作目录，未提交仓库；不使用 `/tmp`。已清理本机 debug target 和完成验收的 case 产物，Linux 分批清理已停止和完成批次的 case/target 产物，保留报告与可复用缓存。磁盘写入争用时降低并发；JSON 命令入口的超时在缓存就绪后单独复验通过。低层链接 fixture 补齐与高层命令一致的工具链参数，三个目标的链接计划与运行断言均通过。
