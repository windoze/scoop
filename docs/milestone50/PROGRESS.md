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
26. 待完成：其余对象和 runtime 重复证明、同类要求及最终验收。
27. 最终本机定向回归与 Linux 完整验证尚未开始，不以静态检查宣称验收。

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
