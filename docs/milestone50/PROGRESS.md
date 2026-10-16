# M50 实施记录

依据 [设计](DESIGN.md)，实施日期：2026-10-17。原始日志与测试工作文件在仓库外 `~/.cache/scoop-m50`；用户原有 M47 文档修改不纳入提交。

## 分批实施

1. 工作文件生命周期：删除 generated-C 源码／对象及 LLVM 对象的只读封存和权限断言；保留实际编译、对象使用和失败清理。
2. CBOR：删除解码后的整包重编码比较、只服务比较的输出限额及解码 trait 对编码 trait 的依赖。现行字段读取、内存边界和 deterministic writer 保留。
3. 诊断源码：删除 source record／context 与 provider 副本的逐项来源证明、专属错误和两项证明测试；保留正常诊断数据和引用。
4. Native 工具输入：include 搜索参数直接交给 C/C++ 工具；删除目录存在性／类型预审，将缺失目录和文件搜索路径改为运行正例，组合覆盖共享头文件与 forced include。删除仅比较 C/C++ 完整版本号／目标名称的配对拒绝及两个伪造文本的 fixture。实际选中源码、工具、头文件、目标和链接失败继续覆盖。
5. LLVM IR：删除生产路径完整 module 验证、自建 RS4GC verifier、专用 pointer witness 和独占 negative tests；保留优化后实际根分组、RS4GC、GC 元数据发射及直接输出／运行回归。
6. Native 对象：删除段名、load command、C++ 符号和可执行栈黑名单及完整 relocation/EH/TLS 复查；删除选择阶段资格包装。支持普通 COMMON 合并和平台强弱绑定；runtime index／缓存规则相应升级。重复 archive TOC 比较及其破坏测试退役。
7. 自产机器对象：删除 codegen 完整机器对象／frame chain／stackmap／LSDA 验证器及独占破坏测试；优化后按实际 invoke 保留 EH atom，删除独立 EH manifest 和仅用于验证的 target 字段。终结调用去重等回归直接断言机器 stackmap 数量。
8. 发布入口：删除无生产调用的旧 single-Cone 资格流水线；C bridge 改为实际成员数据读取，删除 profile/template/flags/unit 和 SDK/tool 版本凭证比对。Code 目录直接投影已生成的 member records，production 投影不深比较计划副本；同步删除独占错误、测试和历史资格要求。
9. Stackmap：以实际符号地址与 LLVM instruction offset 计算返回地址，删除 slib 的第二套 AArch64／x86-64 指令和 frame chain 扫描、member/function/site 覆盖证明及其独占测试；移除两个反汇编依赖。保留实际 section／relocation 读取、根记录归一化和未知引用诊断。
10. Digest 回填：删除六类 registration 结果深比较、coverage 重数、回填后完整记录重建和字节比较；RuntimeImage／root entry 同步清理。仅按实际槽位写入并检查范围，复用对象结构、更新内容摘要。
11. Image／entry 数据：删除 image coordinate、诊断字符串、registration 指针表及 root descriptor 的完整计划回放。直接关联计划与已物化 patch；Executable 分支将计划和槽位放在同一枚举内，删除分离分支的一致性检查。退役专属破坏测试。
12. Callable／safepoint：直接关联 registration 计划与 digest 槽位，删除记录／context key／entry relocation 回放与资格包装。body 指纹统一读取实际 associated atoms，删除 runtime scan 重生成比较、计划／对象副本深比较和已读 relocation value 的重复比较；同步清理 Linux 专用的旧机器码破坏测试。
13. Immortal object：登记 ABI 直接使用生产计划与 canonical shape ABI，删除 String payload、登记字节、指针目标和 digest 图的发布复查，移除专属破坏测试与辅助分支。
14. 静态存储：计划直接关联 scan／layout 槽位，shape 指纹使用既有语义数据，删除初值、scan、空哨兵、登记字节、relocation 与 digest 图的重复回放及专属测试。
15. 初始化：直接关联 Eager gateway 槽位，复用实际 callable body 指纹；gateway 的 body／node／指纹改为单个可选完整值。删除 cell、诊断文本、登记记录和 relocation 回放、生产副本深比较及独占破坏测试。
16. 类型登记：直接关联 descriptor／layout 槽位，按已解析 atom 范围读取 descriptor、diagnostic 与 itable 数据计算实际指纹。删除预期字节重建、relocation／digest 图回放及 requirement 闭包深比较，清理独占错误和破坏测试；共用已有 atom 范围转换。
17. Digest 槽位：统一为 intent／member／offset 数据，宽度固定 32；直接建立查询索引。删除零槽读回、relocation 扫描、graph／覆盖回放及无生产消费者的资格字段，driver 复用 codegen 的成员归属。
18. Code 与归档组装：直接编码已有定义／引用／imports，删除闭包深比较、定义表重建比较和各组件 producer／target 副本检查；数据类型改为实际指纹结果，保留编码与往返测试。
19. generated-C requirements：删除 definition／atom／必需调用／机器 relocation 的 semantic proof 回放及其测试；按实际引用和工具链 registry 分类 helper，复用 bridge 计划处理同名 source extern，Scoop LIR 正常 helper 同样可用。
20. Undefined requirements：直接合并既有分类并保留依赖分区，删除 use 全集重建／coverage／重叠回放、producer／target 副本检查及封口包装；保持实际未知符号和 import 索引诊断。
21. LIR → codegen：删除全模块类型／CFG／ABI／callback／variant 支配／safepoint／根计划预检与专属破坏测试；保留实际发射查询和错误。generated-C 直接渲染现有布局与 bridge，删除 atom／canonical layout 比较，保留真实 C ABI 静态断言；runtime scan 的正向发射测试移入独立短文件。
22. 对象定义：按计划索引实际符号和 atom 范围，删除符号全集／linkage／primary 起点比较、未归属范围与零 padding 扫描；保留缺失和歧义符号、实际范围与 literal 读取。
23. 对象格式读取：Scoop／generated-C 共用按实际属性的 section 分类，删除来源专属白名单、flags 模板、deployment／group 副本、动态符号分区与整表布局审计。保留所需表读取、目标与实际引用边界。
24. 符号属性与引用：读取真实绑定标志，保留未消费符号的普通条目；删除符号描述组合白名单与 undefined 表全集的使用／重名预审，实际引用继续按 table index 解析。
25. Runtime metadata 发射：直接查询和生成声明／登记／scan，删除 global 类型、布局、linkage、初值、producer 副本和发射数量的预检；digest 直接记录实际位置，删除 coverage 与 owner 回放。移除 preparation 包装和独占断言，拆出 image 测试控制文件长度。
26. 对象边界物化：使用实际 primary／section extent 写入 callable 和 generated-C 边界；删除 body／marker／backend 覆盖比较、global alias 预检与已发射边界回放。摘要槽保留写入范围计算，删除零值读回和槽位覆盖检查，移除失去消费者的 layout 参数。
27. Shape／scan 发射：删除 layout／scan／dispatch 计划全集比较、同 id payload 副本比较和已有 scan initializer 文本重建；直接复用 LLVM global，保留实际 typed 查询与 scan／descriptor 编码。callable scan 去掉 owner／role／内容及总数重验。
28. Runtime stackmap：删除运行期 canonical 编码与 SHA-256 复算及专属向量；直接建立实际记录索引，保留 ODR 重复记录的 PC／payload 合并和 GC 帧边界。
29. Runtime 初值：删除静态初值与 immortal relocation 全量回放、cell／storage 零状态扫描及 callable 地址／指纹副本比较；保留实际根登记、存储引用、初始化排序和动态访问边界。
30. Runtime 装载范围：Darwin 使用 loader segment、Linux 使用 PT_LOAD，删除 VM region／proc maps 权限审计与只读／不可执行封口；统一实际范围检查，保留溢出、对齐和跨段边界。
31. Runtime 摘要：删除 release hook 身份重编码与地址副本比较，以及未消费摘要的非零断言；移除失去用途的 SHA-256 平台后备、Mbed TLS 源码与专属测试，保留编译／链接摘要编码及实际 runtime 索引。
32. Runtime shape：删除静态 shape／inline scan 重建比较、继承语义重放与 Context 初值／顺序预检；移除完整 metadata 复核模式和专属 scan 验证器。保留实际对象大小、分配、类型查询、scan 地址边界，以及独立的动态 GC heap 验证。
33. MIR foundation：从完整 Module 直接投影身份，删除全模块类型／CFG／ABI／coroutine／callback／release／constant 验证器、专用破坏测试和错误包装；保留 lowering 的实际结构断言与初始化绑定 fixture。
34. MIR 输入组装：删除 foundation／entry surface 副本比较与已完成 production 关系重验；初始化从实际 unit 和 callable 索引绑定，删除 generated ID／ODR key／签名重建与对应破坏测试，保留缺失 body 引用诊断。
35. HIR representation：删除只由专门测试调用的来源 inventory／字段／variant／CLayout 比较与 Checked 包装；保留实际数据、typed 引用和编码，测试直接验证顺序、intrinsic family 与布局策略。
36. HIR 继承／slot：删除没有生产调用者的 checked graph、来源／签名／继承关系重放和独占测试；保留实际 ancestry 查询、slot selection 编码、局部记录与 wire 引用解析，并精简测试 fixture。
37. 孤立 IR 复核：删除 GC/ZST 全图重算、fact-shape transcript、访问来源副本比较与 LIR gateway 验证器及独占测试；类型事实仍从具体 HIR 投影，辅助形状收回生产模块，仅保存实际需要的子类型。
38. Layout 发布：直接组合既有导出与 production，删除完整 layout／scan／dispatch／callable／shape-support 镜像比较；依赖 lowering 只投影实际语义根，移除第二套 physical inventory／初始化边比较。
39. Darwin 动态库：删除内部 undefined 符号的 C++ 前缀／ordinal 扫描及 flat namespace／weak binding 非空拒绝；保留实际 export、re-export、目标和显式库绑定。新增真实 C++ 异常与 C cleanup 组合，覆盖普通／flat、weak coalescing、moving GC 与已有产物独立链接。
40. Archive 读取：直接使用普通 reader 的成员序列与实际范围，删除第二次完整 header／padding 扫描和成员名前缀黑名单。member ordinal 统一排除 TOC／名称表，相关精简链接计划同步。
41. 文档与平台收尾：同步 M23、M28、M44 历史设计中的权限审计、完整 metadata 重验、SHA 复算和源码隔离要求，修正退役入口链接；移除 Linux cfg 下遗留的 unused import。
42. 最终验收进行中：本机定向回归及公共 runner 规则，Linux release workspace、完整 GNU CLI 与 musl 目标组合。

## 验证

工作文件批次：`cargo fmt --all`、workspace clippy 通过；release codegen 的 generated-C 3 项与对象分区 9 项通过。临时文件保持存活与 drop 清理、实际对象生成及 stackmap／registration 断言继续覆盖。

CBOR 批次：格式化、workspace clippy 通过；release wire／identity 共 368 项测试和 1 项 doctest 通过。

诊断源码批次：格式化、workspace clippy 与 release slib 的 layout compile 10 项通过。跨 Cone 源码诊断的 CLI 组合将在工具链批次构建配套命令后验证。

Native 输入批次：格式化与 workspace clippy 通过；native 工具链 9 项测试通过，C++ 修改后定向补验混合语言／头文件变化 1 项通过。本机正式 CLI 41 项、45 变体、101 进程通过，包含 native C、共享头文件和跨 Cone 迭代诊断；无需重复前一批的全量 Rust 测试。

LLVM IR 批次：格式化、workspace clippy 无警告；本机 release codegen 全部 356 项通过，包含三个目标的 IR／机器发射、真实 runtime／GC／异常测试。此前已清理本机 debug target 2.7 GiB，保留 release 工具。

Native 对象批次：格式化、workspace clippy 通过，release linker 3 项测试通过。本机 CLI 66 项中 65 项首次通过；新增 COMMON／弱定义组合暴露 Darwin 与 ELF 的优先级差异，按目标规则修正后两个新增用例复测通过。完整批次为 87 变体、505 进程、53 个专门链接 plan golden；补验 2 变体、4 进程。Linux COMMON 行为留待最终目标验证。

自产机器对象批次：格式化、workspace clippy 无警告，本机 release codegen 331 项全部通过；含真实 GC／异常运行、三目标机器发射、终结调用与条件 poll 的实际 site 数量。

发布入口批次：格式化、workspace clippy 无警告，release slib 334 项全部通过。配套 release CLI 重建后，C ABI、Scoop ABI、回调重入和 native storage 4 项／8 变体／40 进程通过，保留 8 个专门链接 plan golden。

Stackmap 批次：格式化、workspace clippy 无警告；release slib link_object 定向 215 项通过，包含实际返回地址、根记录与后续指纹回填。

Digest 回填批次：格式化、workspace clippy 无警告，release slib link_object 215 项通过；实际各槽位内容及非槽位字节保持的回归继续通过。

Image／entry 批次：格式化、workspace clippy 无警告；release slib 322 项通过，配套 CLI 重建后两个真实链接用例／4 变体／20 进程通过。新增 ELF 槽位用例受 Linux cfg 限制，本机未执行，留待 Linux 验证。本次清理 debug target 3.0 GiB，保留 release 后 target 约 1.6 GiB。

Callable／safepoint 批次：格式化、workspace clippy 无警告；release slib link_object 198 项通过，实际 body 指纹固定向量、stackmap 输入及 registration 回填继续通过。

Immortal object 批次：格式化、workspace clippy 无警告；release slib link_object 196 项通过，实际登记 ABI、ODR 和最终回填路径继续覆盖。

静态存储批次：格式化、workspace clippy 无警告；release slib link_object 185 项通过，layout／scan 固定向量与最终回填结果保持通过。Linux 编码初值和零初始化的实际槽位用例待目标环境执行。

初始化批次：格式化、workspace clippy 无警告；release slib link_object 180 项通过，Eager 的实际 gateway 指纹回填和 Lazy 无 gateway 行为继续覆盖。

类型登记批次：格式化、workspace clippy 无警告；release slib 全部 281 项通过，descriptor／layout 固定向量、V1／V2 编码兼容和最终回填均通过。

登记清理后的 CLI 定向回归：配套 release 命令重建，7 项／9 变体／44 进程通过，保留 4 个专门链接 plan golden；覆盖泛型初始化派发、类型物化、GC 静态根、Scoop ABI、回调重入和 COMMON 初始化。

Digest 槽位批次：格式化、workspace clippy 无警告；release slib link_object 164 项通过，实际登记／image／entry 槽位回填继续通过。

Code 与组装批次：格式化、workspace clippy 无警告；release slib 全部 275 项通过。上一批后已清理 debug target 1.7 GiB，保留 release 工具。

generated-C requirements 批次：格式化、workspace clippy 无警告；release slib link_object 157 项通过，覆盖实际 helper 分类和同名 source extern。Linux 专用 helper 调用已同步，执行留待目标验证。

Undefined requirements 批次：格式化、workspace clippy 无警告；release slib link_object 154 项通过，实际 requirement 固定编码与跨 Cone 分区结果保持通过。

LIR 发射批次：格式化、workspace clippy 无警告；release codegen 全部 235 项通过，包含三目标发射、真实 GC／异常／线程与 C ABI 运行。首轮遗留的预检断言已逐项闭合：只服务旧验证器的用例退役，实际 C 渲染循环／指针、静态 shape、根与调用操作的错误断言按现有出口更新。

LIR 发射后的 CLI 回归：7 项／9 变体／44 进程通过，保留 4 个专门链接 plan golden。GC 静态根初次因保留工作目录内已有 symlink 在准备阶段失败，改用新目录补跑通过；未发生编译失败。本次已清理 debug target 1.8 GiB，保留 release 工具。

对象定义批次：格式化、workspace clippy 无警告；release slib link_object 152 项通过，缺失／重复符号、空／重叠 atom 与真实定义读取继续覆盖。

对象格式批次：格式化、workspace clippy 无警告；release slib link_object 140 项通过。保留真实符号／relocation、未归属段属性分类和对象表读取错误；Linux ELF 实对象用例留待目标环境执行。

符号属性批次：格式化、workspace clippy 无警告；release slib link_object 139 项通过，真实弱定义、undefined 引用、atom 归属和定义位置边界继续覆盖。

Runtime metadata 发射批次：格式化、workspace clippy 无警告；release codegen 首轮 222 项通过，2 项旧全量预检断言改为实际缺失引用诊断后定向补跑通过，224 项覆盖均闭合。真实 GC／异常／线程与三目标发射保持通过；未重复执行已通过的其余测试。

对象物化批次：格式化、workspace clippy 无警告；release codegen 全部 224 项通过，覆盖三目标实际符号／边界、metadata 槽位及 GC／异常运行。上一批后已清理 debug target 1.8 GiB，保留 release 工具。

对象读取与发射后的 CLI 回归：新工作目录内 7 项／9 变体／44 进程通过，保留 4 个专门链接 plan golden；覆盖 COMMON、native 初始化、泛型派发、类型物化、GC 静态根、Scoop ABI 与回调重入。

Shape／scan 批次：格式化、workspace clippy 无警告；release codegen 的 shape 3 项与 callable runtime scan 1 项定向通过，实际边界／子 scan 及缺失 typed 引用继续覆盖。

Runtime stackmap 批次：格式化、workspace clippy 无警告；release codegen 的 stackmap 登记测试通过，含实际 ODR 合并、PC 冲突、记录边界和 managed frame 错误。

Runtime 初值批次：格式化、workspace clippy 无警告；release codegen 静态根／immortal／初始化登记定向测试通过，保留真实对象边界、独占失败槽、引用和顺序覆盖。

Runtime 装载范围批次：格式化、workspace clippy 无警告；image 4 项及 Darwin 实际装载范围 1 项通过。首轮空 freeze helper 导致的 C 警告已删除并补验；Linux 装载／PIE／静态 musl 路径待目标环境执行。

Runtime 摘要批次：格式化、workspace clippy 无警告；release codegen image 4 项通过。runtime object index 集成测试受 Linux cfg 限制，本机筛选为 0 项，留待目标环境验证；不将其记作已验收。

Runtime shape 批次：格式化、workspace clippy 无警告；image 4 项与 collector 30 项通过，覆盖 ZST／装拆箱／数组、根与搬迁、初始化、Context、pin、线程握手和 release hook。首轮遗留的 interface 继承循环断言已随静态语义重放删除并定向补验，未重复运行其余已通过 image 测试。

Runtime 清理后的 CLI 回归：7 项／9 变体／44 进程通过，保留 4 个专门链接 plan golden；覆盖 COMMON、初始化、GC 静态根、泛型派发、物化、Scoop ABI 和回调。已再次清理 debug target 1.5 GiB，保留 release 工具。

MIR foundation 批次：格式化、workspace clippy 无警告；release MIR 256 项、MIR lowering 117 项、release CFG 2 项、C projection 1 项及 driver shape demand 1 项通过。移除的验证器曾被初始化 fixture 借作构造 helper，现改为该用例自身的完整输入；保留真实绑定和转换覆盖。

MIR 输入组装批次：格式化、workspace clippy 无警告；release MIR 的 cone input 4 项与 production 编解码 13 项通过，真实角色绑定、缺失引用和固定 wire 编码继续覆盖。

HIR representation 批次：格式化、workspace clippy 无警告；release HIR representation 相关 17 项通过，实际字段／variant 顺序、类型身份、CLayout 和 wire 边界继续覆盖。

HIR 继承／slot 批次：格式化、workspace clippy 无警告；release HIR 继承 4 项、slot 9 项通过。测试辅助函数的三处冗余返回绑定已按 lint 简化，实际签名、modality、顺序和缺失引用覆盖保留。

孤立 IR 复核批次：格式化、workspace clippy 无警告；release HIR facts 3 项、access 2 项、HIR lowering 类型语义生产组合 120 项通过。删除后遗留的无消费者错误变体已同步清理；GC/ZST 的真实投影、泛型、存储、继承派发和跨 Cone 组合保持通过。

Layout 发布批次：格式化、workspace clippy 无警告；release LIR production 135 项、shape link 4 项通过，driver layout exports 22 项闭合。首轮发现 external boxing 留有要求 codegen 重验已完成 provider 选择的旧断言，删除该断言后仅补验对应 shape-support 组合；实际产物生成与 ABI 断言继续通过。

Darwin 动态库批次：格式化、workspace clippy 无警告。两个新 fixture 在旧 CLI 上均复现 `___cxa_allocate_exception` 拒绝；修复后 2 项／4 变体／18 进程通过，补充实际非空 weak binding 后与既有 dynamic／C ABI 回归共 11 项通过。独立测试源码实际产生三类被误拒的 C++／C unwind 符号，异常始终在 native 库内部处理。

Archive 批次：格式化、workspace clippy 无警告。8 个定向 CLI 用例中 7 项首轮通过；截断输入仍正确报错，诊断现由普通 archive reader 给出，更新三个目标的期望后仅补验该用例并通过。覆盖普通／长成员名、TOC、递归抽取、重名／冲突、C ABI／callback／TLS 组合及失败不发布；新增 `__.SYMDEF_extra.o` 普通成员正常链接运行。

本机最终组合：公共 fixture runner 50 项规则通过；7 项／9 变体／44 进程 CLI 通过，覆盖 COMMON、泛型派发、初始化、静态 GC 根、物化、Scoop ABI 和 callback。三轮同机性能复测共 24 次 build 成功，冷构建中位数降低 29.8%～35.6%，暖构建均零编译子进程，见 [性能记录](PERFORMANCE.md)。

Linux 验收发现并修正两类测试期望：ELF metadata fixture 实际只有两个 lazy 单元，迁移时误加的 eager gateway 槽预期已改为无槽；移除 `/proc/self/maps` 读取后不再导入 `getline`、`sscanf`、`feof`，40 个 GNU／musl 精简链接计划的系统导入数相应减三。实体选择、归档范围、ABI 与运行断言均保留，按受影响用例补验。
