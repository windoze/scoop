declare void @failure()

define void @f(ptr addrspace(1) %value, i1 %condition) #0 gc "statepoint-example" {
entry:
  br i1 %condition, label %check, label %merge
check:
  %present = icmp ne ptr addrspace(1) %value, null
  br label %merge
merge:
  %enabled = phi i1 [ %present, %check ], [ false, %entry ]
  br i1 %enabled, label %unwrap, label %exit
unwrap:
  %absent = icmp eq ptr addrspace(1) %value, null
  br i1 %absent, label %failure, label %exit
failure:
  call void @failure() #1
  unreachable
exit:
  ret void
}

define void @g(ptr addrspace(1) %value, i1 %condition) #0 gc "statepoint-example" {
entry:
  br i1 %condition, label %unwrap, label %exit
unwrap:
  %absent = icmp eq ptr addrspace(1) %value, null
  br i1 %absent, label %failure, label %exit
failure:
  call void @failure() #2
  unreachable
exit:
  ret void
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
