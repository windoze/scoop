declare {ptr addrspace(1), ptr} @produce() #0
declare void @poll()
declare void @consume(ptr addrspace(1), ptr) #0
declare void @llvm.fake.use(...)

define {ptr addrspace(1), ptr} @f(ptr %output) gc "statepoint-example" {
entry:
  %value = call {ptr addrspace(1), ptr} @produce()
  %object = extractvalue {ptr addrspace(1), ptr} %value, 0
  %itab = extractvalue {ptr addrspace(1), ptr} %value, 1
  call void @poll() #1
  call void (...) @llvm.fake.use(ptr addrspace(1) %object) #2, !scoop.statepoint-root-identity !0
  %first = insertvalue {ptr addrspace(1), ptr} poison, ptr addrspace(1) %object, 0
  %result = insertvalue {ptr addrspace(1), ptr} %first, ptr %itab, 1
  store {ptr addrspace(1), ptr} %result, ptr %output
  call void @consume(ptr addrspace(1) %object, ptr %itab)
  ret {ptr addrspace(1), ptr} %result
}

attributes #0 = { "gc-leaf-function" }
attributes #1 = { noduplicate "statepoint-id"="7" }
attributes #2 = { noduplicate }
!0 = !{i64 7, i64 2, i64 0, i64 0}
