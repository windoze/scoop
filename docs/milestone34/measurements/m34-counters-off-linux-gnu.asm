
tmp/m34/counters-off-linux-gnu/single:	file format elf64-x86-64

Disassembly of section .text:

000000000005e200 <finish_small_allocation>:
   5e200:      	pushq	%rbp
   5e201:      	movq	%rsp, %rbp
   5e204:      	pushq	%r13
   5e206:      	pushq	%r12
   5e208:      	movq	%rdx, %r12
   5e20b:      	pushq	%rbx
   5e20c:      	subq	$0x18, %rsp
   5e210:      	testq	%rsi, %rsi
   5e213:      	sete	%al
   5e216:      	cmpq	$0x7f80, %rdx           # imm = 0x7F80
   5e21d:      	seta	%dl
   5e220:      	orb	%dl, %al
   5e222:      	jne	0x5e2cc <finish_small_allocation+0xcc>
   5e228:      	movq	%rdi, %rbx
   5e22b:      	testq	%rdi, %rdi
   5e22e:      	je	0x5e2cc <finish_small_allocation+0xcc>
   5e234:      	movq	%rdi, %rax
   5e237:      	xorl	%edx, %edx
   5e239:      	movq	%rsi, %r13
   5e23c:      	divq	0x18(%rsi)
   5e240:      	testq	%rdx, %rdx
   5e243:      	jne	0x5e2cc <finish_small_allocation+0xcc>
   5e249:      	leaq	-0x24(%rbp), %rsi
   5e24d:      	callq	0x6b500 <scoop_heap_pointer_block_index>
   5e252:      	testb	%al, %al
   5e254:      	je	0x5e2cc <finish_small_allocation+0xcc>
   5e256:      	movq	%r12, %rdx
   5e259:      	xorl	%esi, %esi
   5e25b:      	movq	%rbx, %rdi
   5e25e:      	callq	0x290b0 <memset@plt>
   5e263:      	movq	%r13, (%rbx)
   5e266:      	movl	-0x24(%rbp), %edi
   5e269:      	xorl	%ecx, %ecx
   5e26b:      	movq	$0x0, 0x8(%rbx)
   5e273:      	movq	%r12, %rdx
   5e276:      	movq	%rbx, %rsi
   5e279:      	callq	0x5bc30 <scoop_heap_record_small_object>
   5e27e:      	leaq	0x12be9b(%rip), %rax    # 0x18a120 <scoop_gc_heap_state>
   5e285:      	lock
   5e286:      	addq	$0x1, 0x78(%rax)
   5e28b:      	lock
   5e28c:      	addq	%r12, 0xa0(%rax)
   5e293:      	movl	-0x24(%rbp), %edx
   5e296:      	shlq	$0x7, %rdx
   5e29a:      	addq	0x40(%rax), %rdx
   5e29e:      	movl	0x8(%rdx), %edx
   5e2a1:      	testl	%edx, %edx
   5e2a3:      	je	0x5e2b0 <finish_small_allocation+0xb0>
   5e2a5:      	addq	$0x18, %rsp
   5e2a9:      	popq	%rbx
   5e2aa:      	popq	%r12
   5e2ac:      	popq	%r13
   5e2ae:      	popq	%rbp
   5e2af:      	retq
   5e2b0:      	lock
   5e2b1:      	addq	$0x1, 0x98(%rax)
   5e2b9:      	lock
   5e2ba:      	addq	%r12, 0xa8(%rax)
   5e2c1:      	addq	$0x18, %rsp
   5e2c5:      	popq	%rbx
   5e2c6:      	popq	%r12
   5e2c8:      	popq	%r13
   5e2ca:      	popq	%rbp
   5e2cb:      	retq
   5e2cc:      	leaq	0x31865(%rip), %rdi     # 0x8fb38 <scoop$1$be$4007edd6593cba18323d1cdf8c36f439d5c36bffc9f5489c3161d38c49b5eb6c+0x3a74>
   5e2d3:      	callq	0x6b100 <scoop_heap_fatal>
   5e2d8:      	nopl	(%rax,%rax)

000000000005e3f0 <scoop_runtime_finish_tlab_alloc>:
   5e3f0:      	endbr64
   5e3f4:      	pushq	%rbp
   5e3f5:      	movq	%rsp, %rbp
   5e3f8:      	pushq	%r13
   5e3fa:      	movq	%rdx, %r13
   5e3fd:      	pushq	%r12
   5e3ff:      	movq	%rdi, %r12
   5e402:      	pushq	%rbx
   5e403:      	movq	%rsi, %rbx
   5e406:      	subq	$0x8, %rsp
   5e40a:      	callq	0x6b4d0 <scoop_gc_stress_move_enabled>
   5e40f:      	testb	%al, %al
   5e411:      	jne	0x5e441 <scoop_runtime_finish_tlab_alloc+0x51>
   5e413:      	cmpq	$0x0, 0x90(%rbx)
   5e41b:      	jne	0x5e441 <scoop_runtime_finish_tlab_alloc+0x51>
   5e41d:      	movq	%r13, %rsi
   5e420:      	movq	%rbx, %rdi
   5e423:      	callq	0x5cfc0 <scoop_shape_normalize_allocation>
   5e428:      	movq	%rbx, %rsi
   5e42b:      	movq	%r12, %rdi
   5e42e:      	movq	%rax, %rdx
   5e431:      	callq	0x5e200 <finish_small_allocation>
   5e436:      	addq	$0x8, %rsp
   5e43a:      	popq	%rbx
   5e43b:      	popq	%r12
   5e43d:      	popq	%r13
   5e43f:      	popq	%rbp
   5e440:      	retq
   5e441:      	leaq	0x31710(%rip), %rdi     # 0x8fb58 <scoop$1$be$4007edd6593cba18323d1cdf8c36f439d5c36bffc9f5489c3161d38c49b5eb6c+0x3a94>
   5e448:      	callq	0x6b100 <scoop_heap_fatal>
   5e44d:      	nopl	(%rax)
