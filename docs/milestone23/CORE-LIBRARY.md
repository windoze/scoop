# core 普通 library 清理

2026-09-21：根据用户新指令替换原M23-6当前目标。停止扩展来源授权及防篡改体系。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

## 实施与完成标准

- [x] 删除driver的源码/输出slot授权对象与路径限制，core可从普通manifest入口构建，产物可位于指定输出位置。集成测试在普通目录新增core函数、修改函数实现、重建后由用户代码消费，并核对依赖fingerprint更新。
- [x] 合并core与普通Cone的缓存键、receipt、源码快照、执行调度和产物完成验证；删除sysroot锁、slot receipt、专用source key及专用completed origin。用户修改源码按通用fingerprint失效与重建。
- [ ] 继续合并默认core发现、manifest限制与driver artifact加载的专用路径。已删除专用TrustedCore图节点；任意目录的core可作为普通Manifest构建根，在默认sysroot不存在时独立构建并命中缓存。已统一manifest parser，删除source/artifact slot包装对象；默认artifact消费不再读取或要求安装core源码，已用真实CLI验证仅安装`.slib`的sysroot。已开放显式core的source/artifact/search-root locator，默认sysroot只在完整显式发现仍无core时读取；prebuilt core共用普通完成验证。真实组合测试覆盖无sysroot的显式core修改、下游重建及删除源码后切换prebuilt并复用下游缓存。已删除TrustedCoreArtifactAuthority及input中coordinate/target/ABI副本，删除通用closure前重复的envelope/graph读取与专用授权校验；真实普通产物误作core由共有identity检查拒绝。已删除core独立file loader和load/validate API，core并入普通direct dependency集合，共用summary、图排序、resource meter、Compile/Link closure与唯一identity session；投影和普通依赖实际共享同一Compile对象，真实测试已核对。已删除BootstrapEmpty、专用空依赖校验及不可查询的状态分支；core也运行共有图检查、Compile/Link closure与identity session，按实际direct依赖计量出边，发布读取共有dependency records与拓扑顺序。已统一生产HIR输入、入口与输出：core和其他Cone使用CurrentConeSources/lower_current_cone/DependencyHirOutput，本地protocol不再导致依赖选择或binding witness被丢弃，driver从共有world完成foundation与公共接口投影。当前core源码按package进入共有import表，内部object常量和外部函数调用均已覆盖，缺失import保留精确共有诊断；sysroot新增37条本地声明binding来源，原515条非binding来源保持不变。已统一MIR入口与DependencyMirOutput，删除对core consumer的禁止；MIR已进一步合并为单一SelectedExternalMirSet，删除泛型MirProtocolSelection及独立协议sidecar；本地或导入协议由HIR决定，所有外部callable共用选择映射与输出验证。core生产路径已使用该入口，新增core外部调用MIR golden及缺失选择负例；协议来源不再由调用方额外指定。已统一driver的MIR/LIR编排、foundation、公共桥和strong sealer；core从共有closure投影实际依赖并保留至LIR，不再替换为空选择。删除MIR sealer与LIR lowering对core consumer的依赖禁令，保留共有归属、数量、签名、GC effect及引用校验。已删除manifest及单Cone请求对core显式依赖的专用禁令；source/artifact/search-root locator、direct/support输入、输出隔离及损坏artifact summary均走共有规则。真实manifest组合验证core→helper→core由共有cycle错误拒绝，保留显式位置和注入边来源；缺失helper在源码发现前报共有locator错误。已统一driver HIR产物与投影：完整HIR、foundation、production、公共interface和classifier由共有结构持有，统一补齐源码位置；strong-profile封装统一消费该结构和完整MIR/LIR，保留共有ODR检查，不再复制core流程。已统一ParsedSingleConeBuildRequest及完整production/publication编排，删除两套parsed分支、生产stage包装和发布错误类型；core/manifest/single-file由同一入口产生dump、投影实际依赖、生成code/artifact并原子发布。旧core阶段适配器仅编译进独立测试。共有失败结果在HIR成功后保留warning及源码上下文，CLI独立渲染，machine failure携带独立typed warning diagnostic。已删除validated current input的core专用源码形态；ValidatedSingleConeBuildRequest以必需的ValidatedCompilerProtocols独立保存当前声明或共有closure成员的协议投影，源码解析只区分Manifest/SingleFile，production不再重建第二份协议来源enum。真实测试保持Compile/Link view指针同一性，并核对core与single-file的源码形态和协议来源。已删除上层child plan对core依赖的清空分支，所有manifest节点均必需读取共有ResolvedDependencyProjection；缺失projection以共有错误拒绝。machine ManifestRoot保留direct/support列表并使用共有数量上限，删除专用bootstrap空依赖限制；删除无路径的TrustedCoreBootstrap请求及旧wire tag，driver不再由该请求隐式发现sysroot源码。直接scoopc build的normalize_direct_build_request仍会在处理显式core artifact前打开默认sysroot，已用无安装产物的环境确认--direct-slib不能绕开该路径；此入口与umbrella的共有发现仍待合并。其余compiler protocol和专用metadata路径继续合并。
- [ ] 基础类型、普通callable/type/value及desugar通过普通声明metadata解析；intrinsic前端正规化后不携带来源授权。已删除core空声明表factory，core与普通Cone共用完整Export HIR投影、源码位置补齐及nominal接口读取；Unit/Any仅保留语言内建typed形状。已修正共有PublicSemanticSurface收集逻辑，内部owner的public override保持槽契约但不进入foreign直接查找表。真实core扩展fixture覆盖struct、alias、const、默认参数的产物往返及typed id查询；已删除semantic world的TrustedCoreImportedProviderInput、TrustedCoreProviderView、独立core角色与重复入口校验；core共用direct/support provider、typed实体索引和package/static名称索引。prelude优先级只在lowerer名称查找层处理，过滤先于目标合并，保留其他重导出路径。真实core索引及用户同名声明fixture已通过。已将普通core函数/property的prelude候选、const读取、命名实参、默认参数及其嵌套调用接入通用依赖选择；删除独立core重载/实参解析器及OrdinaryCoreOnlySources/lower_ordinary_core_only入口，测试使用完整公共接口与共有world。已删除HIR中废弃的core callable/type/value选择凭证、ImportedCoreCall及空arena；ordinary输出直接持有通用依赖选择，不再借用core专用sidecar。默认正文的隐式公共route由通用direct provider绑定保存，重导出保持真实完整路线，support-only provider不进入普通枚举；输入world释放后仍可投影默认参数。已删除重复core HIR callable/value目标表及其wire字段、专用解码和校验；prelude类型绑定直接携带typed类型目标，函数与值仍由共有world处理，测试夹具从共有callable接口取得签名。已删除重复prelude snapshot、公共绑定副本和无消费者的Option/String导入副本；Option身份与结构契约由既有compiler protocol reader统一承载，普通公共surface继续使用共有投影和验证。已删除CorePreludeOnly、ImportedHirSet及lowerer专用类型名称表；基础类型与透明alias从共有world按type/value namespace分别查找，alias引用保留共有绑定完整route，ordinary consumer不再凭Int等短名回退。真实core新增透明alias已被单文件程序消费，并覆盖同名类型/函数、当前包遮蔽和非generic诊断。ABI nominal分类已改为共有nominal interface，driver、HIR lowering和slib reader使用同一推导；删除ImportedCoreInputs的公共接口借用及重复core接口存在性检查。已删除CoreHirInterface类型目标表、wire字段4及专用reader；shape-support从共有DeclaredCurrent绑定/direct public surface推导，不为alias、generic或re-export增加本地根。HIR物化、MIR发布和slib跨层检查复用同一共有投影，无第二份序列化声明清单。共有CallableImplementationV1已将Intrinsic改为携带必需IntrinsicFunctionKind的完整分支，producer、wire reader、semantic world与依赖目录保留同一typed声明属性，旧unsigned格式拒绝。整数常量运算按本地声明或共有依赖目录查询typed kind和GC effect，删除ImportedCoreCompilerOperation及其导入列表，不再按core来源分支查询。常量method/infix及静态initializer同时使用canonical源码名称、共有参数名和infix属性，不保存伪造的本地FunctionId；参数改名、错误命名实参、非infix、转换实参与除零均有独立fixture。整数kind与GC effect及Ordinary execution的关系由共有builder/reader验证。普通实例成员的候选决议与执行仍需接入；Int.compareTo的非const consumer调用尚未完成。其余compiler protocol及native-boundary专用路径仍待合并。
- [ ] 合并MIR/LIR专用调用桥、String TD与shape-support重复分支，以及slib专用requirement closure。已删除通用MIR/LIR export对core的禁止规则及producer/reader跳过分支，core中的合格普通函数按共有规则发布、校验调用桥；真实新增函数的HIR、MIR、LIR接口均已核对。已删除通用MIR/LIR consumer selection及codegen对core provider的禁止规则，普通core函数通过共有typed调用桥产生managed/NoGc外部调用。已删除普通core callable的第二份MIR/LIR发布表、wire字段、reader relation、prelude consumer投影入口及对应kind分支；共有调用桥承担普通函数的完整跨Cone调用，shape-link分区和资源计数同步移除旧表。保留的初始化协议测试使用实际String参数调用，并断言普通函数的增减不改变协议桥；新增负例拒绝已删除的旧metadata字段。已统一M23-6通用MIR/LIR形状支持表，删除core空表禁令、Core/Ordinary root来源枚举及MIR section第二份core shape缓存；source-root覆盖、helper角色/GC、layout/descriptor与typed归属使用共有验证，依赖选择从同一表读取。新增core与普通provider混合选择及canonical wire往返，core值/引用形状与缺失helper/descriptor、错误来源均有验证。已删除MIR production重复CoreMirShapeSupportRootV1及wire字段2、旧reader关系比较和sealer第二份根缓存；MIR直接按HIR声明验证规范顺序、typed source/exact与实际helper物化，LIR继续由共有公共声明投影承担完整性验证。真实core测试覆盖重复/乱序/外来/generic/缺失声明和helper移除；旧callable/shape字段被严格拒绝。已删除LIR production的Core/NotCore形状包装，field 8直接保存共有计划数组，source/root/definition owner使用实际producer；共有reader重建完整计划并拒绝旧包装。shape-link不再整组排除计划中的layout、scan、TD和registration，保留实际definition与重复relocation分区检查；值/引用形状、错误producer及不完整source覆盖均已验证。已统一HIR/MIR/LIR物化需求入口：共有公共绑定按实际exporter选取本地param-free nominal，HIR保存必备LocalShapeSupportPlan，MIR/LIR删除Core/NotCore需求分支并按实际source归属检查完整helper；slib reader也从共有direct public surface重建需求。普通库的struct及struct/enum/class/interface/String字段/alias/private组合已真实构建、发布并回读，六份阶段golden锁定实际形状；预物化暴露的property getter/setter boxing-adjust角色遗漏已修复，普通method与accessor复用typed slot及原identity规则。已统一LIR外部TypeDescriptor实体、typed id、arena及Local/External引用，foundation和layout选择均保留实际provider、exact、symbol及definition，codegen共用发射和重复/归属校验。String角色由明确well-known引用保存，旧协议wire只投影该角色；其他core普通TD必须经过共有layout选择，不能按provider进入旧分区。真实LLVM验证混合String/core普通类型/其他provider的external声明，V2组合完成core普通TD、dispatch与初始化依赖的产物往返；组合测试发现并修复通用ExactDispatch按core provider误选旧协议引用的问题。已统一LIR全部外部callable实体、typed id、arena与调用/dispatch引用，初始化服务、普通core函数和其他provider共用lowering、LLVM声明/调用及ABI/GC校验；初始化角色仅在旧metadata投影边界区分，不能按core provider推断。新增共有重复body、本地/外部重叠、错误provider与缺失通用layout选择负例；真实初始化独立/默认参数与singleton组合fixture保留消息的managed root，隐式String通过共有HIR→MIR类型转换登记。普通外来类型的跨Cone源码构造和分派仍需继续接入；本批组合验证也确认普通consumer的Int.compareTo成员消费尚未接通，独立留待该路径修复。已统一MIR外部调用实体、typed id和arena，初始化unit及普通调用共用External引用；选择引用与GC effect是实体必需字段，共有selected id与实际use id保持不同domain。当前Cone输出与strong sealer复用同一校验及外部引用扫描，删除重复验证和错误类型；混合来源重复implementation与未使用项使用共有错误拒绝。LIR lowering只保留一个MIR到LIR的调用映射，普通core调用不按provider误归类。已删除MIR初始化服务专用selected实体、集合、借用凭证和引用编号，与普通依赖共用完整typed记录与角色。strong输入合并为单入口，consumer、覆盖、引用、重复目标与effect统一验证，driver删除重复MIR投影凭证检查。已删除LIR compiler protocol专用selected实体、集合、编号和借用凭证，初始化服务与普通依赖共用SelectedExternalLirSet和物理ABI分类/物化路径。MIR strong根表统一为完整外部根，角色由MIR/LIR共有语义枚举保存；driver删除第二套LIR输入及编排包装。String foundation投影直接返回完整ExternalTypeDescriptor，正式Local/External输入同时用于生产和测试，删除TestRuntimeString替代分支。已删除MIR production的CoreMirBridge、独立初始化definition/implementation记录及wire字段1；初始化与普通strong callable在同一完整记录中保存共有CallableRole，HIR投影、MIR sealer、LIR投影与reader直接核对同一实现和签名。旧三字段production与缺失角色的记录拒绝重建。LIR初始化发布、旧外部调用与普通依赖的ABI字段、codec、GC root plan及symbol/definition推导已合并为CallableAbiRecordV1和ExternalCallableRootPlan；provider由使用方显式传入，普通声明只附加typed declaration并核对实现。普通调用wire使用两字段product包含共有六字段ABI记录，旧七字段格式拒绝；初始化外层角色继续明确保存。初始化服务、普通调用桥和通用layout/ABI发布已共用typed target到MIR strong签名、实际物化root和LIR body的关联；普通调用与初始化调用共用完整ABI投影，通用layout保留独立layout/physical重放和原有资源计量。删除专用owner/body推导、重复函数查找及重复错误分支。String旧描述符桥已直接保存完整ExternalTypeDescriptor，与foundation、layout选择和LIR arena共用provider/target/symbol/definition及关系校验；删除专用载体、core TD identity helper和独立错误类型。共有wire显式保存provider，reader使用共有identity图并核对完整record，旧隐含CORE的三字段格式拒绝。String与LIR初始化服务的角色外层wire、registration引用、其余IR载体与支持路径仍待合并。
- [ ] 删除M23-6 core source foundation专用入口及仅为防伪设计的授权链。已删除fundamental/Array/Unit/Any/Ptr/FunPtr消费者的重复core归属绑定和默认类型域的第二份core artifact参数；已删除core source foundation factory及其coordinate、Library、导入集合和CoreShapeSupport复核，core和普通Cone统一从已有typed HIR投影source foundation。已删除exact fact/source foundation投影对core归属和第二份core foundation成员资格的重复校验，外部typed声明继续使用共有metadata依赖闭包验证。已将foundation外部source/generic nominal引用表改为所有Cone共有的数据结构与reader规则，删除core禁止外部类型的分支；core与普通consumer均覆盖普通类型、泛型类型的wire往返，并由共有identity/结构检查拒绝本地重复与未使用引用。
- [ ] 增加真实源码验证：修改core现有实现、增加导出函数和类型，重建后由用户代码消费；保留所有Cone共用的metadata、identity、ABI和缓存一致性检查。

每项完成后独立提交，先fmt和workspace lint再测试；构建进程退出后定期清理target。全部清理完成前不宣布目标完成。旧阶段文档尚存的trusted-core、独立authority和冻结专用格式描述属于待删除实现的历史背景，不作为保留理由。

2026-09-21：完整core公共接口投影与通用调用桥export批次通过workspace fmt/clippy及全部5,566项测试，含启用配套scoopc的真实进程测试。

2026-09-21：通用semantic world provider批次通过workspace fmt/clippy及全部5,567项测试，含真实配套scoopc进程测试。剩余类型消费仍需接通普通layout/dispatch能力；新增core类型的metadata往返已覆盖，尚未宣称用户代码可实际构造这些类型。

2026-09-21：普通core调用消费批次通过workspace fmt/clippy及全部5,570项测试，含配套scoopc真实进程。新增组合fixture验证命名参数、默认参数、const及默认正文中的core函数调用；修复单文件产物把外部默认参数的源码诊断记录误计入当前输入数量的问题，producer与reader共用按Cone identity计数。

2026-09-22：废弃core HIR选择路径清理批次通过workspace fmt/clippy及全部5,564项测试，含配套scoopc真实进程；删除旧API专属测试并强化共有world生命周期、重导出路线与support-only不可枚举边界验证。

2026-09-22：共有foundation外部类型引用批次通过workspace fmt/clippy及HIR、HIR lowering、slib全部3,233项测试；按职责拆分引用校验与测试模块。core manifest依赖限制及native-boundary/type消费的剩余专用路径仍待清理。

2026-09-22：普通core callable重复MIR/LIR桥清理批次通过workspace fmt/clippy及全部5,568项测试，含配套scoopc真实进程；按职责拆分MIR protocol/shape metadata与LIR导入、初始化协议测试模块，相关长文件均缩短。

2026-09-22：core HIR callable/value重复表清理批次通过workspace fmt/clippy及全部5,559项测试，含配套scoopc真实进程；删除旧API及其专属测试，新增reader拒绝旧字段的负例，并将core interface wire测试拆分为独立模块。

2026-09-22：重复prelude snapshot清理批次通过workspace fmt/clippy及全部5,556项测试，含配套scoopc真实进程；保留Option字段数量负例并拒绝已删除的旧snapshot字段，协议wire与结构关系验证按职责拆分为849/611行模块。

2026-09-22：core类型名称与透明alias共有查找批次通过workspace fmt/clippy及全部5,561项测试，含配套scoopc真实进程；补充独立和组合源码fixture、精确位置负例及真实产物HIR/MIR/LIR golden。删除测试中残留的空core公共接口，native-boundary导入按职责拆为独立模块。普通struct的跨Cone构造、layout和dispatch仍待接通。

2026-09-22：共有nominal ABI分类批次通过workspace fmt/clippy及全部5,562项测试，含配套scoopc真实进程；验证无core sidecar的具体nominal分类与generic能力边界，保持direct枚举/support精确查询的区别。ordinary lowering入口拆为独立74行模块，主文件缩短至939行。

2026-09-22：core重复类型目标表清理批次通过workspace fmt/clippy、42项production定向测试及全部5,558项workspace测试，含配套scoopc真实进程。删除旧表的877行实现及专属测试，共有shape投影/测试分别为132/159行，core interface缩短至436行；同一测试覆盖core和普通Cone的具体类型、generic、alias、re-export及缺失绑定/声明负例。wire快照已更新，并拒绝旧字段4。

2026-09-22：共有空依赖闭包批次通过workspace fmt/clippy及全部5,559项测试，含配套scoopc真实进程。新增空core闭包可查询、节点/边/深度计量及损坏依赖使用共有summary reader的验证；依赖校验不再强制返回core协议投影，由实际需要导入协议的消费者在共有验证后取得。

2026-09-22：共有当前Cone HIR入口批次通过workspace fmt/clippy、独立来源验证及全部5,562项workspace测试，含配套scoopc真实进程。新增core内部object常量import、独立/条件组合外部调用fixture与HIR golden，以及精确缺失import负例；输入/入口/组合测试模块分别为101/64/197行，主lowering文件934行。验证发现Homebrew默认LLVM已升级到23.1.1，显式选择本机LLVM22.1.8后完整通过；无构建进程时已清理10.6GiB target。

2026-09-22：共有当前Cone MIR入口批次通过workspace fmt/clippy、3项core定向测试及全部5,562项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。MIR protocol输入保留实际类型，不创建空core artifact；新增MIR golden锁定三处调用复用一个共有dependency-strong目标及object初始化。protocol选择模块34行，MIR lowering模块220行，新增组合验证模块74行。测试进程全部退出后清理10.0GiB target。

2026-09-22：共有driver machine-stage批次通过workspace fmt/clippy及全部5,563项测试，含配套scoopc真实进程，使用LLVM22.1.8。新增完整sysroot源码加外部callable的阶段组合测试及HIR/MIR/LIR golden，验证三处managed调用、条件CFG与缺失LIR选择负例。共有实现135行，错误类型70行，测试按provider、selection和source helper分拆。该阶段测试不声称存在可发布的无环core→普通源码库图；循环依赖仍须由共有graph拒绝。

2026-09-22：core manifest/request依赖限制清理批次通过workspace fmt/clippy、6项定向测试及全部5,568项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。新增manifest fixture覆盖共有locator语义、精确错误span、cycle边来源和损坏artifact验证。manifest语义与driver请求模块的测试独立成文件，生产文件分别缩短至566/585行；single-file依赖限制与共有graph校验保留。

2026-09-22：共有driver HIR产物与strong-profile封装批次通过workspace fmt/clippy及全部5,568项测试，含配套scoopc真实进程，使用LLVM22.1.8。原有HIR/MIR/LIR golden无需变化；删除重复投影、封装及两组专用错误类型，共有实现100行、错误模块74行，core/ordinary包装模块分别缩短至345/299行。本轮此前已在无构建进程时清理9.6GiB target。

2026-09-22：共有production/publication入口批次通过workspace fmt/clippy、2项定向测试及全部5,570项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。新增warning源码fixture，在core和普通library中触发真实原子发布失败，验证精确span、message、源码行列及输出目录未被修改；failure response完成独立Error/Warning diagnostic的wire往返。现有stage golden无需变化，byte reproducibility、普通依赖及链式re-export回归通过。发布主模块131行、protocol投影122行、错误模块92行，preflight主模块缩短至428行。

2026-09-22：共有validated request输入批次通过workspace fmt/clippy及全部5,570项测试，含配套scoopc真实进程，使用LLVM22.1.8。删除TrustedCoreBootstrap source-form和CoreOnly命名的验证API；已有真实core、single-file与共享artifact view测试增加结构断言，stage golden不变。验证模块143行，preflight主模块338行，metering模块165行。

2026-09-22：共有core child依赖传递与显式源码请求批次通过workspace fmt/clippy、20项protocol测试、core child plan定向测试及全部5,572项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。新增旧wire tag 3拒绝和core manifest共有direct/support数量限制负例，非空列表完成canonical wire往返；core child缺失projection不再回退空列表。协议测试按职责独立为181行模块，request生产文件缩短至714行，driver请求模块574行。循环依赖仍由共有graph拒绝，本批不宣称可发布core→普通源码库的有环图。

2026-09-22：共有MIR/LIR形状支持表批次通过workspace fmt/clippy、807项MIR/LIR测试及全部5,574项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。MIR source-root校验缩短至66行，section查询83行；新增LIR core测试107行，provider通用fixture及layout辅助模块各294行。原有pipeline golden不变，真实core修改/缓存/发布回归通过；该批验证通用类型metadata，不宣称旧production形状字段或跨Cone类型构造已全部迁移。

2026-09-22：MIR production重复形状根清理批次通过workspace fmt/clippy、10项MIR production定向测试、真实core形状需求负例及全部5,574项workspace测试，含配套scoopc真实进程，使用LLVM22.1.8。固定MIR wire快照随字段删除更新，源码pipeline golden不变。CoreMirBridge模块由350行缩至184行，strong-input主模块缩至495行，实际物化校验独立为73行模块；新增真实core测试按职责独立成文件。删除helper的损坏MIR由共有foundation以明确的unclaimed generated nominal错误提前拒绝，未放宽既有metadata检查。

2026-09-22：共有LIR production形状计划批次通过workspace fmt/clippy、12项形状计划定向测试及全部5,574项workspace测试（37组，零失败、零忽略），含配套scoopc真实进程，使用LLVM22.1.8。删除275行Core/NotCore包装及测试，shape-link旧分区模块缩至42行；跨provider组合与负例独立为199行测试模块。strong-production两版字段8统一为数组，旧wire包装拒绝，stage3/stage6设计及实现规范已同步。现有pipeline golden无需变化，本批不宣称物化需求入口、String TD或跨Cone类型构造已全部迁移。本仓库构建进程全部退出后清理7.7GiB target。

2026-09-22：共有形状物化需求批次通过workspace fmt/clippy、2,924项相关阶段测试、5项boxing-adjust与8项LIR物化测试，以及全部5,578项workspace测试（37组，零失败、零忽略），含配套scoopc真实进程，使用LLVM22.1.8。聚合测试公共绑定按typed exporter选择本地需求，保留source归属检查；普通库组合包含实际interface getter adjust及String字段scan。新增两份源码fixture与六份HIR/MIR/LIR golden，两份既有MIR golden仅增加本地公共类型的coroutine helper。HIR输出主模块缩至395行，形状投影独立为176行；MIR boxing-adjust主模块缩至246行，测试按职责分离。确认本仓库构建进程退出后清理11.2GiB target。

2026-09-22：共有外部TypeDescriptor批次通过workspace fmt/clippy、导入投影/LLVM发射/布局组合定向测试及全部5,585项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。删除两套描述符实体和arena，保留明确String角色并拒绝错误provider、重复exact、本地/外部重叠及缺失通用layout选择；core与其他provider共用V2产物与wire重放。现有pipeline golden不变，旧协议wire契约保留；stage3/stage6设计与实现规范同步。新实体93行，外部校验独立为130行，三份新增测试模块分别为133/122/127行；codegen validation主文件缩短至1,356行。本批不宣称初始化循环selected调用桥或跨Cone源码类型消费已完成。 确认本仓库构建进程全部退出后执行cargo clean，移除21.6GiB构建产物。

2026-09-22：共有LIR外部callable批次通过workspace fmt/clippy、真实core修改与machine pipeline定向测试及全部5,587项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。初始化服务、普通core函数及其他provider共用实体、调用目标、LLVM发射和ABI/GC校验；旧metadata分区只读取明确角色，普通core callable不能冒充初始化协议。新增两份独立/组合源码fixture与六份HIR/MIR/LIR golden，覆盖eager初始化、lazy singleton、默认参数、条件分支和managed消息root；实际发布暴露并修复隐式String缺少source exact identity的问题。统一实体270行，协议/root-plan模块27/54行，共有外部校验71行，新增投影测试141行、源码验证52行。MIR专用selected载体与旧协议wire继续待合并，Int.compareTo等跨Cone成员消费另待接通；本批不宣称整个M23-6完成。确认本仓库构建进程全部退出后执行cargo clean，移除24.7GiB构建产物。

2026-09-22：共有MIR外部调用批次通过workspace fmt/clippy、598项MIR/MIR lowering/LIR lowering测试、5项真实core定向测试及全部5,588项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。删除两套MIR use实体、arena、调用分支及LIR映射；来源和GC effect由完整共有实体保存，输出与strong sealer复用同一来源投影、引用扫描及重复implementation校验。新增混合来源、两类未使用项及跨来源重复目标测试，同时经过实际输出构造与strong sealer；普通core调用排在初始化服务之前也保留正确角色和NoGc/Managed effect。沿用真实初始化独立/组合fixture，增强MIR共有引用断言，五份MIR golden更新，HIR/LIR golden不变。共有实体93行、校验入口47行，输出模块缩短至80行，新测试及fixture辅助分别95/146行；旧selected记录和wire分区仍待合并。确认本仓库构建进程全部退出后执行cargo clean，移除23.6GiB构建产物。

2026-09-22：共有MIR callable选择批次通过workspace fmt、clippy（含all-targets）、601项MIR/MIR lowering/LIR lowering测试、5项真实core定向测试及全部5,591项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。初始化服务和普通依赖共用SelectedExternalMirSet、完整typed记录、明确角色及选择id/ref；删除core专用借用凭证、选择集合、编号、泛型协议sidecar和重复强输入。driver从HIR协议与共有闭包构造一次完整选择，MIR输出和strong sealer统一验证consumer、覆盖、引用、重复目标与Managed初始化effect；旧MIR/LIR metadata仅按角色投影。混合调用在provider对象释放后仍可完成输出及strong sealing，验证两种角色的引用域、未使用项、缺失集合、错误consumer、重复目标、自导入及receiver约束。真实初始化独立/组合fixture全部通过，所有HIR/MIR/LIR golden保持不变。共有选择模块284行、记录52行、验证入口86行、输出65行；新增验证模块137行，driver machine/projection缩短至111/210行。LIR专用选择及旧wire、String/native-boundary和跨Cone类型/成员消费仍待继续合并，不宣称整个M23-6完成。确认所有构建进程退出后cargo clean移除25.9GiB构建产物。

2026-09-22：共有LIR callable选择与物化批次通过workspace fmt、clippy（含all-targets）、1,061项MIR/LIR阶段测试、5项真实core定向测试，以及最终代码的582项LIR/LIR lowering复验和全部5,594项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。初始化服务与普通依赖共用完整SelectedExternalLirSet、MIR strong外部根表、物理ABI分类及ExternalCallable构造器，删除两套LIR选择载体、lowering输入、借用凭证和重复symbol/definition推导。String foundation直接返回完整ExternalTypeDescriptor，正式Local/External输入同时用于生产和测试；provider释放后仍可完成lowering，缺失选择、错误consumer、角色或GC effect不一致，以及错误String exact/provider均由共有检查拒绝。真实core修改、重建与缓存回归及初始化独立/组合fixture全部通过，所有HIR/MIR/LIR golden不变。共有选择214行、记录43行、外部物化117行、String输入50行；foundation导入缩短至252行、driver machine缩短至86行，初始化测试按职责拆为180/230/107行。旧MIR/LIR协议wire、String metadata、native-boundary和跨Cone类型/成员消费仍待继续合并，不宣称整个M23-6完成。确认所有构建进程退出后cargo clean移除28.8GiB构建产物。

2026-09-22：MIR production重复初始化记录清理批次通过workspace fmt、clippy（含all-targets）、1,610项MIR/LIR/slib阶段测试及全部5,599项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。删除CoreMirBridge、独立definition/implementation记录、Core/NotCore包装与wire字段1；初始化角色直接保存在完整StrongCallableBridgeV1中，与MIR/LIR外部选择共用CallableRole，typed identity保持不变。HIR投影设置角色并核对签名，MIR sealer和type bridge使用同一production关系验证，LIR与slib reader直接读取完整记录，不再第二次查找可缺失的协议签名。新增五项测试覆盖缺失/未知角色、重复初始化角色、非function目标、错误producer和同core普通函数的角色误用，builder与reader均验证；旧三字段production及无角色的旧strong记录拒绝，更新两份固定MIR wire快照。真实core修改、重建、显式prebuilt切换、缓存与初始化独立/组合fixture均通过，所有HIR/MIR/LIR源码golden保持不变。production主模块由726行缩至471行，共有callable模块311行；测试按职责拆为375/180/123行，MIR lowering production缩至163行。LIR旧协议wire、String metadata、native-boundary及跨Cone类型/成员消费仍待继续清理，不宣称整个M23-6完成。确认所有构建进程退出后cargo clean移除21.2GiB构建产物。

2026-09-22：共有LIR callable ABI记录批次通过workspace fmt、clippy（含all-targets）、1,133项LIR/LIR lowering/slib阶段测试及全部5,606项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。初始化发布、旧外部调用和普通依赖共用完整CallableAbiRecordV1、codec、ExternalCallableRootPlan及provider显式的symbol/definition推导，删除两种专用ABI载体、重复target解码和core callable identity helper。普通调用wire为typed declaration加完整ABI record的两字段product，旧七字段格式拒绝；共有reader逐字核对派生合同并使用同一foundation/definition关系检查。新增七项测试覆盖实际provider往返、错误provider、symbol/definition不一致、GC root不一致、suspend、声明实现不一致、三种角色包装共享payload及旧格式拒绝。真实core修改、重建、缓存和初始化独立/组合fixture全部通过，所有HIR/MIR/LIR源码golden保持不变。共有数据、wire、关系验证和错误模块分别为140/83/63/107行，初始化发布模块缩短至251行、外部桥模块缩短至437行；新增组合测试177行，初始化fixture辅助独立为75行。初始化角色外层wire、String metadata、native-boundary及跨Cone类型/成员消费仍待清理，不宣称整个M23-6完成。确认所有构建进程退出后cargo clean移除23.1GiB构建产物。

2026-09-22：共有LIR callable物化与ABI生产批次通过workspace fmt、clippy（含all-targets）、593项LIR/LIR lowering阶段测试及最终代码的全部5,610项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。初始化服务、普通调用桥和通用layout/ABI发布共用LocalCallableMaterialization，从typed target核对MIR strong签名并绑定实际MIR函数与LIR body；普通和初始化调用通过同一完整ABI投影检查GC effect、calling convention及参数数量，不再各自重建owner/body与ABI字段。通用layout保留原有source/physical重放，并在共有查找前按相同表长度计量。新增四项测试覆盖三种发布结果一致、缺失实际body的共有typed错误、两种角色的GC/参数数量漂移及预算不足；既有签名负例同步断言共有错误中的target。真实core修改、重建、缓存、显式prebuilt切换、初始化独立/组合及跨Cone发布回归全部通过，所有HIR/MIR/LIR源码golden保持不变。共有物化/ABI模块108行，初始化角色34行，错误模块39行，组合测试202行；lowering主模块由631行缩短至540行，普通调用桥缩至256行。角色外层wire、String metadata、native-boundary及跨Cone类型/成员消费仍待继续清理，不宣称整个M23-6完成。确认构建进程全部退出后cargo clean移除21.4GiB构建产物。

2026-09-22：共有外部TypeDescriptor记录与wire批次通过workspace fmt、clippy（含all-targets）、1,144项LIR/LIR lowering/slib阶段测试及全部5,617项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。String旧外部桥直接保存完整ExternalTypeDescriptor，与普通provider共用四字段codec、实际provider的symbol/definition推导及definition关系校验；删除StrongExternalTypeDescriptorBridgeV1、core TD identity helper和CoreExternalBuildError，已有callable测试迁入所属模块。新增七项测试覆盖core/普通provider往返、缺失identity、字段不一致、provider与definition不一致、旧格式拒绝和String角色校验；导入及初始化组合测试核对完整记录与共有错误。固定wire快照只增加显式provider并调整字段编号，旧三字段格式要求重建产物；全部HIR/MIR/LIR源码golden保持不变。真实core修改、重建、无sysroot构建、source/prebuilt切换、缓存及跨Cone发布回归全部通过。共有实体、wire、关系校验和错误模块分别为76/76/31/79行，外部桥主模块由437行缩至337行，角色选择31行，新测试143行。初始化与String的角色外层wire、registration引用、native-boundary及跨Cone类型/成员消费仍待继续合并，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除21.3GiB构建产物。

2026-09-22：共有intrinsic声明与跨Cone整数常量调用批次通过workspace fmt、clippy（含all-targets）、1,107项HIR测试、20项导入core定向测试、544项slib测试、真实core修改/消费测试，以及最终代码的全部5,625项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。共有CallableImplementationV1完整保存IntrinsicFunctionKind并使用closed-sum wire，builder与reader共同拒绝整数kind和GC/execution矛盾；旧unsigned格式要求重建artifact/cache。删除ImportedCoreCompilerOperation及导入副本，常量运算、method/infix与静态initializer从共有callable目录读取typed kind、canonical源码名称、参数名和infix属性，不再要求本地FunctionId。新增独立与组合fixture覆盖比较、wrapping、转换、位移、div/rem、参数改名、错误方法/实参、非infix及除零的精确诊断；三个新HIR/MIR/LIR golden确认折叠后的值仍经普通外部函数调用消费，既有golden保持不变。实现种类模块73行、共有源码调用视图90行，常量调用主模块由540行拆为176行探测、133行infix和249行method模块；新测试173行。真实core重建、显式prebuilt切换、缓存、跨Cone发布和byte reproducibility全部通过。非const Int.compareTo、普通跨Cone类型构造/分派及其余core专用路径仍需继续接通与清理，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除47.6GiB构建产物。

2026-09-22：整数HIR正规化与跨Cone默认参数批次通过workspace fmt、clippy（含all-targets）、3,359项HIR/HIR lowering/MIR lowering/slib阶段测试、真实core源码修改与artifact消费测试，以及最终代码的全部5,630项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。Export/LocalConcrete HIR的整数operation、conversion及literal pattern equality不再携带源码函数target；NoGc/Managed由封闭operation variant表达，移除concretizer为整数运算物化本地函数的路径。default wire、投影、实例化、资源遍历与引用闭包同步删除冗余callee；旧字段格式拒绝并要求重建artifact/cache。保留操作数、结果与参数个数校验，补充unsigned ushr非法组合检查；普通callable及其访问校验保持。真实`.slib`组合fixture覆盖加法、inc、compareTo、shift、转换、前序参数引用及普通外部函数调用，新增HIR/MIR/LIR golden；既有HIR golden仅删除整数target显示。导入Managed默认值的除零异常仍依赖跨Cone类分配/构造证明，当前在候选探测期报告共有layout能力错误，新增negative fixture断言诊断、位置及不发布产物，避免后续MIR读取本地core而panic；这条成功路径仍是M23-6待完成项，不能以此能力边界代替最终实现。整数wire模块114行，新类型校验测试107行，MIR测试构造器删除93行冗余函数定义。非const显式core成员、普通跨Cone类型/构造/分派及其余core专用路径继续待接通，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除29.4GiB构建产物。

2026-09-22：导入core实例成员批次通过workspace fmt、clippy（含all-targets）、2,699项HIR/HIR lowering阶段测试、真实core修改与artifact消费测试，以及最终代码的全部5,633项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。成员从共有公共callable目录按实际nominal owner id及源码名/typed operator role查询，与普通导入调用共用源码interface/default/source-origin、实参映射、候选私有探测、MSC及winner commit；不伪造binding或本地FunctionId，不扩大M23-5 strong capability。成功候选完整解析receiver、参数和结果，整数NoGc/conversion、BooleanNot及StringConcat/StringCompareTo直接生成既有typed HIR。新增独立、组合与negative fixture覆盖Int.compareTo、转换、infix shift、嵌套运算、整数literal上下文、普通函数组合、参数名随core源码修改、精确实参诊断及@NoGC调用；HIR/MIR/LIR三份golden确认实际artifact生产。扩展回归同时验证适用成员优先、成员不适用时exact/current-package/star扩展仍可选，保留typed operator的alias行为。成员主模块从653行拆至437行，receiver调用模块220行，导入探测301行、receiver模块111行、共有成员目录134行。Managed div/rem仍由共有layout能力诊断标明其异常构造依赖；整数==/!=、literal pattern、普通跨Cone类型/构造/分派及其余core专用路径继续待接通，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除30.6GiB构建产物。

2026-09-22：导入整数相等比较与literal pattern批次通过workspace fmt、clippy（含all-targets）、1,590项HIR lowering测试、真实core修改与artifact消费测试，以及最终代码的全部5,636项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。==/!=与literal pattern通过共有成员目录、实参映射、签名适用性及MSC选择equals；source与已lowered实参使用显式输入variant，模式保留typed subject而不构造虚假receiver/local/AST。已lowered实参不重新参与source literal偏好；模式winner直接生成完整IntegerKind equality plan。unsigned负字面量模式复用已有常量intrinsic查询，删除其只接受本地函数的检查。右侧lowering产生语句时先物化左值，锁定嵌套实参之前完成左操作数求值。新增独立与组合fixture覆盖八种整数、signed/unsigned边界、wrapping负模式、alias、tuple嵌套模式、@NoGC、普通外部函数组合及单次有序求值；混合宽度、signedness、无关extension equals及越界模式均断言精确诊断位置与内容。三份新HIR/MIR/LIR golden覆盖实际object发布；LIR逐宽度比较指令和left/middle/right三次调用顺序均核对，既有生产golden保持不变。通用operator模块由696行拆至478行，相等比较模块307行，调用输入模块104行、模式提交43行，新增测试128行。普通非intrinsic core成员、一般跨Cone类型/构造/分派及Managed div/rem的异常构造成功路径仍待接通，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除26.3GiB构建产物。

2026-09-22：通用继承槽契约生产批次通过workspace fmt、clippy（含all-targets）及全部5,639项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程和修改core后重建、发布与消费artifact的测试，使用LLVM22.1.8。HIR type/inheritance producer复用已解析的schema与实现选择，正式输出每个typed slot的根声明、完整签名、根访问域、独立实现owner及Abstract/Concrete/InterfaceDefault选择，补齐所需definition sources；不再拒绝普通virtual/interface dispatch，也不再输出空槽契约。private interface helper不进入槽表，getter/setter保持不同角色。新增独立virtual源码及组合interface产出golden，覆盖抽象槽、基类前缀、final override、internal open成员、object、菱形继承、默认属性和struct/enum实现；产出经wire往返后与源码选择核对，错误默认实现及收窄访问域被拒绝，增加无关源码文件不改变槽契约字节。三个新增测试全部通过；生产模块193行、测试模块288行，继承入口缩短至247行。protected声明生产、普通跨Cone类型/构造/分派的实际消费路径及完整layout-strong发布仍待接通，不宣称整个M23-6完成。确认构建测试进程退出后cargo clean移除25.6GiB构建产物。

2026-09-22：protected类型接口生产批次通过workspace fmt、clippy（含all-targets）、25项来源闭包测试、477项类型语义生产测试及全部5,643项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。总入口接入已有protected声明投影，删除protected成员、构造和嵌套类型的三类拒绝分支；public/protected构造共同来自完整源码构造记录，每个具体owner保留完整protected引用，递归及generic nested仍按既有源码接口处理。构造参数与protected/nested参数协议按typed声明合并。来源表改用producer/reader共有递归遍历，精确收集继承、private setter、nested support、参数及默认值中的实际来源，删除手工拼接来源和重复构造投影。新增独立成员与嵌套组合fixture及两份golden，整份section编码、回读并核对源码声明、参数、表示及来源闭包；删声明/来源的负例和无关文件引起arena分配变化的稳定性测试通过。参数来源按独立参数协议核对，与声明来源表保持各自职责。继承入口156行、构造投影27行、协议合并57行、来源收集52行，新增集成测试228行。默认值正文仍由现有DefaultTemplateAuthorityRequired诊断阻止不完整输出，成功生产路径需继续接入；一般跨Cone类型/构造/分派消费和layout-strong发布亦未完成。确认构建测试进程退出后cargo clean移除28.7GiB构建产物。

2026-09-22：类型默认值生产批次通过workspace fmt、clippy（含all-targets）、482项类型语义生产测试及最终代码的全部5,648项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。类型接口入口接入共有默认模板生产，删除DefaultTemplateAuthorityRequired拒绝和空模板表；按实际参数协议选择正文，复用已有source body、六类引用和occurrence/receiver索引，相同target与definition origin合并实际使用位置，metadata不伪造expression index。继承默认值保留原provider的root、path、binder、正文及来源，发布key、直接访问域与完整根槽域取当前声明和已解析继承契约，覆盖双接口根槽；generic nested仍保留源码metadata身份。正文各字段移动到既有模板结构，避免复制整棵正文；源码参数owner索引与既有来源生产共用，访问域的长源码路径复制也计入共享预算。新增三个独立/组合fixture、两份golden及五项测试，覆盖构造、六类引用、隐式/显式receiver、局部闭包、继承、泛型嵌套、vararg默认与空省略、缺模板/引用、错误使用位置、共享预算及无关arena分配下的完整section字节稳定性。vararg通过完整core bootstrap与同一个模板生产入口验证，不放宽完整core类型候选对普通泛型enum具体化的现有ODR边界。入口、引用合并、调用域投影及正文转换模块分别69/128/167/28行，新增测试主模块201行、辅助模块220行。一般跨Cone类型/构造/分派消费及layout-strong发布仍待接通，不宣称整个M23-6完成。确认全部构建测试进程退出后cargo clean移除38.1GiB构建产物，target目录已删除。

2026-09-22：enum成员持久身份保留批次通过workspace fmt、clippy（含all-targets）、5项成员身份定向测试及最终代码的全部5,651项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。LocalConcrete HIR与MIR的variant、payload field直接携带非可选typed persistent id，源码具体化从Export HIR的checked member relation复制，MIR再从具体实体传递；泛型实例共享声明成员身份并保留各自exact字段类型。协程Step/Slot在构造实际enum前取得已有generated身份，协议校验同时核对两个variant和payload field，六类错配均拒绝。独立与组合源码fixture覆盖unit/positional/named/default variant、泛型、String和空struct，golden投影锁定跨阶段身份与类型，并验证前置无关声明不改变结果。MIR enum定义与checked引用拆为206行模块，MIR类型主文件486行，相关HIR/MIR测试移入所属子模块。此次补齐实际IR丢失的导出输入，未宣称完整MIR type bridge生产入口、LIR layout/ABI生产、driver layout profile发布或一般外来nominal消费已经闭合；generic fixture也不授予M23-7物化能力。测试进程全部退出后执行cargo clean，清理25,394个文件、27.8GiB，target目录已移除。

2026-09-22：有限形状MIR类型生产批次通过workspace fmt、clippy（含all-targets）、3项真实源码定向测试及最终代码的全部5,654项workspace测试（37组，零失败、零忽略），含SCOOP_TEST_PAIRED_SCOOPC启用的配套scoopc真实进程，使用LLVM22.1.8。CanonicalParamFreeMirTypeExportsV1新增from_finite_shape_support，按sealed source计划从实际MIR物化关系生产BoxedValue、CoroutineStep和CoroutineSlot完整记录，复用共有identity/representation校验及canonical codec；box payload、enum成员、GC facts和接口均取同一MIR。strong计划条目直接绑定源码声明与nominal/exact/物理类型root，LIR沿用其中声明，不再由此生产入口重新哈希并搜索root。生成身份表单次遍历，输出按每个source root至多三种helper预留；查找、排序、分配及复制共用预算。真实core加独立/组合用户类型覆盖空struct、整数值、enum、String、interface/class与实际生成的private box，private box不进入导出；两份golden锁定完整记录，整表回读与前置无关声明后的字节稳定性均通过。缺失identity graph、缺失generated foundation、零复制预算和跨调用累计work耗尽均拒绝。生产入口104行、表示投影135行，新增测试模块均不超过200行。此批实现完整MIR type bridge中的有限支持组成表；普通source nominal、callable/dispatch/object组成表与LIR layout/ABI、driver layout profile发布仍待闭合，不宣称整个M23-6完成。测试进程全部退出后执行cargo clean，移除25,492个文件、22.8GiB，target目录已清理。

2026-09-22：普通源码 MIR 类型生产批次通过 workspace fmt、clippy（含 all-targets）及全部 5,657 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 的真实进程测试，使用 LLVM 22.1.8。

mir-lower 从同次 HIR type-semantics 与 sealed strong MIR 生产普通 source nominal 和 object backing 类型，复用实际 exact registry、struct/enum 成员身份、class 声明字段身份、ZST/GC facts 与直接继承边。class 字段投影跳过实际基类前缀，object 的源码类型与 backing 表示同时输出。lower_type_exports 在共有预算内移动合并源码表及有限 BoxedValue/CoroutineStep/CoroutineSlot 表，经过共有 canonical/type bridge 验证，没有新增 core 来源授权或防伪通道。

独立及组合源码 fixture 覆盖空 struct、嵌套 ZST、CLayout、多字段 enum、String、class 继承、interface 与 object；逐项核对源码字段/variant 身份和 exact 类型，两份 golden 锁定表示，完整类型表经过编码和回读。前置无关 private 声明不改变字节，错误来源对应关系、缺失 identity、零复制预算及累计 work 耗尽均拒绝。真实 core 的范围类型所需 generic interface 继续按 M23-7 ODR 条件拒绝，未放宽门禁。新增实现与测试模块均少于 200 行。

本批完成 MIR 类型组成表的实际生产；callable、dispatch、object-value、shape-support 关系表及 LIR layout/ABI、driver layout-strong 发布和一般跨 Cone nominal 消费仍需继续接通，整个 M23-6 尚未完成。全部构建与测试进程退出后执行 cargo clean，移除 30,729 个文件、30.5 GiB，target 已清理。

2026-09-22：MIR 有限支持关系生产批次通过 workspace fmt、clippy（含 all-targets）、2 项新增真实源码测试、3 项既有真实 core 有限类型测试及全部 5,659 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

strong source shape-support 计划现在同时绑定源码 root 与完整 box/step/slot helper root，保存实际 typed location、nominal 和 exact identity；引用类型显式表示无需 box。sealer 在验证实际物化时完成绑定，有限类型导出不再搜索全部 generated 类型。CanonicalMirShapeSupportsV1 新增 from_strong_input，直接从同一计划与统一类型表产出共有支持关系，继续复用 source、role、GC、provider 与 canonical wire 验证，没有新增来源授权结构。

独立 ZST fixture 与包含 String、struct/enum、class/interface/object、私有 box 的组合 fixture 均核对实际 MIR 位置与身份、完整 root 覆盖及 wire 回读；两份 golden 和前置无关 private 声明后的字节稳定性通过。缺失 source/box/step/slot 任一类型、缺失 identity、零复制预算与累计 work 耗尽均拒绝，空需求表有效。原有有限类型 golden 与编码保持不变。计划模块 136 行、类型导出 100 行、支持关系生产 46 行，新增测试模块 130/126 行。

callable、dispatch 与 object-value 组成表的实际生产、LIR layout/ABI、driver layout-strong 发布和一般跨 Cone nominal 消费仍待接通，整个 M23-6 保持进行中。确认全部构建测试进程退出后 cargo clean 移除 24,831 个文件、24.8 GiB，target 已清理。

2026-09-22：object 值与初始化 callable 共同生产批次通过 workspace fmt、clippy（含 all-targets）、2 项真实源码测试、既有 singleton 身份链测试及全部 5,661 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

LocalConcrete HIR 与 MIR 的 singleton 现在直接保存 Export HIR 已分配的 PersistentObjectValueId。MirObjectValueProductionV1 从 sealed strong MIR 的实际 published root、初始化 unit、initializer/ensure body、签名及 GC effect 同时生产 object-value 与两种 callable 记录。local 类型表决定导出范围，依赖类型通过共有联合索引借用；private object 与顶层 property 初始化不混入 object 导出，没有新增来源授权通道。

独立 object 与包含相互引用、companion、private object 和顶层 property 的组合 fixture 逐项核对三阶段身份、物理 root、签名及 GC effect，两份 golden 与两张表的 canonical wire 回读通过。前置无关 private 声明不改变任一表的字节；缺少 Unit 依赖、object/backing 类型、实际初始化根或 ensure，以及零复制预算、累计 work 耗尽均拒绝。新增生产模块 125/73/98 行，测试模块 127/176/83 行。测试中的 Unit 依赖记录取实际 MIR exact identity，仅用于组成表验证，未宣称已完成真实 core 的新 type profile 发布。

普通成员与构造器 callable、dispatch、LIR layout/ABI、driver layout-strong 发布及一般跨 Cone nominal 消费仍需继续接通，整个 M23-6 尚未完成。确认全部构建测试进程退出后 cargo clean 移除 25,300 个文件、26.3 GiB，target 已清理。

2026-09-22：构造器 MIR binding 实际生产批次通过 workspace fmt、clippy（含 all-targets）、3 项新增真实源码测试及全部 5,664 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

lower_constructor_bindings 接收同次 HIR type-semantics、LocalConcrete 与 sealed strong MIR，按 public/protected 构造记录选择已有 typed materialization，复用 MIR lowering 的 exact 构造签名投影，再与实际函数签名及 GC effect 核对。class 构造保留源码返回 class 与 lowered 隐藏 receiver/Unit 返回的区别，struct 主次构造保持值返回；private 实际构造不会进入导出表，缺失任一所需构造整体失败。类型依赖继续通过共有联合索引借用，未新增来源授权结构。

真实源码暴露并修复了旧 binding 的 GC 规则与既有主构造实现之间的冲突：新增明确的 PrimaryValueConstructor 角色（tag11），保持源码 Managed 合同与实际 NoGc 值组装 leaf；参数必须逐项等于完整字段 exact 类型，允许组装含引用的值。普通次构造保持 Managed，显式 @NoGC 次构造仍要求 GC-free，不能冒充主构造或放宽 effect。同步角色编码、回读、语义引用与 stage6/实现规范，既有 exact identity 格式不变。

独立 ZST 与组合 fixture 覆盖嵌套 ZST、含引用 struct、主次构造、@NoGC、protected 基类、默认参数调用本地构造、继承、private 构造及 object。两份 golden 锁定源码/实际签名与 GC effect，完整表 wire 往返及前置无关 private class 后的字节稳定性通过；缺少 Unit/owner 类型、LocalConcrete/MIR 物化、GC 不一致、错误主构造角色、零复制预算及累计 work 耗尽均拒绝。修复测试辅助代码将本地构造默认引用误判为 core 依赖的问题。生产模块 101/148 行，共有签名模块 221 行，新增测试模块 107/202/169 行。

普通成员/accessor 与生成 adjust/dispatch callable、dispatch 表、LIR layout/ABI、driver layout-strong 发布和一般跨 Cone nominal 消费仍需接通；构造组成表完成不代表完整 M23-6 完成。全部构建测试进程退出后 cargo clean 移除 23,983 个文件、25.4 GiB，target 已清理。

2026-09-22：普通源码 callable MIR binding 实际生产批次通过 workspace fmt、clippy（含 all-targets）、3 项新增真实源码测试及全部 5,667 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

lower_source_callable_bindings 合并同次 HIR 公共 callable、protected 成员及继承槽所需声明/实现，按已有 function/accessor 身份去重，复用 LocalConcrete 的 exact 签名投影并连接 sealed MIR 的实际 body、签名与 GC effect。访问器沿用 Storage/Constant/Body/AbstractSlot 分类，直接存储与常量不虚构 body；属性合同中独立受限的实际 setter 共同进入导出。abstract interface 仅保留签名，abstract class 使用实际 trap；generic、intrinsic、extern 与构造遵循各自入口，未新增来源授权结构。

真实再次抽象化源码揭示并修复旧 trap 规则：子类 abstract override 保留基类 slot，body 属于子类声明。共有验证核对源码 owner、abstract receiver、严格基类链及除 receiver 外相同的签名；无关 class body 或缺失基类类型被拒绝。实现规范与 stage6 设计同步，wire 与 exact identity 格式不变。

独立与组合 fixture 覆盖普通/extension 函数、NoGC、public/protected/private 成员、自定义 getter/setter、直接存储、接口默认与抽象成员、class override、继承后再次抽象化及 object。两份 golden、整表 wire 往返及前置无关声明后的字节稳定性通过；缺失 LocalConcrete/MIR 物化、签名类型、源码结果或 GC 不一致、零复制预算和累计 work 耗尽均拒绝。生产入口及子模块分别 151/166/136/65/33 行，trap 验证模块 97 行，新增测试模块均不超过 163 行。

生成 boxing/dispatch adjust 与 derived equality callable、dispatch 表、LIR layout/ABI、driver layout-strong 发布和一般跨 Cone nominal 消费仍需继续接通，整个 M23-6 尚未完成。确认全部构建测试进程退出后 cargo clean 移除 25,095 个文件、27.2 GiB，target 已清理。

2026-09-22：装箱分派 callable 实际生产批次通过 workspace fmt、clippy（含 all-targets）、2 项新增真实源码测试及全部 5,669 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

BoxingAdjust 现在必需保存同次 lowering 选定的目标 FunctionId，与实际 thunk 及 itable 位置共同保留；目标直接来自 typed HIR conformance。CanonicalMirCallableBindingsV1::from_boxing_adjusts 按本地导出 payload 选择已有 thunk，连接 sealed strong 根、真实目标 body、共有 callable/type 表及实际签名和 GC effect。目标 semantic signature 与 thunk lowered signature 分别保存，支持值方法、getter 和采用 interface default 的同一 box 重解释；private 非导出 payload 的实际 thunk 不进入表。身份与 wire 格式不变，未新增来源授权结构。

菱形继承组合暴露并修复未使用默认 body 的生产缺口：非泛型接口的默认声明均以自身 owner 进入 LocalConcrete，即使当前没有实现者，或全部具体实现都选择子接口 override；abstract slot 保持签名，generic 接口保持既有 application 规则。接口具体化从 nominal 模块拆出为 145 行独立模块，原模块缩短到 414 行。

独立 ZST 与组合 fixture 覆盖 value method/getter、private payload、含引用 struct、enum、接口默认实现、菱形继承、被覆盖及无人使用的默认 body、object 排除。逐项核对真实 MIR 正文中的 direct-call 目标、itable 位置和签名；两份 golden、整表 wire 往返及前置无关声明后的字节稳定性通过。缺失目标 binding、类型、无效目标 arena 位置、目标 GC 与实际 body 不一致、零复制预算及共享 work 耗尽均拒绝。源码 fixture 遵循现有接口实现 effect 一致规则。生产模块 102/90 行，新增测试模块 137/112/123 行。

derived equality、dispatch 表及其余实际 callable 组合、LIR layout/ABI、driver layout-strong 发布和一般跨 Cone nominal 消费仍待接通，整个 M23-6 保持进行中。确认全部构建测试进程退出后 cargo clean 移除 24,254 个文件、26.2 GiB，target 已清理。

2026-09-22：MIR dispatch schema 实际生产批次通过 workspace fmt、clippy（含 all-targets）、3 项新增真实源码测试、23 项 dispatch 组成表测试及全部 5,673 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

lower_dispatch_schemas 将同次 HIR 完整槽序和实现选择连接到 sealed MIR 的实际 vtable、itable 与 boxing adjust，逐项核对目标 Strong identity、receiver、参数、结果及 GC effect。object 与 backing 各自导出 schema 并共享实际表；依赖通过共有 callable/type/schema 索引借用。缺失本地类型、body、目标或依赖整体失败，查询、复制及 canonical 化共享预算，没有新增来源授权结构。入口与三个生产子模块分别为 119、122、102、121 行。

非泛型 abstract interface 声明现在使用原声明 owner 物化真实 fatal trap，继承 conformance 复用同一声明，避免为子接口生成第二份同身份函数。abstract class 再次抽象化保留根 slot，使用实际 derived trap。interface provider 核验保留的共同父槽签名与相对顺序，typed override 抑制后的完整序列仍由 HIR join 验证。object 的方法和 accessor override 已补入共有 virtual family 更新，修复 itable 已更新而 vtable 仍调用 base body 的缺陷；backing→source object receiver 只接受相同 typed object/backing 关系，不进入继承图。

真实 core 发布暴露了原 callable fingerprint 遗漏 trap C 字符串的问题。共有 ObjectDefinition 计算现在纳入当前 body 已验证的 AddressTakenConstant atoms、完整 bytes 和正规化重定位；仅允许指向自身关联 atom，原 runtime-scan 顺序及无常量的固定向量保持不变。原 767 行文件按主流程、runtime scan、constant 和错误类型拆为 438、242、67、114 行。真实 core 双视图发布、source/cache 重建和普通依赖进程测试均通过。

独立与组合 fixture 覆盖 virtual prefix、final override、再次抽象化、默认实现抑制、菱形继承、value/enum 装箱、getter/setter、object/backing 及不同 singleton receiver 隔离；两份 golden、wire 往返、前置无关声明后的字节稳定性及共享预算耗尽均通过。仅检查 HIR source metadata 的旧测试直接使用 HIR fixture，保留 suspend 声明合同覆盖；真实 MIR 测试直接引用原接口 typed declaration，移除手工重复声明。suspend 执行所需 generic coroutine 协议仍遵守相邻 ODR 阶段的边界。

derived equality 与其余实际 callable 组合、完整 MIR type bridge 组装、LIR layout/ABI、driver layout-strong 发布、一般跨 Cone nominal 消费及剩余 ZST 矩阵仍需继续接通，整个 M23-6 尚未完成。确认全部构建测试进程退出后 cargo clean 移除 43,048 个文件、43.5 GiB，target 已清理。

2026-09-22：derived equality MIR binding 实际生产批次通过 workspace fmt、clippy（含 all-targets）、2 项新增真实源码测试及全部 5,675 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

lower_derived_equality_bindings 遍历同次 LocalConcrete 已请求的完整生成函数，按已有 DerivedEquality key 连接本地导出 owner 与 sealed MIR 的实际 Strong root。源码签名复用共有 exact 投影，semantic GC 来自 HIR，lowered GC 来自实际 MIR，逐项检查 receiver、唯一同类型参数、Boolean 结果和 effect。私有 owner、未请求候选与显式同签名 equals 不虚构派生导出；身份、wire 和 stage 依赖保持原规则，未新增来源授权结构。

独立和组合 fixture 覆盖空 struct、嵌套字段、enum、自定义 equals、不同参数 overload、非可比较字段及私有类型。两份 golden 锁定实际 MIR 调用关系，wire 往返、前置无关声明后的字节稳定性、缺失 Boolean 类型或实际 Strong root、零复制预算和共享 work 耗尽均通过验证。测试复用已有完整依赖图导入流程，补全 Boolean 的 nominal key；依赖类型仍只是组成表 fixture，不表示 full core 已通过新 profile。生产三个模块分别 61、66、37 行，新增测试模块均不超过 102 行。

其余实际 callable 组合、完整 MIR type bridge 组装、LIR layout/ABI、driver layout-strong 发布、一般跨 Cone nominal 消费、剩余 ZST 矩阵与 core 专用资格清理仍需继续完成，M23-6 保持进行中。确认全部构建测试进程退出后 cargo clean 移除 22,653 个文件、24.4 GiB，target 已清理。

2026-09-22：canonical ABI 共有重放批次通过 workspace fmt、clippy（含 all-targets）及全部 5,682 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。随后补充普通 consumer 的真实 Unit/Int/String 调用组合，再次通过 fmt、lint 与完整 core 修改重建/消费集成测试。

旧 callable bridge 删除 current == CORE 豁免，按每个实际 callable 所属 artifact 的 identity 图和可达依赖 witness 重放；同一 owner 的重复 witness 必须完整一致。layout profile 删除 core foundation 回退，在完整本地与依赖 layout section 校验后，查询每个 exact type 唯一的 ManagedValue layout。两条路径使用 LIR 共有的 direct/indirect/ZST 分类，并保留完整 logical signature、GC effect、参数顺序与返回方式比较；查询、重复记录比较和签名复制使用当前 artifact 的连续预算。

新增七项独立/组合/negative 测试覆盖 receiver、Unit、scalar、managed reference、非零与零尺寸 aggregate、tagged/niche enum、混合 provider 的同名类型、缺失/重复/wrong-role layout、冲突 witness、相同错误 ABI 在 core 和普通 provider 中被拒绝，以及共享预算耗尽。真实 core 新增两个普通导出函数，golden 核对 ABI；普通 consumer 嵌套调用 Unit 函数，并消费 Unit/Int/String 混合参数函数。wire 格式、persistent identity 与 runtime C ABI 不变。共有分类、依赖类型收集和布局 ABI 查询分别为 42、67、102 行，新增测试和辅助模块均不超过 117 行。

本批完成两处 ABI 重放豁免清理；native-boundary 专用外来类型入口与固定 core 身份识别、protocol 来源资格、String/初始化角色外层、Link 专用闭包，以及 M23-6 原有完整 MIR/LIR 生产、跨 Cone nominal 实际消费、双 view 发布和剩余 ZST 矩阵仍需继续完成。确认全部构建测试进程退出后 cargo clean 移除 27,902 个文件、29.4 GiB，target 已清理。

2026-09-22：native-boundary reader 共有声明解析批次通过 workspace fmt、clippy（含 all-targets）、HIR/identity 的 1,461 项定向测试及全部 5,685 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程，使用 LLVM 22.1.8。

删除 resolve_with_external_core、from_external_core_source 及其专用错误分支。foundation reader 对本地和外来 owner 调用同一 canonical 声明解析，保留 kind、参数个数、binder、字段及 variant owner 验证；只有外来 id 而没有实际声明 key 的记录被拒绝。字段/variant 完整性改用同一已验证 identity graph 中当前与依赖的 canonical records，缺失依赖成员不能以空形状通过。新增共有 closure_records 查询复用原记录重建实现，共享 canonical key，不将依赖重新登记为本地定义；扫描、排序与输出分配使用调用方预算。

五项新测试替换两项旧 core 特许路径测试，覆盖 core/普通 provider、core/普通 consumer、非空 struct 与 enum、generic 声明的结构往返、缺失 canonical key、遗漏字段/variant、依赖重复折叠、key 共享、本地 inventory 不变及连续预算耗尽。现有 source-kind/参数/成员负例与真实 core 修改重建、ABI 混合参数消费、缓存及双 view 回归全部通过；wire 格式、persistent identity、runtime ABI 及 ODR gate 不变，generic 结构解析不授予 application 执行能力。记录查询独立为 122 行，reader 组合入口为 28 行，native-boundary 主体与 decoder 分别缩短至 566/513 行，新增测试辅助模块不超过 184 行。

本批完成 reader 的 external-core 特殊恢复清理。TrustedCore/ImportedCoreNativeBoundaryTypes producer 输入、后端 CoreNativeBoundaryNominal 识别、其余 protocol/String/初始化/Link 清理，以及 M23-6 原有完整生产与消费验收仍继续推进。确认全部构建测试进程退出后 cargo clean 移除 21,494 个文件、23.1 GiB，target 已清理。

2026-09-22：已补齐 native-boundary 共有 producer 所需的 struct 声明策略。共有 public source shape、nominal source contract 和 nested source support 必需保存实际 HIR 的 CLayout policy，ordinary、泛型和 protected nested 声明使用同一投影。`cross-cone-interface` capability 升级至 v2，struct shape 增加必需 policy 字段；旧 v1 section（包括 optional 混入）与缺 policy 的旧 payload 拒绝，profile descriptor/fingerprint、registry、Compile/Link inventory 和 wire golden 同步更新。普通与带预算的 representation reader 均逐项核对字段、aligned/packed，拒绝策略不一致及空 CLayout；native-boundary witness/identity/runtime C ABI bytes 不变。source-shape wire 按职责拆为独立模块。新增独立、嵌套/泛型组合源码 fixture 与 golden，并覆盖完整 policy wire、旧格式、错误 policy 和共享预算。格式化与全 workspace lint 通过，定向三 crate 回归 3,289 项通过；补齐 metered reader 后全 workspace 5,691 项通过；随后只增加独立 fixture 并重新运行 fmt/lint 与两项源码 fixture，全部通过。确认无构建进程后 cargo clean 删除 27,055 文件、31.5 GiB。`TrustedCore`/ImportedCoreNativeBoundaryTypes producer 输入和后端固定身份分类仍未删除，下一步接通共有依赖声明查询及字段/variant 传递闭包；本条不表示 M23-6 完成。

2026-09-22：native-boundary producer 共有依赖闭包批次通过 workspace fmt、clippy（含 all-targets）、66 项定向测试及全部 5,701 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程。随后补充私有引用声明与 provider 顺序稳定性验证，再次通过 fmt、lint 和 67 项定向测试。

删除 HirNativeBoundaryExternalTypes 的 CurrentArtifactOnly/TrustedCore 输入、ImportedCoreNativeBoundaryTypes 固定名单及其专用错误适配。实际 HIR lowering 直接借用共有 ImportedSemanticWorld，从声明真实 provider 的 canonical key、共有 v2 source shape 和字段/variant key 生成最小 witness；非公开声明只沿已有 typed 引用复用 provider 的已验证 witness，Reference 由真实声明 kind 决定，不增加公共名称查找。公开 shape 与已有 witness 同时存在时必须一致，缺少 value shape 不猜测默认布局。本地和外来记录进入同一字段、variant payload 与嵌套 signature-type 遍历，按 owner 去重排序。

独立与组合源码 fixture 覆盖窄整数、全部八种整数、Boolean/String/Unit、本地 CLayout、嵌套 struct/enum 和未使用类型排除；共有查询覆盖 core 与普通 provider、direct/support 混合闭包、泛型 binder、私有引用声明、typed 私有 witness、canonical 成员缺失或错 owner、source kind/binder 错误、shape 冲突及缺失传递 provider。两份 golden 与 wire 往返通过，provider 顺序扰动不改变输出 bytes。生产查询与 shape 投影各 68 行，闭包遍历独立为 22 行；新增测试按生产、闭包、拒绝与 fixture 支持拆分。native witness wire、persistent identity、runtime C ABI 和 ODR gate 不变。

本批完成 producer 的 core 专用外来类型入口清理。后端 CoreNativeBoundaryNominal 固定身份识别、protocol/String/初始化/Link 清理，以及 M23-6 原有完整 MIR/LIR 生产、双 view 发布、跨 Cone nominal 消费和剩余 ZST 矩阵仍待完成，目标保持进行中。确认全部构建测试进程退出后 cargo clean 删除 24,903 个文件、30.4 GiB，target 已清理。

2026-09-22：compiler protocol constituent 来源资格清理批次通过 workspace fmt、clippy（含 all-targets）、29 项 protocol 定向测试及全部 5,708 项 workspace 测试（37 组，零失败、零忽略），含配套 scoopc 真实进程及实际 core 修改、重建、缓存、双 view 发布和普通依赖消费回归。

删除 protocol callable 与 product producer 的 CORE gate、reader 的 require_core_source/require_core_nominal_owner 与 NonCoreDefinition、导入的 FoundationNotCore 判定及对应错误分支。实际调用链继续解析同一 foundation 的 canonical 声明和 definition origin，完整检查 signature、owner/own binder、constructor result、generated adapter source、enum kind/member owner、role effect、operation 覆盖和协议间关系；Unit 保留既有语言内建身份。ImportedCoreProtocols 直接借用已验证的协议组成表，逐项解析当前共有 semantic session 的 typed id，不重新投影专用来源凭证。旧 Core/NotCore 外层和重复 operation 表仍待后续迁移，本批不将删除 constituent gate 等同于整个 protocol 清理完成。

共有 metadata fixture 现在按真实 library coordinate 构造 provider。六项新增测试及原有 callable 测试的双 provider 组合覆盖普通/core product 往返、definition origin 缺失、错误 enum owner、generated member、constructor 与生成 adapter 的 source/result、缺失 adapter source、错误 operation result、另一 provider 的同名目标替换及共有导入 id 保持。错误由现有 typed 关系和缺失 identity 诊断拒绝，不借名称补目标；原 wire/profile golden 不变，前端 intrinsic 使用边界、runtime ABI 和 ODR gate 不变。

原 994 行 callable 模块按数据/wire、生产、验证、错误拆为 208/363/323/98 行；原 849 行 product wire 拆出角色布局、引用验证与错误，主体缩短至 576 行。原 876 行测试构造按入口、identity builder、product 和 operation 分离；新增测试模块分别为 77/99/110/49 行。后端固定 core 身份分类、protocol 外层与重复投影、String/初始化/Link 共有消费及 M23-6 原有完整 MIR/LIR 生产、跨 Cone nominal 与 ZST 验收仍继续推进。确认全部构建测试进程退出后 cargo clean 删除 20,977 个文件、23.4 GiB，target 已清理。

2026-09-22：完整 intrinsic operation 重复表退役批次通过 workspace fmt、clippy（含 all-targets）、HIR/SLIB 1,688 项回归、HIR lowering 1,627 项回归，以及配套 scoopc 的全部 5,716 项 workspace 测试（37 组，零失败、零忽略）。真实 core 源码修改、重建、任意产物路径消费、缓存与双 view 回归均通过。

删除 CoreCompilerOperationV1、CoreCompilerOperationProtocolV1、全表 producer/reader 和重复协议交叉投影；compiler protocol product 仅保留 field 1～8，field 9 永久退役。core-bootstrap-interface 升级为 v2，三个 strong profile 的 inventory、descriptor/fingerprint 与 wire golden 同步更新；按 namespace/name 统一拒绝旧版本或混用版本，即使旧 section 声称 optional。原 constituent typed 引用、persistent identity 与 runtime C ABI 编码保持不变。

完整 intrinsic kind 来自共有 callable effects，普通 Compile 及 layout profile 的两个 public-source 验证入口均重放 canonical source owner、own binder、参数、结果和 execution，并核对固定语言角色的声明 id；重复 kind、缺少或冲突角色、错误 kind/owner/签名不能通过。验证复用当前 artifact 的 identity graph、可达 provider 与连续预算，不再发布完整 operation 名单。固定语言角色继续保存必要 typed 引用与合同；原先只由旧表检查的 GC control 声明合同前移到 HIR，基础库缺失 GcCollect/GcStats 或参数、结果、receiver、泛型、suspend 形状错误均在源码位置诊断。

新增共有合同测试覆盖所有 intrinsic kind 的 core/普通 provider 与 wire 往返、整数 owner/位宽和 shift 参数替换、固定角色目标替换、wrong source kind/binder/effect，以及实际 Compile 入口的缺失角色、重复 kind 和零/连续预算耗尽；旧九字段 payload 和 capability 均有拒绝测试。按职责新增的生产模块分别为 39/81/91/114 行，product 主体与 wire 缩短到 585/455 行，测试与构造辅助模块均独立。确认无构建测试进程后 cargo clean 删除 33,598 个文件、41.0 GiB。

本批完成完整 operation 表退役，未将其等同于 M23-6 完成。Core/NotCore 外层、native-boundary 后端固定身份重建、String/初始化与 Link 共有化，以及原有完整 MIR/LIR 生产、跨 Cone nominal 消费、双 view 与 ZST 验收仍继续推进。

2026-09-22：共有 nominal intrinsic 与公开常量/vararg 查询批次通过 workspace fmt、clippy（含 all-targets，无警告）、独立与组合 producer fixture，以及配套 scoopc 的全部 5,730 项 workspace 测试（37 组，零失败、零忽略），含真实 core 修改、重建、缓存、任意输出路径消费与双 view 回归。

共有 NominalSourceShapeV1 新增 tag 6，保存已有 NominalIntrinsicRepresentationV1 的完整 family；public nominal、source contract 与 nested support 复用 kind、binder 数量和 bounds 合同。producer 不再把 intrinsic 退化成空 struct 或普通 class，普通与带预算的 representation join 均核对完整 family。cross-cone-interface 升级为 v3，旧 v1/v2 section（包括 optional 混入）和缺 family 的 payload 拒绝并重建；profile descriptor/fingerprint、Compile/Link inventory 与 golden 同步。native witness 继续按真实声明 kind 投影既有格式，persistent identity 和 runtime C ABI 不变。

公开常量按自身 value type id、vararg 按实际参数中的 generic template id 查询同一 identity graph 和可达 provider，验证该声明的 intrinsic family 与完整元素关系。删除这两条消费路径的 trusted_core 查询、current_core 输入及 protocol 的 const_value_source_type/array_source_type 重复 getter；不从 CORE、名称、相同布局或其他候选补目标。共有查询直接借用 canonical key，provider 与记录查找使用当前 artifact 的连续预算。测试覆盖全部十四种 intrinsic family、三类 source builder、wire 往返与错误 payload、普通 provider 和多个同名 provider、缺失声明/不可达 provider、错误 family/kind/binder、常量非 nominal 类型、vararg 元素/参数数量不符及连续预算耗尽；独立源码 golden 使用实际整数声明名，透明 alias 不生成新身份。

nominal shape 的 enum 数据与 wire 按职责拆分，主文件缩短到 366 行；共有 reader 的 nominal 查询与 intrinsic 检查分别独立为 92/119 行。全部构建测试会话结束后 cargo clean 删除 30,183 个文件、37.9 GiB。本批完成公开常量/vararg 路径；protected/internal 查询、Core/NotCore 外层、native 后端固定身份分类、String/初始化/Link 共有化，以及原有 M23-6 完整生产、跨 Cone 消费和 ZST 验收继续推进。

2026-09-22：protected callable 与 dispatch slot 的 Unit 输入清理通过 workspace fmt、clippy（含 all-targets，无警告）及 HIR/HIR-lower/SLIB 共 3,329 项回归（零失败、零忽略）。删除 bind_protected_callable_sources、bind_slot_sources 仅为取得 Unit 而接收的 ImportedCoreFundamentalTypeProtocol 参数，protected binding 不再保存第二份 Unit nominal；普通 nominal member、dispatch 与 setter 统一引用语言内建 CoreBuiltinNominal::Unit。exact Unit key、setter 参数和结果、slot signature、实际实现选择、effect、owner 与资源预算检查保持。

已有独立与组合源码 fixture 及 golden 覆盖 protected method/generic/accessor/override、class/interface slot、默认实现、继承和完整 sysroot source foundation；错误 setter 返回/参数、错误 owner、实际实现替换、同步伪造 root/target effect 及预算耗尽负例全部通过。测试辅助入口同步删除多余 core 协议导入，相关生产模块均少于 160 行；wire/profile、persistent identity、runtime ABI 与 ODR gate 不变。常量、Array 和其他源码 nominal 不能使用 Unit 内建例外，其共有声明查询以及其余 M23-6 实现与验收继续推进。

2026-09-22：HIR/MIR 协议定义外层清理通过 workspace fmt、clippy（含 all-targets，无警告）及配套 scoopc 的全部 5,735 项 workspace 回归（37 组，零失败、零忽略）。测试含真实 core 修改与重建、公共缓存、任意输出路径消费、Compile/Link 双 view、普通 provider 协议导入，以及 CORE/普通 provider × library/executable × 初始化角色有无的 MIR 组合。

HIR 删除 CoreHirInterfaceBranchV1 和 Core/NotCore 资格分支，以 CompilerProtocolDefinitionsV1 保存实际完整定义。core-bootstrap-interface 升级为 v3：section 保留 field 2 output 与 field 3 direct surface，新增 field 4 的 0/1 definitions array；旧 field 1 及其 tag 1、2 退役，旧 v1/v2、旧字段、多份定义与旧 sum 替换 array 均拒绝。producer 依据实际 Defined/Imported 发布，reader 按同一 foundation 校验；三个生产 profile 的 descriptor/fingerprint、Compile/Link inventory 和 golden 同步更新。公开 intrinsic 按自身 canonical 声明 origin 查询对应可达 provider 的类型角色，缺失或重复 provider 不能由其他候选补足，查询继续计入当前 artifact 的预算。

String capability 删除固定 CORE、名称、包、owner 链与 scope 检查，保留非泛型 class、source/exact nominal 关系及与 compiler protocol 的 String 身份一致性；新负例验证两份记录各自有效但指向不同 class 时仍失败。MIR 删除按 CORE 判定初始化角色必须存在、禁止出现或必须搭配 library 的分支，实际角色由相邻 HIR 定义核对。角色唯一性、source function、完整 strong signature 覆盖与重放、ordinary effect、String 参数与 Unit 返回、main/entry 归属和实际实现检查继续执行，旧角色字段、persistent identity、String 表示和 runtime C ABI 不变。

String 的测试按职责拆到独立模块；相关 HIR section、String capability 和 MIR production 主文件分别为 348、298、451 行。全部构建与测试结束后 cargo clean 删除 40,949 个文件、41.8 GiB。本批完成 HIR/MIR 外层清理，LIR 初始化桥、String descriptor/registration、Link 的 core 独立闭包、native 后端固定身份重建，以及 M23-6 原有完整 MIR/LIR 生产、跨 Cone nominal/member/dispatch 实际消费、双 view 发布与 ZST 验收仍须继续完成。

2026-09-22：初始化服务 LIR 共有 ABI 与导入选择清理通过 workspace fmt、clippy（含 all-targets，无警告）及配套 scoopc 的全部 5,738 项 workspace 回归（37 组，零失败、零忽略）。测试含真实 core 修改与重建、公共缓存、任意输出路径消费、Compile/Link 双 view，以及普通 provider 的初始化 callable 投影、选择、物化和混合调用。

删除 CoreLirBridgeV1、CoreLirBridgeBranchV1、对应 decoder 和来源资格错误，LIR 直接发布完整 CallableAbiRecordV1。strong production 两种 schema 均退役 field 10 和旧 Core/NotCore 包装，field 11 使用严格 0/1 array；strong-production capability 从 /1、/2 升级为 /3、/4，三个 profile 的 descriptor、fingerprint、Compile/Link inventory、producer/reader 与规范引用同步。旧字段、旧 sum、多份 ABI、旧版本及 optional 交叉版本混入均拒绝。

MIR/LIR 的 ImportedFoundation 投影和 SelectedExternal 集合不再追加 CORE provider 条件，ShapeLink 按实际 provider 核对 ABI definition，并按 typed target 保持用途互斥。普通与 CORE provider 的实际 body、symbol、definition、canonical ABI、effect 和 root plan 检查保持；负例覆盖错误 provider、同名异源 target、缺 body/definition、错签名、重复角色/目标、self-import，以及相邻 MIR/LIR 角色有无和完整逻辑签名不一致。

新的初始化 ABI 编解码模块为 56 行，测试和导入投影验证按职责保留独立模块。全部构建测试会话结束后 cargo clean 删除 26,562 个文件、26.9 GiB。本批完成初始化 ABI 发布与导入选择清理；String descriptor/registration、旧 external bridge 中的固定 CORE provider、Link 独立 core owner/requirement 通道、native 后端固定身份识别，以及 M23-6 原有完整 MIR/LIR 生产、跨 Cone nominal/member/dispatch 实际消费、双 view 发布与 ZST 验收仍须继续完成。

2026-09-22：String 与初始化服务的共有外部引用及 registration 编码已完成本批实现和验收。

外部 callable 表直接复用完整 SelectedDependencyLirCallableV1 与普通依赖解码器，provider、声明和完整 ABI 一同保存；String 继续复用 ExternalTypeDescriptor。删除 CORE consumer 空表限制与 String lowering 的固定来源分支，按实际 provider 检查 self-import 和合同。TD、可选 TD、dispatch callable 及 immortal type registration 均删除 CoreExternal，并显式保存 provider 与 exact/body。两个生产 schema 共用引用 codec；reader 同时核对 provider、typed target、已选择的 definition 及用途分区，服务与 layout 的重复来源仍拒绝。未使用 String 的 MIR 无需物化对应类型条目；LIR 保留完整运行时描述符，MIR 实际使用 String 时继续核对 exact identity。

strong production 的旧 field 1 退役，field 12 保存共有外部引用表；callable tag 1 退役，tag 3 保存完整共有记录。TD、optional TD、dispatch 和 immortal 的旧 core tag 均拒绝且不复用。strong-production capability 升级至 /5、/6，cross-cone-layout-abi 与 cross-cone-layout-link-closure 升级至 /2；三个生产 profile 的 registry、inventory、descriptor、fingerprint、Compile/Link reader、固定向量及文档引用同步。初始化 field 11 的 0/1 array、local/runtime/absent 编码和 runtime C ABI 保持。

workspace fmt 与 all-targets Clippy 通过；定向 LIR/LIR-lower/SLIB 1,169 项通过，补齐未使用 String 的边界后 lowering 131 项通过；最终全部 5,744 项 workspace 测试通过（37 组，零失败、零忽略），启用配套 scoopc 真实进程，覆盖 core 修改、无 sysroot 构建、source/prebuilt 切换、缓存、跨 Cone 调用与独立 Compile/Link 发布。普通 provider 的完整 String/初始化服务组合、provider 与 definition 漂移、self-import、同一 target 的不同 provider、重复证明、旧字段/tag/版本与 optional 混入均有验证。共有外部表主体 136 行，wire/错误/引用查询分别独立；immortal 模块从 750 行左右拆为登记 515 行与语义 266 行。全部构建测试会话结束后 cargo clean 删除 23,950 个文件、21.5 GiB。

本批完成 LIR 外部引用与 registration 编码迁移；Link 独立 core owner/requirement 通道仍须并入共有实际 provider 索引，native 后端固定身份分类，以及 M23-6 原有完整 MIR/LIR 生产、跨 Cone nominal/member/dispatch 实际消费、双 view 发布和剩余 ZST 验收继续推进。

2026-09-23：native intrinsic 表示与标量 ABI 重放

- NativeBoundaryNominalShape 以新 tag 4 保存完整 NominalIntrinsicRepresentationV1；本地 struct/class 与依赖 source-shape 投影保留实际 family，构造和 reader 核对声明 kind、泛型参数数量、字段覆盖及实际 provider。
- HIR identity-foundation 升为 /2，退役 native witness field 30 并以 field 33 保存记录；foundation、single-cone、cross-cone semantics、layout 四种 profile 的 inventory、固定向量与 fingerprint 同步，旧版本及 optional 混入均拒绝。历史阶段格式和 M24 后续 major 预留同步说明。
- C ABI 的整数/Boolean 分类及 canonical Scoop ABI 的 intrinsic 布局通过实际 typed 声明记录计算；Unit 使用语言内建 identity。普通 callable 可从 provider 已验证的共有 nominal 表取得 intrinsic 表示，即使没有 extern 使用也能完成 ABI 重放；已有 native witness 与共有声明必须一致，错误 provider、表示冲突、缺失声明及预算耗尽均拒绝。Ptr/FunPtr 仍要求结构化 exact key。
- 结构、错误、解码与物理布局计算按职责拆分；增加完整 family 往返、kind/arity、字段覆盖、普通 provider、缺失 witness、混合 provider ABI、旧字段/major 和持续预算验证，更新 native source golden 与缓存固定向量。
- 已通过 cargo fmt --all、cargo clippy --workspace --all-targets、HIR/lowering/SLIB 定向回归及配套 scoopc 的完整 cargo test --workspace：37 组、5759 项通过，0 失败、0 忽略。完成后 cargo clean 清理 33260 个文件、35.2 GiB。

本批完成 intrinsic 表示和对应 ABI 消费。Option、PinnedPtr/GcHandle 的固定身份路径、独立 core Link requirement/owner 分区及 M23-6 其余完成门继续实施，未据此把整个 M23-6 标记为完成。

2026-09-23：共有 Link dependency requirement 与实际 provider

删除独立 core requirement proof、core owner 参数及空 core owner 补位。String descriptor、初始化 callable、type-registration support 和 ordinary callable 在同一个分类步骤中使用实际 provider、规范化 symbol 与 typed owner 查询；所有依赖来自已验证 artifact owner 集合。保留 self-import、缺失或重复 provider、owner 漂移、symbol 重复、用途重复认领、未实际使用的 callable、relocation 互斥与联合覆盖检查；metadata-only descriptor 与 native/runtime/target 余集保持原有语义。driver、Compile/Link reader、artifact 重开与 fingerprint pipeline 已使用同一入口。

最终 requirement 的 CoreStrong tag 2 退役，DependencyStrong tag 8 显式保存 provider/typed owner；对象 definition fingerprint 的独立扁平 target 域使用新 tag 13，保留已有 8～12 的含义。link-identity-closure 升级为 /2，三个生产 profile、fixed vectors、writer/reader 与旧版本拒绝同步；immortal registration 的验证和 fingerprint 保留实际 provider，runtime C ABI 不变。

新增 CORE consumer 同时引用三个实际 provider 的 callable/descriptor/registration/ordinary-callable 组合、依赖枚举顺序稳定、错误 provider/owner、重复用途及最终 requirement/wire 的正反例。先执行 cargo fmt --all、cargo clippy --workspace --all-targets（无警告），SLIB 577 项测试通过，重建配套 scoopc 后完整 workspace 37 组、5764 项通过，0 failed、0 ignored。分类主体、依赖查询、错误及新增组合测试分别为 327、138、86、232 行。全部测试进程退出后 cargo clean 删除 22,535 个文件、23.7 GiB。

本批完成共有 Link requirement 合并。native Option/PinnedPtr/GcHandle 固定身份路径、跨 Cone nominal/member/dispatch 的真实消费、新 layout profile 完整生产发布与其余 M23-6 完成门继续推进。

2026-09-23：GC handle 的 Scoop ABI 按实际字段重放

删除 native canonical Scoop ABI 中 PinnedPtr/GcHandle 的固定 core 身份标量分支。二者现在与其他 declared struct 共用完整 nominal/字段闭包、布局与 GC-free 重放，非空值按 typed ABI 间接传参和返回；C 透明表示不再覆盖 Scoop 表示。语言规范 14.1 明确透明性只适用于 C ABI，runtime 4.2 同步真实 core wrapper/runtime intrinsic 的职责；不改变 runtime C 入口、handle 编码、wire 或 persistent identity。

新增独立 typed fixture 验证 CORE 与其他 provider 的同名 struct、实际 UInt64/UInt8 字段布局、缺失 nominal/字段 witness 拒绝；组合 fixture 与 golden 锁定标量、handle、Unit 混合参数、indirect result 和完整签名 wire 往返。先 cargo fmt --all 与 cargo clippy --workspace --all-targets（无警告），SLIB 580 项测试全部通过；重建配套 scoopc 后 driver 的 43 项测试全部通过，包括真实修改 core、双 view 发布、跨 Cone 调用和 source extern。新增测试模块 252 行。测试进程退出后 cargo clean 删除 13,713 个文件、15.7 GiB。

本批修复 Scoop ABI 重放；C 边界的 Option/PinnedPtr/GcHandle 固定身份分类与透明表示生产仍在后续迁移范围，未将该修复等同于完整 M23-6 完成。

2026-09-23：显式 native C 投影与共有表示重放

删除 CoreNativeBoundaryNominal 及 native 后端的固定 CORE 身份分类。native witness 以必需的 NativeBoundaryCAbiV1 保存 SourceRepresentation、带真实 PersistentFieldId 的 UInt64Field，或带真实 variant/payload id 的 NullablePointer；builder、reader 和目标重放分别验证完整结构、引用及实际字段类型。前端按已有 typed 声明角色正规化，共有依赖查询保留并核对投影；同名、同大小或相同 niche 的普通声明不会自动获得 C ABI。HIR concrete、MIR 与 LIR 显式传递 handle 的 UInt64 字段投影，Scoop 继续使用普通间接 aggregate，C 参数、返回、CLayout 字段、指针和 callback 使用 UInt64 storage。

HIR identity-foundation 升为 /3，native field 33 退役、field 34 保存四字段记录；旧三字段记录、旧 major/field 与 optional 版本混入拒绝。四类 profile 的 descriptor/fingerprint、HIR semantic hash、cache receipt 固定向量及诊断路径同步，M24 HIR major 预留顺延为 /4。runtime C ABI、String 表示、extern/callback canonical key 与 ODR 生产边界保持。

独立与组合源码 fixture、精确位置 negative fixture、HIR/MIR/LIR golden、typed wire 往返及错字段/variant/表示闭包负例通过。先 cargo fmt --all 与 cargo clippy --workspace --all-targets（无警告），相关 IR 的 3,876 项测试通过；重建配套 scoopc 并启用真实子进程后，完整 workspace 37 组中 5,780 项通过，唯一旧 cache receipt 固定向量按新 bytes 更新后定向复验通过，总计覆盖 5,781 项、0 ignored。driver 的 43 项包含真实 core 修改、双 view 发布与跨 Cone 调用回归。新生产逻辑按投影、wire、concretize 和 MIR 校验职责分拆。全部构建和测试进程退出后 cargo clean 删除 39,322 个文件、41.9 GiB。

本批完成 native 固定身份分类迁移。M23-6 的新 layout profile 正式生产、跨 Cone nominal/member/dispatch 实际消费及其余完成门继续推进。


2026-09-23：完整 MIR export 组装入口已接通既有实际生产器。source/有限 helper type、普通 callable/constructor、object 初始化 callable、boxing adjust、derived equality、dispatch 和 shape-support 按同一次 sealed HIR/MIR 输入组成六张完整表。依赖 type/callable/schema 使用共有只读索引；本地表不复制外来定义。旧 callable bridge 中的实际 typed implementation 逐项核对完整 exact signature 后只保留旧归属；不从 CORE、名称或 ABI 形状推断。显式 initialization-use 继续核对实际 local unit 物化 root 和 typed provider，所有投影、索引、合并和检查共享预算。未新增来源授权结构，七字段 section 的 source/selected 闭包仍由已有通用入口承担。

独立与组合 fixture 覆盖 ZST/nested value、primary/secondary constructor、protected constructor/member、继承、interface default、abstract trap、boxing adjust、derived equality、object/backing/ensure 和普通窄 callable 共存。两份 golden 锁定完整组成关系；六张表分别编解码往返，前置无关 private declaration 后字节不变。缺失/重复依赖、错误 Cone、旧 callable 导出缺项、未物化 local initialization unit 与共享预算耗尽均拒绝；非空 initialization-use 原样保留，借用依赖不进入本地 export。

cargo fmt --all 与 cargo clippy --workspace --all-targets 通过；scoop-hir-lower、scoop-mir、scoop-mir-lower 共 2,123 项测试通过、0 失败、0 忽略。本批统一关闭增量缓存和 debug 信息以控制 target 增长。完整 layout profile 正式生产、终端依赖消费闭包、一般跨 Cone nominal/member/dispatch 及其余 M23-6 完成门继续推进，本项组装完成不代表整个阶段完成。

2026-09-23：LIR 五表组装从同次 sealed MIR/LIR 与实际 Strong V2 registration 重放 layout、descriptor、dispatch、callable ABI 和有限 shape-support。callable 保留 receiver、重复参数及 Unit result，dispatch 逐项连接真实物理表与 MIR schema；BoxedValue 沿 payload 关系读取源码 schema，有限 step/slot helper 验证实际空表。依赖按实际 provider/target 查询，投影和检查共享预算。layout profile 在封存 LIR 前生成 canonical descriptor diagnostic name，后续 registration、导出与 object 使用同一实际名称。

修复 LocalConcrete → MIR 把所有非泛型 nominal 当作当前 Cone 的问题：source exact 使用完整 origin 分支保存真实 provider、application specialization 或 structural 归属；导入协议的 nominal provider 来自声明 key。外部 Unit、Boolean、整数等不再由 consumer 重发 TD/layout。String descriptor 同时核对 exact identity 与实际 provider。BoxingAdjust 的 receiver/local/正文保留实际 interface 类型，MIR 验证拒绝 Any 擦除；持久身份和 runtime C ABI 不变。

新增真实 core 依赖下的独立、组合源码 fixture 与两份导出 golden，覆盖 ZST、嵌套 value、重复参数、构造、继承、protected、interface default、abstract trap、boxing adjust、object 和派生相等性；缺失或重复依赖、缺 MIR type、缺 coordinate、错误 String provider 和共享预算耗尽均拒绝，前置无关 private function 不改变五表 bytes。修正旧测试的 nominal provider 构造及 consumer 重复定义快照。新生产模块最长 349 行，LIR 入口和测试 builder 按职责拆分。

每批变更先执行 cargo fmt --all 与 cargo clippy --workspace --all-targets；定向 MIR/MIR-lower、LIR-lower 及实际源码导出测试通过。更新并核对受归属修复影响的 golden 后，重建配套 scoopc，完整 workspace 37 组、5,786 项全部通过，0 failed、0 ignored，含真实 core 修改、缓存、任意输出路径消费与 Compile/Link 双 view 回归。确认本任务构建测试会话结束后 cargo clean 删除 2,871 个文件、6.5 GiB。

本批完成五表投影入口。无关私有 nominal 的本地物理定义与跨 Cone 导出边界、新 layout profile 正式发布、一般跨 Cone nominal/member/dispatch 消费以及其余完成门继续推进，M23-6 保持进行中。

2026-09-23：私有类型的本地物理定义与跨 Cone 导出范围按实际依赖闭包分别闭合。HIR 来源根遍历 struct、enum payload、class 与 object backing 的全部存储字段，递归保留 nominal/application、tuple、function 和 pointer 的组成关系；字段可见性不截断表示依赖。所有遍历使用同一预算，按 typed type/owner 去重，不扫描函数正文或展开无关私有 sibling。generic 字段只补充来源关系，既有 ODR gate 保持。

LIR 五表从完整 MIR source/support/helper 清单选择实际物理定义；无关私有 layout、TD、dispatch 和 body 继续进入完整 Strong production、registration、object 与 fingerprint。有限 shape-support 根及 helper 缺 MIR type 时仍拒绝。dispatch constituent 逐项验证实际 foundation，完整 section 要求导出 TD 的 vtable/itable 与 dispatch export 精确对应；Strong V2 join 核对每项 export 与实际 registration，并继续验证未导出本地定义的外部依赖。

真实源码组合 fixture 验证 Exposed → Hidden → Deep 的私有字段传递导出及缺失支持类型拒绝；三个源码场景前置无关 LocalOnly 后五表 bytes 不变，LocalOnly 的本地 layout/TD/registration 完整保留。新增 golden 锁定私有支持类型的布局、ABI 与 dispatch。独立 HIR fixture 覆盖 object、enum、tuple、function、pointer 与 generic 字段闭包，复用既有 core 源码入口验证指针结构，不放开 ordinary generic 导入。完整 section 的缺失或多余 dispatch、私有 registration 保留及其外部依赖缺项负例通过。

每批代码变更先执行 cargo fmt --all 与 cargo clippy --workspace --all-targets，定向来源根和真实源码导出测试通过。重建配套 scoopc 后完整 workspace 37 组、5,790 项全部通过，0 failed、0 ignored，包含 core 修改与重建、缓存、跨 Cone 调用和 Compile/Link 双 view。新增生产模块为 38～106 行，测试按职责拆分。确认构建测试会话全部退出后 cargo clean 删除 2,788 个文件、6.4 GiB。

本批完成私有类型导出边界。新 layout profile 正式发布、一般跨 Cone nominal/member/dispatch 的实际消费及其余完成门继续推进，M23-6 保持进行中。


2026-09-23：MIR source-join 接入独立的实际来源投影。适配器只接收同次 sealed HIR/MIR、借用依赖表与显式已提交 initialization-use，构造接口不接收待验证 candidate。完整 type/callable/dispatch/object 预期与 inventory 复用实际生产器投影，有限 shape roots 独立取 LocalConcrete 源码物化计划并核对；查询返回完整 typed 合同，不按名称、CORE 或布局猜测。投影与共有 validate_sources 沿用同一预算，不增加新的终端来源资格。

复用独立和组合源码 fixture 验证真实六表候选与独立来源相等；删除任一 type/callable/dispatch/object/shape 支持表均拒绝。同 identity、相同 facts 下调换 struct 字段顺序仍被完整 record 比较拒绝，非空初始化关系完整保留，删除关系或引用未物化 local unit 失败。共享预算耗尽与错误 source 查询覆盖。现有六表 golden 和编解码回归保持。

每批变更先 cargo fmt --all 与 cargo clippy --workspace --all-targets，随后 scoop-hir-lower、scoop-mir、scoop-mir-lower 共 2,129 项测试通过，0 failed、0 ignored。新增生产模块 54～70 行，测试模块 44～120 行；关闭增量缓存与 debug 信息后 target 当前约 2.5 GiB。本批完成实际 export source-join；HIR committed-use 来源、七字段 section 的正式生产、reader 来源重建、完整 layout profile 发布与一般跨 Cone 类型消费仍需继续，M23-6 未完成。

2026-09-23：默认参数 access profile 从同次 sealed source 独立投影。完整 nominal 默认来源同时保留参数表、正文与逐参数 profile，包含没有任何引用的默认值；canonical profile 表通过完整 default key 精确闭合，reader 拒绝重复、乱序、未知 tag/identity、缺失或额外字段，并共享资源预算。来源表不直接实现 profile authority，后续完整默认来源事务仍须重放来源与访问语义。

分类按实际直接 nominal owner、发布调用域及正文六类引用的调用/槽/目标域判断。只有函数自身 generic 时继续使用 ParamFree；public static nested 不因外层 generic qualifier 自动成为 metadata；generic enum variant 与仍含 generic subclass 约束的域保留 GenericSourceMetadata。候选 default witness 使用同一来源分类。完整 type-semantics 发布 fixture 验证静态嵌套 protected 成员、函数自身 generic 与继承 default 的实际 witness，并用 golden 锁定分类。独立及组合 source fixture 另覆盖 literal、generic enum、protected static nested 与继承正文；旧 public projection 测试辅助器补齐本地函数 typed identity，避免把本地引用误判为外来 CORE。

组合 fixture 同时暴露现有完整发布边界：param-free static nested 若受 generic 外层 class 的 protected 域约束，继承域尚不能取得所需 concrete exact identity。该场景目前只完成 source profile 投影与编解码验证，未宣称完整发布成功；它与 core 的 generic 继承物化边界一起保留为后续完成项，没有伪造 exact、删减继承约束或放开执行 gate。

每批代码先 cargo fmt --all 与 cargo clippy --workspace --all-targets，三个目标测试通过。重建配套 scoopc 后完整 workspace 37 组、5,796 项通过，0 failed、0 ignored。新增生产模块按 profile 分类、canonical 表与 wire 分开，原默认来源模块为 191 行；公共接口测试辅助器拆为独立模块。确认构建与测试退出后，cargo clean 删除 2,769 个文件、5.8 GiB。本批完成默认 source profile 生产；完整 default authority、HIR committed-use、正式 layout profile 发布和一般跨 Cone 类型消费继续推进，M23-6 保持进行中。

2026-09-23：默认来源域注册表扩展为共有 DefaultSourceDomainsV1，复用既有 Type 查询及同一 provider/identity graph 检查，增加 Constructor、Global、Singleton、Field 四类目标域重放。路由只从 canonical typed key 取得实际 provider，再在其 artifact 中核对目标记录、constructor/variant、logical property、object backing、零参 adapter 和 applied owner/arity 关系。原始域按真实声明重算，tuple 字段保持没有额外声明约束的结构语义；不从 witness、名称或所声称的 owner type 反推来源。

新增完整四类 occurrence 绑定，消费已完成声明、origin、nested identity 和正文引用闭包的结果，要求与当前访问表共用同一 bound foundation。每次独立比较完整来源域，重复引用、继承默认值与空正文均计入共享预算；错误保留 template key、引用种类与序号。返回凭证只覆盖这四类来源域相等，Type、Callable、receiver、操作类型、profile 和调用域 coverage 各自继续验证。

独立和泛型组合源码 fixture/golden 覆盖 struct/class/object 字段、tuple、constructor/variant、global、singleton、重复引用及继承默认值。三个同名 provider 的真实 artifact 验证逐目标来源路由；缺失 provider、错 foundation、缺访问记录、伪造 owner type、member property 冒充 global、逐次 witness 篡改与预算耗尽均拒绝。generated adapter 额外覆盖 graph 可解析但 artifact 缺记录以及其他 generated role 拒绝。每批代码先 cargo fmt --all 与 cargo clippy --workspace --all-targets，默认来源定向 122 项和完整 scoop-hir/scoop-hir-lower 2,804 项全部通过，0 failed、0 ignored。模块按路由、绑定和错误拆分；本轮沿用关闭增量缓存与 debug 信息的构建设置。完整 default authority、committed-use 和正式跨 Cone layout 发布/消费继续推进，M23-6 未完成。
