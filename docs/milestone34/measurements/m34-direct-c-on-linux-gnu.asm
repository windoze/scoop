000000000005acc0 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49>:
   5acc0:      	pushq	%rbp
   5acc1:      	movq	%rsp, %rbp
   5acc4:      	pushq	%r15
   5acc6:      	pushq	%r14
   5acc8:      	pushq	%r13
   5acca:      	pushq	%r12
   5accc:      	pushq	%rbx
   5accd:      	pushq	%rax
   5acce:      	movl	%edi, %ebx
   5acd0:      	movq	%fs:0x0, %rax
   5acd9:      	leaq	-0x10(%rax), %rax
   5ace0:      	movq	(%rax), %r12
   5ace3:      	leaq	0x137746(%rip), %rax    # 0x192430 <scoop_thread_gc_epoch>
   5acea:      	movq	(%rax), %rax
   5aced:      	leaq	0x137744(%rip), %r13    # 0x192438 <scoop_thread_world_phase>
   5acf4:      	movl	(%r13), %esi
   5acf8:      	movl	(%r12), %edx
   5acfc:      	movq	0x8(%r12), %rcx
   5ad01:      	testl	%esi, %esi
   5ad03:      	jne	0x5ad0f <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x4f>
   5ad05:      	cmpl	$0x1, %edx
   5ad08:      	jne	0x5ad0f <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x4f>
   5ad0a:      	cmpq	%rax, %rcx
   5ad0d:      	je	0x5ad14 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x54>
   5ad0f:      	callq	0x5e0d0 <scoop_rt_safepoint>
   5ad14:      	xorl	%r14d, %r14d
   5ad17:      	xorl	%r15d, %r15d
   5ad1a:      	nopw	(%rax,%rax)
   5ad20:      	leaq	0x137709(%rip), %rax    # 0x192430 <scoop_thread_gc_epoch>
   5ad27:      	movq	(%rax), %rax
   5ad2a:      	movl	(%r13), %esi
   5ad2e:      	movl	(%r12), %edx
   5ad32:      	movq	0x8(%r12), %rcx
   5ad37:      	testl	%esi, %esi
   5ad39:      	jne	0x5ad45 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x85>
   5ad3b:      	cmpl	$0x1, %edx
   5ad3e:      	jne	0x5ad45 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x85>
   5ad40:      	cmpq	%rax, %rcx
   5ad43:      	je	0x5ad4a <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x8a>
   5ad45:      	callq	0x5e0d0 <scoop_rt_safepoint>
   5ad4a:      	cmpl	%ebx, %r14d
   5ad4d:      	jge	0x5ad65 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0xa5>
   5ad4f:      	movq	%r14, %rdi
   5ad52:      	movq	%r14, %rsi
   5ad55:      	callq	0x76240 <bench_pair>
   5ad5a:      	addq	%rax, %rdx
   5ad5d:      	addq	%rdx, %r15
   5ad60:      	incq	%r14
   5ad63:      	jmp	0x5ad20 <scoop$1$cb$727eec538eaabbd8c1cdb35be1783bf1e0a6fc261c5554e3c35621512e276f49+0x60>
   5ad65:      	movq	%r15, %rax
   5ad68:      	addq	$0x8, %rsp
   5ad6c:      	popq	%rbx
   5ad6d:      	popq	%r12
   5ad6f:      	popq	%r13
   5ad71:      	popq	%r14
   5ad73:      	popq	%r15
   5ad75:      	popq	%rbp
   5ad76:      	retq


000000000005b2d0 <scoop$1$cb$4802a025b776cfaa4b4d6f1276193dcff63e64f0f8cf99fc004b462585fe063a>:
   5b2d0:      	pushq	%rbp
   5b2d1:      	movq	%rsp, %rbp
   5b2d4:      	callq	0x76240 <bench_pair>
   5b2d9:      	popq	%rbp
   5b2da:      	retq


000000000005bcb0 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e>:
   5bcb0:      	pushq	%rbp
   5bcb1:      	movq	%rsp, %rbp
   5bcb4:      	pushq	%r15
   5bcb6:      	pushq	%r14
   5bcb8:      	pushq	%r13
   5bcba:      	pushq	%r12
   5bcbc:      	pushq	%rbx
   5bcbd:      	subq	$0x68, %rsp
   5bcc1:      	movl	%edi, -0x2c(%rbp)
   5bcc4:      	movq	%fs:0x0, %rax
   5bccd:      	leaq	-0x10(%rax), %rax
   5bcd4:      	movq	(%rax), %rdi
   5bcd7:      	leaq	0x136752(%rip), %rax    # 0x192430 <scoop_thread_gc_epoch>
   5bcde:      	movq	(%rax), %rax
   5bce1:      	leaq	0x136750(%rip), %rcx    # 0x192438 <scoop_thread_world_phase>
   5bce8:      	movl	(%rcx), %esi
   5bcea:      	movl	(%rdi), %edx
   5bcec:      	movq	%rdi, -0x38(%rbp)
   5bcf0:      	movq	0x8(%rdi), %rcx
   5bcf4:      	testl	%esi, %esi
   5bcf6:      	jne	0x5bd02 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x52>
   5bcf8:      	cmpl	$0x1, %edx
   5bcfb:      	jne	0x5bd02 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x52>
   5bcfd:      	cmpq	%rax, %rcx
   5bd00:      	je	0x5bd07 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x57>
   5bd02:      	callq	0x5e0d0 <scoop_rt_safepoint>
   5bd07:      	xorl	%r14d, %r14d
   5bd0a:      	xorl	%ebx, %ebx
   5bd0c:      	nopl	(%rax)
   5bd10:      	leaq	0x136719(%rip), %rax    # 0x192430 <scoop_thread_gc_epoch>
   5bd17:      	movq	(%rax), %rax
   5bd1a:      	leaq	0x136717(%rip), %rcx    # 0x192438 <scoop_thread_world_phase>
   5bd21:      	movl	(%rcx), %esi
   5bd23:      	movq	-0x38(%rbp), %rcx
   5bd27:      	movl	(%rcx), %edx
   5bd29:      	movq	0x8(%rcx), %rcx
   5bd2d:      	testl	%esi, %esi
   5bd2f:      	jne	0x5bd3b <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x8b>
   5bd31:      	cmpl	$0x1, %edx
   5bd34:      	jne	0x5bd3b <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x8b>
   5bd36:      	cmpq	%rax, %rcx
   5bd39:      	je	0x5bd40 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x90>
   5bd3b:      	callq	0x5e0d0 <scoop_rt_safepoint>
   5bd40:      	cmpl	-0x2c(%rbp), %r14d
   5bd44:      	jge	0x5bdc0 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x110>
   5bd46:      	xorps	%xmm0, %xmm0
   5bd49:      	movups	%xmm0, -0x50(%rbp)
   5bd4d:      	movq	$0x0, -0x40(%rbp)
   5bd55:      	leaq	-0x50(%rbp), %rdi
   5bd59:      	xorl	%esi, %esi
   5bd5b:      	xorl	%edx, %edx
   5bd5d:      	callq	0x6d7d0 <scoop_rt_push_caller_roots>
   5bd62:      	xorps	%xmm0, %xmm0
   5bd65:      	movups	%xmm0, -0x90(%rbp)
   5bd6c:      	movups	%xmm0, -0x80(%rbp)
   5bd70:      	movups	%xmm0, -0x70(%rbp)
   5bd74:      	movups	%xmm0, -0x60(%rbp)
   5bd78:      	leaq	-0x90(%rbp), %r13
   5bd7f:      	movq	%r13, %rdi
   5bd82:      	movq	%r13, %rsi
   5bd85:      	callq	0x5e2d0 <scoop_rt_enter_native_safe>
   5bd8a:      	movq	%r14, %rdi
   5bd8d:      	movq	%r14, %rsi
   5bd90:      	callq	0x76240 <bench_pair>
   5bd95:      	movq	%rbx, %r12
   5bd98:      	movq	%rax, %rbx
   5bd9b:      	movq	%rdx, %r15
   5bd9e:      	movq	%r13, %rdi
   5bda1:      	callq	0x61490 <scoop_rt_leave_native_safe>
   5bda6:      	leaq	-0x50(%rbp), %rdi
   5bdaa:      	callq	0x6d880 <scoop_rt_pop_caller_roots>
   5bdaf:      	addq	%rbx, %r15
   5bdb2:      	movq	%r12, %rbx
   5bdb5:      	addq	%r15, %rbx
   5bdb8:      	incq	%r14
   5bdbb:      	jmp	0x5bd10 <scoop$1$cb$d0796ce9ba7a213ba17765b3c6b43fa113c969faa05d1696a2fae58e6629f93e+0x60>
   5bdc0:      	movq	%rbx, %rax
   5bdc3:      	addq	$0x68, %rsp
   5bdc7:      	popq	%rbx
   5bdc8:      	popq	%r12
   5bdca:      	popq	%r13
   5bdcc:      	popq	%r14
   5bdce:      	popq	%r15
   5bdd0:      	popq	%rbp
   5bdd1:      	retq


000000000005bea0 <scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab>:
   5bea0:      	pushq	%rbp
   5bea1:      	movq	%rsp, %rbp
   5bea4:      	pushq	%r15
   5bea6:      	pushq	%r14
   5bea8:      	pushq	%r12
   5beaa:      	pushq	%rbx
   5beab:      	subq	$0x60, %rsp
   5beaf:      	movq	%rsi, %rbx
   5beb2:      	movq	%rdi, %r14
   5beb5:      	movq	%fs:0x0, %rax
   5bebe:      	leaq	-0x10(%rax), %rax
   5bec5:      	movq	(%rax), %rcx
   5bec8:      	leaq	0x136561(%rip), %rax    # 0x192430 <scoop_thread_gc_epoch>
   5becf:      	movq	(%rax), %rax
   5bed2:      	leaq	0x13655f(%rip), %rdx    # 0x192438 <scoop_thread_world_phase>
   5bed9:      	movl	(%rdx), %esi
   5bedb:      	movl	(%rcx), %edx
   5bedd:      	movq	0x8(%rcx), %rcx
   5bee1:      	testl	%esi, %esi
   5bee3:      	jne	0x5beef <scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x4f>
   5bee5:      	cmpl	$0x1, %edx
   5bee8:      	jne	0x5beef <scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x4f>
   5beea:      	cmpq	%rax, %rcx
   5beed:      	je	0x5bef4 <scoop$1$cb$3d4d2ef74f3e937ff2690df2a3f94a5b39561dbba24a97c4f84f983d86073fab+0x54>
   5beef:      	callq	0x5e0d0 <scoop_rt_safepoint>
   5bef4:      	xorps	%xmm0, %xmm0
   5bef7:      	movups	%xmm0, -0x38(%rbp)
   5befb:      	movq	$0x0, -0x28(%rbp)
   5bf03:      	leaq	-0x38(%rbp), %r15
   5bf07:      	movq	%r15, %rdi
   5bf0a:      	xorl	%esi, %esi
   5bf0c:      	xorl	%edx, %edx
   5bf0e:      	callq	0x6d7d0 <scoop_rt_push_caller_roots>
   5bf13:      	xorps	%xmm0, %xmm0
   5bf16:      	movups	%xmm0, -0x78(%rbp)
   5bf1a:      	movups	%xmm0, -0x68(%rbp)
   5bf1e:      	movups	%xmm0, -0x58(%rbp)
   5bf22:      	movups	%xmm0, -0x48(%rbp)
   5bf26:      	leaq	-0x78(%rbp), %r12
   5bf2a:      	movq	%r12, %rdi
   5bf2d:      	movq	%r12, %rsi
   5bf30:      	callq	0x5e2d0 <scoop_rt_enter_native_safe>
   5bf35:      	movq	%r14, %rdi
   5bf38:      	movq	%rbx, %rsi
   5bf3b:      	callq	0x76240 <bench_pair>
   5bf40:      	movq	%rax, %rbx
   5bf43:      	movq	%rdx, %r14
   5bf46:      	movq	%r12, %rdi
   5bf49:      	callq	0x61490 <scoop_rt_leave_native_safe>
   5bf4e:      	movq	%r15, %rdi
   5bf51:      	callq	0x6d880 <scoop_rt_pop_caller_roots>
   5bf56:      	movq	%rbx, %rax
   5bf59:      	movq	%r14, %rdx
   5bf5c:      	addq	$0x60, %rsp
   5bf60:      	popq	%rbx
   5bf61:      	popq	%r12
   5bf63:      	popq	%r14
   5bf65:      	popq	%r15
   5bf67:      	popq	%rbp
   5bf68:      	retq


0000000000076280 <run_worker>:
   76280:      	endbr64
   76284:      	pushq	%rbp
   76285:      	movq	%rsp, %rbp
   76288:      	pushq	%r13
   7628a:      	pushq	%r12
   7628c:      	pushq	%rbx
   7628d:      	subq	$0x8, %rsp
   76291:      	lock
   76292:      	addl	$0x1, 0x2a2e37(%rip)    # 0x3190d0 <ready>
   76299:      	jmp	0x762a5 <run_worker+0x25>
   7629b:      	nopl	(%rax,%rax)
   762a0:      	callq	0x2a2f0 <sched_yield@plt>
   762a5:      	movzbl	0x2a2e1c(%rip), %eax    # 0x3190c8 <start>
   762ac:      	testb	%al, %al
   762ae:      	je	0x762a0 <run_worker+0x20>
   762b0:      	movl	0x2a2e1e(%rip), %eax    # 0x3190d4 <selected>
   762b6:      	movslq	0x2a2e1b(%rip), %rdi    # 0x3190d8 <iterations>
   762bd:      	testl	%eax, %eax
   762bf:      	jne	0x76320 <run_worker+0xa0>
   762c1:      	testl	%edi, %edi
   762c3:      	jle	0x763a5 <run_worker+0x125>
   762c9:      	xorl	%ebx, %ebx
   762cb:      	xorl	%r12d, %r12d
   762ce:      	nop
   762d0:      	movq	%rbx, %rdi
   762d3:      	addq	$0x1, %rbx
   762d7:      	callq	0x76230 <bench_scalar>
   762dc:      	movslq	0x2a2df5(%rip), %rdi    # 0x3190d8 <iterations>
   762e3:      	addq	%rax, %r12
   762e6:      	cmpl	%ebx, %edi
   762e8:      	jg	0x762d0 <run_worker+0x50>
   762ea:      	movl	0x2a2de4(%rip), %eax    # 0x3190d4 <selected>
   762f0:      	subl	$0x3, %eax
   762f3:      	cmpl	$0x2, %eax
   762f6:      	ja	0x76353 <run_worker+0xd3>
   762f8:      	leaq	0x2(%rdi), %rax
   762fc:      	imulq	%rdi, %rax
   76300:      	cmpq	%r12, %rax
   76303:      	jne	0x2a480 <run_worker.cold>
   76309:      	lock
   7630a:      	addl	$0x1, 0x2a2dbb(%rip)    # 0x3190cc <finished>
   76311:      	xorl	%eax, %eax
   76313:      	addq	$0x8, %rsp
   76317:      	popq	%rbx
   76318:      	popq	%r12
   7631a:      	popq	%r13
   7631c:      	popq	%rbp
   7631d:      	retq
   7631e:      	nop
   76320:      	cmpl	$0x3, %eax
   76323:      	je	0x76370 <run_worker+0xf0>
   76325:      	cmpl	$0x6, %eax
   76328:      	je	0x763b0 <run_worker+0x130>
   7632e:      	movq	0x2a2db3(%rip), %rsi    # 0x3190e8 <worker_context>
   76335:      	callq	*0x2a2dbd(%rip)         # 0x3190f8 <worker>
   7633b:      	movslq	0x2a2d96(%rip), %rdi    # 0x3190d8 <iterations>
   76342:      	movq	%rax, %r12
   76345:      	movl	0x2a2d89(%rip), %eax    # 0x3190d4 <selected>
   7634b:      	subl	$0x3, %eax
   7634e:      	cmpl	$0x2, %eax
   76351:      	jbe	0x762f8 <run_worker+0x78>
   76353:      	leaq	0x1(%rdi), %rdx
   76357:      	imulq	%rdi, %rdx
   7635b:      	movq	%rdx, %rax
   7635e:      	shrq	$0x3f, %rax
   76362:      	addq	%rdx, %rax
   76365:      	sarq	%rax
   76368:      	jmp	0x76300 <run_worker+0x80>
   7636a:      	nopw	(%rax,%rax)
   76370:      	testl	%edi, %edi
   76372:      	jle	0x76401 <run_worker+0x181>
   76378:      	xorl	%ebx, %ebx
   7637a:      	xorl	%r12d, %r12d
   7637d:      	nopl	(%rax)
   76380:      	movq	%rbx, %rdi
   76383:      	movq	%rbx, %rsi
   76386:      	addq	$0x1, %rbx
   7638a:      	callq	0x76240 <bench_pair>
   7638f:      	movslq	0x2a2d42(%rip), %rdi    # 0x3190d8 <iterations>
   76396:      	addq	%rdx, %rax
   76399:      	addq	%rax, %r12
   7639c:      	cmpl	%ebx, %edi
   7639e:      	jg	0x76380 <run_worker+0x100>
   763a0:      	jmp	0x762ea <run_worker+0x6a>
   763a5:      	xorl	%r12d, %r12d
   763a8:      	jmp	0x76353 <run_worker+0xd3>
   763aa:      	nopw	(%rax,%rax)
   763b0:      	testl	%edi, %edi
   763b2:      	jle	0x763a5 <run_worker+0x125>
   763b4:      	callq	0x2a040 <__errno_location@plt>
   763b9:      	xorl	%ebx, %ebx
   763bb:      	xorl	%r12d, %r12d
   763be:      	movq	%rax, %r13
   763c1:      	nopl	(%rax)
   763c5:      	nopw	%cs:(%rax,%rax)
   763d0:      	movl	$0x0, (%r13)
   763d8:      	movq	%rbx, %rdi
   763db:      	addq	$0x1, %rbx
   763df:      	callq	0x76230 <bench_scalar>
   763e4:      	movslq	0x2a2ced(%rip), %rdi    # 0x3190d8 <iterations>
   763eb:      	movq	%rax, %rdx
   763ee:      	movslq	(%r13), %rax
   763f2:      	addq	%rdx, %rax
   763f5:      	addq	%rax, %r12
   763f8:      	cmpl	%ebx, %edi
   763fa:      	jg	0x763d0 <run_worker+0x150>
   763fc:      	jmp	0x762ea <run_worker+0x6a>
   76401:      	xorl	%r12d, %r12d
   76404:      	jmp	0x762f8 <run_worker+0x78>
   76409:      	nopl	(%rax)


/home/chenxu/repos/scoop/tmp/m34/direct-c-after-gnu/leaf.o:	file format elf64-x86-64

Disassembly of section .text:

0000000000000000 <bench_scalar>:
       0:      	endbr64
       4:      	leaq	0x1(%rdi), %rax
       8:      	retq
       9:      	nopl	(%rax)

0000000000000010 <bench_pair>:
      10:      	endbr64
      14:      	leaq	0x2(%rsi), %rax
      18:      	addq	$0x1, %rdi
      1c:      	movq	%rax, %rdx
      1f:      	movq	%rdi, %rax
      22:      	retq
