# M63 包管理实施记录

状态：实施中。更新日期：2026-10-28。目标合同见[包管理规范](../specs/SCOOP-PACKAGE-SPEC.md)，阶段完成条件见[设计](DESIGN.md#5-实施步骤与完成条件)。

## 已完成的功能

### 原始包声明与本地 workspace

- 新建独立 `scoop-pm` crate/binary，`scoop pm` 转发 argv、标准流与退出状态，可显式选择配套程序。PM 不依赖 LLVM 或编译 stage 实现。
- `new --workspace` 创建 virtual workspace；`members` 本地展开 root/virtual workspace、显式成员、default-members、`-p`/`--workspace` 选择及共享 Cone 字段/依赖。继承路径以定义处为基准，错误保留定义与引用位置。
- manifest 数据层新增版本要求和 path/Git/GitHub/registry/artifact 声明，复用既有 source/native 解析规则；版本要求覆盖 caret、tilde、交集、pre-release 与精确 build metadata。
- 此批仅提供本地声明和成员能力。现有构建尚未切到 PM；prepare、lock、求解、来源获取、发布及多根调度仍待完成，第一、二实施阶段未整体完成。

### 便携锁文件与共享缓存位置

- `Cone.lock` 格式、精确坐标/来源及每个 root/purpose/target 的闭合图已落地；读取检查缺节点、重复包、循环、测试边和不可达节点。
- 锁文件写入按文件锁串行合并所选解析单位，保留其他根/用途，移除已删除成员；失败不替换旧文件，未变化不重写。
- `deps` 可只读输出已锁定图及 JSON；`--why` 的声明原因链随求解器接入。本批不把锁文件存储能力视为完整 resolve/update 已完成。
- PM 缓存位置按 CLI、环境变量、`~/.scoop/packages/` 的优先级选择；registry/Git 目录键包含规范化来源。下载与解包尚待后续来源模块接入。

## 验证记录

- `cargo clippy --workspace --all-targets` 通过。
- manifest 77 项、PM workspace 8 项、PM 真实 CLI 2 项测试通过；PM 依赖树无 LLVM 或编译 stage 实现。
- Scoop CLI 参数转发 4 项、fixture runner 公共规则 58 项及 `pm-local-workspace` 正式 fixture 通过。后者验证真实 `scoop → scoop-pm`、根/成员选择与失败退出，不调用编译器。
- 锁文件批次：全 workspace lint、PM 15 项单元与 2 项真实 CLI 测试通过；正式 fixture 增加 deps 只读输出和缺失测试解析记录，8 次实际进程调用通过。
- Linux 独立环境下，第一批 manifest 77 项、PM 8 项单元及 2 项真实 CLI 测试通过，无 LLVM 依赖。
- 原始日志与临时工作文件在仓库外。阶段边界清理了一次 `target/debug/incremental`，保留当前二进制与依赖构建结果。
