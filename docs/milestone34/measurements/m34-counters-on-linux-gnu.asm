
tmp/m34/counters-on-linux-gnu/single:	file format elf64-x86-64

Disassembly of section .text:

000000000005d860 <record_allocation>:
   5d860:      	movq	0x100(%rdi), %rax
   5d867:      	addq	$0x1, %rax
   5d86b:      	movq	%rax, 0x100(%rdi)
   5d872:      	movq	0x110(%rdi), %rax
   5d879:      	addq	%rsi, %rax
   5d87c:      	movq	%rax, 0x110(%rdi)
   5d883:      	testb	%dl, %dl
   5d885:      	jne	0x5d890 <record_allocation+0x30>
   5d887:      	retq
   5d888:      	nopl	(%rax,%rax)
   5d890:      	movq	0x108(%rdi), %rax
   5d897:      	movq	0x118(%rdi), %rdx
   5d89e:      	addq	$0x1, %rax
   5d8a2:      	addq	%rdx, %rsi
   5d8a5:      	movq	%rax, 0x108(%rdi)
   5d8ac:      	movq	%rsi, 0x118(%rdi)
   5d8b3:      	retq
   5d8b4:      	nop
   5d8b5:      	nopw	%cs:(%rax,%rax)

000000000005d8c0 <finish_small_allocation>:
   5d8c0:      	pushq	%rbp
   5d8c1:      	movq	%rsp, %rbp
   5d8c4:      	pushq	%r14
   5d8c6:      	pushq	%r13
   5d8c8:      	pushq	%r12
   5d8ca:      	movq	%rdx, %r12
   5d8cd:      	pushq	%rbx
   5d8ce:      	subq	$0x10, %rsp
   5d8d2:      	testq	%rdx, %rdx
   5d8d5:      	sete	%al
   5d8d8:      	cmpq	$0x7f80, %rcx           # imm = 0x7F80
   5d8df:      	seta	%dl
   5d8e2:      	orb	%dl, %al
   5d8e4:      	jne	0x5d977 <finish_small_allocation+0xb7>
   5d8ea:      	movq	%rsi, %rbx
   5d8ed:      	testq	%rsi, %rsi
   5d8f0:      	je	0x5d977 <finish_small_allocation+0xb7>
   5d8f6:      	movq	%rsi, %rax
   5d8f9:      	xorl	%edx, %edx
   5d8fb:      	divq	0x18(%r12)
   5d900:      	testq	%rdx, %rdx
   5d903:      	jne	0x5d977 <finish_small_allocation+0xb7>
   5d905:      	movq	%rdi, %r14
   5d908:      	leaq	-0x24(%rbp), %rsi
   5d90c:      	movq	%rbx, %rdi
   5d90f:      	movq	%rcx, %r13
   5d912:      	callq	0x703c0 <scoop_heap_pointer_block_index>
   5d917:      	testb	%al, %al
   5d919:      	je	0x5d977 <finish_small_allocation+0xb7>
   5d91b:      	movq	%r13, %rdx
   5d91e:      	xorl	%esi, %esi
   5d920:      	movq	%rbx, %rdi
   5d923:      	callq	0x290c0 <memset@plt>
   5d928:      	movq	%r12, (%rbx)
   5d92b:      	movl	-0x24(%rbp), %edi
   5d92e:      	movq	%r13, %rdx
   5d931:      	movq	$0x0, 0x8(%rbx)
   5d939:      	movq	%rbx, %rsi
   5d93c:      	xorl	%ecx, %ecx
   5d93e:      	callq	0x739f0 <scoop_heap_record_small_object>
   5d943:      	movl	-0x24(%rbp), %eax
   5d946:      	movq	%r13, %rsi
   5d949:      	movq	%r14, %rdi
   5d94c:      	leaq	0x12c7cd(%rip), %rdx    # 0x18a120 <scoop_gc_heap_state>
   5d953:      	shlq	$0x7, %rax
   5d957:      	addq	0x40(%rdx), %rax
   5d95b:      	xorl	%edx, %edx
   5d95d:      	movl	0x8(%rax), %eax
   5d960:      	testl	%eax, %eax
   5d962:      	sete	%dl
   5d965:      	callq	0x5d860 <record_allocation>
   5d96a:      	addq	$0x10, %rsp
   5d96e:      	popq	%rbx
   5d96f:      	popq	%r12
   5d971:      	popq	%r13
   5d973:      	popq	%r14
   5d975:      	popq	%rbp
   5d976:      	retq
   5d977:      	leaq	0x32232(%rip), %rdi     # 0x8fbb0 <scoop$1$be$4007edd6593cba18323d1cdf8c36f439d5c36bffc9f5489c3161d38c49b5eb6c+0x3aec>
   5d97e:      	callq	0x6ffc0 <scoop_heap_fatal>
   5d983:      	nop
   5d985:      	nopw	%cs:(%rax,%rax)

000000000005da90 <scoop_runtime_finish_tlab_alloc>:
   5da90:      	endbr64
   5da94:      	pushq	%rbp
   5da95:      	movq	%rsp, %rbp
   5da98:      	pushq	%r13
   5da9a:      	movq	%rdx, %r13
   5da9d:      	pushq	%r12
   5da9f:      	movq	%rdi, %r12
   5daa2:      	pushq	%rbx
   5daa3:      	movq	%rsi, %rbx
   5daa6:      	subq	$0x8, %rsp
   5daaa:      	callq	0x70390 <scoop_gc_stress_move_enabled>
   5daaf:      	testb	%al, %al
   5dab1:      	jne	0x5daec <scoop_runtime_finish_tlab_alloc+0x5c>
   5dab3:      	cmpq	$0x0, 0x90(%rbx)
   5dabb:      	jne	0x5daec <scoop_runtime_finish_tlab_alloc+0x5c>
   5dabd:      	movq	%r13, %rsi
   5dac0:      	movq	%rbx, %rdi
   5dac3:      	callq	0x5c6f0 <scoop_shape_normalize_allocation>
   5dac8:      	movq	%rax, %r13
   5dacb:      	callq	0x62ba0 <scoop_thread_current_required>
   5dad0:      	movq	%r13, %rcx
   5dad3:      	movq	%rbx, %rdx
   5dad6:      	movq	%r12, %rsi
   5dad9:      	movq	%rax, %rdi
   5dadc:      	callq	0x5d8c0 <finish_small_allocation>
   5dae1:      	addq	$0x8, %rsp
   5dae5:      	popq	%rbx
   5dae6:      	popq	%r12
   5dae8:      	popq	%r13
   5daea:      	popq	%rbp
   5daeb:      	retq
   5daec:      	leaq	0x320dd(%rip), %rdi     # 0x8fbd0 <scoop$1$be$4007edd6593cba18323d1cdf8c36f439d5c36bffc9f5489c3161d38c49b5eb6c+0x3b0c>
   5daf3:      	callq	0x6ffc0 <scoop_heap_fatal>
   5daf8:      	nopl	(%rax,%rax)
