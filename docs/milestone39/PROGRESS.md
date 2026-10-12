# M39 实施记录

基线：`e88f7d73`（仓库历史压缩后的实际 HEAD），2026-10-12 开始。工作区初始干净，无 target 目录。

## 顺序与当前状态

1. 已同步三份规范与路线图：明确布局／签名准备、codegen ABI 规划、完整 LIR 完成，typed 身份与消费者支持边界，工具执行及 runtime 组件职责。
2. 已完成：C toolchain 宿主 invocation 移出 LIR，generated-C 编译交由 driver 编排。
3. 已完成全部 Scoop/C ABI 规划迁移、显式 manifest 目录、布局／LLVM 配置登记及 target/backend/C toolchain 身份拆分。
4. 已完成 runtime 组件、ELF 容器／机器规则及 Linux machine／libc 工具链选择。
5. 本机定向回归已通过；待完成 Linux Rust 与正式 CLI 完整覆盖、最终边界审查及清理。

## 验证环境与纪律

- 本机使用 LLVM 22.1：`/opt/homebrew/opt/llvm@22`；临时文件与日志放在 `target/m39/`，设置 TMPDIR 指向该目录下的 tmp。
- Linux 环境为 `nuc12:~/repos/scoop`。用户已明确说明远端无有用文件，可直接重置、同步及清理；保留现有 native unwind 安装以避免重复构建。
- 每批 Rust 变更先 fmt、workspace clippy，再定向测试；正式 CLI 复用已有真实功能组合，避免重复全量测试。
- 当前仅记录历史验收为参考：M40 三 target 完整 fixture 已通过；本次尚未执行的测试不计为通过。

## 功能 1：C toolchain invocation 与编译执行归属

- `ValidatedCBridgeToolchainInvocation`、宿主路径检查、命令与环境移入 toolchain；按 invocation、command、tests 拆分，单文件最大 257 行。
- driver 持有 generated-C 的实际编译动作；codegen 复用现有发射流程渲染源码、完成对象边界，不依赖 toolchain 实现 crate。LIR 只保存持久化合同。
- 已通过本机 fmt、workspace clippy；工具链 7 项、generated-C 对象 4 项、持久化合同与固定指纹 6 项测试；debug/release 三个 CLI 均构建成功。
- 真实 CLI 选择 errno 独立／组合／跨 Cone、callback 及 global/TLS，共 24 项；本机全部通过（34 变体、132 进程、10 次 golden 检查），没有更新期望；报告见 [01-darwin.json](validation/01-darwin.json)。
- 清理首次构建的 `target/debug/incremental`，释放约 6.20 GiB；保留编译二进制、依赖库、私有 fixture cache 与日志。

## 功能 2：runtime 的 OS、image 与 arch 组件

- 目标构建按实际 OS、image、arch 组件组合源码、flags 和 includes；源码清单与构建选择分别为 109／92 行。
- 浮点环境初始化移到两个现有架构的专用 C 文件；通用 `floating.c` 不含架构宏或机器寄存器操作。`-mno-red-zone` 属于 x86-64 组件，ELF visibility／SHA 配置属于 image 组件。
- 保留原 `scoop_float_init_environment` 入口与 fesetenv／FPCR／MXCSR 行为，不改公共 runtime ABI。移植后的纯 frame decoder 仍能在其他宿主运行结构测试。
- 本机 fmt、workspace clippy、22 项 toolchain 测试、29 项 runtime collector／线程／初始化回归通过；release CLI 构建成功。
- 浮点环境扰动恢复、值类型、FFI、协程、JSON 及跨 Cone 6 项 CLI 全部通过（9 变体、54 进程），见 [02-darwin.json](validation/02-darwin.json)。无快照更新。

## 功能 3：消费提供方的物理 ABI

- 外部 callable 直接将提供方的 canonical ABI 绑定到消费者本地存储与类型，不再先重新分类后比较；保留参数数量、尺寸／对齐、carrier 范围及既有 exact signature、GC root 协议校验。
- 删除重复的 ABI 比较模块；存储绑定与测试分开。新增隐藏 sret 导致 SysV aggregate 整组回退、后续浮点参数和短 carrier 的精确测试，并覆盖错误存储诊断。
- 本机 fmt、workspace clippy、476 项 LIR 测试、6 项 lowering 外部调用测试、3 项 codegen 外部调用测试通过；release CLI 构建成功。
- 小值跨库产物、suspend member、泛型 release 及 native aggregate release 四项正式 CLI 通过（5 变体、40 进程、4 次 golden 检查），无期望更新，见 [03-darwin.json](validation/03-darwin.json)。

## 功能 4：Linux 工具链的机器与 libc 事实

- 将实际支持的 Linux machine 与 libc 选择集中到工具链边界；编译 probe 和最终 executable 验证读取选中机器，loader 名称由 machine／libc 对决定。
- GNU 使用已选择的实际 GCC；musl wrapper 使用显式 REALGCC 或 PATH 中的 gcc，不再额外要求架构前缀命名的 GCC。仍通过真实编译验证 LP64、libc headers 与 ELF machine，不放宽支持矩阵。
- glibc 动态 PIE／musl 默认静态及显式动态的策略保持；缓存／产物 wire 未变。
- 本机 fmt、workspace clippy 与 23 项工具链测试通过；新增 GCC locator 回归涵盖直接 compiler、不含交叉前缀的 PATH、musl 显式 override 与缺失 driver。Linux 编译／最终镜像的现有集成测试留到完整 Linux 验收运行。

## 功能 5：ELF 容器与机器规则

- codegen 的 ELF machine、stackmap/EH relocation 和 return-address register 通过已选机器规则读取；AArch64 的未实现 ELF 组合在规则选择处拒绝，不再推断 AArch64 必定属于 Mach-O。memory libcall 识别同时检查真实 machine 与 relocation。
- slib 在对象读取边界选择 ELF 机器并检查 header flags、relocation 宽度；envelope 复用已检查的 flags。最终 linker 用相同机器规则解释 PIE relative pointer relocation，保持无 LLVM 依赖。
- 原本限定 x86-64 的已验证及观测 RELA form 明确命名为 ElfX86_64Rela，指针／调用／TLS 消费无需把 ELF 容器当作机器。既有 wire tag 11 与字段编码保持，固定向量通过。
- 本机 fmt、workspace clippy；5 项跨目标真实 LLVM ELF 对象测试（含 machine／flags 损坏拒绝）、21 项机器／EH artifact 测试、39 项 slib relocation 测试与 32 项 wire 测试通过。没有新增架构或更新 golden。
- 再次清理本机已完成 fixture 的 cases，保留 report/cache；已清理远端旧 target（初始约 21 GiB），仅保留重新建立的 M39 临时目录，native unwind 安装未动。

## 功能 6：显式的 manifest 目标目录

- manifest 解析入口接收 SourceTarget 数据目录，源码／native source／native library 条件共用该输入；不再读取 identity 的支持列表或隐含宿主目标。
- toolchain 作为 driver 共用入口，明确提供当前三个 target 的 os／arch／env／triple。谓词保存匹配到的 typed identity 与 canonical triple；按 triple 排序，保持现有编码及目录重排稳定性。
- 本机 fmt、workspace clippy、45 项 manifest 与 5 项编译缓存键测试通过；新增子目录支持范围与目录顺序不影响编码的验证。
- 34 项源码选择、非法条件／路径、symlink、选择后缺失源码及缓存正式 CLI 全部通过（35 变体、45 进程），见 [06-darwin.json](validation/06-darwin.json)，未更新 golden。

## 功能 7：由后端统一规划全部物理调用 ABI

- driver 显式编排 `prepare → codegen::plan_abi → complete`。准备状态独立于完整 LIR Module，复用已完成的类型布局、C 合同和依赖 ABI；正文只构造一次。
- Scoop、DirectC、HFA／eightbyte、SysV 整组回退和 C 扩展规则移到 codegen 所属的 LLVM-free `scoop-abi-plan`；lir-lower 只在测试依赖中使用该规划器，生产 stage 之间仍仅经 IR 数据通信。codegen 的 C 计划校验复用相同目标规则。
- 计划覆盖函数／Scoop extern、interface receiver、closure／function bridge／suspend continuation、errno bridge、root／startup gateway、release 与 runtime／safepoint；Scoop extern 规范合同与实际声明共用物理计划。删除旧 LIR／lowering 分类器及重复 runtime 原型构造。
- 聚合准备按 typed storage ID 共享字段节点，保留 tagged payload 的完整整数存储描述；后端仅展开可能直接传递的聚合。新增大型 payload 不按字节展开、sret 与前序参数导致整组回退、后续 SSE 容量保留、ZST／扫描存储等验证。新增模块最长 192 行。
- 本机 fmt、workspace clippy 通过；5 项 ABI planner、154 项 LIR lowering 测试通过，迁移过程中的 473 项 LIR 数据测试通过。首次回归发现的 Alloc machine-byte-size 与非法 C 类型检查顺序已修正，没有修改任何 golden。
- 32 项正式 CLI（43 变体、179 进程、6 次 golden）通过，覆盖 native／Scoop 小值、跨 Cone、接口默认方法、闭包、suspend、context 及 errno；字段节点共享调整后又定向复跑全部 5 项小值 CLI（10 变体、50 进程、6 次 golden），全部通过。报告见 [07-darwin.json](validation/07-darwin.json) 与 [07b-darwin.json](validation/07b-darwin.json)。

## 功能 8：静态目标合同与 LLVM 配置登记

- LIR 只保留完整布局、backend 合同与 canonical 编码；当前三个 target 的 data layout、CPU/features、C profile 构造集中到 codegen 所属的 `scoop-target-data`。该静态模块不含 LLVM、ABI 分类器或工具执行，供真实 artifact-only 消费者复用。新增源文件最长 72 行。
- backend identity 改为 canonical name/version 数据；codegen 明确拒绝未知身份、修改后的 CPU 合同及不匹配的目标组合。slib 兼容性读取同时核对所选 target/backend 身份与各自指纹，拒绝未知身份搭配已知指纹。
- 本机 fmt、workspace clippy；344 项 identity、473 项 LIR、23 项 toolchain、9 项兼容性读取、5 项 backend 选择测试通过。既有布局／backend／C toolchain 固定向量未改。
- release 三个 CLI 构建成功；5 项小值／跨库／native 正式 CLI 通过（10 变体、50 进程、6 次 golden），见 [08-darwin.json](validation/08-darwin.json)。已清理 9.27 GiB debug incremental。

## 功能 9：C producer 身份与工具执行支持范围

- C toolchain typed identity 仅校验 namespace、名称和版本；实际三个 producer 名称集中在目标数据模块，删除 identity 中未使用的 backend 专用构造器。
- invocation 边界明确校验所选 target 的 producer 身份，拒绝格式合法但未知、版本不支持或平台不匹配的 profile。纯 LIR profile 与生产记录继续保留原编码。
- 本机 fmt、workspace clippy、9 项身份格式、14 项 C 合同及 3 项 invocation 测试通过；固定指纹未变。正式 CLI 在完成剩余 target 身份迁移后合并回归。

## 功能 10：native 符号归一化由目标数据显式提供

- 将 native symbol normalization 作为纯数据传给身份构造；删除身份层隐含 Darwin 目标的符号、library 和 layout 构造器。对象前缀不再由 identity 查询 target 名称。
- identity 只检查身份和符号字节格式；slib 在已有 native 目标消费边界按源码重建并核对完整合同，保留 Mach-O／ELF 前缀不匹配拒绝。新增回归同时覆盖两种真实目标的正确及错误归一化方式。
- 本机 fmt、workspace clippy、345 项身份测试、1 项 native 消费边界、4 项 native lowering 与 2 项 native global codegen 测试通过。固定符号身份与 library 编码未变；Linux 专有 bridge 测试留到 Linux 全量验收。

## 功能 11：target 身份格式与消费者支持范围

- `TargetProfileId` 只保存 canonical 名称／版本；删除 identity 的 ALL、OS／arch／libc／triple／对象前缀及具体目标构造器。三个实现身份登记在 `scoop-target-data`，manifest 和 native 预处理读取 driver 的显式来源目录。
- codegen、ABI planner、对象／stackmap reader、C/runtime toolchain 与缓存分别明确拒绝未知或不匹配的目标。driver 区分 ABI 规划错误与 lowering 错误；runtime 保存选中的完整组件，final-link 复用 invocation 保存的目标描述。旧缓存标签仍由缓存消费者编码，不改变 wire。
- target wire identity 用共享不可变数据保存，避免身份开放后放大错误值和重复克隆字符串；LIR 仍只存完整描述，不依赖任何目标登记模块。新增 ABI planner 目标选择每批请求只执行一次。
- 本机 fmt、workspace clippy 通过。定向测试：345 identity、473 LIR、154 lowering、45 manifest、567 slib、25 toolchain、6 ABI planner、6 codegen target、9 cache、4 driver pipeline，全部通过。固定编码／指纹和负向诊断未更新。
- release CLI 构建成功；源码选择／缓存、非法条件、错误目标 core、errno callback／跨库、小值 native／artifact 组合共 58 项通过、1 项不适用于本机（67 变体、155 进程、6 次 golden），见 [11-darwin.json](validation/11-darwin.json)。
- 生产依赖树确认 `scoop`、`scoop-linker`、`scoop-toolchain` 均不包含 `scoop-codegen`、`scoop-abi-plan`、inkwell 或 llvm-sys；公共 identity/LIR/lir-lower 中的具体 ABI 搜索仅剩数据注释和 provider-storage 测试名称。

## Linux 验收修复：对象测试文件名

- 首轮 workspace 验证完成 scoop、ABI planner、AST 及 codegen 的 507 项测试；其中 codegen 的 terminal poll 测试因 target Debug 文本包含 `/`，在写对象时失败，其余 506 项通过。
- 对象文件名改用 canonical target 名称，保留三个真实目标的 debug/release LLVM 发射和机器站点检查；搜索未发现其他将 profile Debug 文本作为文件名的用法。
- 本机 fmt、workspace clippy 与该测试通过；Linux 补验失败项后，继续尚未执行的 workspace 包及 doctest，不重跑已通过的真实构建图与 codegen 测试。
