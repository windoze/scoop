# M63 包管理实施记录

状态：实施中。更新日期：2026-10-28。目标合同见[包管理规范](../specs/SCOOP-PACKAGE-SPEC.md)，阶段完成条件见[设计](DESIGN.md#5-实施步骤与完成条件)。

## 已完成的功能

### 原始包声明与本地 workspace

- 新建独立 `scoop-pm` crate/binary，`scoop pm` 转发 argv、标准流与退出状态，可显式选择配套程序。PM 不依赖 LLVM 或编译 stage 实现。
- `new --workspace` 创建 virtual workspace；`members` 本地展开 root/virtual workspace、显式成员、default-members、`-p`/`--workspace` 选择及共享 Cone 字段/依赖。继承路径以定义处为基准，错误保留定义与引用位置。
- manifest 数据层新增版本要求和 path/Git/GitHub/registry/artifact 声明，复用既有 source/native 解析规则；版本要求覆盖 caret、tilde、交集、pre-release 与精确 build metadata。
- 此批提供本地声明和成员能力。现有构建尚未切到 PM；构建接入、发布及多根编译调度仍待完成，第一、二实施阶段未整体完成。

### 便携锁文件与共享缓存位置

- `Cone.lock` 格式、精确坐标/来源及每个 root/purpose/target 的闭合图已落地；读取检查缺节点、重复包、循环、测试边和不可达节点。
- 锁文件写入按文件锁串行合并所选解析单位，保留其他根/用途，移除已删除成员；失败不替换旧文件，未变化不重写。
- `deps` 可只读输出已锁定图及 JSON；`--why` 的声明原因输出待 metadata 接口接入。
- PM 缓存位置按 CLI、环境变量、`~/.scoop/packages/` 的优先级选择；registry/Git 目录键包含规范化来源。

### Git 与 registry 的共享获取

- 系统 Git 获取默认分支、branch/tag/rev，并按规范化仓库与完整 commit 共享 object 库和 checkout；lock 命中不刷新 ref，离线可复用已缓存源码或对象。
- registry 按需读取单包 JSON 索引，只下载选中的 tar.zst；首次下载校验 SHA-256，解包与 manifest/index 匹配成功后发布共享目录，保留已解析清单供后续准备使用。
- 普通文件锁串行发布同一版本，缓存命中不重读源码或归档计算摘要；来源/摘要冲突和下载、解包错误不覆盖已有包。使用 `ureq 3.5`、`tar 0.4` 和仓库已有的 `zstd 0.14`，不手写 HTTP/归档协议。
- 获取模块已连接 resolve/fetch/update。真实 CLI 覆盖 Git workspace、同仓库 path、submodule 的 gitlink 固定和 selector 变化；编译消费闭环仍待 driver 接入。

### 单版本解析与 resolve/fetch/update

- 使用 PubGrub 0.4 实现每个 root/purpose/target 的单版本求解，候选包含实际来源；锁定优先、全图回溯、pre-release/build metadata、撤回版本和根覆盖遵守同一约束。PM 的 Rust 最低版本为 1.92，driver 不链接该求解库。
- 独立 CLI 支持 resolve、locked fetch、选择性 update/precise 与 offline/frozen；输出版本/来源变化及相关要求。失败保留原 lock，多个根的结果在全部准备成功后一起发布。
- Git lock 增加声明 selector 记录，防止改 branch/tag 后误用旧 commit。有效 lock 直接匹配已有选择；首次 registry 解析获取最新索引，旧选择无法满足新要求时只刷新冲突涉及的来源再回溯。
- path、Git、registry、sysroot 和显式 artifact 均经同一来源目录进入求解；同包不同版本的独立根不合并。registry 在求解成功后才下载归档；同进程多个根共享已取得的 metadata。
- manifest、用户配置和产物摘要按路径、mtime、大小与工具版本缓存。预编译依赖保持精确坐标和 semantic fingerprint，目标不匹配或同坐标产物歧义明确失败；不把源码目录变成构建输出目录。
- 本批完成独立 PM 的解析/获取/lock 流程。driver 与 child 输入、普通构建多根调度、package/静态 registry 维护、LSP/formatter 和发行迁移仍待完成。

### prepare 与展开配置

- `scoop-pm prepare --request --response` 实现版本 1 的 JSON 接口，一次准备全部所选根；成功后原子交付响应，失败保留既有成功文件。共享类型放在 `scoop-protocol`，不引入 PM 实现依赖。
- 每个源码 Cone 交付已绑定精确依赖的生产配置，只有测试根携带测试配置；保留原清单文本与 span，继承字段/依赖定位到成员的继承声明。预编译节点复用现有 `.slib` 摘要；host path 使用无损载体，JSON 解码检查路径编码和格式。
- prepare 记录先检查配置、manifest、lock、artifact 及发现候选位置的更新状态，命中后直接交付已有展开结果。新增 workspace、用户配置、search-root 产物会失效；普通源码正文改变不重新选择版本。锁文件的观察记录在 lock 文件锁释放前取得，避免把旧图与并发更新后的 lock 状态配对。
- 两个项目取得同一 registry 版本时，交付相同共享源码目录；prepare 不复制源码到项目中。本批尚未改变 `scoop → scoopc` 的 machine request，普通构建与 LSP 接入继续在下一批完成。

## 验证记录

- `cargo clippy --workspace --all-targets` 通过。
- manifest 77 项、PM workspace 8 项、PM 真实 CLI 2 项测试通过；PM 依赖树无 LLVM 或编译 stage 实现。
- Scoop CLI 参数转发 4 项、fixture runner 公共规则 58 项及 `pm-local-workspace` 正式 fixture 通过。后者验证真实 `scoop → scoop-pm`、根/成员选择与失败退出，不调用编译器。
- 锁文件批次：全 workspace lint、PM 15 项单元与 2 项真实 CLI 测试通过；正式 fixture 增加 deps 只读输出和缺失测试解析记录，8 次实际进程调用通过。
- Linux 独立环境下，第一批 manifest 77 项、PM 8 项单元及 2 项真实 CLI 测试通过，无 LLVM 依赖。
- 来源批次：全 workspace lint 通过；macOS 的 PM 回归与新增索引错误测试通过，Linux 的 PM 25 项单元与 2 项真实 CLI 测试通过。真实 Git 仓库覆盖 ref 更新/锁定/离线；本地 HTTP 服务覆盖跨项目并发共享、来源隔离、撤回版本、校验/解包失败和离线缺失，不删除源码或工具来验证缓存命中。
- 解析批次：全 workspace lint 通过；macOS/Linux 的 PM 34 项单元、2 项基础 CLI 和 6 项解析 CLI 场景通过，覆盖真实 Git/submodule、HTTP、workspace、override、生产/测试隔离、旧索引/commit 不满足新要求时的刷新，以及 artifact 精确语义与目标检查。正式编译链路验收尚待后续批次。
- prepare 批次：全 workspace lint 通过；macOS/Linux 的 manifest 77 项、protocol 41 项、PM 34 项单元、2 项基础 CLI 和 12 项解析/prepare CLI 场景通过。新增覆盖多根不同版本、继承诊断位置、测试根隔离、预编译摘要传输、共享 registry 路径，以及源码更新复用/新配置与 workspace/新搜索产物失效。真实非 UTF-8 目录由 Linux 覆盖；macOS 文件系统拒绝创建该名称，两端均验证路径载体的无损编码。
- 原始日志与临时工作文件在仓库外。按阶段清理 `target/debug/incremental`，保留当前二进制与依赖构建结果。
