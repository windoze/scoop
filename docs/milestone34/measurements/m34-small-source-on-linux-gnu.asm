0000000000059f00 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe>:
   59f00:      	pushq	%rbp
   59f01:      	movq	%rsp, %rbp
   59f04:      	pushq	%r15
   59f06:      	pushq	%r14
   59f08:      	pushq	%r13
   59f0a:      	pushq	%r12
   59f0c:      	pushq	%rbx
   59f0d:      	pushq	%rax
   59f0e:      	movq	%fs:0x0, %rax
   59f17:      	leaq	-0x10(%rax), %rax
   59f1e:      	movq	(%rax), %r15
   59f21:      	leaq	0x131508(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59f28:      	movq	(%rax), %rax
   59f2b:      	leaq	0x131506(%rip), %r13    # 0x18b438 <scoop_thread_world_phase>
   59f32:      	movl	(%r13), %esi
   59f36:      	movl	(%r15), %edx
   59f39:      	movq	0x8(%r15), %rcx
   59f3d:      	testl	%esi, %esi
   59f3f:      	jne	0x59f4b <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x4b>
   59f41:      	cmpl	$0x1, %edx
   59f44:      	jne	0x59f4b <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x4b>
   59f46:      	cmpq	%rax, %rcx
   59f49:      	je	0x59f50 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x50>
   59f4b:      	callq	0x5bf10 <scoop_rt_safepoint>
   59f50:      	xorl	%r12d, %r12d
   59f53:      	movl	$0xa, %esi
   59f58:      	xorl	%edi, %edi
   59f5a:      	callq	0x59da0 <scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>
   59f5f:      	movq	%rax, %rbx
   59f62:      	movq	%rdx, %r14
   59f65:      	nopw	%cs:(%rax,%rax)
   59f70:      	leaq	0x1314b9(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59f77:      	movq	(%rax), %rax
   59f7a:      	movl	(%r13), %esi
   59f7e:      	movl	(%r15), %edx
   59f81:      	movq	0x8(%r15), %rcx
   59f85:      	testl	%esi, %esi
   59f87:      	jne	0x59f93 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x93>
   59f89:      	cmpl	$0x1, %edx
   59f8c:      	jne	0x59f93 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x93>
   59f8e:      	cmpq	%rax, %rcx
   59f91:      	je	0x59f98 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x98>
   59f93:      	callq	0x5bf10 <scoop_rt_safepoint>
   59f98:      	cmpl	$0x989680, %r12d        # imm = 0x989680
   59f9f:      	jge	0x59fb7 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xb7>
   59fa1:      	movq	%rbx, %rdi
   59fa4:      	movq	%r14, %rsi
   59fa7:      	callq	0x59ef0 <scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>
   59fac:      	movq	%rax, %rbx
   59faf:      	movq	%rdx, %r14
   59fb2:      	incl	%r12d
   59fb5:      	jmp	0x59f70 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x70>
   59fb7:      	addq	%r14, %rbx
   59fba:      	movq	%rbx, %rdi
   59fbd:      	callq	0x59fe0 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   59fc2:      	addq	$0x8, %rsp
   59fc6:      	popq	%rbx
   59fc7:      	popq	%r12
   59fc9:      	popq	%r13
   59fcb:      	popq	%r14
   59fcd:      	popq	%r15
   59fcf:      	popq	%rbp
   59fd0:      	retq


; Callee / provider code
0000000000059920 <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18>:
   59920:      	pushq	%rbp
   59921:      	movq	%rsp, %rbp
   59924:      	movq	%fs:0x0, %rax
   5992d:      	leaq	-0x10(%rax), %rax
   59934:      	movq	(%rax), %rcx
   59937:      	leaq	0x131af2(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5993e:      	movq	(%rax), %rax
   59941:      	leaq	0x131af0(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59948:      	movl	(%rdx), %esi
   5994a:      	movl	(%rcx), %edx
   5994c:      	movq	0x8(%rcx), %rcx
   59950:      	testl	%esi, %esi
   59952:      	jne	0x5996a <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   59954:      	cmpl	$0x1, %edx
   59957:      	jne	0x5996a <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   59959:      	cmpq	%rax, %rcx
   5995c:      	jne	0x5996a <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   5995e:      	leaq	0x326ab(%rip), %rdi     # 0x8c010 <scoop$1$bs$9bc83e826cd4d5e3a3793dc5bd199cbfde4f00c4aa0fcdbe7b4944aaf8e7d6a7>
   59965:      	callq	0x60940 <scoop_rt_trap>
   5996a:      	callq	0x5bf10 <scoop_rt_safepoint>
   5996f:      	leaq	0x3269a(%rip), %rdi     # 0x8c010 <scoop$1$bs$9bc83e826cd4d5e3a3793dc5bd199cbfde4f00c4aa0fcdbe7b4944aaf8e7d6a7>
   59976:      	callq	0x60940 <scoop_rt_trap>


0000000000059980 <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>:
   59980:      	pushq	%rbp
   59981:      	movq	%rsp, %rbp
   59984:      	movq	%fs:0x0, %rax
   5998d:      	leaq	-0x10(%rax), %rax
   59994:      	movq	(%rax), %rcx
   59997:      	leaq	0x131a92(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5999e:      	movq	(%rax), %rax
   599a1:      	leaq	0x131a90(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   599a8:      	movl	(%rdx), %esi
   599aa:      	movl	(%rcx), %edx
   599ac:      	movq	0x8(%rcx), %rcx
   599b0:      	testl	%esi, %esi
   599b2:      	jne	0x599ca <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   599b4:      	cmpl	$0x1, %edx
   599b7:      	jne	0x599ca <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   599b9:      	cmpq	%rax, %rcx
   599bc:      	jne	0x599ca <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   599be:      	leaq	0x3272b(%rip), %rdi     # 0x8c0f0 <scoop$1$bs$eef28c67632ed62c81fb4ec30dedfa52467e063c429b84eaecfa443ce5bb7c30>
   599c5:      	callq	0x60940 <scoop_rt_trap>
   599ca:      	callq	0x5bf10 <scoop_rt_safepoint>
   599cf:      	leaq	0x3271a(%rip), %rdi     # 0x8c0f0 <scoop$1$bs$eef28c67632ed62c81fb4ec30dedfa52467e063c429b84eaecfa443ce5bb7c30>
   599d6:      	callq	0x60940 <scoop_rt_trap>


00000000000599e0 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe>:
   599e0:      	pushq	%rbp
   599e1:      	movq	%rsp, %rbp
   599e4:      	pushq	%r15
   599e6:      	pushq	%r14
   599e8:      	pushq	%r13
   599ea:      	pushq	%r12
   599ec:      	pushq	%rbx
   599ed:      	subq	$0x148, %rsp            # imm = 0x148
   599f4:      	movq	%rcx, %r14
   599f7:      	movq	%rdx, %r15
   599fa:      	movq	%rsi, %rbx
   599fd:      	movq	%rdi, %r12
   59a00:      	movq	%fs:0x0, %rax
   59a09:      	leaq	-0x10(%rax), %rax
   59a10:      	movq	(%rax), %rcx
   59a13:      	movq	%r12, -0x58(%rbp)
   59a17:      	movq	%r15, -0x40(%rbp)
   59a1b:      	movq	%r14, -0x38(%rbp)
   59a1f:      	movq	$0x0, -0x70(%rbp)
   59a27:      	movq	$0x0, -0x78(%rbp)
   59a2f:      	movq	$0x0, -0x80(%rbp)
   59a37:      	movq	$0x0, -0x88(%rbp)
   59a42:      	leaq	0x1319e7(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59a49:      	movq	(%rax), %rax
   59a4c:      	leaq	0x1319e5(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59a53:      	movl	(%rdx), %esi
   59a55:      	movl	(%rcx), %edx
   59a57:      	movq	0x8(%rcx), %rcx
   59a5b:      	testl	%esi, %esi
   59a5d:      	jne	0x59a69 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x89>
   59a5f:      	cmpl	$0x1, %edx
   59a62:      	jne	0x59a69 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x89>
   59a64:      	cmpq	%rax, %rcx
   59a67:      	je	0x59a8e <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0xae>
   59a69:      	movq	-0x58(%rbp), %rax
   59a6d:      	movq	-0x40(%rbp), %rcx
   59a71:      	movq	%rax, -0x50(%rbp)
   59a75:      	movq	%rcx, -0x48(%rbp)
   59a79:      	callq	0x5bf10 <scoop_rt_safepoint>
   59a7e:      	movq	-0x48(%rbp), %rax
   59a82:      	movq	-0x50(%rbp), %rcx
   59a86:      	movq	%rcx, -0x58(%rbp)
   59a8a:      	movq	%rax, -0x40(%rbp)
   59a8e:      	callq	0x5ba00 <scoop_rt_context_snapshot>
   59a93:      	movq	-0x58(%rbp), %rcx
   59a97:      	movq	-0x40(%rbp), %rdx
   59a9b:      	movq	%rax, -0x60(%rbp)
   59a9f:      	movq	%rcx, -0x50(%rbp)
   59aa3:      	movq	%rdx, -0x48(%rbp)
   59aa7:      	leaq	0x9b9a2(%rip), %rsi     # 0xf5450 <scoop$1$td$87490392603c1b9907331e3d0317a314f8b46008efab47286c4f62af746c6745>
   59aae:      	movq	%rax, %rdi
   59ab1:      	callq	0x5c0d0 <scoop_rt_context_fork>
   59ab6:      	movq	-0x60(%rbp), %rcx
   59aba:      	movq	-0x48(%rbp), %rdx
   59abe:      	movq	-0x50(%rbp), %rsi
   59ac2:      	movq	%rsi, -0x58(%rbp)
   59ac6:      	movq	%rdx, -0x40(%rbp)
   59aca:      	movq	%rcx, -0x88(%rbp)
   59ad1:      	movq	%rax, %rdi
   59ad4:      	callq	0x5beb0 <scoop_rt_context_enter>
   59ad9:      	movq	%rax, -0x30(%rbp)
   59add:      	movq	-0x58(%rbp), %rax
   59ae1:      	movq	-0x58(%rbp), %r14
   59ae5:      	movq	%r14, -0x80(%rbp)
   59ae9:      	movq	-0x40(%rbp), %r15
   59aed:      	movq	-0x38(%rbp), %r12
   59af1:      	leaq	-0x40(%rbp), %rax
   59af5:      	movq	%rax, -0x168(%rbp)
   59afc:      	leaq	0x106fcd(%rip), %rax    # 0x160ad0 <scoop$1$bs$d838558236e5d68de5433acd73aae0845ce7c5e178f9f30fc49bf147960abccf>
   59b03:      	movq	%rax, -0x160(%rbp)
   59b0a:      	leaq	-0x30(%rbp), %r13
   59b0e:      	movq	%r13, -0x158(%rbp)
   59b15:      	leaq	0x106fc4(%rip), %rax    # 0x160ae0 <scoop$1$bs$3bd3bf5dda75a42e5d618b52e341dd730fe494f426a28a1010c75d388f80b7b8>
   59b1c:      	movq	%rax, -0x150(%rbp)
   59b23:      	leaq	-0x80(%rbp), %rax
   59b27:      	movq	%rax, -0x148(%rbp)
   59b2e:      	leaq	0x106fbb(%rip), %rax    # 0x160af0 <scoop$1$bs$76a1d41017149d26c27140164906c3cea684a3e784c0b2533c872cf9d41190f3>
   59b35:      	movq	%rax, -0x140(%rbp)
   59b3c:      	xorps	%xmm0, %xmm0
   59b3f:      	movups	%xmm0, -0xe8(%rbp)
   59b46:      	movq	$0x0, -0xd8(%rbp)
   59b51:      	leaq	-0xe8(%rbp), %rdi
   59b58:      	leaq	-0x168(%rbp), %rsi
   59b5f:      	movl	$0x3, %edx
   59b64:      	callq	0x6b710 <scoop_rt_push_compiler_roots>
   59b69:      	movq	(%rbx), %rax
   59b6c:      	leaq	-0xa0(%rbp), %rdi
   59b73:      	movq	%r14, %rsi
   59b76:      	movq	%r15, %rdx
   59b79:      	movq	%r12, %rcx
   59b7c:      	callq	*%rax
   59b7e:      	movq	-0x40(%rbp), %rbx
   59b82:      	movq	-0x38(%rbp), %r14
   59b86:      	movq	-0x30(%rbp), %r15
   59b8a:      	leaq	-0xe8(%rbp), %rdi
   59b91:      	callq	0x6b7c0 <scoop_rt_pop_compiler_roots>
   59b96:      	movq	%r14, -0x38(%rbp)
   59b9a:      	movq	%rbx, -0x40(%rbp)
   59b9e:      	movq	%r15, -0x30(%rbp)
   59ba2:      	cmpq	$0x0, -0xa0(%rbp)
   59baa:      	jne	0x59c4c <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x26c>
   59bb0:      	movq	-0x98(%rbp), %rbx
   59bb7:      	movq	-0x90(%rbp), %r14
   59bbe:      	movq	-0x40(%rbp), %rax
   59bc2:      	movq	-0x38(%rbp), %r12
   59bc6:      	movq	-0x40(%rbp), %r15
   59bca:      	movq	%r15, -0x70(%rbp)
   59bce:      	movq	%r13, -0x108(%rbp)
   59bd5:      	leaq	0x106ea4(%rip), %rax    # 0x160a80 <scoop$1$bs$fea5aa0086f471ccf24f9b6eb0d3547a1cd8de733080ea0ccde863a4315d68e5>
   59bdc:      	movq	%rax, -0x100(%rbp)
   59be3:      	leaq	-0x70(%rbp), %rax
   59be7:      	movq	%rax, -0xf8(%rbp)
   59bee:      	leaq	0x106e9b(%rip), %rax    # 0x160a90 <scoop$1$bs$1de6d529dcef63cbd7f68381ef455cac0ac1b5b75ebe12ffdb13574d29e1c90f>
   59bf5:      	movq	%rax, -0xf0(%rbp)
   59bfc:      	xorps	%xmm0, %xmm0
   59bff:      	movups	%xmm0, -0xb8(%rbp)
   59c06:      	movq	$0x0, -0xa8(%rbp)
   59c11:      	leaq	-0xb8(%rbp), %rdi
   59c18:      	leaq	-0x108(%rbp), %rsi
   59c1f:      	movl	$0x2, %edx
   59c24:      	callq	0x6b710 <scoop_rt_push_compiler_roots>
   59c29:      	movq	(%r12), %rax
   59c2d:      	movq	%r15, %rdi
   59c30:      	movq	%rbx, %rsi
   59c33:      	movq	%r14, %rdx
   59c36:      	callq	*%rax
   59c38:      	movq	-0x30(%rbp), %rbx
   59c3c:      	leaq	-0xb8(%rbp), %rdi
   59c43:      	callq	0x6b7c0 <scoop_rt_pop_compiler_roots>
   59c48:      	movq	%rbx, -0x30(%rbp)
   59c4c:      	movq	-0x30(%rbp), %rdi
   59c50:      	callq	0x5bee0 <scoop_rt_context_leave>
   59c55:      	addq	$0x148, %rsp            # imm = 0x148
   59c5c:      	popq	%rbx
   59c5d:      	popq	%r12
   59c5f:      	popq	%r13
   59c61:      	popq	%r14
   59c63:      	popq	%r15
   59c65:      	popq	%rbp
   59c66:      	retq
   59c67:      	movq	%rax, %rbx
   59c6a:      	movq	-0x40(%rbp), %r14
   59c6e:      	movq	-0x38(%rbp), %r15
   59c72:      	movq	-0x30(%rbp), %r12
   59c76:      	callq	0x6b810 <scoop_rt_pop_top_compiler_roots>
   59c7b:      	movq	%r15, -0x38(%rbp)
   59c7f:      	movq	%r14, -0x40(%rbp)
   59c83:      	movq	%r12, -0x30(%rbp)
   59c87:      	movq	%rbx, %rdi
   59c8a:      	callq	0x67fa0 <scoop_rt_begin_catch>
   59c8f:      	movq	-0x40(%rbp), %rcx
   59c93:      	movq	-0x30(%rbp), %rdx
   59c97:      	movq	%rax, -0x60(%rbp)
   59c9b:      	movq	%rcx, -0x50(%rbp)
   59c9f:      	movq	%rdx, -0x48(%rbp)
   59ca3:      	movq	%rax, %rdi
   59ca6:      	callq	0x5bfd0 <scoop_rt_materialize_exception>
   59cab:      	movq	-0x60(%rbp), %rcx
   59caf:      	movq	-0x48(%rbp), %rdx
   59cb3:      	movq	-0x50(%rbp), %rsi
   59cb7:      	movq	%rsi, -0x40(%rbp)
   59cbb:      	movq	%rdx, -0x30(%rbp)
   59cbf:      	movq	%rcx, -0x170(%rbp)
   59cc6:      	movq	%rax, -0x68(%rbp)
   59cca:      	callq	0x68080 <scoop_rt_end_catch>
   59ccf:      	movq	-0x40(%rbp), %rax
   59cd3:      	movq	-0x38(%rbp), %r15
   59cd7:      	movq	-0x40(%rbp), %rbx
   59cdb:      	movq	%rbx, -0x78(%rbp)
   59cdf:      	movq	-0x68(%rbp), %r14
   59ce3:      	leaq	-0x68(%rbp), %rax
   59ce7:      	movq	%rax, -0x138(%rbp)
   59cee:      	leaq	0x106dab(%rip), %rax    # 0x160aa0 <scoop$1$bs$9872cdf7da438a41b8ffee0f4d9767d8ab494ae67b907779f00702fa698662e9>
   59cf5:      	movq	%rax, -0x130(%rbp)
   59cfc:      	movq	%r13, -0x128(%rbp)
   59d03:      	leaq	0x106da6(%rip), %rax    # 0x160ab0 <scoop$1$bs$09120727c90376dc3e9629a82c16171e19925bf1bf702cbaaa132e253452c419>
   59d0a:      	movq	%rax, -0x120(%rbp)
   59d11:      	leaq	-0x78(%rbp), %rax
   59d15:      	movq	%rax, -0x118(%rbp)
   59d1c:      	leaq	0x106d9d(%rip), %rax    # 0x160ac0 <scoop$1$bs$e9b98455ff4ff3d3713cf38ff7154d28ecd1f053350e2f82195185de23382760>
   59d23:      	movq	%rax, -0x110(%rbp)
   59d2a:      	xorps	%xmm0, %xmm0
   59d2d:      	movups	%xmm0, -0xd0(%rbp)
   59d34:      	movq	$0x0, -0xc0(%rbp)
   59d3f:      	leaq	-0xd0(%rbp), %rdi
   59d46:      	leaq	-0x138(%rbp), %rsi
   59d4d:      	movl	$0x3, %edx
   59d52:      	callq	0x6b710 <scoop_rt_push_compiler_roots>
   59d57:      	movq	0x8(%r15), %rax
   59d5b:      	movq	%rbx, %rdi
   59d5e:      	movq	%r14, %rsi
   59d61:      	callq	*%rax
   59d63:      	movq	-0x30(%rbp), %rbx
   59d67:      	leaq	-0xd0(%rbp), %rdi
   59d6e:      	jmp	0x59c43 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x263>
   59d73:      	movq	%rax, %rbx
   59d76:      	movq	-0x30(%rbp), %r14
   59d7a:      	callq	0x6b810 <scoop_rt_pop_top_compiler_roots>
   59d7f:      	movq	%r14, -0x30(%rbp)
   59d83:      	movq	-0x30(%rbp), %rdi
   59d87:      	callq	0x5bee0 <scoop_rt_context_leave>
   59d8c:      	movq	%rbx, %rdi
   59d8f:      	callq	0x29400 <_Unwind_Resume>


0000000000059da0 <scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>:
   59da0:      	pushq	%rbp
   59da1:      	movq	%rsp, %rbp
   59da4:      	movq	%rsi, %rdx
   59da7:      	movq	%rdi, %rax
   59daa:      	popq	%rbp
   59dab:      	retq


0000000000059db0 <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e>:
   59db0:      	pushq	%rbp
   59db1:      	movq	%rsp, %rbp
   59db4:      	movq	%fs:0x0, %rax
   59dbd:      	leaq	-0x10(%rax), %rax
   59dc4:      	movq	(%rax), %rcx
   59dc7:      	leaq	0x131662(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59dce:      	movq	(%rax), %rax
   59dd1:      	leaq	0x131660(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59dd8:      	movl	(%rdx), %esi
   59dda:      	movl	(%rcx), %edx
   59ddc:      	movq	0x8(%rcx), %rcx
   59de0:      	testl	%esi, %esi
   59de2:      	jne	0x59dfa <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   59de4:      	cmpl	$0x1, %edx
   59de7:      	jne	0x59dfa <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   59de9:      	cmpq	%rax, %rcx
   59dec:      	jne	0x59dfa <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   59dee:      	leaq	0x3232b(%rip), %rdi     # 0x8c120 <scoop$1$bs$16fca5c8b23b886ebb2b482824651319245072c6ab6dc17abf3cda36df97aecc>
   59df5:      	callq	0x60940 <scoop_rt_trap>
   59dfa:      	callq	0x5bf10 <scoop_rt_safepoint>
   59dff:      	leaq	0x3231a(%rip), %rdi     # 0x8c120 <scoop$1$bs$16fca5c8b23b886ebb2b482824651319245072c6ab6dc17abf3cda36df97aecc>
   59e06:      	callq	0x60940 <scoop_rt_trap>


0000000000059e10 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389>:
   59e10:      	pushq	%rbp
   59e11:      	movq	%rsp, %rbp
   59e14:      	pushq	%r15
   59e16:      	pushq	%r14
   59e18:      	pushq	%r12
   59e1a:      	pushq	%rbx
   59e1b:      	movq	%rcx, %rbx
   59e1e:      	movq	%rdx, %r15
   59e21:      	movq	%rsi, %r14
   59e24:      	movq	%rdi, %r12
   59e27:      	movq	%fs:0x0, %rax
   59e30:      	leaq	-0x10(%rax), %rax
   59e37:      	movq	(%rax), %rcx
   59e3a:      	leaq	0x1315ef(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59e41:      	movq	(%rax), %rax
   59e44:      	leaq	0x1315ed(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59e4b:      	movl	(%rdx), %esi
   59e4d:      	movl	(%rcx), %edx
   59e4f:      	movq	0x8(%rcx), %rcx
   59e53:      	testl	%esi, %esi
   59e55:      	jne	0x59e6e <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x5e>
   59e57:      	cmpl	$0x1, %edx
   59e5a:      	jne	0x59e6e <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x5e>
   59e5c:      	cmpq	%rax, %rcx
   59e5f:      	jne	0x59e6e <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x5e>
   59e61:      	cmpq	%r15, %r12
   59e64:      	jne	0x59e78 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x68>
   59e66:      	cmpq	%rbx, %r14
   59e69:      	sete	%al
   59e6c:      	jmp	0x59e7a <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x6a>
   59e6e:      	callq	0x5bf10 <scoop_rt_safepoint>
   59e73:      	cmpq	%r15, %r12
   59e76:      	je	0x59e66 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x56>
   59e78:      	xorl	%eax, %eax
   59e7a:      	popq	%rbx
   59e7b:      	popq	%r12
   59e7d:      	popq	%r14
   59e7f:      	popq	%r15
   59e81:      	popq	%rbp
   59e82:      	retq


0000000000059e90 <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03>:
   59e90:      	pushq	%rbp
   59e91:      	movq	%rsp, %rbp
   59e94:      	movq	%fs:0x0, %rax
   59e9d:      	leaq	-0x10(%rax), %rax
   59ea4:      	movq	(%rax), %rcx
   59ea7:      	leaq	0x131582(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59eae:      	movq	(%rax), %rax
   59eb1:      	leaq	0x131580(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59eb8:      	movl	(%rdx), %esi
   59eba:      	movl	(%rcx), %edx
   59ebc:      	movq	0x8(%rcx), %rcx
   59ec0:      	testl	%esi, %esi
   59ec2:      	jne	0x59eda <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   59ec4:      	cmpl	$0x1, %edx
   59ec7:      	jne	0x59eda <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   59ec9:      	cmpq	%rax, %rcx
   59ecc:      	jne	0x59eda <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   59ece:      	leaq	0x3255b(%rip), %rdi     # 0x8c430 <scoop$1$bs$bcfe2ea1fa66203f30659828e5e934155733245e98e4cef8e44324cbf7d7eb39>
   59ed5:      	callq	0x60940 <scoop_rt_trap>
   59eda:      	callq	0x5bf10 <scoop_rt_safepoint>
   59edf:      	leaq	0x3254a(%rip), %rdi     # 0x8c430 <scoop$1$bs$bcfe2ea1fa66203f30659828e5e934155733245e98e4cef8e44324cbf7d7eb39>
   59ee6:      	callq	0x60940 <scoop_rt_trap>


0000000000059ef0 <scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>:
   59ef0:      	pushq	%rbp
   59ef1:      	movq	%rsp, %rbp
   59ef4:      	movq	%rsi, %rdx
   59ef7:      	leaq	0x1(%rdi), %rax
   59efb:      	popq	%rbp
   59efc:      	retq
