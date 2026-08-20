000000000005a180 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe>:
   5a180:      	pushq	%rbp
   5a181:      	movq	%rsp, %rbp
   5a184:      	pushq	%r15
   5a186:      	pushq	%r14
   5a188:      	pushq	%r13
   5a18a:      	pushq	%r12
   5a18c:      	pushq	%rbx
   5a18d:      	subq	$0x48, %rsp
   5a191:      	movq	%fs:0x0, %rax
   5a19a:      	leaq	-0x10(%rax), %rax
   5a1a1:      	movq	(%rax), %r15
   5a1a4:      	leaq	0x131285(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5a1ab:      	movq	(%rax), %rax
   5a1ae:      	leaq	0x131283(%rip), %rcx    # 0x18b438 <scoop_thread_world_phase>
   5a1b5:      	movl	(%rcx), %esi
   5a1b7:      	movl	(%r15), %edx
   5a1ba:      	movq	0x8(%r15), %rcx
   5a1be:      	testl	%esi, %esi
   5a1c0:      	jne	0x5a1cc <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x4c>
   5a1c2:      	cmpl	$0x1, %edx
   5a1c5:      	jne	0x5a1cc <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x4c>
   5a1c7:      	cmpq	%rax, %rcx
   5a1ca:      	je	0x5a1d1 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x51>
   5a1cc:      	callq	0x5c1d0 <scoop_rt_safepoint>
   5a1d1:      	xorl	%r12d, %r12d
   5a1d4:      	leaq	-0x58(%rbp), %r14
   5a1d8:      	movl	$0xa, %edx
   5a1dd:      	movq	%r14, %rdi
   5a1e0:      	xorl	%esi, %esi
   5a1e2:      	callq	0x5a020 <scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>
   5a1e7:      	movq	-0x50(%rbp), %r13
   5a1eb:      	leaq	-0x48(%rbp), %rbx
   5a1ef:      	nop
   5a1f0:      	movq	(%r14), %r14
   5a1f3:      	leaq	0x131236(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5a1fa:      	movq	(%rax), %rax
   5a1fd:      	leaq	0x131234(%rip), %rcx    # 0x18b438 <scoop_thread_world_phase>
   5a204:      	movl	(%rcx), %esi
   5a206:      	movl	(%r15), %edx
   5a209:      	movq	0x8(%r15), %rcx
   5a20d:      	testl	%esi, %esi
   5a20f:      	jne	0x5a21b <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x9b>
   5a211:      	cmpl	$0x1, %edx
   5a214:      	jne	0x5a21b <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x9b>
   5a216:      	cmpq	%rax, %rcx
   5a219:      	je	0x5a220 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xa0>
   5a21b:      	callq	0x5c1d0 <scoop_rt_safepoint>
   5a220:      	cmpl	$0x989680, %r12d        # imm = 0x989680
   5a227:      	jge	0x5a24d <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0xcd>
   5a229:      	movq	%r14, -0x38(%rbp)
   5a22d:      	movq	%r13, -0x30(%rbp)
   5a231:      	movups	-0x38(%rbp), %xmm0
   5a235:      	movups	%xmm0, (%rsp)
   5a239:      	movq	%rbx, %rdi
   5a23c:      	callq	0x5a160 <scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>
   5a241:      	movq	-0x40(%rbp), %r13
   5a245:      	incl	%r12d
   5a248:      	movq	%rbx, %r14
   5a24b:      	jmp	0x5a1f0 <scoop$1$cb$92f77913599bedd8759a863ee0d383930e2a491523a11adcee411a36fa8b5abe+0x70>
   5a24d:      	addq	%r13, %r14
   5a250:      	movq	%r14, %rdi
   5a253:      	callq	0x5a270 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   5a258:      	addq	$0x48, %rsp
   5a25c:      	popq	%rbx
   5a25d:      	popq	%r12
   5a25f:      	popq	%r13
   5a261:      	popq	%r14
   5a263:      	popq	%r15
   5a265:      	popq	%rbp
   5a266:      	retq


; Callee / provider code
0000000000059ba0 <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18>:
   59ba0:      	pushq	%rbp
   59ba1:      	movq	%rsp, %rbp
   59ba4:      	movq	%fs:0x0, %rax
   59bad:      	leaq	-0x10(%rax), %rax
   59bb4:      	movq	(%rax), %rcx
   59bb7:      	leaq	0x131872(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59bbe:      	movq	(%rax), %rax
   59bc1:      	leaq	0x131870(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59bc8:      	movl	(%rdx), %esi
   59bca:      	movl	(%rcx), %edx
   59bcc:      	movq	0x8(%rcx), %rcx
   59bd0:      	testl	%esi, %esi
   59bd2:      	jne	0x59bea <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   59bd4:      	cmpl	$0x1, %edx
   59bd7:      	jne	0x59bea <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   59bd9:      	cmpq	%rax, %rcx
   59bdc:      	jne	0x59bea <scoop$1$cb$58ace877fd64c66cadf071faa774e98d01c8743b2943f36a1165c3f6ebc7dc18+0x4a>
   59bde:      	leaq	0x3242b(%rip), %rdi     # 0x8c010 <scoop$1$bs$9bc83e826cd4d5e3a3793dc5bd199cbfde4f00c4aa0fcdbe7b4944aaf8e7d6a7>
   59be5:      	callq	0x60c00 <scoop_rt_trap>
   59bea:      	callq	0x5c1d0 <scoop_rt_safepoint>
   59bef:      	leaq	0x3241a(%rip), %rdi     # 0x8c010 <scoop$1$bs$9bc83e826cd4d5e3a3793dc5bd199cbfde4f00c4aa0fcdbe7b4944aaf8e7d6a7>
   59bf6:      	callq	0x60c00 <scoop_rt_trap>


0000000000059c00 <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081>:
   59c00:      	pushq	%rbp
   59c01:      	movq	%rsp, %rbp
   59c04:      	movq	%fs:0x0, %rax
   59c0d:      	leaq	-0x10(%rax), %rax
   59c14:      	movq	(%rax), %rcx
   59c17:      	leaq	0x131812(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59c1e:      	movq	(%rax), %rax
   59c21:      	leaq	0x131810(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59c28:      	movl	(%rdx), %esi
   59c2a:      	movl	(%rcx), %edx
   59c2c:      	movq	0x8(%rcx), %rcx
   59c30:      	testl	%esi, %esi
   59c32:      	jne	0x59c4a <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   59c34:      	cmpl	$0x1, %edx
   59c37:      	jne	0x59c4a <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   59c39:      	cmpq	%rax, %rcx
   59c3c:      	jne	0x59c4a <scoop$1$cb$73df65ae6f593504965879a3c15081e885407f77f3bb5c9ebf9e5562a64c2081+0x4a>
   59c3e:      	leaq	0x324ab(%rip), %rdi     # 0x8c0f0 <scoop$1$bs$eef28c67632ed62c81fb4ec30dedfa52467e063c429b84eaecfa443ce5bb7c30>
   59c45:      	callq	0x60c00 <scoop_rt_trap>
   59c4a:      	callq	0x5c1d0 <scoop_rt_safepoint>
   59c4f:      	leaq	0x3249a(%rip), %rdi     # 0x8c0f0 <scoop$1$bs$eef28c67632ed62c81fb4ec30dedfa52467e063c429b84eaecfa443ce5bb7c30>
   59c56:      	callq	0x60c00 <scoop_rt_trap>


0000000000059c60 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe>:
   59c60:      	pushq	%rbp
   59c61:      	movq	%rsp, %rbp
   59c64:      	pushq	%r15
   59c66:      	pushq	%r14
   59c68:      	pushq	%r13
   59c6a:      	pushq	%r12
   59c6c:      	pushq	%rbx
   59c6d:      	subq	$0x168, %rsp            # imm = 0x168
   59c74:      	movq	%rcx, %r14
   59c77:      	movq	%rdx, %r15
   59c7a:      	movq	%rsi, %rbx
   59c7d:      	movq	%rdi, %r12
   59c80:      	movq	%fs:0x0, %rax
   59c89:      	leaq	-0x10(%rax), %rax
   59c90:      	movq	(%rax), %rcx
   59c93:      	movq	%r12, -0x58(%rbp)
   59c97:      	movq	%r15, -0x40(%rbp)
   59c9b:      	movq	%r14, -0x38(%rbp)
   59c9f:      	movq	$0x0, -0x70(%rbp)
   59ca7:      	movq	$0x0, -0x78(%rbp)
   59caf:      	movq	$0x0, -0x80(%rbp)
   59cb7:      	movq	$0x0, -0x88(%rbp)
   59cc2:      	leaq	0x131767(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   59cc9:      	movq	(%rax), %rax
   59ccc:      	leaq	0x131765(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   59cd3:      	movl	(%rdx), %esi
   59cd5:      	movl	(%rcx), %edx
   59cd7:      	movq	0x8(%rcx), %rcx
   59cdb:      	testl	%esi, %esi
   59cdd:      	jne	0x59ce9 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x89>
   59cdf:      	cmpl	$0x1, %edx
   59ce2:      	jne	0x59ce9 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x89>
   59ce4:      	cmpq	%rax, %rcx
   59ce7:      	je	0x59d0e <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0xae>
   59ce9:      	movq	-0x58(%rbp), %rax
   59ced:      	movq	-0x40(%rbp), %rcx
   59cf1:      	movq	%rax, -0x50(%rbp)
   59cf5:      	movq	%rcx, -0x48(%rbp)
   59cf9:      	callq	0x5c1d0 <scoop_rt_safepoint>
   59cfe:      	movq	-0x48(%rbp), %rax
   59d02:      	movq	-0x50(%rbp), %rcx
   59d06:      	movq	%rcx, -0x58(%rbp)
   59d0a:      	movq	%rax, -0x40(%rbp)
   59d0e:      	callq	0x5bcc0 <scoop_rt_context_snapshot>
   59d13:      	movq	-0x58(%rbp), %rcx
   59d17:      	movq	-0x40(%rbp), %rdx
   59d1b:      	movq	%rax, -0x60(%rbp)
   59d1f:      	movq	%rcx, -0x50(%rbp)
   59d23:      	movq	%rdx, -0x48(%rbp)
   59d27:      	leaq	0x9b722(%rip), %rsi     # 0xf5450 <scoop$1$td$87490392603c1b9907331e3d0317a314f8b46008efab47286c4f62af746c6745>
   59d2e:      	movq	%rax, %rdi
   59d31:      	callq	0x5c390 <scoop_rt_context_fork>
   59d36:      	movq	-0x60(%rbp), %rcx
   59d3a:      	movq	-0x48(%rbp), %rdx
   59d3e:      	movq	-0x50(%rbp), %rsi
   59d42:      	movq	%rsi, -0x58(%rbp)
   59d46:      	movq	%rdx, -0x40(%rbp)
   59d4a:      	movq	%rcx, -0x88(%rbp)
   59d51:      	movq	%rax, %rdi
   59d54:      	callq	0x5c170 <scoop_rt_context_enter>
   59d59:      	movq	%rax, -0x30(%rbp)
   59d5d:      	movq	-0x58(%rbp), %rax
   59d61:      	movq	-0x58(%rbp), %r14
   59d65:      	movq	%r14, -0x80(%rbp)
   59d69:      	movq	-0x40(%rbp), %r15
   59d6d:      	movq	-0x38(%rbp), %r12
   59d71:      	leaq	-0x40(%rbp), %rax
   59d75:      	movq	%rax, -0x150(%rbp)
   59d7c:      	leaq	0x106d4d(%rip), %rax    # 0x160ad0 <scoop$1$bs$d838558236e5d68de5433acd73aae0845ce7c5e178f9f30fc49bf147960abccf>
   59d83:      	movq	%rax, -0x148(%rbp)
   59d8a:      	leaq	-0x30(%rbp), %r13
   59d8e:      	movq	%r13, -0x140(%rbp)
   59d95:      	leaq	0x106d44(%rip), %rax    # 0x160ae0 <scoop$1$bs$3bd3bf5dda75a42e5d618b52e341dd730fe494f426a28a1010c75d388f80b7b8>
   59d9c:      	movq	%rax, -0x138(%rbp)
   59da3:      	leaq	-0x80(%rbp), %rax
   59da7:      	movq	%rax, -0x130(%rbp)
   59dae:      	leaq	0x106d3b(%rip), %rax    # 0x160af0 <scoop$1$bs$76a1d41017149d26c27140164906c3cea684a3e784c0b2533c872cf9d41190f3>
   59db5:      	movq	%rax, -0x128(%rbp)
   59dbc:      	xorps	%xmm0, %xmm0
   59dbf:      	movups	%xmm0, -0xd0(%rbp)
   59dc6:      	movq	$0x0, -0xc0(%rbp)
   59dd1:      	leaq	-0xd0(%rbp), %rdi
   59dd8:      	leaq	-0x150(%rbp), %rsi
   59ddf:      	movl	$0x3, %edx
   59de4:      	callq	0x6b9d0 <scoop_rt_push_compiler_roots>
   59de9:      	movq	(%rbx), %rax
   59dec:      	leaq	-0x168(%rbp), %rdi
   59df3:      	movq	%r14, %rsi
   59df6:      	movq	%r15, %rdx
   59df9:      	movq	%r12, %rcx
   59dfc:      	callq	*%rax
   59dfe:      	movq	-0x40(%rbp), %rbx
   59e02:      	movq	-0x38(%rbp), %r14
   59e06:      	movq	-0x30(%rbp), %r15
   59e0a:      	leaq	-0xd0(%rbp), %rdi
   59e11:      	callq	0x6ba80 <scoop_rt_pop_compiler_roots>
   59e16:      	movq	%r14, -0x38(%rbp)
   59e1a:      	movq	%rbx, -0x40(%rbp)
   59e1e:      	movq	%r15, -0x30(%rbp)
   59e22:      	cmpq	$0x0, -0x168(%rbp)
   59e2a:      	jne	0x59ed0 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x270>
   59e30:      	movups	-0x160(%rbp), %xmm0
   59e37:      	movq	-0x40(%rbp), %rax
   59e3b:      	movq	-0x38(%rbp), %r14
   59e3f:      	movq	-0x40(%rbp), %rbx
   59e43:      	movq	%rbx, -0x70(%rbp)
   59e47:      	movups	%xmm0, -0x180(%rbp)
   59e4e:      	movq	%r13, -0xf0(%rbp)
   59e55:      	leaq	0x106c24(%rip), %rax    # 0x160a80 <scoop$1$bs$fea5aa0086f471ccf24f9b6eb0d3547a1cd8de733080ea0ccde863a4315d68e5>
   59e5c:      	movq	%rax, -0xe8(%rbp)
   59e63:      	leaq	-0x70(%rbp), %rax
   59e67:      	movq	%rax, -0xe0(%rbp)
   59e6e:      	leaq	0x106c1b(%rip), %rax    # 0x160a90 <scoop$1$bs$1de6d529dcef63cbd7f68381ef455cac0ac1b5b75ebe12ffdb13574d29e1c90f>
   59e75:      	movq	%rax, -0xd8(%rbp)
   59e7c:      	xorps	%xmm0, %xmm0
   59e7f:      	movups	%xmm0, -0xa0(%rbp)
   59e86:      	movq	$0x0, -0x90(%rbp)
   59e91:      	leaq	-0xa0(%rbp), %rdi
   59e98:      	leaq	-0xf0(%rbp), %rsi
   59e9f:      	movl	$0x2, %edx
   59ea4:      	callq	0x6b9d0 <scoop_rt_push_compiler_roots>
   59ea9:      	movq	(%r14), %rax
   59eac:      	movups	-0x180(%rbp), %xmm0
   59eb3:      	movups	%xmm0, (%rsp)
   59eb7:      	movq	%rbx, %rdi
   59eba:      	callq	*%rax
   59ebc:      	movq	-0x30(%rbp), %rbx
   59ec0:      	leaq	-0xa0(%rbp), %rdi
   59ec7:      	callq	0x6ba80 <scoop_rt_pop_compiler_roots>
   59ecc:      	movq	%rbx, -0x30(%rbp)
   59ed0:      	movq	-0x30(%rbp), %rdi
   59ed4:      	callq	0x5c1a0 <scoop_rt_context_leave>
   59ed9:      	addq	$0x168, %rsp            # imm = 0x168
   59ee0:      	popq	%rbx
   59ee1:      	popq	%r12
   59ee3:      	popq	%r13
   59ee5:      	popq	%r14
   59ee7:      	popq	%r15
   59ee9:      	popq	%rbp
   59eea:      	retq
   59eeb:      	movq	%rax, %rbx
   59eee:      	movq	-0x40(%rbp), %r14
   59ef2:      	movq	-0x38(%rbp), %r15
   59ef6:      	movq	-0x30(%rbp), %r12
   59efa:      	callq	0x6bad0 <scoop_rt_pop_top_compiler_roots>
   59eff:      	movq	%r15, -0x38(%rbp)
   59f03:      	movq	%r14, -0x40(%rbp)
   59f07:      	movq	%r12, -0x30(%rbp)
   59f0b:      	movq	%rbx, %rdi
   59f0e:      	callq	0x68260 <scoop_rt_begin_catch>
   59f13:      	movq	-0x40(%rbp), %rcx
   59f17:      	movq	-0x30(%rbp), %rdx
   59f1b:      	movq	%rax, -0x60(%rbp)
   59f1f:      	movq	%rcx, -0x50(%rbp)
   59f23:      	movq	%rdx, -0x48(%rbp)
   59f27:      	movq	%rax, %rdi
   59f2a:      	callq	0x5c290 <scoop_rt_materialize_exception>
   59f2f:      	movq	-0x60(%rbp), %rcx
   59f33:      	movq	-0x48(%rbp), %rdx
   59f37:      	movq	-0x50(%rbp), %rsi
   59f3b:      	movq	%rsi, -0x40(%rbp)
   59f3f:      	movq	%rdx, -0x30(%rbp)
   59f43:      	movq	%rcx, -0x170(%rbp)
   59f4a:      	movq	%rax, -0x68(%rbp)
   59f4e:      	callq	0x68340 <scoop_rt_end_catch>
   59f53:      	movq	-0x40(%rbp), %rax
   59f57:      	movq	-0x38(%rbp), %r15
   59f5b:      	movq	-0x40(%rbp), %rbx
   59f5f:      	movq	%rbx, -0x78(%rbp)
   59f63:      	movq	-0x68(%rbp), %r14
   59f67:      	leaq	-0x68(%rbp), %rax
   59f6b:      	movq	%rax, -0x120(%rbp)
   59f72:      	leaq	0x106b27(%rip), %rax    # 0x160aa0 <scoop$1$bs$9872cdf7da438a41b8ffee0f4d9767d8ab494ae67b907779f00702fa698662e9>
   59f79:      	movq	%rax, -0x118(%rbp)
   59f80:      	movq	%r13, -0x110(%rbp)
   59f87:      	leaq	0x106b22(%rip), %rax    # 0x160ab0 <scoop$1$bs$09120727c90376dc3e9629a82c16171e19925bf1bf702cbaaa132e253452c419>
   59f8e:      	movq	%rax, -0x108(%rbp)
   59f95:      	leaq	-0x78(%rbp), %rax
   59f99:      	movq	%rax, -0x100(%rbp)
   59fa0:      	leaq	0x106b19(%rip), %rax    # 0x160ac0 <scoop$1$bs$e9b98455ff4ff3d3713cf38ff7154d28ecd1f053350e2f82195185de23382760>
   59fa7:      	movq	%rax, -0xf8(%rbp)
   59fae:      	xorps	%xmm0, %xmm0
   59fb1:      	movups	%xmm0, -0xb8(%rbp)
   59fb8:      	movq	$0x0, -0xa8(%rbp)
   59fc3:      	leaq	-0xb8(%rbp), %rdi
   59fca:      	leaq	-0x120(%rbp), %rsi
   59fd1:      	movl	$0x3, %edx
   59fd6:      	callq	0x6b9d0 <scoop_rt_push_compiler_roots>
   59fdb:      	movq	0x8(%r15), %rax
   59fdf:      	movq	%rbx, %rdi
   59fe2:      	movq	%r14, %rsi
   59fe5:      	callq	*%rax
   59fe7:      	movq	-0x30(%rbp), %rbx
   59feb:      	leaq	-0xb8(%rbp), %rdi
   59ff2:      	jmp	0x59ec7 <scoop$1$cb$ac9074a8958a9cc59d833bedf3af709a119395a9e3dd4efd0d51b67687ecdbbe+0x267>
   59ff7:      	movq	%rax, %rbx
   59ffa:      	movq	-0x30(%rbp), %r14
   59ffe:      	callq	0x6bad0 <scoop_rt_pop_top_compiler_roots>
   5a003:      	movq	%r14, -0x30(%rbp)
   5a007:      	movq	-0x30(%rbp), %rdi
   5a00b:      	callq	0x5c1a0 <scoop_rt_context_leave>
   5a010:      	movq	%rbx, %rdi
   5a013:      	callq	0x29400 <_Unwind_Resume>


000000000005a020 <scoop$1$cb$acd3e5dda25b7bdf7138e34b146c3daeb32f8f25d217af5a91e32bcd2aeda727>:
   5a020:      	pushq	%rbp
   5a021:      	movq	%rsp, %rbp
   5a024:      	movq	%rdi, %rax
   5a027:      	movq	%rsi, (%rdi)
   5a02a:      	movq	%rdx, 0x8(%rdi)
   5a02e:      	popq	%rbp
   5a02f:      	retq


000000000005a030 <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e>:
   5a030:      	pushq	%rbp
   5a031:      	movq	%rsp, %rbp
   5a034:      	movq	%fs:0x0, %rax
   5a03d:      	leaq	-0x10(%rax), %rax
   5a044:      	movq	(%rax), %rcx
   5a047:      	leaq	0x1313e2(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5a04e:      	movq	(%rax), %rax
   5a051:      	leaq	0x1313e0(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   5a058:      	movl	(%rdx), %esi
   5a05a:      	movl	(%rcx), %edx
   5a05c:      	movq	0x8(%rcx), %rcx
   5a060:      	testl	%esi, %esi
   5a062:      	jne	0x5a07a <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   5a064:      	cmpl	$0x1, %edx
   5a067:      	jne	0x5a07a <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   5a069:      	cmpq	%rax, %rcx
   5a06c:      	jne	0x5a07a <scoop$1$cb$502e0e1e1ff0d62b495720e5649604cb752e121c08833420556cca1cb1fb964e+0x4a>
   5a06e:      	leaq	0x320ab(%rip), %rdi     # 0x8c120 <scoop$1$bs$16fca5c8b23b886ebb2b482824651319245072c6ab6dc17abf3cda36df97aecc>
   5a075:      	callq	0x60c00 <scoop_rt_trap>
   5a07a:      	callq	0x5c1d0 <scoop_rt_safepoint>
   5a07f:      	leaq	0x3209a(%rip), %rdi     # 0x8c120 <scoop$1$bs$16fca5c8b23b886ebb2b482824651319245072c6ab6dc17abf3cda36df97aecc>
   5a086:      	callq	0x60c00 <scoop_rt_trap>


000000000005a090 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389>:
   5a090:      	pushq	%rbp
   5a091:      	movq	%rsp, %rbp
   5a094:      	pushq	%r14
   5a096:      	pushq	%rbx
   5a097:      	leaq	0x20(%rbp), %rbx
   5a09b:      	leaq	0x10(%rbp), %r14
   5a09f:      	movq	%fs:0x0, %rax
   5a0a8:      	leaq	-0x10(%rax), %rax
   5a0af:      	movq	(%rax), %rcx
   5a0b2:      	leaq	0x131377(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5a0b9:      	movq	(%rax), %rax
   5a0bc:      	leaq	0x131375(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   5a0c3:      	movl	(%rdx), %esi
   5a0c5:      	movl	(%rcx), %edx
   5a0c7:      	movq	0x8(%rcx), %rcx
   5a0cb:      	testl	%esi, %esi
   5a0cd:      	jne	0x5a0d9 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x49>
   5a0cf:      	cmpl	$0x1, %edx
   5a0d2:      	jne	0x5a0d9 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x49>
   5a0d4:      	cmpq	%rax, %rcx
   5a0d7:      	je	0x5a0de <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x4e>
   5a0d9:      	callq	0x5c1d0 <scoop_rt_safepoint>
   5a0de:      	movq	(%r14), %rax
   5a0e1:      	cmpq	(%rbx), %rax
   5a0e4:      	jne	0x5a0f3 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x63>
   5a0e6:      	movq	0x8(%r14), %rax
   5a0ea:      	cmpq	0x8(%rbx), %rax
   5a0ee:      	sete	%al
   5a0f1:      	jmp	0x5a0f5 <scoop$1$cb$2cd772d1386a69047f4b17050c78ed02c2c9ecf015b0f79ef3c7b29e54dd1389+0x65>
   5a0f3:      	xorl	%eax, %eax
   5a0f5:      	popq	%rbx
   5a0f6:      	popq	%r14
   5a0f8:      	popq	%rbp
   5a0f9:      	retq


000000000005a100 <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03>:
   5a100:      	pushq	%rbp
   5a101:      	movq	%rsp, %rbp
   5a104:      	movq	%fs:0x0, %rax
   5a10d:      	leaq	-0x10(%rax), %rax
   5a114:      	movq	(%rax), %rcx
   5a117:      	leaq	0x131312(%rip), %rax    # 0x18b430 <scoop_thread_gc_epoch>
   5a11e:      	movq	(%rax), %rax
   5a121:      	leaq	0x131310(%rip), %rdx    # 0x18b438 <scoop_thread_world_phase>
   5a128:      	movl	(%rdx), %esi
   5a12a:      	movl	(%rcx), %edx
   5a12c:      	movq	0x8(%rcx), %rcx
   5a130:      	testl	%esi, %esi
   5a132:      	jne	0x5a14a <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   5a134:      	cmpl	$0x1, %edx
   5a137:      	jne	0x5a14a <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   5a139:      	cmpq	%rax, %rcx
   5a13c:      	jne	0x5a14a <scoop$1$cb$a4256dbfa17f2208a4fca43acee925757bb5c1528ac49cee3740b6539a815d03+0x4a>
   5a13e:      	leaq	0x322eb(%rip), %rdi     # 0x8c430 <scoop$1$bs$bcfe2ea1fa66203f30659828e5e934155733245e98e4cef8e44324cbf7d7eb39>
   5a145:      	callq	0x60c00 <scoop_rt_trap>
   5a14a:      	callq	0x5c1d0 <scoop_rt_safepoint>
   5a14f:      	leaq	0x322da(%rip), %rdi     # 0x8c430 <scoop$1$bs$bcfe2ea1fa66203f30659828e5e934155733245e98e4cef8e44324cbf7d7eb39>
   5a156:      	callq	0x60c00 <scoop_rt_trap>


000000000005a160 <scoop$1$cb$64a215114cc4a4dba695a2ad62a852d38ee16f2cec89138f711ddd904704b462>:
   5a160:      	pushq	%rbp
   5a161:      	movq	%rsp, %rbp
   5a164:      	movq	%rdi, %rax
   5a167:      	movq	0x10(%rbp), %rcx
   5a16b:      	movq	0x18(%rbp), %rdx
   5a16f:      	incq	%rcx
   5a172:      	movq	%rcx, (%rdi)
   5a175:      	movq	%rdx, 0x8(%rdi)
   5a179:      	popq	%rbp
   5a17a:      	retq
