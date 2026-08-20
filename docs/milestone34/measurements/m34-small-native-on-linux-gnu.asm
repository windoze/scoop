0000000000059920 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab>:
   59920:      	pushq	%rbp
   59921:      	movq	%rsp, %rbp
   59924:      	pushq	%r15
   59926:      	pushq	%r14
   59928:      	pushq	%r12
   5992a:      	pushq	%rbx
   5992b:      	subq	$0x60, %rsp
   5992f:      	movq	%rsi, %rbx
   59932:      	movq	%rdi, %r14
   59935:      	movq	%fs:0x0, %rax
   5993e:      	leaq	-0x10(%rax), %rax
   59945:      	movq	(%rax), %rcx
   59948:      	leaq	0x130ae1(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   5994f:      	movq	(%rax), %rax
   59952:      	leaq	0x130adf(%rip), %rdx    # 0x18a438 <scoop_thread_world_phase>
   59959:      	movl	(%rdx), %esi
   5995b:      	movl	(%rcx), %edx
   5995d:      	movq	0x8(%rcx), %rcx
   59961:      	testl	%esi, %esi
   59963:      	jne	0x5996f <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x4f>
   59965:      	cmpl	$0x1, %edx
   59968:      	jne	0x5996f <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x4f>
   5996a:      	cmpq	%rax, %rcx
   5996d:      	je	0x59974 <scoop$1$cb$876080337ed15248d5e9fa1fab2571ec0837e8f5d914999561f4181250c985ab+0x54>
   5996f:      	callq	0x5ba90 <scoop_rt_safepoint>
   59974:      	xorps	%xmm0, %xmm0
   59977:      	movups	%xmm0, -0x38(%rbp)
   5997b:      	movq	$0x0, -0x28(%rbp)
   59983:      	leaq	-0x38(%rbp), %r15
   59987:      	movq	%r15, %rdi
   5998a:      	xorl	%esi, %esi
   5998c:      	xorl	%edx, %edx
   5998e:      	callq	0x6b190 <scoop_rt_push_caller_roots>
   59993:      	xorps	%xmm0, %xmm0
   59996:      	movups	%xmm0, -0x78(%rbp)
   5999a:      	movups	%xmm0, -0x68(%rbp)
   5999e:      	movups	%xmm0, -0x58(%rbp)
   599a2:      	movups	%xmm0, -0x48(%rbp)
   599a6:      	leaq	-0x78(%rbp), %r12
   599aa:      	movq	%r12, %rdi
   599ad:      	movq	%r12, %rsi
   599b0:      	callq	0x5bcb0 <scoop_rt_enter_native_borrowed>
   599b5:      	movq	%r14, %rdi
   599b8:      	movq	%rbx, %rsi
   599bb:      	callq	0x73bf0 <pair_step>
   599c0:      	movq	%rax, %rbx
   599c3:      	movq	%rdx, %r14
   599c6:      	movq	%r12, %rdi
   599c9:      	callq	0x5ee90 <scoop_rt_leave_native_borrowed>
   599ce:      	movq	%r15, %rdi
   599d1:      	callq	0x6b240 <scoop_rt_pop_caller_roots>
   599d6:      	movq	%rbx, %rax
   599d9:      	movq	%r14, %rdx
   599dc:      	addq	$0x60, %rsp
   599e0:      	popq	%rbx
   599e1:      	popq	%r12
   599e3:      	popq	%r14
   599e5:      	popq	%r15
   599e7:      	popq	%rbp
   599e8:      	retq


0000000000059b60 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   59b60:      	pushq	%rbp
   59b61:      	movq	%rsp, %rbp
   59b64:      	pushq	%r15
   59b66:      	pushq	%r14
   59b68:      	pushq	%r13
   59b6a:      	pushq	%r12
   59b6c:      	pushq	%rbx
   59b6d:      	subq	$0x68, %rsp
   59b71:      	movq	%fs:0x0, %rax
   59b7a:      	leaq	-0x10(%rax), %rax
   59b81:      	movq	(%rax), %rcx
   59b84:      	leaq	0x1308a5(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   59b8b:      	movq	(%rax), %rax
   59b8e:      	leaq	0x1308a3(%rip), %rdx    # 0x18a438 <scoop_thread_world_phase>
   59b95:      	movl	(%rdx), %esi
   59b97:      	movl	(%rcx), %edx
   59b99:      	movq	%rcx, -0x30(%rbp)
   59b9d:      	movq	0x8(%rcx), %rcx
   59ba1:      	movl	$0xa, %ebx
   59ba6:      	testl	%esi, %esi
   59ba8:      	jne	0x59bb4 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x54>
   59baa:      	cmpl	$0x1, %edx
   59bad:      	jne	0x59bb4 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x54>
   59baf:      	cmpq	%rax, %rcx
   59bb2:      	je	0x59bb9 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x59>
   59bb4:      	callq	0x5ba90 <scoop_rt_safepoint>
   59bb9:      	xorl	%r14d, %r14d
   59bbc:      	xorl	%r13d, %r13d
   59bbf:      	nop
   59bc0:      	leaq	0x130869(%rip), %rax    # 0x18a430 <scoop_thread_gc_epoch>
   59bc7:      	movq	(%rax), %rax
   59bca:      	leaq	0x130867(%rip), %rcx    # 0x18a438 <scoop_thread_world_phase>
   59bd1:      	movl	(%rcx), %esi
   59bd3:      	movq	-0x30(%rbp), %rcx
   59bd7:      	movl	(%rcx), %edx
   59bd9:      	movq	0x8(%rcx), %rcx
   59bdd:      	testl	%esi, %esi
   59bdf:      	jne	0x59beb <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x8b>
   59be1:      	cmpl	$0x1, %edx
   59be4:      	jne	0x59beb <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x8b>
   59be6:      	cmpq	%rax, %rcx
   59be9:      	je	0x59bf0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x90>
   59beb:      	callq	0x5ba90 <scoop_rt_safepoint>
   59bf0:      	cmpl	$0x989680, %r13d        # imm = 0x989680
   59bf7:      	jge	0x59c69 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x109>
   59bf9:      	xorps	%xmm0, %xmm0
   59bfc:      	movups	%xmm0, -0x48(%rbp)
   59c00:      	movq	$0x0, -0x38(%rbp)
   59c08:      	leaq	-0x48(%rbp), %r15
   59c0c:      	movq	%r15, %rdi
   59c0f:      	xorl	%esi, %esi
   59c11:      	xorl	%edx, %edx
   59c13:      	callq	0x6b190 <scoop_rt_push_caller_roots>
   59c18:      	xorps	%xmm0, %xmm0
   59c1b:      	movups	%xmm0, -0x88(%rbp)
   59c22:      	movups	%xmm0, -0x78(%rbp)
   59c26:      	movups	%xmm0, -0x68(%rbp)
   59c2a:      	movups	%xmm0, -0x58(%rbp)
   59c2e:      	leaq	-0x88(%rbp), %r12
   59c35:      	movq	%r12, %rdi
   59c38:      	movq	%r12, %rsi
   59c3b:      	callq	0x5bcb0 <scoop_rt_enter_native_borrowed>
   59c40:      	movq	%r14, %rdi
   59c43:      	movq	%rbx, %rsi
   59c46:      	callq	0x73bf0 <pair_step>
   59c4b:      	movq	%rax, %r14
   59c4e:      	movq	%rdx, %rbx
   59c51:      	movq	%r12, %rdi
   59c54:      	callq	0x5ee90 <scoop_rt_leave_native_borrowed>
   59c59:      	movq	%r15, %rdi
   59c5c:      	callq	0x6b240 <scoop_rt_pop_caller_roots>
   59c61:      	incl	%r13d
   59c64:      	jmp	0x59bc0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x60>
   59c69:      	addq	%rbx, %r14
   59c6c:      	movq	%r14, %rdi
   59c6f:      	callq	0x59ad0 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   59c74:      	addq	$0x68, %rsp
   59c78:      	popq	%rbx
   59c79:      	popq	%r12
   59c7b:      	popq	%r13
   59c7d:      	popq	%r14
   59c7f:      	popq	%r15
   59c81:      	popq	%rbp
   59c82:      	retq


; Callee / provider code

/home/chenxu/repos/scoop/tmp/m34/small-values-after-gnu/native.o:	file format elf64-x86-64

Disassembly of section .text:

0000000000000000 <pair_step>:
       0:      	movq	%rsi, %rdx
       3:      	leaq	0x1(%rdi), %rax
       7:      	retq
