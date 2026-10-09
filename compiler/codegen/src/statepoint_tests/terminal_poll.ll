@poll_state = external global ptr
@gc_epoch = external global i64
@world_phase = external global i32

declare void @tick()
declare ptr addrspace(1) @finish(i32)

define ptr addrspace(1) @f(i32 %code) #0 gc "statepoint-example" {
entry:
  %state = load ptr, ptr @poll_state, align 8
  %epoch = load atomic i64, ptr @gc_epoch acquire, align 8
  %phase = load atomic i32, ptr @world_phase acquire, align 4
  %observed_slot = getelementptr inbounds i8, ptr %state, i64 8
  %mode = load atomic i32, ptr %state acquire, align 4
  %observed = load atomic i64, ptr %observed_slot acquire, align 8
  %running = icmp eq i32 %phase, 0
  %managed = icmp eq i32 %mode, 1
  %current = icmp eq i64 %observed, %epoch
  %active = and i1 %running, %managed
  %fast = and i1 %active, %current
  br i1 %fast, label %poll.continue, label %poll.slow

poll.slow:
  call void @tick() #1
  br label %poll.continue

poll.continue:
  %result = call ptr addrspace(1) @finish(i32 %code) #2
  unreachable
}

attributes #0 = { "disable-tail-calls"="true" "frame-pointer"="all" }
attributes #1 = { "statepoint-id"="7" }
attributes #2 = { "statepoint-id"="8" }
