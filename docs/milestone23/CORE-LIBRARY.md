# core 普通 library 清理

2026-09-21：根据用户新指令替换原M23-6当前目标。停止扩展来源授权及防篡改体系。

core 是可由用户修改、扩展和重建的普通 library Cone。源码层面的特殊处理仅限于前端识别 `@Intrinsic`，并把它正规化为既有 typed IR，以及 desugar 通过普通声明引用使用基础库提供的类型和函数。sysroot 是默认查找位置，不是信任边界；源码目录、输出位置、相同 coordinate 或用户修改过的 core 不需要授权 token。metadata 解码、typed identity 一致性、依赖闭包、ABI、缓存失效和 slib fingerprint 使用所有 Cone 共用的规则。不得为 core 另建来源防伪、slot 授权、receipt 信任链或重复 pipeline；既有专用实现须合并或删除，旧文档的冻结条款不阻止此次清理。

## 实施与完成标准

- [x] 删除driver的源码/输出slot授权对象与路径限制，core可从普通manifest入口构建，产物可位于指定输出位置。集成测试在普通目录新增core函数、修改函数实现、重建后由用户代码消费，并核对依赖fingerprint更新。
- [x] 合并core与普通Cone的缓存键、receipt、源码快照、执行调度和产物完成验证；删除sysroot锁、slot receipt、专用source key及专用completed origin。用户修改源码按通用fingerprint失效与重建。
- [ ] 继续合并默认core发现、manifest限制与driver artifact加载的专用路径。已删除专用TrustedCore图节点；任意目录的core可作为普通Manifest构建根，在默认sysroot不存在时独立构建并命中缓存。
- [ ] 基础类型、普通callable/type/value及desugar通过普通声明metadata解析；intrinsic前端正规化后不携带来源授权。
- [ ] 合并MIR/LIR专用调用桥、String TD与shape-support重复分支，以及slib专用requirement closure。
- [ ] 删除M23-6 core source foundation专用入口及仅为防伪设计的授权链。已删除fundamental/Array/Unit/Any/Ptr/FunPtr消费者的重复core归属绑定和默认类型域的第二份core artifact参数；已删除core source foundation factory及其coordinate、Library、导入集合和CoreShapeSupport复核，core和普通Cone统一从已有typed HIR投影source foundation。
- [ ] 增加真实源码验证：修改core现有实现、增加导出函数和类型，重建后由用户代码消费；保留所有Cone共用的metadata、identity、ABI和缓存一致性检查。

每项完成后独立提交，先fmt和workspace lint再测试；构建进程退出后定期清理target。全部清理完成前不宣布目标完成。旧阶段文档尚存的trusted-core、独立authority和冻结专用格式描述属于待删除实现的历史背景，不作为保留理由。
