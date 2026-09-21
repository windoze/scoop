# core 普通 library 清理

2026-09-21：根据用户新指令替换原M23-6当前目标。停止扩展来源授权及防篡改体系。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

## 实施与完成标准

- [x] 删除driver的源码/输出slot授权对象与路径限制，core可从普通manifest入口构建，产物可位于指定输出位置。集成测试在普通目录新增core函数、修改函数实现、重建后由用户代码消费，并核对依赖fingerprint更新。
- [x] 合并core与普通Cone的缓存键、receipt、源码快照、执行调度和产物完成验证；删除sysroot锁、slot receipt、专用source key及专用completed origin。用户修改源码按通用fingerprint失效与重建。
- [ ] 继续合并默认core发现、manifest限制与driver artifact加载的专用路径。已删除专用TrustedCore图节点；任意目录的core可作为普通Manifest构建根，在默认sysroot不存在时独立构建并命中缓存。已统一manifest parser，删除source/artifact slot包装对象；默认artifact消费不再读取或要求安装core源码，已用真实CLI验证仅安装`.slib`的sysroot。已开放显式core的source/artifact/search-root locator，默认sysroot只在完整显式发现仍无core时读取；prebuilt core共用普通完成验证。真实组合测试覆盖无sysroot的显式core修改、下游重建及删除源码后切换prebuilt并复用下游缓存。已删除TrustedCoreArtifactAuthority及input中coordinate/target/ABI副本，删除通用closure前重复的envelope/graph读取与专用授权校验；真实普通产物误作core由共有identity检查拒绝。已删除core独立file loader和load/validate API，core并入普通direct dependency集合，共用summary、图排序、resource meter、Compile/Link closure与唯一identity session；投影和普通依赖实际共享同一Compile对象，真实测试已核对。专用manifest限制仍待合并。
- [ ] 基础类型、普通callable/type/value及desugar通过普通声明metadata解析；intrinsic前端正规化后不携带来源授权。已删除core空声明表factory，core与普通Cone共用完整Export HIR投影、源码位置补齐及nominal接口读取；Unit/Any仅保留语言内建typed形状。已修正共有PublicSemanticSurface收集逻辑，内部owner的public override保持槽契约但不进入foreign直接查找表。真实core扩展fixture覆盖struct、alias、const、默认参数的产物往返及typed id查询；已删除semantic world的TrustedCoreImportedProviderInput、TrustedCoreProviderView、独立core角色与重复入口校验；core共用direct/support provider、typed实体索引和package/static名称索引。prelude优先级只在lowerer名称查找层处理，过滤先于目标合并，保留其他重导出路径。真实core索引及用户同名声明fixture已通过。已将普通core函数/property的prelude候选、const读取、命名实参、默认参数及其嵌套调用接入通用依赖选择；删除独立core重载/实参解析器及OrdinaryCoreOnlySources/lower_ordinary_core_only入口，测试使用完整公共接口与共有world。ImportedCoreInputs/CorePreludeOnly中的类型与compiler protocol路径仍待合并。
- [ ] 合并MIR/LIR专用调用桥、String TD与shape-support重复分支，以及slib专用requirement closure。已删除通用MIR/LIR export对core的禁止规则及producer/reader跳过分支，core中的合格普通函数按共有规则发布、校验调用桥；真实新增函数的HIR、MIR、LIR接口均已核对。已删除通用MIR/LIR consumer selection及codegen对core provider的禁止规则，普通core函数通过共有typed调用桥产生managed/NoGc外部调用。compiler protocol专用selected调用桥、旧IR载体与其余支持路径仍待删除。
- [ ] 删除M23-6 core source foundation专用入口及仅为防伪设计的授权链。已删除fundamental/Array/Unit/Any/Ptr/FunPtr消费者的重复core归属绑定和默认类型域的第二份core artifact参数；已删除core source foundation factory及其coordinate、Library、导入集合和CoreShapeSupport复核，core和普通Cone统一从已有typed HIR投影source foundation。
- [ ] 增加真实源码验证：修改core现有实现、增加导出函数和类型，重建后由用户代码消费；保留所有Cone共用的metadata、identity、ABI和缓存一致性检查。

每项完成后独立提交，先fmt和workspace lint再测试；构建进程退出后定期清理target。全部清理完成前不宣布目标完成。旧阶段文档尚存的trusted-core、独立authority和冻结专用格式描述属于待删除实现的历史背景，不作为保留理由。

2026-09-21：完整core公共接口投影与通用调用桥export批次通过workspace fmt/clippy及全部5,566项测试，含启用配套scoopc的真实进程测试。

2026-09-21：通用semantic world provider批次通过workspace fmt/clippy及全部5,567项测试，含真实配套scoopc进程测试。剩余类型消费仍需接通普通layout/dispatch能力；新增core类型的metadata往返已覆盖，尚未宣称用户代码可实际构造这些类型。

2026-09-21：普通core调用消费批次通过workspace fmt/clippy及全部5,570项测试，含配套scoopc真实进程。新增组合fixture验证命名参数、默认参数、const及默认正文中的core函数调用；修复单文件产物把外部默认参数的源码诊断记录误计入当前输入数量的问题，producer与reader共用按Cone identity计数。
