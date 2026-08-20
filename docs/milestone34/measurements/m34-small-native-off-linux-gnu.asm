0000000000059ba0 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab>:
   59ba0:      	pushq	%rbp
   59ba1:      	movq	%rsp, %rbp
   59ba4:      	pushq	%r15
   59ba6:      	pushq	%r14
   59ba8:      	pushq	%rbx
   59ba9:      	subq	$0x88, %rsp
   59bb0:      	movq	%rdi, %rbx
   59bb3:      	leaq	0x10(%rbp), %r14
   59bb7:      	movq	%fs:0x0, %rax
   59bc0:      	leaq	-0x10(%rax), %rax
   59bc7:      	movq	(%rax), %rcx
   59bca:      	leaq	0x13085f(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   59bd1:      	movq	(%rax), %rax
   59bd4:      	leaq	0x13085d(%rip), %rdx    # 0x18a438 <scoop_thread_world_phase>
   59bdb:      	movl	(%rdx), %esi
   59bdd:      	movl	(%rcx), %edx
   59bdf:      	movq	0x8(%rcx), %rcx
   59be3:      	testl	%esi, %esi
   59be5:      	jne	0x59bf1 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x51>
   59be7:      	cmpl	$0x1, %edx
   59bea:      	jne	0x59bf1 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x51>
   59bec:      	cmpq	%rax, %rcx
   59bef:      	je	0x59bf6 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x56>
   59bf1:      	callq	0x5bd10 <scoop_rt_safepoint>
   59bf6:      	movups	(%r14), %xmm0
   59bfa:      	movups	%xmm0, -0x38(%rbp)
   59bfe:      	xorps	%xmm0, %xmm0
   59c01:      	movups	%xmm0, -0x50(%rbp)
   59c05:      	movq	$0x0, -0x40(%rbp)
   59c0d:      	leaq	-0x50(%rbp), %r14
   59c11:      	movq	%r14, %rdi
   59c14:      	xorl	%esi, %esi
   59c16:      	xorl	%edx, %edx
   59c18:      	callq	0x6b410 <scoop_rt_push_caller_roots>
   59c1d:      	xorps	%xmm0, %xmm0
   59c20:      	movups	%xmm0, -0x90(%rbp)
   59c27:      	movups	%xmm0, -0x80(%rbp)
   59c2b:      	movups	%xmm0, -0x70(%rbp)
   59c2f:      	movups	%xmm0, -0x60(%rbp)
   59c33:      	leaq	-0x90(%rbp), %r15
   59c3a:      	movq	%r15, %rdi
   59c3d:      	movq	%r15, %rsi
   59c40:      	callq	0x5bf30 <scoop_rt_enter_native_borrowed>
   59c45:      	movups	-0x38(%rbp), %xmm0
   59c49:      	movups	%xmm0, (%rsp)
   59c4d:      	leaq	-0x28(%rbp), %rdi
   59c51:      	callq	0x73e70 <pair_step>
   59c56:      	movq	%r15, %rdi
   59c59:      	callq	0x5f110 <scoop_rt_leave_native_borrowed>
   59c5e:      	movq	%r14, %rdi
   59c61:      	callq	0x6b4c0 <scoop_rt_pop_caller_roots>
   59c66:      	movups	-0x28(%rbp), %xmm0
   59c6a:      	movups	%xmm0, (%rbx)
   59c6d:      	movq	%rbx, %rax
   59c70:      	addq	$0x88, %rsp
   59c77:      	popq	%rbx
   59c78:      	popq	%r14
   59c7a:      	popq	%r15
   59c7c:      	popq	%rbp
   59c7d:      	retq


0000000000059df0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   59df0:      	pushq	%rbp
   59df1:      	movq	%rsp, %rbp
   59df4:      	pushq	%r15
   59df6:      	pushq	%r14
   59df8:      	pushq	%r13
   59dfa:      	pushq	%r12
   59dfc:      	pushq	%rbx
   59dfd:      	subq	$0x88, %rsp
   59e04:      	movq	%fs:0x0, %rax
   59e0d:      	leaq	-0x10(%rax), %rax
   59e14:      	movq	(%rax), %r15
   59e17:      	leaq	0x130612(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   59e1e:      	movq	(%rax), %rax
   59e21:      	leaq	0x130610(%rip), %r13    # 0x18a438 <scoop_thread_world_phase>
   59e28:      	movl	(%r13), %esi
   59e2c:      	movl	(%r15), %edx
   59e2f:      	movq	0x8(%r15), %rcx
   59e33:      	movl	$0xa, %r14d
   59e39:      	testl	%esi, %esi
   59e3b:      	jne	0x59e47 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x57>
   59e3d:      	cmpl	$0x1, %edx
   59e40:      	jne	0x59e47 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x57>
   59e42:      	cmpq	%rax, %rcx
   59e45:      	je	0x59e4c <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x5c>
   59e47:      	callq	0x5bd10 <scoop_rt_safepoint>
   59e4c:      	xorl	%ebx, %ebx
   59e4e:      	xorl	%r12d, %r12d
   59e51:      	nopw	%cs:(%rax,%rax)
   59e60:      	leaq	0x1305c9(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   59e67:      	movq	(%rax), %rax
   59e6a:      	movl	(%r13), %esi
   59e6e:      	movl	(%r15), %edx
   59e71:      	movq	0x8(%r15), %rcx
   59e75:      	testl	%esi, %esi
   59e77:      	jne	0x59e83 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x93>
   59e79:      	cmpl	$0x1, %edx
   59e7c:      	jne	0x59e83 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x93>
   59e7e:      	cmpq	%rax, %rcx
   59e81:      	je	0x59e88 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x98>
   59e83:      	callq	0x5bd10 <scoop_rt_safepoint>
   59e88:      	cmpl	$0x989680, %r12d        # imm = 0x989680
   59e8f:      	jge	0x59f18 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x128>
   59e95:      	movq	%rbx, -0x48(%rbp)
   59e99:      	movq	%r14, -0x40(%rbp)
   59e9d:      	xorps	%xmm0, %xmm0
   59ea0:      	movups	%xmm0, -0x60(%rbp)
   59ea4:      	movq	$0x0, -0x50(%rbp)
   59eac:      	leaq	-0x60(%rbp), %rbx
   59eb0:      	movq	%rbx, %rdi
   59eb3:      	xorl	%esi, %esi
   59eb5:      	xorl	%edx, %edx
   59eb7:      	callq	0x6b410 <scoop_rt_push_caller_roots>
   59ebc:      	xorps	%xmm0, %xmm0
   59ebf:      	movups	%xmm0, -0xa0(%rbp)
   59ec6:      	movups	%xmm0, -0x90(%rbp)
   59ecd:      	movups	%xmm0, -0x80(%rbp)
   59ed1:      	movups	%xmm0, -0x70(%rbp)
   59ed5:      	leaq	-0xa0(%rbp), %r14
   59edc:      	movq	%r14, %rdi
   59edf:      	movq	%r14, %rsi
   59ee2:      	callq	0x5bf30 <scoop_rt_enter_native_borrowed>
   59ee7:      	movups	-0x48(%rbp), %xmm0
   59eeb:      	movups	%xmm0, (%rsp)
   59eef:      	leaq	-0x38(%rbp), %rdi
   59ef3:      	callq	0x73e70 <pair_step>
   59ef8:      	movq	%r14, %rdi
   59efb:      	callq	0x5f110 <scoop_rt_leave_native_borrowed>
   59f00:      	movq	%rbx, %rdi
   59f03:      	callq	0x6b4c0 <scoop_rt_pop_caller_roots>
   59f08:      	movq	-0x38(%rbp), %rbx
   59f0c:      	movq	-0x30(%rbp), %r14
   59f10:      	incl	%r12d
   59f13:      	jmp	0x59e60 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x70>
   59f18:      	addq	%r14, %rbx
   59f1b:      	movq	%rbx, %rdi
   59f1e:      	callq	0x59d60 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   59f23:      	addq	$0x88, %rsp
   59f2a:      	popq	%rbx
   59f2b:      	popq	%r12
   59f2d:      	popq	%r13
   59f2f:      	popq	%r14
   59f31:      	popq	%r15
   59f33:      	popq	%rbp
   59f34:      	retq


; Callee / provider code

/home/chenxu/repos/scoop/tmp/m34/small-values-before-gnu/native.o:	file format elf64-x86-64

Disassembly of section .text:

0000000000000000 <pair_step>:
       0:      	movq	%rdi, %rax
       3:      	movq	0x8(%rsp), %rcx
       8:      	movq	0x10(%rsp), %rdx
       d:      	incq	%rcx
      10:      	movq	%rdx, 0x8(%rdi)
      14:      	movq	%rcx, (%rdi)
      17:      	retq
