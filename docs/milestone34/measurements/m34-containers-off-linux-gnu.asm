; lowOccupancy, MIR fn1, LIR scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000008d130 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2>:
   8d130:      	pushq	%rbp
   8d131:      	movq	%rsp, %rbp
   8d134:      	pushq	%r15
   8d136:      	pushq	%r14
   8d138:      	pushq	%r13
   8d13a:      	pushq	%r12
   8d13c:      	pushq	%rbx
   8d13d:      	subq	$0x268, %rsp            # imm = 0x268
   8d144:      	movq	%rdi, -0x60(%rbp)
   8d148:      	movq	%fs:0x0, %rax
   8d151:      	leaq	-0x10(%rax), %rax
   8d158:      	movq	(%rax), %rcx
   8d15b:      	leaq	0x1d74ae(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   8d162:      	movq	(%rax), %rax
   8d165:      	leaq	0x1d74ac(%rip), %rdx    # 0x264618 <scoop_thread_world_phase>
   8d16c:      	movl	(%rdx), %esi
   8d16e:      	movl	(%rcx), %edx
   8d170:      	movq	%rcx, -0x68(%rbp)
   8d174:      	movq	0x8(%rcx), %rcx
   8d178:      	testl	%esi, %esi
   8d17a:      	jne	0x8d186 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x56>
   8d17c:      	cmpl	$0x1, %edx
   8d17f:      	jne	0x8d186 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x56>
   8d181:      	cmpq	%rax, %rcx
   8d184:      	je	0x8d18b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5b>
   8d186:      	callq	0x9b0c0 <scoop_rt_safepoint>
   8d18b:      	movabsq	$0x7fffffffffffffff, %r12 # imm = 0x7FFFFFFFFFFFFFFF
   8d195:      	xorps	%xmm0, %xmm0
   8d198:      	movups	%xmm0, -0xe8(%rbp)
   8d19f:      	movq	$0x0, -0xd8(%rbp)
   8d1aa:      	leaq	-0xe8(%rbp), %r14
   8d1b1:      	movq	%r14, %rdi
   8d1b4:      	xorl	%esi, %esi
   8d1b6:      	xorl	%edx, %edx
   8d1b8:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d1bd:      	xorps	%xmm0, %xmm0
   8d1c0:      	movups	%xmm0, -0x188(%rbp)
   8d1c7:      	movups	%xmm0, -0x178(%rbp)
   8d1ce:      	movups	%xmm0, -0x168(%rbp)
   8d1d5:      	movups	%xmm0, -0x158(%rbp)
   8d1dc:      	leaq	-0x188(%rbp), %r15
   8d1e3:      	movq	%r15, %rdi
   8d1e6:      	movq	%r15, %rsi
   8d1e9:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d1ee:      	callq	0xb4500 <m34_container_begin>
   8d1f3:      	movq	%r15, %rdi
   8d1f6:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d1fb:      	movq	%r14, %rdi
   8d1fe:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d203:      	movq	0x18380e(%rip), %r14    # 0x210a18 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb+0x18>
   8d20a:      	movq	%r14, %r13
   8d20d:      	negq	%r13
   8d210:      	leaq	(%r14,%r12), %rax
   8d214:      	cmpq	$-0x20, %rax
   8d218:      	setae	%bl
   8d21b:      	leaq	0x1f(%r14), %r15
   8d21f:      	andq	%r13, %r15
   8d222:      	movq	%fs:0x0, %rax
   8d22b:      	leaq	-0x8(%rax), %rax
   8d232:      	movq	(%rax), %rax
   8d235:      	movq	(%rax), %rcx
   8d238:      	addq	%rcx, %r14
   8d23b:      	decq	%r14
   8d23e:      	andq	%r13, %r14
   8d241:      	leaq	(%r14,%r15), %rcx
   8d245:      	testq	%r14, %r14
   8d248:      	sete	%dl
   8d24b:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   8d252:      	setae	%sil
   8d256:      	cmpq	0x8(%rax), %rcx
   8d25a:      	seta	%dil
   8d25e:      	cmpq	$0x0, 0x18382a(%rip)    # 0x210a90 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb+0x90>
   8d266:      	setne	%r8b
   8d26a:      	orb	%sil, %r8b
   8d26d:      	orb	%dl, %r8b
   8d270:      	orb	%bl, %r8b
   8d273:      	orb	%dil, %r8b
   8d276:      	jne	0x8d28f <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15f>
   8d278:      	movq	%rcx, (%rax)
   8d27b:      	leaq	0x18377e(%rip), %rsi    # 0x210a00 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb>
   8d282:      	movq	%r14, %rdi
   8d285:      	movq	%r15, %rdx
   8d288:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   8d28d:      	jmp	0x8d2a3 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x173>
   8d28f:      	leaq	0x18376a(%rip), %rdi    # 0x210a00 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb>
   8d296:      	movl	$0x20, %esi
   8d29b:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   8d2a0:      	movq	%rax, %r14
   8d2a3:      	movq	%r14, -0x30(%rbp)
   8d2a7:      	movq	%r14, %rdi
   8d2aa:      	movq	-0x60(%rbp), %rsi
   8d2ae:      	callq	0x96130 <scoop$1$cb$8492f4f1e40312d9a91dc01340f047a06736ed40daece5034713c3ce75f36d3a>
   8d2b3:      	movq	-0x30(%rbp), %rax
   8d2b7:      	movq	%rax, -0xd0(%rbp)
   8d2be:      	movq	%rax, -0x38(%rbp)
   8d2c2:      	leaq	-0x38(%rbp), %rax
   8d2c6:      	movq	%rax, -0x80(%rbp)
   8d2ca:      	leaq	0x16de4f(%rip), %rax    # 0x1fb120 <scoop$1$bs$8e0c9f84dcb272dd6e27fb1eb4673c930eeecf18c61bbc1619e4cd7998fb3608>
   8d2d1:      	movq	%rax, -0x78(%rbp)
   8d2d5:      	xorps	%xmm0, %xmm0
   8d2d8:      	movups	%xmm0, -0x100(%rbp)
   8d2df:      	movq	$0x0, -0xf0(%rbp)
   8d2ea:      	leaq	-0x100(%rbp), %rbx
   8d2f1:      	leaq	-0x80(%rbp), %rsi
   8d2f5:      	movl	$0x1, %edx
   8d2fa:      	movq	%rbx, %rdi
   8d2fd:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d302:      	xorps	%xmm0, %xmm0
   8d305:      	movups	%xmm0, -0x1c8(%rbp)
   8d30c:      	movups	%xmm0, -0x1b8(%rbp)
   8d313:      	movups	%xmm0, -0x1a8(%rbp)
   8d31a:      	movups	%xmm0, -0x198(%rbp)
   8d321:      	leaq	-0x1c8(%rbp), %r14
   8d328:      	movq	%r14, %rdi
   8d32b:      	movq	%r14, %rsi
   8d32e:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d333:      	xorl	%edi, %edi
   8d335:      	callq	0xb45a0 <m34_container_end>
   8d33a:      	movq	%r14, %rdi
   8d33d:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d342:      	movq	-0x38(%rbp), %r15
   8d346:      	movq	%rbx, %rdi
   8d349:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d34e:      	movq	%r15, -0x38(%rbp)
   8d352:      	movq	%r15, -0x50(%rbp)
   8d356:      	movq	0x185b5b(%rip), %rbx    # 0x212eb8 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x18>
   8d35d:      	movq	%rbx, %r13
   8d360:      	negq	%r13
   8d363:      	addq	%rbx, %r12
   8d366:      	cmpq	$-0x18, %r12
   8d36a:      	setae	%r12b
   8d36e:      	leaq	0x17(%rbx), %r14
   8d372:      	andq	%r13, %r14
   8d375:      	movq	%fs:0x0, %rax
   8d37e:      	leaq	-0x8(%rax), %rax
   8d385:      	movq	(%rax), %rax
   8d388:      	movq	(%rax), %rcx
   8d38b:      	addq	%rcx, %rbx
   8d38e:      	decq	%rbx
   8d391:      	andq	%r13, %rbx
   8d394:      	leaq	(%rbx,%r14), %rcx
   8d398:      	testq	%rbx, %rbx
   8d39b:      	sete	%dl
   8d39e:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   8d3a5:      	setae	%sil
   8d3a9:      	cmpq	0x8(%rax), %rcx
   8d3ad:      	seta	%dil
   8d3b1:      	cmpq	$0x0, 0x185b77(%rip)    # 0x212f30 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x90>
   8d3b9:      	setne	%r8b
   8d3bd:      	orb	%sil, %r8b
   8d3c0:      	orb	%dl, %r8b
   8d3c3:      	orb	%r12b, %r8b
   8d3c6:      	orb	%dil, %r8b
   8d3c9:      	jne	0x8d3e2 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x2b2>
   8d3cb:      	movq	%rcx, (%rax)
   8d3ce:      	leaq	0x185acb(%rip), %rsi    # 0x212ea0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8d3d5:      	movq	%rbx, %rdi
   8d3d8:      	movq	%r14, %rdx
   8d3db:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   8d3e0:      	jmp	0x8d412 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x2e2>
   8d3e2:      	movq	-0x38(%rbp), %rax
   8d3e6:      	movq	%rax, -0x30(%rbp)
   8d3ea:      	movq	%r15, -0x40(%rbp)
   8d3ee:      	leaq	0x185aab(%rip), %rdi    # 0x212ea0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8d3f5:      	movl	$0x18, %esi
   8d3fa:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   8d3ff:      	movq	%rax, %rbx
   8d402:      	movq	-0x40(%rbp), %rax
   8d406:      	movq	-0x30(%rbp), %rcx
   8d40a:      	movq	%rcx, -0x38(%rbp)
   8d40e:      	movq	%rax, -0x50(%rbp)
   8d412:      	movl	$0x7, 0x10(%rbx)
   8d419:      	movq	-0x38(%rbp), %rax
   8d41d:      	movq	-0x50(%rbp), %rdi
   8d421:      	movq	%rbx, -0x40(%rbp)
   8d425:      	movq	%rdi, -0x30(%rbp)
   8d429:      	movq	%rax, -0x58(%rbp)
   8d42d:      	movq	%rbx, %rsi
   8d430:      	callq	0x91d50 <scoop$1$cb$0f1041b9eb91408efa4bc77ccb6f369d37f6c7070c63fb1f153f47cdf3e294f8>
   8d435:      	movq	-0x58(%rbp), %rax
   8d439:      	movq	-0x40(%rbp), %rcx
   8d43d:      	movq	-0x30(%rbp), %rdx
   8d441:      	movq	%rax, -0x38(%rbp)
   8d445:      	movq	%rdx, -0x50(%rbp)
   8d449:      	movq	%rcx, -0xc8(%rbp)
   8d450:      	movq	%rax, -0x48(%rbp)
   8d454:      	leaq	-0x38(%rbp), %rax
   8d458:      	movq	%rax, -0xc0(%rbp)
   8d45f:      	leaq	0x16dcea(%rip), %rax    # 0x1fb150 <scoop$1$bs$c09917c0399bcaa0cd2d4061806fd6aad7b455d45380d1e931b4622784220a26>
   8d466:      	movq	%rax, -0xb8(%rbp)
   8d46d:      	leaq	-0x48(%rbp), %rax
   8d471:      	movq	%rax, -0xb0(%rbp)
   8d478:      	leaq	0x16dce1(%rip), %rax    # 0x1fb160 <scoop$1$bs$dd63e8327353c664d559879a00238d151851f41dd9ba9521794aa396f7f4d1e8>
   8d47f:      	movq	%rax, -0xa8(%rbp)
   8d486:      	xorps	%xmm0, %xmm0
   8d489:      	movups	%xmm0, -0x148(%rbp)
   8d490:      	movq	$0x0, -0x138(%rbp)
   8d49b:      	leaq	-0x148(%rbp), %rbx
   8d4a2:      	leaq	-0xc0(%rbp), %rsi
   8d4a9:      	movl	$0x2, %edx
   8d4ae:      	movq	%rbx, %rdi
   8d4b1:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d4b6:      	xorps	%xmm0, %xmm0
   8d4b9:      	movups	%xmm0, -0x288(%rbp)
   8d4c0:      	movups	%xmm0, -0x278(%rbp)
   8d4c7:      	movups	%xmm0, -0x268(%rbp)
   8d4ce:      	movups	%xmm0, -0x258(%rbp)
   8d4d5:      	leaq	-0x288(%rbp), %r14
   8d4dc:      	movq	%r14, %rdi
   8d4df:      	movq	%r14, %rsi
   8d4e2:      	callq	0x9b2e0 <scoop_rt_enter_native_borrowed>
   8d4e7:      	movq	-0x48(%rbp), %rdi
   8d4eb:      	callq	0xb46d0 <m34_container_layout>
   8d4f0:      	movq	%r14, %rdi
   8d4f3:      	callq	0x9dc70 <scoop_rt_leave_native_borrowed>
   8d4f8:      	movq	-0x38(%rbp), %r14
   8d4fc:      	movq	-0x48(%rbp), %r15
   8d500:      	movq	%rbx, %rdi
   8d503:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d508:      	movq	%r14, -0x38(%rbp)
   8d50c:      	movq	%r15, -0x48(%rbp)
   8d510:      	movq	%r14, -0x30(%rbp)
   8d514:      	callq	0x9b0e0 <scoop_rt_gc_collect>
   8d519:      	movq	-0x30(%rbp), %rax
   8d51d:      	movq	%rax, -0x38(%rbp)
   8d521:      	xorl	%ebx, %ebx
   8d523:      	xorl	%r13d, %r13d
   8d526:      	nopw	%cs:(%rax,%rax)
   8d530:      	leaq	0x1d70d9(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   8d537:      	movq	(%rax), %rax
   8d53a:      	leaq	0x1d70d7(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   8d541:      	movl	(%rcx), %esi
   8d543:      	movq	-0x68(%rbp), %rcx
   8d547:      	movl	(%rcx), %edx
   8d549:      	movq	0x8(%rcx), %rcx
   8d54d:      	testl	%esi, %esi
   8d54f:      	jne	0x8d55b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x42b>
   8d551:      	cmpl	$0x1, %edx
   8d554:      	jne	0x8d55b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x42b>
   8d556:      	cmpq	%rax, %rcx
   8d559:      	je	0x8d570 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x440>
   8d55b:      	movq	-0x38(%rbp), %rax
   8d55f:      	movq	%rax, -0x30(%rbp)
   8d563:      	callq	0x9b0c0 <scoop_rt_safepoint>
   8d568:      	movq	-0x30(%rbp), %rax
   8d56c:      	movq	%rax, -0x38(%rbp)
   8d570:      	cmpl	$0x5, %r13d
   8d574:      	jge	0x8d6e7 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5b7>
   8d57a:      	leaq	-0x38(%rbp), %r12
   8d57e:      	movq	%r12, -0x90(%rbp)
   8d585:      	leaq	0x16dba4(%rip), %rax    # 0x1fb130 <scoop$1$bs$9ac53e80ac7e59a3c5319bf5bce7949e6233c62f549f27a1956533d23e19898b>
   8d58c:      	movq	%rax, -0x88(%rbp)
   8d593:      	xorps	%xmm0, %xmm0
   8d596:      	movups	%xmm0, -0x118(%rbp)
   8d59d:      	movq	$0x0, -0x108(%rbp)
   8d5a8:      	movl	$0x1, %edx
   8d5ad:      	leaq	-0x118(%rbp), %r15
   8d5b4:      	movq	%r15, %rdi
   8d5b7:      	leaq	-0x90(%rbp), %rsi
   8d5be:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d5c3:      	xorps	%xmm0, %xmm0
   8d5c6:      	movups	%xmm0, -0x208(%rbp)
   8d5cd:      	movups	%xmm0, -0x1f8(%rbp)
   8d5d4:      	movups	%xmm0, -0x1e8(%rbp)
   8d5db:      	movups	%xmm0, -0x1d8(%rbp)
   8d5e2:      	leaq	-0x208(%rbp), %r14
   8d5e9:      	movq	%r14, %rdi
   8d5ec:      	movq	%r14, %rsi
   8d5ef:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d5f4:      	callq	0xb4500 <m34_container_begin>
   8d5f9:      	movq	%r14, %rdi
   8d5fc:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d601:      	movq	-0x38(%rbp), %r14
   8d605:      	movq	%r15, %rdi
   8d608:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d60d:      	movq	%r14, -0x38(%rbp)
   8d611:      	movq	%r14, -0x30(%rbp)
   8d615:      	callq	0x9b0e0 <scoop_rt_gc_collect>
   8d61a:      	movq	-0x30(%rbp), %rax
   8d61e:      	movq	%rax, -0x38(%rbp)
   8d622:      	movq	%r12, -0xa0(%rbp)
   8d629:      	leaq	0x16db10(%rip), %rax    # 0x1fb140 <scoop$1$bs$eb451bdf32bc9cd18e03f449cf70d4cdd6eed366aebdd7c65a6ec43f7d8ffb1c>
   8d630:      	movq	%rax, -0x98(%rbp)
   8d637:      	xorps	%xmm0, %xmm0
   8d63a:      	movups	%xmm0, -0x130(%rbp)
   8d641:      	movq	$0x0, -0x120(%rbp)
   8d64c:      	movl	$0x1, %edx
   8d651:      	leaq	-0x130(%rbp), %r15
   8d658:      	movq	%r15, %rdi
   8d65b:      	leaq	-0xa0(%rbp), %rsi
   8d662:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d667:      	xorps	%xmm0, %xmm0
   8d66a:      	movups	%xmm0, -0x248(%rbp)
   8d671:      	movups	%xmm0, -0x238(%rbp)
   8d678:      	movups	%xmm0, -0x228(%rbp)
   8d67f:      	movups	%xmm0, -0x218(%rbp)
   8d686:      	leaq	-0x248(%rbp), %r14
   8d68d:      	movq	%r14, %rdi
   8d690:      	movq	%r14, %rsi
   8d693:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d698:      	movl	$0x4, %edi
   8d69d:      	callq	0xb45a0 <m34_container_end>
   8d6a2:      	movq	%r14, %rdi
   8d6a5:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d6aa:      	movq	-0x38(%rbp), %r14
   8d6ae:      	movq	%r15, %rdi
   8d6b1:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d6b6:      	movq	%r14, -0x38(%rbp)
   8d6ba:      	movq	%r14, -0x70(%rbp)
   8d6be:      	movq	%r14, -0x30(%rbp)
   8d6c2:      	movq	%r14, %rdi
   8d6c5:      	xorl	%esi, %esi
   8d6c7:      	callq	0x87500 <scoop$1$cb$024b41165bf0c15c7a2f298bb7e4010921d9073586171166bc7f632b3c7aa0b5>
   8d6cc:      	movq	-0x30(%rbp), %rcx
   8d6d0:      	movq	%rcx, -0x38(%rbp)
   8d6d4:      	movq	%rcx, -0x70(%rbp)
   8d6d8:      	movslq	0x10(%rax), %rax
   8d6dc:      	addq	%rax, %rbx
   8d6df:      	incl	%r13d
   8d6e2:      	jmp	0x8d530 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x400>
   8d6e7:      	xorl	%edi, %edi
   8d6e9:      	cmpq	$0x23, %rbx
   8d6ed:      	sete	%dil
   8d6f1:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8d6f6:      	movq	%rbx, %rax
   8d6f9:      	addq	$0x268, %rsp            # imm = 0x268
   8d700:      	popq	%rbx
   8d701:      	popq	%r12
   8d703:      	popq	%r13
   8d705:      	popq	%r14
   8d707:      	popq	%r15
   8d709:      	popq	%rbp
   8d70a:      	retq

; buildStrings, MIR fn2, LIR scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000007f1a0 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc>:
   7f1a0:      	pushq	%rbp
   7f1a1:      	movq	%rsp, %rbp
   7f1a4:      	pushq	%r15
   7f1a6:      	pushq	%r14
   7f1a8:      	pushq	%r13
   7f1aa:      	pushq	%r12
   7f1ac:      	pushq	%rbx
   7f1ad:      	subq	$0x1f8, %rsp            # imm = 0x1F8
   7f1b4:      	movq	%rdi, %rbx
   7f1b7:      	movq	%fs:0x0, %rax
   7f1c0:      	leaq	-0x10(%rax), %rax
   7f1c7:      	movq	(%rax), %r12
   7f1ca:      	leaq	0x1e543f(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   7f1d1:      	movq	(%rax), %rax
   7f1d4:      	leaq	0x1e543d(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   7f1db:      	movl	(%rcx), %esi
   7f1dd:      	movl	(%r12), %edx
   7f1e1:      	movq	0x8(%r12), %rcx
   7f1e6:      	testl	%esi, %esi
   7f1e8:      	jne	0x7f1f4 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x54>
   7f1ea:      	cmpl	$0x1, %edx
   7f1ed:      	jne	0x7f1f4 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x54>
   7f1ef:      	cmpq	%rax, %rcx
   7f1f2:      	je	0x7f1f9 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x59>
   7f1f4:      	callq	0x9b0c0 <scoop_rt_safepoint>
   7f1f9:      	movq	0xbd5a8(%rip), %r14     # 0x13c7a8 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192+0x18>
   7f200:      	movq	%r14, %r13
   7f203:      	negq	%r13
   7f206:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   7f210:      	addq	%r14, %rax
   7f213:      	cmpq	$-0x18, %rax
   7f217:      	setae	-0x39(%rbp)
   7f21b:      	leaq	0x17(%r14), %r15
   7f21f:      	andq	%r13, %r15
   7f222:      	movq	%fs:0x0, %rax
   7f22b:      	leaq	-0x8(%rax), %rax
   7f232:      	movq	(%rax), %rax
   7f235:      	movq	(%rax), %rcx
   7f238:      	addq	%rcx, %r14
   7f23b:      	decq	%r14
   7f23e:      	andq	%r13, %r14
   7f241:      	leaq	(%r14,%r15), %rcx
   7f245:      	testq	%r14, %r14
   7f248:      	sete	%dl
   7f24b:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   7f252:      	setae	%sil
   7f256:      	cmpq	0x8(%rax), %rcx
   7f25a:      	seta	%dil
   7f25e:      	cmpq	$0x0, 0xbd5ba(%rip)     # 0x13c820 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192+0x90>
   7f266:      	setne	%r8b
   7f26a:      	orb	%sil, %r8b
   7f26d:      	orb	%dl, %r8b
   7f270:      	orb	-0x39(%rbp), %r8b
   7f274:      	orb	%dil, %r8b
   7f277:      	jne	0x7f290 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0xf0>
   7f279:      	movq	%rcx, (%rax)
   7f27c:      	leaq	0xbd50d(%rip), %rsi     # 0x13c790 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192>
   7f283:      	movq	%r14, %rdi
   7f286:      	movq	%r15, %rdx
   7f289:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   7f28e:      	jmp	0x7f2a4 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
   7f290:      	leaq	0xbd4f9(%rip), %rdi     # 0x13c790 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192>
   7f297:      	movl	$0x18, %esi
   7f29c:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   7f2a1:      	movq	%rax, %r14
   7f2a4:      	movq	%r14, -0x30(%rbp)
   7f2a8:      	movq	%r14, %rdi
   7f2ab:      	callq	0x573a0 <scoop$1$cb$7fd0de94188adc1500f086a2b0e914cb1973f941c791324bcba453f6b7bbfd25>
   7f2b0:      	movq	-0x30(%rbp), %rax
   7f2b4:      	movq	%rax, -0xb8(%rbp)
   7f2bb:      	movq	%rax, -0x38(%rbp)
   7f2bf:      	leaq	-0x38(%rbp), %rax
   7f2c3:      	movq	%rax, -0x78(%rbp)
   7f2c7:      	leaq	0x1585e2(%rip), %rax    # 0x1d78b0 <scoop$1$bs$155609da96eba8ad9ba5a41acdf9f0ce21d17cff97c4b4bd58f560ff46fef257>
   7f2ce:      	movq	%rax, -0x70(%rbp)
   7f2d2:      	xorps	%xmm0, %xmm0
   7f2d5:      	movups	%xmm0, -0xd0(%rbp)
   7f2dc:      	movq	$0x0, -0xc0(%rbp)
   7f2e7:      	leaq	-0xd0(%rbp), %r14
   7f2ee:      	leaq	-0x78(%rbp), %rsi
   7f2f2:      	movl	$0x1, %edx
   7f2f7:      	movq	%r14, %rdi
   7f2fa:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   7f2ff:      	xorps	%xmm0, %xmm0
   7f302:      	movups	%xmm0, -0x158(%rbp)
   7f309:      	movups	%xmm0, -0x148(%rbp)
   7f310:      	movups	%xmm0, -0x138(%rbp)
   7f317:      	movups	%xmm0, -0x128(%rbp)
   7f31e:      	leaq	-0x158(%rbp), %r15
   7f325:      	movq	%r15, %rdi
   7f328:      	movq	%r15, %rsi
   7f32b:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   7f330:      	callq	0xb4500 <m34_container_begin>
   7f335:      	movq	%r15, %rdi
   7f338:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   7f33d:      	movq	-0x38(%rbp), %r15
   7f341:      	movq	%r14, %rdi
   7f344:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   7f349:      	movq	%r15, -0x38(%rbp)
   7f34d:      	xorl	%r14d, %r14d
   7f350:      	xorl	%r15d, %r15d
   7f353:      	leaq	0x1e52b6(%rip), %r13    # 0x264610 <scoop_thread_gc_epoch>
   7f35a:      	nopw	(%rax,%rax)
   7f360:      	movq	(%r13), %rax
   7f364:      	leaq	0x1e52ad(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   7f36b:      	movl	(%rcx), %esi
   7f36d:      	movl	(%r12), %edx
   7f371:      	movq	0x8(%r12), %rcx
   7f376:      	testl	%esi, %esi
   7f378:      	jne	0x7f384 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1e4>
   7f37a:      	cmpl	$0x1, %edx
   7f37d:      	jne	0x7f384 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1e4>
   7f37f:      	cmpq	%rax, %rcx
   7f382:      	je	0x7f399 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1f9>
   7f384:      	movq	-0x38(%rbp), %rax
   7f388:      	movq	%rax, -0x30(%rbp)
   7f38c:      	callq	0x9b0c0 <scoop_rt_safepoint>
   7f391:      	movq	-0x30(%rbp), %rax
   7f395:      	movq	%rax, -0x38(%rbp)
   7f399:      	cmpq	%rbx, %r14
   7f39c:      	jge	0x7f40e <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x26e>
   7f39e:      	movq	-0x38(%rbp), %rax
   7f3a2:      	movq	%rax, -0x30(%rbp)
   7f3a6:      	movq	%r14, %rdi
   7f3a9:      	callq	0x490f0 <scoop$1$cb$ef730a437b587319fa729136c1c496c97d793c073731fe4cbfd67870ec8e2cc0>
   7f3ae:      	movq	-0x30(%rbp), %rdi
   7f3b2:      	movq	%rdi, -0x38(%rbp)
   7f3b6:      	movq	%rax, -0x58(%rbp)
   7f3ba:      	movq	%rdi, -0x68(%rbp)
   7f3be:      	movq	%rax, -0x60(%rbp)
   7f3c2:      	movq	%rax, -0x50(%rbp)
   7f3c6:      	movq	%rax, %rsi
   7f3c9:      	callq	0x47c80 <scoop$1$cb$bf2a768a0cb1375f89a63ed498891d7c57dab520218a10fddd14a0197c926150>
   7f3ce:      	movq	-0x50(%rbp), %rdi
   7f3d2:      	movq	-0x30(%rbp), %rax
   7f3d6:      	movq	%rax, -0x38(%rbp)
   7f3da:      	movq	%rdi, -0x58(%rbp)
   7f3de:      	movq	%rax, -0x68(%rbp)
   7f3e2:      	movq	%rdi, -0x60(%rbp)
   7f3e6:      	movq	-0x38(%rbp), %rax
   7f3ea:      	movq	%rax, -0x30(%rbp)
   7f3ee:      	callq	0x57db0 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7f3f3:      	movq	-0x50(%rbp), %rcx
   7f3f7:      	movq	-0x30(%rbp), %rdx
   7f3fb:      	movq	%rdx, -0x38(%rbp)
   7f3ff:      	movq	%rcx, -0x58(%rbp)
   7f403:      	addq	%rax, %r15
   7f406:      	incq	%r14
   7f409:      	jmp	0x7f360 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1c0>
   7f40e:      	leaq	-0x38(%rbp), %r12
   7f412:      	movq	%r12, -0x88(%rbp)
   7f419:      	leaq	0x1584a0(%rip), %rax    # 0x1d78c0 <scoop$1$bs$3339f6d95e6e9e5deab0576cb1ad89976cd8650888695dcb4a34160b58f19e84>
   7f420:      	movq	%rax, -0x80(%rbp)
   7f424:      	xorps	%xmm0, %xmm0
   7f427:      	movups	%xmm0, -0xe8(%rbp)
   7f42e:      	movq	$0x0, -0xd8(%rbp)
   7f439:      	leaq	-0xe8(%rbp), %rbx
   7f440:      	leaq	-0x88(%rbp), %rsi
   7f447:      	movl	$0x1, %edx
   7f44c:      	movq	%rbx, %rdi
   7f44f:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   7f454:      	xorps	%xmm0, %xmm0
   7f457:      	movups	%xmm0, -0x198(%rbp)
   7f45e:      	movups	%xmm0, -0x188(%rbp)
   7f465:      	movups	%xmm0, -0x178(%rbp)
   7f46c:      	movups	%xmm0, -0x168(%rbp)
   7f473:      	leaq	-0x198(%rbp), %r14
   7f47a:      	movq	%r14, %rdi
   7f47d:      	movq	%r14, %rsi
   7f480:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   7f485:      	movl	$0x1, %edi
   7f48a:      	callq	0xb45a0 <m34_container_end>
   7f48f:      	movq	%r14, %rdi
   7f492:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   7f497:      	movq	-0x38(%rbp), %r14
   7f49b:      	movq	%rbx, %rdi
   7f49e:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   7f4a3:      	movq	%r14, -0x38(%rbp)
   7f4a7:      	movq	%r12, -0x98(%rbp)
   7f4ae:      	leaq	0x15841b(%rip), %rax    # 0x1d78d0 <scoop$1$bs$6d1b78b7f57c69160f9b8c16c3b0df8401dc0c69fb204db37f962d1d7a3491e5>
   7f4b5:      	movq	%rax, -0x90(%rbp)
   7f4bc:      	xorps	%xmm0, %xmm0
   7f4bf:      	movups	%xmm0, -0x100(%rbp)
   7f4c6:      	movq	$0x0, -0xf0(%rbp)
   7f4d1:      	leaq	-0x100(%rbp), %rbx
   7f4d8:      	leaq	-0x98(%rbp), %rsi
   7f4df:      	movl	$0x1, %edx
   7f4e4:      	movq	%rbx, %rdi
   7f4e7:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   7f4ec:      	xorps	%xmm0, %xmm0
   7f4ef:      	movups	%xmm0, -0x1d8(%rbp)
   7f4f6:      	movups	%xmm0, -0x1c8(%rbp)
   7f4fd:      	movups	%xmm0, -0x1b8(%rbp)
   7f504:      	movups	%xmm0, -0x1a8(%rbp)
   7f50b:      	leaq	-0x1d8(%rbp), %r14
   7f512:      	movq	%r14, %rdi
   7f515:      	movq	%r14, %rsi
   7f518:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   7f51d:      	callq	0xb4500 <m34_container_begin>
   7f522:      	movq	%r14, %rdi
   7f525:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   7f52a:      	movq	-0x38(%rbp), %r14
   7f52e:      	movq	%rbx, %rdi
   7f531:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   7f536:      	movq	%r14, -0x38(%rbp)
   7f53a:      	movq	%r14, -0x30(%rbp)
   7f53e:      	movq	%r14, %rdi
   7f541:      	callq	0x41cf0 <scoop$1$cb$22ffc90119bb7e713d02ab7beca1a040fd100be545734e1a344db51e6165d4ce>
   7f546:      	movq	-0x30(%rbp), %rcx
   7f54a:      	movq	%rcx, -0xb0(%rbp)
   7f551:      	movq	%rax, -0x48(%rbp)
   7f555:      	leaq	-0x48(%rbp), %rax
   7f559:      	movq	%rax, -0xa8(%rbp)
   7f560:      	leaq	0x158379(%rip), %rax    # 0x1d78e0 <scoop$1$bs$9e649712ef74d131942a46acef503ed4d7c9d93cdd1e31bb977b6bbf6739b592>
   7f567:      	movq	%rax, -0xa0(%rbp)
   7f56e:      	xorps	%xmm0, %xmm0
   7f571:      	movups	%xmm0, -0x118(%rbp)
   7f578:      	movq	$0x0, -0x108(%rbp)
   7f583:      	leaq	-0x118(%rbp), %rbx
   7f58a:      	leaq	-0xa8(%rbp), %rsi
   7f591:      	movl	$0x1, %edx
   7f596:      	movq	%rbx, %rdi
   7f599:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   7f59e:      	xorps	%xmm0, %xmm0
   7f5a1:      	movups	%xmm0, -0x218(%rbp)
   7f5a8:      	movups	%xmm0, -0x208(%rbp)
   7f5af:      	movups	%xmm0, -0x1f8(%rbp)
   7f5b6:      	movups	%xmm0, -0x1e8(%rbp)
   7f5bd:      	leaq	-0x218(%rbp), %r14
   7f5c4:      	movq	%r14, %rdi
   7f5c7:      	movq	%r14, %rsi
   7f5ca:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   7f5cf:      	movl	$0x2, %edi
   7f5d4:      	callq	0xb45a0 <m34_container_end>
   7f5d9:      	movq	%r14, %rdi
   7f5dc:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   7f5e1:      	movq	-0x48(%rbp), %r14
   7f5e5:      	movq	%rbx, %rdi
   7f5e8:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   7f5ed:      	movq	%r14, -0x48(%rbp)
   7f5f1:      	movq	%r14, -0x30(%rbp)
   7f5f5:      	movq	%r14, %rdi
   7f5f8:      	callq	0x57db0 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7f5fd:      	movq	-0x30(%rbp), %rcx
   7f601:      	movq	%rcx, -0x48(%rbp)
   7f605:      	xorl	%edi, %edi
   7f607:      	cmpq	%r15, %rax
   7f60a:      	sete	%dil
   7f60e:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   7f613:      	movq	-0x30(%rbp), %rdi
   7f617:      	movq	%rdi, -0x48(%rbp)
   7f61b:      	callq	0x57db0 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7f620:      	movq	-0x30(%rbp), %rcx
   7f624:      	movq	%rcx, -0x48(%rbp)
   7f628:      	addq	$0x1f8, %rsp            # imm = 0x1F8
   7f62f:      	popq	%rbx
   7f630:      	popq	%r12
   7f632:      	popq	%r13
   7f634:      	popq	%r14
   7f636:      	popq	%r15
   7f638:      	popq	%rbp
   7f639:      	retq

; jsonRoundTrip, MIR fn3, LIR scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

0000000000087bb0 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364>:
   87bb0:      	pushq	%rbp
   87bb1:      	movq	%rsp, %rbp
   87bb4:      	pushq	%r15
   87bb6:      	pushq	%r14
   87bb8:      	pushq	%r13
   87bba:      	pushq	%r12
   87bbc:      	pushq	%rbx
   87bbd:      	subq	$0x3e8, %rsp            # imm = 0x3E8
   87bc4:      	movq	%rdi, -0x90(%rbp)
   87bcb:      	movq	%fs:0x0, %rax
   87bd4:      	leaq	-0x10(%rax), %rax
   87bdb:      	movq	(%rax), %r12
   87bde:      	leaq	0x1dca2b(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   87be5:      	movq	(%rax), %rax
   87be8:      	leaq	0x1dca29(%rip), %rbx    # 0x264618 <scoop_thread_world_phase>
   87bef:      	movl	(%rbx), %esi
   87bf1:      	movl	(%r12), %edx
   87bf5:      	movq	0x8(%r12), %rcx
   87bfa:      	testl	%esi, %esi
   87bfc:      	jne	0x87c08 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x58>
   87bfe:      	cmpl	$0x1, %edx
   87c01:      	jne	0x87c08 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x58>
   87c03:      	cmpq	%rax, %rcx
   87c06:      	je	0x87c0d <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x5d>
   87c08:      	callq	0x9b0c0 <scoop_rt_safepoint>
   87c0d:      	movq	0x1836e4(%rip), %r14    # 0x20b2f8 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x18>
   87c14:      	movq	%r14, %r13
   87c17:      	negq	%r13
   87c1a:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   87c24:      	addq	%r14, %rax
   87c27:      	cmpq	$-0x20, %rax
   87c2b:      	setae	-0x49(%rbp)
   87c2f:      	leaq	0x1f(%r14), %r15
   87c33:      	andq	%r13, %r15
   87c36:      	movq	%fs:0x0, %rax
   87c3f:      	leaq	-0x8(%rax), %rax
   87c46:      	movq	(%rax), %rax
   87c49:      	movq	(%rax), %rcx
   87c4c:      	addq	%rcx, %r14
   87c4f:      	decq	%r14
   87c52:      	andq	%r13, %r14
   87c55:      	leaq	(%r14,%r15), %rcx
   87c59:      	testq	%r14, %r14
   87c5c:      	sete	%dl
   87c5f:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   87c66:      	setae	%sil
   87c6a:      	cmpq	0x8(%rax), %rcx
   87c6e:      	seta	%dil
   87c72:      	cmpq	$0x0, 0x1836f6(%rip)    # 0x20b370 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x90>
   87c7a:      	setne	%r8b
   87c7e:      	orb	%sil, %r8b
   87c81:      	orb	%dl, %r8b
   87c84:      	orb	-0x49(%rbp), %r8b
   87c88:      	orb	%dil, %r8b
   87c8b:      	jne	0x87ca4 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0xf4>
   87c8d:      	movq	%rcx, (%rax)
   87c90:      	leaq	0x183649(%rip), %rsi    # 0x20b2e0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   87c97:      	movq	%r14, %rdi
   87c9a:      	movq	%r15, %rdx
   87c9d:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   87ca2:      	jmp	0x87cb8 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
   87ca4:      	leaq	0x183635(%rip), %rdi    # 0x20b2e0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   87cab:      	movl	$0x20, %esi
   87cb0:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   87cb5:      	movq	%rax, %r14
   87cb8:      	movq	-0x90(%rbp), %rsi
   87cbf:      	movq	%r14, -0x30(%rbp)
   87cc3:      	movq	%r14, %rdi
   87cc6:      	callq	0x884c0 <scoop$1$cb$fee77490ca77ad5e976dbdde0ee39371498372f80a070cf4802c2070dd379995>
   87ccb:      	movq	-0x30(%rbp), %rax
   87ccf:      	movq	%rax, -0x1c0(%rbp)
   87cd6:      	movq	%rax, -0x48(%rbp)
   87cda:      	movq	%rax, -0x88(%rbp)
   87ce1:      	leaq	-0x48(%rbp), %r14
   87ce5:      	movq	%r14, -0x160(%rbp)
   87cec:      	leaq	0x1653dd(%rip), %rax    # 0x1ed0d0 <scoop$1$bs$49c66c70f85e0f0f1ff93ca5b71d3c1326a359c41b10c8f369725fc037bf4ca1>
   87cf3:      	movq	%rax, -0x158(%rbp)
   87cfa:      	leaq	-0x88(%rbp), %rax
   87d01:      	movq	%rax, -0x150(%rbp)
   87d08:      	leaq	0x1653d1(%rip), %rax    # 0x1ed0e0 <scoop$1$bs$b2c272680e1595ce40df5d814864efc59018da7269d6283634e06c08d420667f>
   87d0f:      	movq	%rax, -0x148(%rbp)
   87d16:      	xorps	%xmm0, %xmm0
   87d19:      	movups	%xmm0, -0x140(%rbp)
   87d20:      	movq	$0x0, -0x130(%rbp)
   87d2b:      	leaq	-0x140(%rbp), %rdi
   87d32:      	leaq	-0x160(%rbp), %rsi
   87d39:      	movl	$0x2, %edx
   87d3e:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   87d43:      	xorps	%xmm0, %xmm0
   87d46:      	movups	%xmm0, -0x290(%rbp)
   87d4d:      	movups	%xmm0, -0x280(%rbp)
   87d54:      	movups	%xmm0, -0x270(%rbp)
   87d5b:      	movups	%xmm0, -0x260(%rbp)
   87d62:      	leaq	-0x290(%rbp), %r15
   87d69:      	movq	%r15, %rdi
   87d6c:      	movq	%r15, %rsi
   87d6f:      	callq	0x9b2e0 <scoop_rt_enter_native_borrowed>
   87d74:      	movq	-0x88(%rbp), %rdi
   87d7b:      	callq	0xb46d0 <m34_container_layout>
   87d80:      	movq	%r15, %rdi
   87d83:      	callq	0x9dc70 <scoop_rt_leave_native_borrowed>
   87d88:      	movq	-0x48(%rbp), %r15
   87d8c:      	movq	-0x88(%rbp), %r13
   87d93:      	leaq	-0x140(%rbp), %rdi
   87d9a:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   87d9f:      	movq	%r15, -0x48(%rbp)
   87da3:      	movq	%r13, -0x88(%rbp)
   87daa:      	movq	-0x90(%rbp), %r13
   87db1:      	movq	%r14, -0xe8(%rbp)
   87db8:      	leaq	0x165331(%rip), %rax    # 0x1ed0f0 <scoop$1$bs$9483cebc8b7b69e7dcb8c58ffe984236fc15c13b286041d8339c882587a9912a>
   87dbf:      	movq	%rax, -0xe0(%rbp)
   87dc6:      	xorps	%xmm0, %xmm0
   87dc9:      	movups	%xmm0, -0x1d8(%rbp)
   87dd0:      	movq	$0x0, -0x1c8(%rbp)
   87ddb:      	leaq	-0x1d8(%rbp), %r14
   87de2:      	leaq	-0xe8(%rbp), %rsi
   87de9:      	movl	$0x1, %edx
   87dee:      	movq	%r14, %rdi
   87df1:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   87df6:      	xorps	%xmm0, %xmm0
   87df9:      	movups	%xmm0, -0x2d0(%rbp)
   87e00:      	movups	%xmm0, -0x2c0(%rbp)
   87e07:      	movups	%xmm0, -0x2b0(%rbp)
   87e0e:      	movups	%xmm0, -0x2a0(%rbp)
   87e15:      	leaq	-0x2d0(%rbp), %r15
   87e1c:      	movq	%r15, %rdi
   87e1f:      	movq	%r15, %rsi
   87e22:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   87e27:      	callq	0xb4500 <m34_container_begin>
   87e2c:      	movq	%r15, %rdi
   87e2f:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   87e34:      	movq	-0x48(%rbp), %r15
   87e38:      	movq	%r14, %rdi
   87e3b:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   87e40:      	movq	%r15, -0x48(%rbp)
   87e44:      	xorl	%r14d, %r14d
   87e47:      	leaq	0x1dc7c2(%rip), %r15    # 0x264610 <scoop_thread_gc_epoch>
   87e4e:      	nop
   87e50:      	movq	(%r15), %rax
   87e53:      	movl	(%rbx), %esi
   87e55:      	movl	(%r12), %edx
   87e59:      	movq	0x8(%r12), %rcx
   87e5e:      	testl	%esi, %esi
   87e60:      	jne	0x87e6c <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2bc>
   87e62:      	cmpl	$0x1, %edx
   87e65:      	jne	0x87e6c <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2bc>
   87e67:      	cmpq	%rax, %rcx
   87e6a:      	je	0x87e81 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2d1>
   87e6c:      	movq	-0x48(%rbp), %rax
   87e70:      	movq	%rax, -0x30(%rbp)
   87e74:      	callq	0x9b0c0 <scoop_rt_safepoint>
   87e79:      	movq	-0x30(%rbp), %rax
   87e7d:      	movq	%rax, -0x48(%rbp)
   87e81:      	cmpq	%r13, %r14
   87e84:      	jge	0x87eb1 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x301>
   87e86:      	movq	-0x48(%rbp), %rdi
   87e8a:      	movq	%rdi, -0xb8(%rbp)
   87e91:      	movq	%rdi, -0x30(%rbp)
   87e95:      	movl	%r14d, %esi
   87e98:      	callq	0x95000 <scoop$1$cb$6571e66f09cbc869977ca80836b265c26b4c58d100cec7b4dc74f71fda171ea3>
   87e9d:      	movq	-0x30(%rbp), %rax
   87ea1:      	movq	%rax, -0x48(%rbp)
   87ea5:      	movq	%rax, -0xb8(%rbp)
   87eac:      	incq	%r14
   87eaf:      	jmp	0x87e50 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2a0>
   87eb1:      	leaq	-0x48(%rbp), %r12
   87eb5:      	movq	%r12, -0xf8(%rbp)
   87ebc:      	leaq	0x16523d(%rip), %rax    # 0x1ed100 <scoop$1$bs$d1068a954b40d440779cb882b70d44c0918c4a0476fa778625b9024fdb53e5be>
   87ec3:      	movq	%rax, -0xf0(%rbp)
   87eca:      	xorps	%xmm0, %xmm0
   87ecd:      	movups	%xmm0, -0x1f0(%rbp)
   87ed4:      	movq	$0x0, -0x1e0(%rbp)
   87edf:      	leaq	-0x1f0(%rbp), %r14
   87ee6:      	leaq	-0xf8(%rbp), %rsi
   87eed:      	movl	$0x1, %edx
   87ef2:      	movq	%r14, %rdi
   87ef5:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   87efa:      	xorps	%xmm0, %xmm0
   87efd:      	movups	%xmm0, -0x310(%rbp)
   87f04:      	movups	%xmm0, -0x300(%rbp)
   87f0b:      	movups	%xmm0, -0x2f0(%rbp)
   87f12:      	movups	%xmm0, -0x2e0(%rbp)
   87f19:      	leaq	-0x310(%rbp), %r15
   87f20:      	movq	%r15, %rdi
   87f23:      	movq	%r15, %rsi
   87f26:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   87f2b:      	movl	$0x1, %edi
   87f30:      	callq	0xb45a0 <m34_container_end>
   87f35:      	movq	%r15, %rdi
   87f38:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   87f3d:      	movq	-0x48(%rbp), %rbx
   87f41:      	movq	%r14, %rdi
   87f44:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   87f49:      	movq	%rbx, -0x48(%rbp)
   87f4d:      	movq	%r12, -0x108(%rbp)
   87f54:      	leaq	0x1651b5(%rip), %rax    # 0x1ed110 <scoop$1$bs$42931e5c32a0faa35dc1de349255af6bcd5cd0f7bf3b4f8212609241135eb98e>
   87f5b:      	movq	%rax, -0x100(%rbp)
   87f62:      	xorps	%xmm0, %xmm0
   87f65:      	movups	%xmm0, -0x208(%rbp)
   87f6c:      	movq	$0x0, -0x1f8(%rbp)
   87f77:      	leaq	-0x208(%rbp), %r14
   87f7e:      	leaq	-0x108(%rbp), %rsi
   87f85:      	movl	$0x1, %edx
   87f8a:      	movq	%r14, %rdi
   87f8d:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   87f92:      	xorps	%xmm0, %xmm0
   87f95:      	movups	%xmm0, -0x350(%rbp)
   87f9c:      	movups	%xmm0, -0x340(%rbp)
   87fa3:      	movups	%xmm0, -0x330(%rbp)
   87faa:      	movups	%xmm0, -0x320(%rbp)
   87fb1:      	leaq	-0x350(%rbp), %r15
   87fb8:      	movq	%r15, %rdi
   87fbb:      	movq	%r15, %rsi
   87fbe:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   87fc3:      	callq	0xb4500 <m34_container_begin>
   87fc8:      	movq	%r15, %rdi
   87fcb:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   87fd0:      	movq	-0x48(%rbp), %rbx
   87fd4:      	movq	%r14, %rdi
   87fd7:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   87fdc:      	movq	%rbx, -0x48(%rbp)
   87fe0:      	movq	%rbx, -0x30(%rbp)
   87fe4:      	callq	0x7c8a0 <scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
   87fe9:      	movq	-0x30(%rbp), %rax
   87fed:      	movq	%rax, -0x48(%rbp)
   87ff1:      	movq	0x1dc588(%rip), %rax    # 0x264580 <scoop$1$ss$026945f606bf54daf4171a1ecb8b4384e089880c55f3eb80fb1b8fa8a5fad2ad>
   87ff8:      	movq	%rax, -0x38(%rbp)
   87ffc:      	callq	0x8aca0 <scoop$1$cb$1d386762e04a0637ccab94d264901e5abb5d29e334df1df131050f782310aa58>
   88001:      	movq	-0x30(%rbp), %rax
   88005:      	movq	-0x38(%rbp), %rcx
   88009:      	movq	%rcx, -0x78(%rbp)
   8800d:      	movq	%rax, -0xa0(%rbp)
   88014:      	movq	0x1dc575(%rip), %rax    # 0x264590 <scoop$1$ss$175d083d63b380e686d8a1d190f67523f972e8cb41414cbc333e1e80b4d54d75>
   8801b:      	movq	%rax, -0x58(%rbp)
   8801f:      	callq	0x4d310 <scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
   88024:      	movq	-0x30(%rbp), %rax
   88028:      	movq	-0x38(%rbp), %rcx
   8802c:      	movq	-0x58(%rbp), %rdx
   88030:      	movq	%rdx, -0xb0(%rbp)
   88037:      	movq	%rcx, -0x78(%rbp)
   8803b:      	movq	%rax, -0xa0(%rbp)
   88042:      	movq	0x1dc46f(%rip), %rax    # 0x2644b8 <scoop$1$ss$d3fe97594dacdfc6152caa8910564318ae43b1abfd37e48f1ba87d4a4aed7e7b>
   88049:      	movq	0xb4ac0(%rip), %rcx     # 0x13cb10 <scoop$1$td$125eccdd45921ed82fc373b6db8bbebfe3f4a5fee3aaca18197e2202ff70aa67+0x60>
   88050:      	movq	0x18(%rcx), %rdx
   88054:      	movq	%rax, -0xd8(%rbp)
   8805b:      	movq	-0xd8(%rbp), %rsi
   88062:      	movq	-0xb0(%rbp), %rdi
   88069:      	movq	-0x78(%rbp), %rax
   8806d:      	movq	-0xa0(%rbp), %rcx
   88074:      	movq	%rdi, -0x30(%rbp)
   88078:      	movq	%rax, -0x58(%rbp)
   8807c:      	movq	%rcx, -0x60(%rbp)
   88080:      	movq	%rsi, -0x38(%rbp)
   88084:      	callq	0x84fd0 <scoop$1$cb$c6fdbb04db5cf532f4362ad4e151e4a1eabd8894c38f88c356bfab2246151c9f>
   88089:      	movq	%rdx, %rcx
   8808c:      	movq	-0x60(%rbp), %rsi
   88090:      	movq	-0x58(%rbp), %rdx
   88094:      	movq	-0x38(%rbp), %rdi
   88098:      	movq	-0x30(%rbp), %r8
   8809c:      	movq	%r8, -0xb0(%rbp)
   880a3:      	movq	%rdi, -0x1b8(%rbp)
   880aa:      	movq	%rdx, -0x78(%rbp)
   880ae:      	movq	%rsi, -0xa0(%rbp)
   880b5:      	movq	%rax, -0xd0(%rbp)
   880bc:      	movq	-0xd0(%rbp), %rdx
   880c3:      	movq	-0x78(%rbp), %rdi
   880c7:      	movq	%rdi, -0x30(%rbp)
   880cb:      	movq	%rdx, -0x38(%rbp)
   880cf:      	callq	0x84a30 <scoop$1$cb$d9dff153dcecd73d8284f8a22832dcee9c1843022762eaa02a94fb367c53d959>
   880d4:      	movq	-0x38(%rbp), %rcx
   880d8:      	movq	-0x60(%rbp), %rdx
   880dc:      	movq	-0x30(%rbp), %rsi
   880e0:      	movq	%rsi, -0x78(%rbp)
   880e4:      	movq	%rdx, -0x1b0(%rbp)
   880eb:      	movq	%rcx, -0x1a8(%rbp)
   880f2:      	movq	%rax, -0x40(%rbp)
   880f6:      	leaq	-0x40(%rbp), %rbx
   880fa:      	movq	%rbx, -0x118(%rbp)
   88101:      	leaq	0x165018(%rip), %rax    # 0x1ed120 <scoop$1$bs$ecc44cb58159f6c0bab76c83012c8adf52edcf172cdf11769cd013d73fc9af25>
   88108:      	movq	%rax, -0x110(%rbp)
   8810f:      	xorps	%xmm0, %xmm0
   88112:      	movups	%xmm0, -0x220(%rbp)
   88119:      	movq	$0x0, -0x210(%rbp)
   88124:      	leaq	-0x220(%rbp), %r14
   8812b:      	leaq	-0x118(%rbp), %rsi
   88132:      	movl	$0x1, %edx
   88137:      	movq	%r14, %rdi
   8813a:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8813f:      	xorps	%xmm0, %xmm0
   88142:      	movups	%xmm0, -0x390(%rbp)
   88149:      	movups	%xmm0, -0x380(%rbp)
   88150:      	movups	%xmm0, -0x370(%rbp)
   88157:      	movups	%xmm0, -0x360(%rbp)
   8815e:      	leaq	-0x390(%rbp), %r15
   88165:      	movq	%r15, %rdi
   88168:      	movq	%r15, %rsi
   8816b:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   88170:      	movl	$0x2, %edi
   88175:      	callq	0xb45a0 <m34_container_end>
   8817a:      	movq	%r15, %rdi
   8817d:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   88182:      	movq	-0x40(%rbp), %r15
   88186:      	movq	%r14, %rdi
   88189:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8818e:      	movq	%r15, -0x40(%rbp)
   88192:      	movq	%rbx, -0x128(%rbp)
   88199:      	leaq	0x164f90(%rip), %rax    # 0x1ed130 <scoop$1$bs$088eba55011e01cfa69e06024d2e450a87d700547df2cb55d5bac1f31d62eee0>
   881a0:      	movq	%rax, -0x120(%rbp)
   881a7:      	xorps	%xmm0, %xmm0
   881aa:      	movups	%xmm0, -0x238(%rbp)
   881b1:      	movq	$0x0, -0x228(%rbp)
   881bc:      	leaq	-0x238(%rbp), %r14
   881c3:      	leaq	-0x128(%rbp), %rsi
   881ca:      	movl	$0x1, %edx
   881cf:      	movq	%r14, %rdi
   881d2:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   881d7:      	xorps	%xmm0, %xmm0
   881da:      	movups	%xmm0, -0x3d0(%rbp)
   881e1:      	movups	%xmm0, -0x3c0(%rbp)
   881e8:      	movups	%xmm0, -0x3b0(%rbp)
   881ef:      	movups	%xmm0, -0x3a0(%rbp)
   881f6:      	leaq	-0x3d0(%rbp), %r15
   881fd:      	movq	%r15, %rdi
   88200:      	movq	%r15, %rsi
   88203:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   88208:      	callq	0xb4500 <m34_container_begin>
   8820d:      	movq	%r15, %rdi
   88210:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   88215:      	movq	-0x40(%rbp), %r15
   88219:      	movq	%r14, %rdi
   8821c:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   88221:      	movq	%r15, -0x40(%rbp)
   88225:      	movq	%r15, -0x30(%rbp)
   88229:      	callq	0x7c8a0 <scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
   8822e:      	movq	-0x30(%rbp), %rax
   88232:      	movq	%rax, -0x40(%rbp)
   88236:      	movq	0x1dc343(%rip), %rax    # 0x264580 <scoop$1$ss$026945f606bf54daf4171a1ecb8b4384e089880c55f3eb80fb1b8fa8a5fad2ad>
   8823d:      	movq	%rax, -0x38(%rbp)
   88241:      	callq	0x8aca0 <scoop$1$cb$1d386762e04a0637ccab94d264901e5abb5d29e334df1df131050f782310aa58>
   88246:      	movq	-0x30(%rbp), %rax
   8824a:      	movq	-0x38(%rbp), %rcx
   8824e:      	movq	%rax, -0x40(%rbp)
   88252:      	movq	%rcx, -0x70(%rbp)
   88256:      	movq	%rax, -0x98(%rbp)
   8825d:      	movq	0x1dc32c(%rip), %rax    # 0x264590 <scoop$1$ss$175d083d63b380e686d8a1d190f67523f972e8cb41414cbc333e1e80b4d54d75>
   88264:      	movq	-0x40(%rbp), %rcx
   88268:      	movq	%rcx, -0x58(%rbp)
   8826c:      	movq	%rax, -0x60(%rbp)
   88270:      	callq	0x4d310 <scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
   88275:      	movq	-0x30(%rbp), %rax
   88279:      	movq	-0x38(%rbp), %rcx
   8827d:      	movq	-0x60(%rbp), %rdx
   88281:      	movq	-0x58(%rbp), %rsi
   88285:      	movq	%rsi, -0x40(%rbp)
   88289:      	movq	%rdx, -0xa8(%rbp)
   88290:      	movq	%rcx, -0x70(%rbp)
   88294:      	movq	%rax, -0x98(%rbp)
   8829b:      	movq	0x1dc216(%rip), %rax    # 0x2644b8 <scoop$1$ss$d3fe97594dacdfc6152caa8910564318ae43b1abfd37e48f1ba87d4a4aed7e7b>
   882a2:      	movq	0xb4867(%rip), %rcx     # 0x13cb10 <scoop$1$td$125eccdd45921ed82fc373b6db8bbebfe3f4a5fee3aaca18197e2202ff70aa67+0x60>
   882a9:      	movq	0x8(%rcx), %rdx
   882ad:      	movq	%rax, -0xc8(%rbp)
   882b4:      	movq	-0xc8(%rbp), %rsi
   882bb:      	movq	-0x40(%rbp), %rax
   882bf:      	movq	-0xa8(%rbp), %rdi
   882c6:      	movq	-0x70(%rbp), %rcx
   882ca:      	movq	-0x98(%rbp), %r8
   882d1:      	movq	%rax, -0x30(%rbp)
   882d5:      	movq	%rdi, -0x38(%rbp)
   882d9:      	movq	%rcx, -0x60(%rbp)
   882dd:      	movq	%r8, -0x80(%rbp)
   882e1:      	movq	%rsi, -0x58(%rbp)
   882e5:      	callq	0x7e180 <scoop$1$cb$c61dc38f54807de9a33aea25bbdb9b2f907d97e85c3bc4af08d02b91907fd188>
   882ea:      	movq	%rdx, %rcx
   882ed:      	movq	-0x80(%rbp), %rsi
   882f1:      	movq	-0x60(%rbp), %rdx
   882f5:      	movq	-0x58(%rbp), %rdi
   882f9:      	movq	-0x38(%rbp), %r8
   882fd:      	movq	-0x30(%rbp), %r9
   88301:      	movq	%r9, -0x40(%rbp)
   88305:      	movq	%r8, -0xa8(%rbp)
   8830c:      	movq	%rdi, -0x1a0(%rbp)
   88313:      	movq	%rdx, -0x70(%rbp)
   88317:      	movq	%rsi, -0x98(%rbp)
   8831e:      	movq	%rax, -0xc0(%rbp)
   88325:      	movq	-0xc0(%rbp), %rdx
   8832c:      	movq	-0x40(%rbp), %rax
   88330:      	movq	-0x70(%rbp), %rdi
   88334:      	movq	%rax, -0x30(%rbp)
   88338:      	movq	%rdi, -0x38(%rbp)
   8833c:      	movq	%rdx, -0x58(%rbp)
   88340:      	callq	0x93270 <scoop$1$cb$0ba70f6c06fb5b9f1dc70e010618cf6127649aaa072c115f6e2153721bacf5bb>
   88345:      	movq	-0x58(%rbp), %rcx
   88349:      	movq	-0x80(%rbp), %rdx
   8834d:      	movq	-0x38(%rbp), %rsi
   88351:      	movq	-0x30(%rbp), %rdi
   88355:      	movq	%rdi, -0x40(%rbp)
   88359:      	movq	%rsi, -0x70(%rbp)
   8835d:      	movq	%rdx, -0x198(%rbp)
   88364:      	movq	%rcx, -0x190(%rbp)
   8836b:      	movq	%rax, -0x68(%rbp)
   8836f:      	movq	%rbx, -0x180(%rbp)
   88376:      	leaq	0x164dc3(%rip), %rax    # 0x1ed140 <scoop$1$bs$9fa9f9beae1fedf905007eae116ed2997d3bcbf5d968e2695d9d878dabd7c333>
   8837d:      	movq	%rax, -0x178(%rbp)
   88384:      	leaq	-0x68(%rbp), %rax
   88388:      	movq	%rax, -0x170(%rbp)
   8838f:      	leaq	0x164dba(%rip), %rax    # 0x1ed150 <scoop$1$bs$823eda99e6d3c5523bf2e8962e0c1167209419cf90614881a9da40f9090ce4a0>
   88396:      	movq	%rax, -0x168(%rbp)
   8839d:      	xorps	%xmm0, %xmm0
   883a0:      	movups	%xmm0, -0x250(%rbp)
   883a7:      	movq	$0x0, -0x240(%rbp)
   883b2:      	leaq	-0x250(%rbp), %r14
   883b9:      	leaq	-0x180(%rbp), %rsi
   883c0:      	movl	$0x2, %edx
   883c5:      	movq	%r14, %rdi
   883c8:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   883cd:      	xorps	%xmm0, %xmm0
   883d0:      	movups	%xmm0, -0x410(%rbp)
   883d7:      	movups	%xmm0, -0x400(%rbp)
   883de:      	movups	%xmm0, -0x3f0(%rbp)
   883e5:      	movups	%xmm0, -0x3e0(%rbp)
   883ec:      	leaq	-0x410(%rbp), %r15
   883f3:      	movq	%r15, %rdi
   883f6:      	movq	%r15, %rsi
   883f9:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   883fe:      	movl	$0x3, %edi
   88403:      	callq	0xb45a0 <m34_container_end>
   88408:      	movq	%r15, %rdi
   8840b:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   88410:      	movq	-0x40(%rbp), %rbx
   88414:      	movq	-0x68(%rbp), %r15
   88418:      	movq	%r14, %rdi
   8841b:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   88420:      	movq	%rbx, -0x40(%rbp)
   88424:      	movq	%r15, -0x68(%rbp)
   88428:      	cmpq	%r13, 0x18(%r15)
   8842c:      	jne	0x88468 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x8b8>
   8842e:      	leaq	-0x1(%r13), %rsi
   88432:      	movq	-0x40(%rbp), %rax
   88436:      	movq	%rax, -0x30(%rbp)
   8843a:      	movq	%r15, -0x38(%rbp)
   8843e:      	movq	%r15, %rdi
   88441:      	callq	0x85e10 <scoop$1$cb$e678f8a630461fa24384e1c95aa1201c5fb96088bf850cea5d070c63f1f249b6>
   88446:      	movq	-0x38(%rbp), %rcx
   8844a:      	movq	-0x30(%rbp), %rdx
   8844e:      	movq	%rdx, -0x40(%rbp)
   88452:      	movq	%rcx, -0x68(%rbp)
   88456:      	movq	%rcx, -0x188(%rbp)
   8845d:      	decl	%r13d
   88460:      	cmpl	%r13d, %eax
   88463:      	sete	%al
   88466:      	jmp	0x8846a <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x8ba>
   88468:      	xorl	%eax, %eax
   8846a:      	movq	-0x40(%rbp), %rcx
   8846e:      	movq	-0x68(%rbp), %rdx
   88472:      	movq	%rcx, -0x30(%rbp)
   88476:      	movq	%rdx, -0x38(%rbp)
   8847a:      	movzbl	%al, %edi
   8847d:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   88482:      	movq	-0x38(%rbp), %rax
   88486:      	movq	-0x30(%rbp), %rdi
   8848a:      	movq	%rdi, -0x40(%rbp)
   8848e:      	movq	%rax, -0x68(%rbp)
   88492:      	callq	0x57db0 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   88497:      	movq	-0x38(%rbp), %rcx
   8849b:      	movq	-0x30(%rbp), %rdx
   8849f:      	movq	%rdx, -0x40(%rbp)
   884a3:      	movq	%rcx, -0x68(%rbp)
   884a7:      	addq	0x18(%rcx), %rax
   884ab:      	addq	$0x3e8, %rsp            # imm = 0x3E8
   884b2:      	popq	%rbx
   884b3:      	popq	%r12
   884b5:      	popq	%r13
   884b7:      	popq	%r14
   884b9:      	popq	%r15
   884bb:      	popq	%rbp
   884bc:      	retq

; scalarList, MIR fn4, LIR scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

0000000000093bb0 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b>:
   93bb0:      	pushq	%rbp
   93bb1:      	movq	%rsp, %rbp
   93bb4:      	pushq	%r15
   93bb6:      	pushq	%r14
   93bb8:      	pushq	%r13
   93bba:      	pushq	%r12
   93bbc:      	pushq	%rbx
   93bbd:      	subq	$0x408, %rsp            # imm = 0x408
   93bc4:      	movq	%rdi, -0x58(%rbp)
   93bc8:      	movq	%fs:0x0, %rax
   93bd1:      	leaq	-0x10(%rax), %rax
   93bd8:      	movq	(%rax), %r13
   93bdb:      	leaq	0x1d0a2e(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   93be2:      	movq	(%rax), %rax
   93be5:      	leaq	0x1d0a2c(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   93bec:      	movl	(%rcx), %esi
   93bee:      	movl	(%r13), %edx
   93bf2:      	movq	0x8(%r13), %rcx
   93bf6:      	testl	%esi, %esi
   93bf8:      	jne	0x93c04 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x54>
   93bfa:      	cmpl	$0x1, %edx
   93bfd:      	jne	0x93c04 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x54>
   93bff:      	cmpq	%rax, %rcx
   93c02:      	je	0x93c09 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x59>
   93c04:      	callq	0x9b0c0 <scoop_rt_safepoint>
   93c09:      	xorps	%xmm0, %xmm0
   93c0c:      	movups	%xmm0, -0x160(%rbp)
   93c13:      	movq	$0x0, -0x150(%rbp)
   93c1e:      	leaq	-0x160(%rbp), %r14
   93c25:      	movq	%r14, %rdi
   93c28:      	xorl	%esi, %esi
   93c2a:      	xorl	%edx, %edx
   93c2c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   93c31:      	xorps	%xmm0, %xmm0
   93c34:      	movups	%xmm0, -0x230(%rbp)
   93c3b:      	movups	%xmm0, -0x220(%rbp)
   93c42:      	movups	%xmm0, -0x210(%rbp)
   93c49:      	movups	%xmm0, -0x200(%rbp)
   93c50:      	leaq	-0x230(%rbp), %r15
   93c57:      	movq	%r15, %rdi
   93c5a:      	movq	%r15, %rsi
   93c5d:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   93c62:      	callq	0xb4500 <m34_container_begin>
   93c67:      	movq	%r15, %rdi
   93c6a:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   93c6f:      	movq	%r14, %rdi
   93c72:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   93c77:      	movq	0x17767a(%rip), %rbx    # 0x20b2f8 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x18>
   93c7e:      	movq	%rbx, %r12
   93c81:      	negq	%r12
   93c84:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   93c8e:      	addq	%rbx, %rax
   93c91:      	cmpq	$-0x20, %rax
   93c95:      	setae	-0x39(%rbp)
   93c99:      	leaq	0x1f(%rbx), %r15
   93c9d:      	andq	%r12, %r15
   93ca0:      	movq	%fs:0x0, %rax
   93ca9:      	leaq	-0x8(%rax), %rax
   93cb0:      	movq	(%rax), %rax
   93cb3:      	movq	(%rax), %rcx
   93cb6:      	leaq	(%rbx,%rcx), %r14
   93cba:      	decq	%r14
   93cbd:      	andq	%r12, %r14
   93cc0:      	leaq	(%r14,%r15), %rcx
   93cc4:      	testq	%r14, %r14
   93cc7:      	sete	%dl
   93cca:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   93cd1:      	setae	%sil
   93cd5:      	cmpq	0x8(%rax), %rcx
   93cd9:      	seta	%dil
   93cdd:      	cmpq	$0x0, 0x17768b(%rip)    # 0x20b370 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x90>
   93ce5:      	setne	%r8b
   93ce9:      	orb	%sil, %r8b
   93cec:      	orb	%dl, %r8b
   93cef:      	orb	-0x39(%rbp), %r8b
   93cf3:      	orb	%dil, %r8b
   93cf6:      	jne	0x93d0f <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x15f>
   93cf8:      	movq	%rcx, (%rax)
   93cfb:      	leaq	0x1775de(%rip), %rsi    # 0x20b2e0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   93d02:      	movq	%r14, %rdi
   93d05:      	movq	%r15, %rdx
   93d08:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   93d0d:      	jmp	0x93d23 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x173>
   93d0f:      	leaq	0x1775ca(%rip), %rdi    # 0x20b2e0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   93d16:      	movl	$0x20, %esi
   93d1b:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   93d20:      	movq	%rax, %r14
   93d23:      	movq	%r14, -0x38(%rbp)
   93d27:      	movq	%r14, %rdi
   93d2a:      	movq	-0x58(%rbp), %rsi
   93d2e:      	callq	0x884c0 <scoop$1$cb$fee77490ca77ad5e976dbdde0ee39371498372f80a070cf4802c2070dd379995>
   93d33:      	movq	-0x38(%rbp), %rax
   93d37:      	movq	%rax, -0x148(%rbp)
   93d3e:      	movq	%rax, -0x30(%rbp)
   93d42:      	leaq	-0x30(%rbp), %r12
   93d46:      	movq	%r12, -0x88(%rbp)
   93d4d:      	leaq	0x17825c(%rip), %rax    # 0x20bfb0 <scoop$1$bs$bcb99a0ff252efde9f2461bf5d378d64d441b17151300bffe65e797b4fb3e9c4>
   93d54:      	movq	%rax, -0x80(%rbp)
   93d58:      	xorps	%xmm0, %xmm0
   93d5b:      	movups	%xmm0, -0x100(%rbp)
   93d62:      	movq	$0x0, -0xf0(%rbp)
   93d6d:      	leaq	-0x100(%rbp), %rdi
   93d74:      	leaq	-0x88(%rbp), %rsi
   93d7b:      	movl	$0x1, %edx
   93d80:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   93d85:      	xorps	%xmm0, %xmm0
   93d88:      	movups	%xmm0, -0x270(%rbp)
   93d8f:      	movups	%xmm0, -0x260(%rbp)
   93d96:      	movups	%xmm0, -0x250(%rbp)
   93d9d:      	movups	%xmm0, -0x240(%rbp)
   93da4:      	leaq	-0x270(%rbp), %r15
   93dab:      	movq	%r15, %rdi
   93dae:      	movq	%r15, %rsi
   93db1:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   93db6:      	xorl	%r14d, %r14d
   93db9:      	xorl	%edi, %edi
   93dbb:      	callq	0xb45a0 <m34_container_end>
   93dc0:      	movq	%r15, %rdi
   93dc3:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   93dc8:      	movq	-0x30(%rbp), %rbx
   93dcc:      	leaq	-0x100(%rbp), %rdi
   93dd3:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   93dd8:      	movq	%rbx, -0x30(%rbp)
   93ddc:      	movq	%rbx, -0x50(%rbp)
   93de0:      	movq	%r12, -0x138(%rbp)
   93de7:      	leaq	0x1781d2(%rip), %rax    # 0x20bfc0 <scoop$1$bs$aa08dcb8614caef12239188fa5ab82725d322897150ae91401a493d924e26fc4>
   93dee:      	movq	%rax, -0x130(%rbp)
   93df5:      	leaq	-0x50(%rbp), %rax
   93df9:      	movq	%rax, -0x128(%rbp)
   93e00:      	leaq	0x1781c9(%rip), %rax    # 0x20bfd0 <scoop$1$bs$7fad4c10f0e5cf7847850fcda78285dcae00dbc4b5afb43515918b1e14e79d87>
   93e07:      	movq	%rax, -0x120(%rbp)
   93e0e:      	xorps	%xmm0, %xmm0
   93e11:      	movups	%xmm0, -0x118(%rbp)
   93e18:      	movq	$0x0, -0x108(%rbp)
   93e23:      	leaq	-0x118(%rbp), %rdi
   93e2a:      	leaq	-0x138(%rbp), %rsi
   93e31:      	movl	$0x2, %edx
   93e36:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   93e3b:      	xorps	%xmm0, %xmm0
   93e3e:      	movups	%xmm0, -0x2b0(%rbp)
   93e45:      	movups	%xmm0, -0x2a0(%rbp)
   93e4c:      	movups	%xmm0, -0x290(%rbp)
   93e53:      	movups	%xmm0, -0x280(%rbp)
   93e5a:      	leaq	-0x2b0(%rbp), %r15
   93e61:      	movq	%r15, %rdi
   93e64:      	movq	%r15, %rsi
   93e67:      	callq	0x9b2e0 <scoop_rt_enter_native_borrowed>
   93e6c:      	movq	-0x50(%rbp), %rdi
   93e70:      	callq	0xb46d0 <m34_container_layout>
   93e75:      	movq	%r15, %rdi
   93e78:      	callq	0x9dc70 <scoop_rt_leave_native_borrowed>
   93e7d:      	movq	-0x30(%rbp), %rbx
   93e81:      	movq	-0x50(%rbp), %r15
   93e85:      	leaq	-0x118(%rbp), %rdi
   93e8c:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   93e91:      	movq	%rbx, -0x30(%rbp)
   93e95:      	movq	%r15, -0x50(%rbp)
   93e99:      	movq	%r12, -0x98(%rbp)
   93ea0:      	leaq	0x178139(%rip), %rax    # 0x20bfe0 <scoop$1$bs$8154c610379b43f30fec6b45882b9a8c1f5296d4cf7d6bc8d37a44c995908446>
   93ea7:      	movq	%rax, -0x90(%rbp)
   93eae:      	xorps	%xmm0, %xmm0
   93eb1:      	movups	%xmm0, -0x178(%rbp)
   93eb8:      	movq	$0x0, -0x168(%rbp)
   93ec3:      	leaq	-0x178(%rbp), %r12
   93eca:      	leaq	-0x98(%rbp), %rsi
   93ed1:      	movl	$0x1, %edx
   93ed6:      	movq	%r12, %rdi
   93ed9:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   93ede:      	xorps	%xmm0, %xmm0
   93ee1:      	movups	%xmm0, -0x2f0(%rbp)
   93ee8:      	movups	%xmm0, -0x2e0(%rbp)
   93eef:      	movups	%xmm0, -0x2d0(%rbp)
   93ef6:      	movups	%xmm0, -0x2c0(%rbp)
   93efd:      	leaq	-0x2f0(%rbp), %r15
   93f04:      	movq	%r15, %rdi
   93f07:      	movq	%r15, %rsi
   93f0a:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   93f0f:      	callq	0xb4500 <m34_container_begin>
   93f14:      	movq	%r15, %rdi
   93f17:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   93f1c:      	movq	-0x30(%rbp), %rbx
   93f20:      	movq	%r12, %rdi
   93f23:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   93f28:      	movq	%rbx, -0x30(%rbp)
   93f2c:      	movabsq	$0x51d07eae2f8151d1, %rbx # imm = 0x51D07EAE2F8151D1
   93f36:      	xorl	%r12d, %r12d
   93f39:      	nopl	(%rax)
   93f40:      	movq	%r14, %rax
   93f43:      	mulq	%rbx
   93f46:      	movq	%rdx, %r15
   93f49:      	leaq	0x1d06c0(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   93f50:      	movq	(%rax), %rax
   93f53:      	leaq	0x1d06be(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   93f5a:      	movl	(%rcx), %esi
   93f5c:      	movl	(%r13), %edx
   93f60:      	movq	0x8(%r13), %rcx
   93f64:      	testl	%esi, %esi
   93f66:      	jne	0x93f72 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3c2>
   93f68:      	cmpl	$0x1, %edx
   93f6b:      	jne	0x93f72 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3c2>
   93f6d:      	cmpq	%rax, %rcx
   93f70:      	je	0x93f87 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3d7>
   93f72:      	movq	-0x30(%rbp), %rax
   93f76:      	movq	%rax, -0x38(%rbp)
   93f7a:      	callq	0x9b0c0 <scoop_rt_safepoint>
   93f7f:      	movq	-0x38(%rbp), %rax
   93f83:      	movq	%rax, -0x30(%rbp)
   93f87:      	cmpq	-0x58(%rbp), %r14
   93f8b:      	jge	0x93ff8 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x448>
   93f8d:      	movq	%r14, %rax
   93f90:      	subq	%r15, %rax
   93f93:      	shrq	%rax
   93f96:      	addq	%r15, %rax
   93f99:      	shrq	$0x6, %rax
   93f9d:      	imull	$0x61, %eax, %eax
   93fa0:      	movl	%r14d, %esi
   93fa3:      	subl	%eax, %esi
   93fa5:      	movq	%r14, %rax
   93fa8:      	movabsq	$-0x5717c0a8e83f5717, %rcx # imm = 0xA8E83F5717C0A8E9
   93fb2:      	imulq	%rcx
   93fb5:      	addq	%r14, %rdx
   93fb8:      	movq	%rdx, %rax
   93fbb:      	shrq	$0x3f, %rax
   93fbf:      	sarq	$0x6, %rdx
   93fc3:      	addq	%rax, %rdx
   93fc6:      	imulq	$0x61, %rdx, %rax
   93fca:      	movq	%r14, %r15
   93fcd:      	subq	%rax, %r15
   93fd0:      	movq	-0x30(%rbp), %rdi
   93fd4:      	movq	%rdi, -0x78(%rbp)
   93fd8:      	movq	%rdi, -0x38(%rbp)
   93fdc:      	callq	0x95000 <scoop$1$cb$6571e66f09cbc869977ca80836b265c26b4c58d100cec7b4dc74f71fda171ea3>
   93fe1:      	movq	-0x38(%rbp), %rax
   93fe5:      	movq	%rax, -0x30(%rbp)
   93fe9:      	movq	%rax, -0x78(%rbp)
   93fed:      	addq	%r15, %r12
   93ff0:      	incq	%r14
   93ff3:      	jmp	0x93f40 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x390>
   93ff8:      	leaq	-0x30(%rbp), %rax
   93ffc:      	movq	%rax, -0xa8(%rbp)
   94003:      	leaq	0x177fe6(%rip), %rax    # 0x20bff0 <scoop$1$bs$8293c51dde1c1ee9623a4e4806deb5dfa27735faeabe6a4a42d9d53172184c49>
   9400a:      	movq	%rax, -0xa0(%rbp)
   94011:      	xorps	%xmm0, %xmm0
   94014:      	movups	%xmm0, -0x190(%rbp)
   9401b:      	movq	$0x0, -0x180(%rbp)
   94026:      	leaq	-0x190(%rbp), %r14
   9402d:      	leaq	-0xa8(%rbp), %rsi
   94034:      	movl	$0x1, %edx
   94039:      	movq	%r14, %rdi
   9403c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   94041:      	xorps	%xmm0, %xmm0
   94044:      	movups	%xmm0, -0x330(%rbp)
   9404b:      	movups	%xmm0, -0x320(%rbp)
   94052:      	movups	%xmm0, -0x310(%rbp)
   94059:      	movups	%xmm0, -0x300(%rbp)
   94060:      	leaq	-0x330(%rbp), %r15
   94067:      	movq	%r15, %rdi
   9406a:      	movq	%r15, %rsi
   9406d:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   94072:      	movl	$0x1, %edi
   94077:      	callq	0xb45a0 <m34_container_end>
   9407c:      	movq	%r15, %rdi
   9407f:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   94084:      	movq	-0x30(%rbp), %rbx
   94088:      	movq	%r14, %rdi
   9408b:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   94090:      	movq	%rbx, -0x30(%rbp)
   94094:      	leaq	-0x30(%rbp), %rax
   94098:      	movq	%rax, -0xb8(%rbp)
   9409f:      	leaq	0x177f5a(%rip), %rax    # 0x20c000 <scoop$1$bs$12e7e0bcae28d4be2ecf772279a88b4f8cc520101d08f1e4193f652e07e2ae9c>
   940a6:      	movq	%rax, -0xb0(%rbp)
   940ad:      	xorps	%xmm0, %xmm0
   940b0:      	movups	%xmm0, -0x1a8(%rbp)
   940b7:      	movq	$0x0, -0x198(%rbp)
   940c2:      	leaq	-0x1a8(%rbp), %r14
   940c9:      	leaq	-0xb8(%rbp), %rsi
   940d0:      	movl	$0x1, %edx
   940d5:      	movq	%r14, %rdi
   940d8:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   940dd:      	xorps	%xmm0, %xmm0
   940e0:      	movups	%xmm0, -0x370(%rbp)
   940e7:      	movups	%xmm0, -0x360(%rbp)
   940ee:      	movups	%xmm0, -0x350(%rbp)
   940f5:      	movups	%xmm0, -0x340(%rbp)
   940fc:      	leaq	-0x370(%rbp), %r15
   94103:      	movq	%r15, %rdi
   94106:      	movq	%r15, %rsi
   94109:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   9410e:      	callq	0xb4500 <m34_container_begin>
   94113:      	movq	%r15, %rdi
   94116:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   9411b:      	movq	-0x30(%rbp), %rbx
   9411f:      	movq	%r14, %rdi
   94122:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   94127:      	movq	%rbx, -0x30(%rbp)
   9412b:      	xorl	%r14d, %r14d
   9412e:      	xorl	%r15d, %r15d
   94131:      	nopw	%cs:(%rax,%rax)
   94140:      	leaq	0x1d04c9(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   94147:      	movq	(%rax), %rax
   9414a:      	leaq	0x1d04c7(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   94151:      	movl	(%rcx), %esi
   94153:      	movl	(%r13), %edx
   94157:      	movq	0x8(%r13), %rcx
   9415b:      	testl	%esi, %esi
   9415d:      	jne	0x94169 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5b9>
   9415f:      	cmpl	$0x1, %edx
   94162:      	jne	0x94169 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5b9>
   94164:      	cmpq	%rax, %rcx
   94167:      	je	0x9417e <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5ce>
   94169:      	movq	-0x30(%rbp), %rax
   9416d:      	movq	%rax, -0x38(%rbp)
   94171:      	callq	0x9b0c0 <scoop_rt_safepoint>
   94176:      	movq	-0x38(%rbp), %rax
   9417a:      	movq	%rax, -0x30(%rbp)
   9417e:      	cmpq	-0x58(%rbp), %r15
   94182:      	jge	0x941f8 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x648>
   94184:      	movq	-0x30(%rbp), %rdi
   94188:      	movq	%rdi, -0x70(%rbp)
   9418c:      	movq	%rdi, -0x38(%rbp)
   94190:      	movq	%r15, %rsi
   94193:      	callq	0x85e10 <scoop$1$cb$e678f8a630461fa24384e1c95aa1201c5fb96088bf850cea5d070c63f1f249b6>
   94198:      	movq	-0x38(%rbp), %rdi
   9419c:      	movq	%rdi, -0x30(%rbp)
   941a0:      	movq	%rdi, -0x70(%rbp)
   941a4:      	cltq
   941a6:      	addq	%rax, %r14
   941a9:      	movq	%rdi, -0x60(%rbp)
   941ad:      	movq	%rdi, -0x68(%rbp)
   941b1:      	movq	-0x30(%rbp), %rax
   941b5:      	movq	%rax, -0x48(%rbp)
   941b9:      	movq	%r15, %rsi
   941bc:      	callq	0x85e10 <scoop$1$cb$e678f8a630461fa24384e1c95aa1201c5fb96088bf850cea5d070c63f1f249b6>
   941c1:      	movq	-0x38(%rbp), %rdi
   941c5:      	movq	-0x48(%rbp), %rcx
   941c9:      	movq	%rcx, -0x30(%rbp)
   941cd:      	movq	%rdi, -0x68(%rbp)
   941d1:      	movq	%rdi, -0x60(%rbp)
   941d5:      	leal	0x1(%rax), %edx
   941d8:      	movq	%r15, %rsi
   941db:      	callq	0x7e500 <scoop$1$cb$ccfa7da2b94cdac76f7ef88f24a6c3c0f26b22e9b6c28b9ca348f29995c47f8c>
   941e0:      	movq	-0x38(%rbp), %rax
   941e4:      	movq	-0x48(%rbp), %rcx
   941e8:      	movq	%rcx, -0x30(%rbp)
   941ec:      	movq	%rax, -0x60(%rbp)
   941f0:      	incq	%r15
   941f3:      	jmp	0x94140 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x590>
   941f8:      	leaq	-0x30(%rbp), %r13
   941fc:      	movq	%r13, -0xc8(%rbp)
   94203:      	leaq	0x177e06(%rip), %rax    # 0x20c010 <scoop$1$bs$58ef8f7f24e1c902e9b2e0565447e1f6b2009cc91f904580dc3e7c02e5c9eaef>
   9420a:      	movq	%rax, -0xc0(%rbp)
   94211:      	xorps	%xmm0, %xmm0
   94214:      	movups	%xmm0, -0x1c0(%rbp)
   9421b:      	movq	$0x0, -0x1b0(%rbp)
   94226:      	leaq	-0x1c0(%rbp), %rbx
   9422d:      	leaq	-0xc8(%rbp), %rsi
   94234:      	movl	$0x1, %edx
   94239:      	movq	%rbx, %rdi
   9423c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   94241:      	xorps	%xmm0, %xmm0
   94244:      	movups	%xmm0, -0x3b0(%rbp)
   9424b:      	movups	%xmm0, -0x3a0(%rbp)
   94252:      	movups	%xmm0, -0x390(%rbp)
   94259:      	movups	%xmm0, -0x380(%rbp)
   94260:      	leaq	-0x3b0(%rbp), %r15
   94267:      	movq	%r15, %rdi
   9426a:      	movq	%r15, %rsi
   9426d:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   94272:      	movl	$0x2, %edi
   94277:      	callq	0xb45a0 <m34_container_end>
   9427c:      	movq	%r15, %rdi
   9427f:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   94284:      	movq	-0x30(%rbp), %r15
   94288:      	movq	%rbx, %rdi
   9428b:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   94290:      	movq	%r15, -0x30(%rbp)
   94294:      	xorl	%edi, %edi
   94296:      	cmpq	%r12, %r14
   94299:      	sete	%dil
   9429d:      	movq	%r15, -0x38(%rbp)
   942a1:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   942a6:      	movq	-0x38(%rbp), %rax
   942aa:      	movq	%rax, -0x30(%rbp)
   942ae:      	movq	%r13, -0xd8(%rbp)
   942b5:      	leaq	0x177d64(%rip), %rax    # 0x20c020 <scoop$1$bs$d3600b0213aa68037cc0823e687015cbf3e60338d644e3efd6f41f0b280c701d>
   942bc:      	movq	%rax, -0xd0(%rbp)
   942c3:      	xorps	%xmm0, %xmm0
   942c6:      	movups	%xmm0, -0x1d8(%rbp)
   942cd:      	movq	$0x0, -0x1c8(%rbp)
   942d8:      	leaq	-0x1d8(%rbp), %rbx
   942df:      	leaq	-0xd8(%rbp), %rsi
   942e6:      	movl	$0x1, %edx
   942eb:      	movq	%rbx, %rdi
   942ee:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   942f3:      	xorps	%xmm0, %xmm0
   942f6:      	movups	%xmm0, -0x3f0(%rbp)
   942fd:      	movups	%xmm0, -0x3e0(%rbp)
   94304:      	movups	%xmm0, -0x3d0(%rbp)
   9430b:      	movups	%xmm0, -0x3c0(%rbp)
   94312:      	leaq	-0x3f0(%rbp), %r15
   94319:      	movq	%r15, %rdi
   9431c:      	movq	%r15, %rsi
   9431f:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   94324:      	callq	0xb4500 <m34_container_begin>
   94329:      	movq	%r15, %rdi
   9432c:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   94331:      	movq	-0x30(%rbp), %r15
   94335:      	movq	%rbx, %rdi
   94338:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   9433d:      	movq	%r15, -0x30(%rbp)
   94341:      	movq	%r15, -0x38(%rbp)
   94345:      	movq	%r15, %rdi
   94348:      	callq	0x86da0 <scoop$1$cb$1b62e78994e6929f86ac69ff970c434030a57c0e62b785f6ad4e4c34ef5cc7c1>
   9434d:      	movq	-0x38(%rbp), %rax
   94351:      	movq	%rax, -0x30(%rbp)
   94355:      	movq	%rax, -0x140(%rbp)
   9435c:      	movq	%r13, -0xe8(%rbp)
   94363:      	leaq	0x177cc6(%rip), %rax    # 0x20c030 <scoop$1$bs$dc7cb9f7620b61f1342ffa4248e1d28aea6b95227e2b2c1fd17df8bc58bc08ea>
   9436a:      	movq	%rax, -0xe0(%rbp)
   94371:      	xorps	%xmm0, %xmm0
   94374:      	movups	%xmm0, -0x1f0(%rbp)
   9437b:      	movq	$0x0, -0x1e0(%rbp)
   94386:      	leaq	-0x1f0(%rbp), %rbx
   9438d:      	leaq	-0xe8(%rbp), %rsi
   94394:      	movl	$0x1, %edx
   94399:      	movq	%rbx, %rdi
   9439c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   943a1:      	xorps	%xmm0, %xmm0
   943a4:      	movups	%xmm0, -0x430(%rbp)
   943ab:      	movups	%xmm0, -0x420(%rbp)
   943b2:      	movups	%xmm0, -0x410(%rbp)
   943b9:      	movups	%xmm0, -0x400(%rbp)
   943c0:      	leaq	-0x430(%rbp), %r15
   943c7:      	movq	%r15, %rdi
   943ca:      	movq	%r15, %rsi
   943cd:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   943d2:      	movl	$0x3, %edi
   943d7:      	callq	0xb45a0 <m34_container_end>
   943dc:      	movq	%r15, %rdi
   943df:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   943e4:      	movq	-0x30(%rbp), %r15
   943e8:      	movq	%rbx, %rdi
   943eb:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   943f0:      	movq	%r15, -0x30(%rbp)
   943f4:      	xorl	%edi, %edi
   943f6:      	cmpq	$0x0, 0x18(%r15)
   943fb:      	sete	%dil
   943ff:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   94404:      	movq	%r14, %rax
   94407:      	addq	$0x408, %rsp            # imm = 0x408
   9440e:      	popq	%rbx
   9440f:      	popq	%r12
   94411:      	popq	%r13
   94413:      	popq	%r14
   94415:      	popq	%r15
   94417:      	popq	%rbp
   94418:      	retq

; interfaceList, MIR fn5, LIR scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000008d710 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c>:
   8d710:      	pushq	%rbp
   8d711:      	movq	%rsp, %rbp
   8d714:      	pushq	%r15
   8d716:      	pushq	%r14
   8d718:      	pushq	%r13
   8d71a:      	pushq	%r12
   8d71c:      	pushq	%rbx
   8d71d:      	subq	$0x428, %rsp            # imm = 0x428
   8d724:      	movq	%rdi, -0x58(%rbp)
   8d728:      	movq	%fs:0x0, %rax
   8d731:      	leaq	-0x10(%rax), %rax
   8d738:      	movq	(%rax), %rcx
   8d73b:      	movq	$0x0, -0x78(%rbp)
   8d743:      	leaq	0x1d6ec6(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   8d74a:      	movq	(%rax), %rax
   8d74d:      	leaq	0x1d6ec4(%rip), %rdx    # 0x264618 <scoop_thread_world_phase>
   8d754:      	movl	(%rdx), %esi
   8d756:      	movl	(%rcx), %edx
   8d758:      	movq	%rcx, -0x50(%rbp)
   8d75c:      	movq	0x8(%rcx), %rcx
   8d760:      	testl	%esi, %esi
   8d762:      	jne	0x8d76e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x5e>
   8d764:      	cmpl	$0x1, %edx
   8d767:      	jne	0x8d76e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x5e>
   8d769:      	cmpq	%rax, %rcx
   8d76c:      	je	0x8d773 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x63>
   8d76e:      	callq	0x9b0c0 <scoop_rt_safepoint>
   8d773:      	movabsq	$0x7fffffffffffffff, %r13 # imm = 0x7FFFFFFFFFFFFFFF
   8d77d:      	xorps	%xmm0, %xmm0
   8d780:      	movups	%xmm0, -0x150(%rbp)
   8d787:      	movq	$0x0, -0x140(%rbp)
   8d792:      	leaq	-0x150(%rbp), %r14
   8d799:      	movq	%r14, %rdi
   8d79c:      	xorl	%esi, %esi
   8d79e:      	xorl	%edx, %edx
   8d7a0:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d7a5:      	xorps	%xmm0, %xmm0
   8d7a8:      	movups	%xmm0, -0x250(%rbp)
   8d7af:      	movups	%xmm0, -0x240(%rbp)
   8d7b6:      	movups	%xmm0, -0x230(%rbp)
   8d7bd:      	movups	%xmm0, -0x220(%rbp)
   8d7c4:      	leaq	-0x250(%rbp), %r15
   8d7cb:      	movq	%r15, %rdi
   8d7ce:      	movq	%r15, %rsi
   8d7d1:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d7d6:      	callq	0xb4500 <m34_container_begin>
   8d7db:      	movq	%r15, %rdi
   8d7de:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d7e3:      	movq	%r14, %rdi
   8d7e6:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d7eb:      	movq	0x15b486(%rip), %rbx    # 0x1e8c78 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d+0x18>
   8d7f2:      	movq	%rbx, %r12
   8d7f5:      	negq	%r12
   8d7f8:      	leaq	(%rbx,%r13), %rax
   8d7fc:      	cmpq	$-0x20, %rax
   8d800:      	setae	%r13b
   8d804:      	leaq	0x1f(%rbx), %r15
   8d808:      	andq	%r12, %r15
   8d80b:      	movq	%fs:0x0, %rax
   8d814:      	leaq	-0x8(%rax), %rax
   8d81b:      	movq	(%rax), %rax
   8d81e:      	movq	(%rax), %rcx
   8d821:      	leaq	(%rbx,%rcx), %r14
   8d825:      	decq	%r14
   8d828:      	andq	%r12, %r14
   8d82b:      	leaq	(%r14,%r15), %rcx
   8d82f:      	testq	%r14, %r14
   8d832:      	sete	%dl
   8d835:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   8d83c:      	setae	%sil
   8d840:      	cmpq	0x8(%rax), %rcx
   8d844:      	seta	%dil
   8d848:      	cmpq	$0x0, 0x15b4a0(%rip)    # 0x1e8cf0 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d+0x90>
   8d850:      	setne	%r8b
   8d854:      	orb	%sil, %r8b
   8d857:      	orb	%dl, %r8b
   8d85a:      	orb	%r13b, %r8b
   8d85d:      	orb	%dil, %r8b
   8d860:      	jne	0x8d879 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x169>
   8d862:      	movq	%rcx, (%rax)
   8d865:      	leaq	0x15b3f4(%rip), %rsi    # 0x1e8c60 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d>
   8d86c:      	movq	%r14, %rdi
   8d86f:      	movq	%r15, %rdx
   8d872:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   8d877:      	jmp	0x8d88d <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x17d>
   8d879:      	leaq	0x15b3e0(%rip), %rdi    # 0x1e8c60 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d>
   8d880:      	movl	$0x20, %esi
   8d885:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   8d88a:      	movq	%rax, %r14
   8d88d:      	movq	-0x58(%rbp), %r13
   8d891:      	movq	%r14, -0x38(%rbp)
   8d895:      	movq	%r14, %rdi
   8d898:      	movq	%r13, %rsi
   8d89b:      	callq	0x8f7f0 <scoop$1$cb$618fe03087f04fe6de447a9d1eef329119340be9a3335c5e390675ee699883ef>
   8d8a0:      	movq	-0x38(%rbp), %rax
   8d8a4:      	movq	%rax, -0x138(%rbp)
   8d8ab:      	movq	%rax, -0x30(%rbp)
   8d8af:      	leaq	-0x30(%rbp), %r14
   8d8b3:      	movq	%r14, -0xa8(%rbp)
   8d8ba:      	leaq	0x16e88f(%rip), %rax    # 0x1fc150 <scoop$1$bs$0ccf51103070cd5973bbd27997fe267f45aeaa291fe0b77198fb9dd565d8ce03>
   8d8c1:      	movq	%rax, -0xa0(%rbp)
   8d8c8:      	xorps	%xmm0, %xmm0
   8d8cb:      	movups	%xmm0, -0x168(%rbp)
   8d8d2:      	movq	$0x0, -0x158(%rbp)
   8d8dd:      	leaq	-0x168(%rbp), %r12
   8d8e4:      	leaq	-0xa8(%rbp), %rsi
   8d8eb:      	movl	$0x1, %edx
   8d8f0:      	movq	%r12, %rdi
   8d8f3:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d8f8:      	xorps	%xmm0, %xmm0
   8d8fb:      	movups	%xmm0, -0x290(%rbp)
   8d902:      	movups	%xmm0, -0x280(%rbp)
   8d909:      	movups	%xmm0, -0x270(%rbp)
   8d910:      	movups	%xmm0, -0x260(%rbp)
   8d917:      	leaq	-0x290(%rbp), %r15
   8d91e:      	movq	%r15, %rdi
   8d921:      	movq	%r15, %rsi
   8d924:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8d929:      	xorl	%edi, %edi
   8d92b:      	callq	0xb45a0 <m34_container_end>
   8d930:      	movq	%r15, %rdi
   8d933:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8d938:      	movq	-0x30(%rbp), %rbx
   8d93c:      	movq	%r12, %rdi
   8d93f:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d944:      	movq	%rbx, -0x30(%rbp)
   8d948:      	movq	%rbx, -0x48(%rbp)
   8d94c:      	movq	%r14, -0x128(%rbp)
   8d953:      	leaq	0x16e806(%rip), %rax    # 0x1fc160 <scoop$1$bs$dc1e38a95e7aa7813dea4c23668cd78f9bba3a02d4d526a208c168e880ec34c4>
   8d95a:      	movq	%rax, -0x120(%rbp)
   8d961:      	leaq	-0x48(%rbp), %rax
   8d965:      	movq	%rax, -0x118(%rbp)
   8d96c:      	leaq	0x16e7fd(%rip), %rax    # 0x1fc170 <scoop$1$bs$aab25ec34d3c92f7452d4da325ec2f2c57f433d8ea7954916d98cae0718b9a81>
   8d973:      	movq	%rax, -0x110(%rbp)
   8d97a:      	xorps	%xmm0, %xmm0
   8d97d:      	movups	%xmm0, -0x180(%rbp)
   8d984:      	movq	$0x0, -0x170(%rbp)
   8d98f:      	leaq	-0x180(%rbp), %r12
   8d996:      	leaq	-0x128(%rbp), %rsi
   8d99d:      	movl	$0x2, %edx
   8d9a2:      	movq	%r12, %rdi
   8d9a5:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8d9aa:      	xorps	%xmm0, %xmm0
   8d9ad:      	movups	%xmm0, -0x2d0(%rbp)
   8d9b4:      	movups	%xmm0, -0x2c0(%rbp)
   8d9bb:      	movups	%xmm0, -0x2b0(%rbp)
   8d9c2:      	movups	%xmm0, -0x2a0(%rbp)
   8d9c9:      	leaq	-0x2d0(%rbp), %r15
   8d9d0:      	movq	%r15, %rdi
   8d9d3:      	movq	%r15, %rsi
   8d9d6:      	callq	0x9b2e0 <scoop_rt_enter_native_borrowed>
   8d9db:      	movq	-0x48(%rbp), %rdi
   8d9df:      	callq	0xb46d0 <m34_container_layout>
   8d9e4:      	movq	%r15, %rdi
   8d9e7:      	callq	0x9dc70 <scoop_rt_leave_native_borrowed>
   8d9ec:      	movq	-0x30(%rbp), %rbx
   8d9f0:      	movq	-0x48(%rbp), %r14
   8d9f4:      	movq	%r12, %rdi
   8d9f7:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8d9fc:      	movq	%rbx, -0x30(%rbp)
   8da00:      	movq	%r14, -0x48(%rbp)
   8da04:      	leaq	-0x30(%rbp), %rax
   8da08:      	movq	%rax, -0xb8(%rbp)
   8da0f:      	leaq	0x16e76a(%rip), %rax    # 0x1fc180 <scoop$1$bs$4a9eed8bc07d443add3de6fce27dbc079379d7b42a1bdfd844e29ed838d3406c>
   8da16:      	movq	%rax, -0xb0(%rbp)
   8da1d:      	xorps	%xmm0, %xmm0
   8da20:      	movups	%xmm0, -0x198(%rbp)
   8da27:      	movq	$0x0, -0x188(%rbp)
   8da32:      	leaq	-0x198(%rbp), %r12
   8da39:      	leaq	-0xb8(%rbp), %rsi
   8da40:      	movl	$0x1, %edx
   8da45:      	movq	%r12, %rdi
   8da48:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8da4d:      	xorps	%xmm0, %xmm0
   8da50:      	movups	%xmm0, -0x310(%rbp)
   8da57:      	movups	%xmm0, -0x300(%rbp)
   8da5e:      	movups	%xmm0, -0x2f0(%rbp)
   8da65:      	movups	%xmm0, -0x2e0(%rbp)
   8da6c:      	leaq	-0x310(%rbp), %r15
   8da73:      	movq	%r15, %rdi
   8da76:      	movq	%r15, %rsi
   8da79:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8da7e:      	callq	0xb4500 <m34_container_begin>
   8da83:      	movq	%r15, %rdi
   8da86:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8da8b:      	movq	-0x30(%rbp), %rbx
   8da8f:      	movq	%r12, %rdi
   8da92:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8da97:      	movq	%rbx, -0x30(%rbp)
   8da9b:      	xorl	%ebx, %ebx
   8da9d:      	movq	-0x50(%rbp), %r12
   8daa1:      	jmp	0x8db47 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x437>
   8daa6:      	nopw	%cs:(%rax,%rax)
   8dab0:      	movq	-0x30(%rbp), %rax
   8dab4:      	movq	%rax, -0x38(%rbp)
   8dab8:      	movq	%rbx, -0x40(%rbp)
   8dabc:      	movl	$0x18, %esi
   8dac1:      	leaq	0x1853d8(%rip), %rdi    # 0x212ea0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8dac8:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   8dacd:      	movq	%rax, %r13
   8dad0:      	movq	-0x40(%rbp), %rax
   8dad4:      	movq	-0x38(%rbp), %rcx
   8dad8:      	movq	%rcx, -0x30(%rbp)
   8dadc:      	movq	%rax, -0x60(%rbp)
   8dae0:      	movq	-0x50(%rbp), %r12
   8dae4:      	movq	-0x80(%rbp), %rbx
   8dae8:      	movl	%ebx, 0x10(%r13)
   8daec:      	movq	0x18540d(%rip), %rax    # 0x212f00 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x60>
   8daf3:      	movq	0x8(%rax), %rdx
   8daf7:      	movq	%r13, -0x90(%rbp)
   8dafe:      	movq	-0x90(%rbp), %rsi
   8db05:      	movq	%rsi, -0x88(%rbp)
   8db0c:      	movq	-0x30(%rbp), %rax
   8db10:      	movq	-0x60(%rbp), %rdi
   8db14:      	movq	%rax, -0x38(%rbp)
   8db18:      	movq	%rdi, -0x40(%rbp)
   8db1c:      	movq	%rsi, -0x68(%rbp)
   8db20:      	callq	0x89df0 <scoop$1$cb$6d53c57dfb720cf365bbd7bd3500d138e2ff85a16ea4802cf62d05958722c237>
   8db25:      	movq	-0x68(%rbp), %rax
   8db29:      	movq	-0x40(%rbp), %rcx
   8db2d:      	movq	-0x38(%rbp), %rdx
   8db31:      	movq	%rdx, -0x30(%rbp)
   8db35:      	movq	%rcx, -0x60(%rbp)
   8db39:      	movq	%rax, -0x88(%rbp)
   8db40:      	incq	%rbx
   8db43:      	movq	-0x58(%rbp), %r13
   8db47:      	leaq	0x1d6ac2(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   8db4e:      	movq	(%rax), %rax
   8db51:      	leaq	0x1d6ac0(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   8db58:      	movl	(%rcx), %esi
   8db5a:      	movl	(%r12), %edx
   8db5e:      	movq	0x8(%r12), %rcx
   8db63:      	testl	%esi, %esi
   8db65:      	jne	0x8db71 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x461>
   8db67:      	cmpl	$0x1, %edx
   8db6a:      	jne	0x8db71 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x461>
   8db6c:      	cmpq	%rax, %rcx
   8db6f:      	je	0x8db86 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x476>
   8db71:      	movq	-0x30(%rbp), %rax
   8db75:      	movq	%rax, -0x38(%rbp)
   8db79:      	callq	0x9b0c0 <scoop_rt_safepoint>
   8db7e:      	movq	-0x38(%rbp), %rax
   8db82:      	movq	%rax, -0x30(%rbp)
   8db86:      	cmpq	%r13, %rbx
   8db89:      	jge	0x8dc38 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x528>
   8db8f:      	movq	%rbx, -0x80(%rbp)
   8db93:      	movq	-0x30(%rbp), %rbx
   8db97:      	movq	%rbx, -0x60(%rbp)
   8db9b:      	movq	0x185316(%rip), %r13    # 0x212eb8 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x18>
   8dba2:      	movq	%r13, %r15
   8dba5:      	negq	%r15
   8dba8:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8dbb2:      	addq	%r13, %rax
   8dbb5:      	cmpq	$-0x18, %rax
   8dbb9:      	setb	%r14b
   8dbbd:      	leaq	0x17(%r13), %r12
   8dbc1:      	andq	%r15, %r12
   8dbc4:      	movq	%fs:0x0, %rax
   8dbcd:      	leaq	-0x8(%rax), %rax
   8dbd4:      	movq	(%rax), %rax
   8dbd7:      	movq	(%rax), %rcx
   8dbda:      	addq	%rcx, %r13
   8dbdd:      	decq	%r13
   8dbe0:      	andq	%r15, %r13
   8dbe3:      	leaq	(%r12,%r13), %rcx
   8dbe7:      	testq	%r13, %r13
   8dbea:      	setne	%dl
   8dbed:      	cmpq	$0x7f81, %r12           # imm = 0x7F81
   8dbf4:      	setb	%sil
   8dbf8:      	cmpq	0x8(%rax), %rcx
   8dbfc:      	setbe	%dil
   8dc00:      	cmpq	$0x0, 0x185328(%rip)    # 0x212f30 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x90>
   8dc08:      	sete	%r8b
   8dc0c:      	andb	%sil, %r8b
   8dc0f:      	andb	%dl, %r8b
   8dc12:      	andb	%dil, %r8b
   8dc15:      	testb	%r14b, %r8b
   8dc18:      	je	0x8dab0 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x3a0>
   8dc1e:      	movq	%rcx, (%rax)
   8dc21:      	movq	%r13, %rdi
   8dc24:      	leaq	0x185275(%rip), %rsi    # 0x212ea0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8dc2b:      	movq	%r12, %rdx
   8dc2e:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   8dc33:      	jmp	0x8dae0 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x3d0>
   8dc38:      	leaq	-0x30(%rbp), %rax
   8dc3c:      	movq	%rax, -0xc8(%rbp)
   8dc43:      	leaq	0x16e546(%rip), %rax    # 0x1fc190 <scoop$1$bs$c9b66f30bb517bd5c355e2a5ec1a5a28a25b388c4f7ea5721bc02694e6ab46df>
   8dc4a:      	movq	%rax, -0xc0(%rbp)
   8dc51:      	xorps	%xmm0, %xmm0
   8dc54:      	movups	%xmm0, -0x1b0(%rbp)
   8dc5b:      	movq	$0x0, -0x1a0(%rbp)
   8dc66:      	leaq	-0x1b0(%rbp), %r14
   8dc6d:      	leaq	-0xc8(%rbp), %rsi
   8dc74:      	movl	$0x1, %edx
   8dc79:      	movq	%r14, %rdi
   8dc7c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8dc81:      	xorps	%xmm0, %xmm0
   8dc84:      	movups	%xmm0, -0x350(%rbp)
   8dc8b:      	movups	%xmm0, -0x340(%rbp)
   8dc92:      	movups	%xmm0, -0x330(%rbp)
   8dc99:      	movups	%xmm0, -0x320(%rbp)
   8dca0:      	leaq	-0x350(%rbp), %r15
   8dca7:      	movq	%r15, %rdi
   8dcaa:      	movq	%r15, %rsi
   8dcad:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8dcb2:      	movl	$0x1, %edi
   8dcb7:      	callq	0xb45a0 <m34_container_end>
   8dcbc:      	movq	%r15, %rdi
   8dcbf:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8dcc4:      	movq	-0x30(%rbp), %rbx
   8dcc8:      	movq	%r14, %rdi
   8dccb:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8dcd0:      	movq	%rbx, -0x30(%rbp)
   8dcd4:      	leaq	-0x30(%rbp), %rax
   8dcd8:      	movq	%rax, -0xd8(%rbp)
   8dcdf:      	leaq	0x16e4ba(%rip), %rax    # 0x1fc1a0 <scoop$1$bs$bbe2b5508974aa89339bcd35ca3ef528fada9ae570265f72cf96c525c4d95384>
   8dce6:      	movq	%rax, -0xd0(%rbp)
   8dced:      	xorps	%xmm0, %xmm0
   8dcf0:      	movups	%xmm0, -0x1c8(%rbp)
   8dcf7:      	movq	$0x0, -0x1b8(%rbp)
   8dd02:      	leaq	-0x1c8(%rbp), %r14
   8dd09:      	leaq	-0xd8(%rbp), %rsi
   8dd10:      	movl	$0x1, %edx
   8dd15:      	movq	%r14, %rdi
   8dd18:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8dd1d:      	xorps	%xmm0, %xmm0
   8dd20:      	movups	%xmm0, -0x390(%rbp)
   8dd27:      	movups	%xmm0, -0x380(%rbp)
   8dd2e:      	movups	%xmm0, -0x370(%rbp)
   8dd35:      	movups	%xmm0, -0x360(%rbp)
   8dd3c:      	leaq	-0x390(%rbp), %r15
   8dd43:      	movq	%r15, %rdi
   8dd46:      	movq	%r15, %rsi
   8dd49:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8dd4e:      	callq	0xb4500 <m34_container_begin>
   8dd53:      	movq	%r15, %rdi
   8dd56:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8dd5b:      	movq	-0x30(%rbp), %rbx
   8dd5f:      	movq	%r14, %rdi
   8dd62:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8dd67:      	movq	%rbx, -0x30(%rbp)
   8dd6b:      	xorl	%r14d, %r14d
   8dd6e:      	xorl	%r15d, %r15d
   8dd71:      	nopw	%cs:(%rax,%rax)
   8dd80:      	leaq	0x1d6889(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   8dd87:      	movq	(%rax), %rax
   8dd8a:      	leaq	0x1d6887(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   8dd91:      	movl	(%rcx), %esi
   8dd93:      	movl	(%r12), %edx
   8dd97:      	movq	0x8(%r12), %rcx
   8dd9c:      	testl	%esi, %esi
   8dd9e:      	jne	0x8ddaa <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x69a>
   8dda0:      	cmpl	$0x1, %edx
   8dda3:      	jne	0x8ddaa <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x69a>
   8dda5:      	cmpq	%rax, %rcx
   8dda8:      	je	0x8ddbf <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x6af>
   8ddaa:      	movq	-0x30(%rbp), %rax
   8ddae:      	movq	%rax, -0x38(%rbp)
   8ddb2:      	callq	0x9b0c0 <scoop_rt_safepoint>
   8ddb7:      	movq	-0x38(%rbp), %rax
   8ddbb:      	movq	%rax, -0x30(%rbp)
   8ddbf:      	cmpq	%r13, %r15
   8ddc2:      	jge	0x8de28 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x718>
   8ddc4:      	movq	-0x30(%rbp), %rdi
   8ddc8:      	movq	%rdi, -0x98(%rbp)
   8ddcf:      	movq	%rdi, -0x38(%rbp)
   8ddd3:      	movq	%r15, %rsi
   8ddd6:      	callq	0x80e20 <scoop$1$cb$cebee366d69cd694b67be107c5ec7b451903ea78ff4b935558c3573b7d72dfe1>
   8dddb:      	movq	-0x38(%rbp), %rcx
   8dddf:      	movq	%rcx, -0x30(%rbp)
   8dde3:      	movq	%rcx, -0x98(%rbp)
   8ddea:      	movq	%rax, -0x70(%rbp)
   8ddee:      	movq	-0x70(%rbp), %rax
   8ddf2:      	movq	-0x70(%rbp), %rdi
   8ddf6:      	movq	%rdi, -0x78(%rbp)
   8ddfa:      	movq	-0x30(%rbp), %rax
   8ddfe:      	movq	(%rdx), %rcx
   8de01:      	movq	%rax, -0x38(%rbp)
   8de05:      	movq	%rdi, -0x40(%rbp)
   8de09:      	callq	*%rcx
   8de0b:      	movq	-0x40(%rbp), %rcx
   8de0f:      	movq	-0x38(%rbp), %rdx
   8de13:      	movq	%rdx, -0x30(%rbp)
   8de17:      	movq	%rcx, -0x78(%rbp)
   8de1b:      	cltq
   8de1d:      	addq	%rax, %r14
   8de20:      	incq	%r15
   8de23:      	jmp	0x8dd80 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x670>
   8de28:      	leaq	-0x30(%rbp), %rax
   8de2c:      	movq	%rax, -0xe8(%rbp)
   8de33:      	leaq	0x16e376(%rip), %rax    # 0x1fc1b0 <scoop$1$bs$6ac7501ee1ec08f3b5df91b4d802fe29ce42c93f07f61fa3f910073972d2dde7>
   8de3a:      	movq	%rax, -0xe0(%rbp)
   8de41:      	xorps	%xmm0, %xmm0
   8de44:      	movups	%xmm0, -0x1e0(%rbp)
   8de4b:      	movq	$0x0, -0x1d0(%rbp)
   8de56:      	leaq	-0x1e0(%rbp), %r15
   8de5d:      	leaq	-0xe8(%rbp), %rsi
   8de64:      	movl	$0x1, %edx
   8de69:      	movq	%r15, %rdi
   8de6c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8de71:      	xorps	%xmm0, %xmm0
   8de74:      	movups	%xmm0, -0x3d0(%rbp)
   8de7b:      	movups	%xmm0, -0x3c0(%rbp)
   8de82:      	movups	%xmm0, -0x3b0(%rbp)
   8de89:      	movups	%xmm0, -0x3a0(%rbp)
   8de90:      	leaq	-0x3d0(%rbp), %r12
   8de97:      	movq	%r12, %rdi
   8de9a:      	movq	%r12, %rsi
   8de9d:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8dea2:      	movl	$0x2, %edi
   8dea7:      	callq	0xb45a0 <m34_container_end>
   8deac:      	movq	%r12, %rdi
   8deaf:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8deb4:      	movq	-0x30(%rbp), %rbx
   8deb8:      	movq	%r15, %rdi
   8debb:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8dec0:      	movq	%rbx, -0x30(%rbp)
   8dec4:      	leaq	-0x1(%r13), %rax
   8dec8:      	imulq	%r13, %rax
   8decc:      	movq	%rax, %rcx
   8decf:      	shrq	$0x3f, %rcx
   8ded3:      	addq	%rax, %rcx
   8ded6:      	sarq	%rcx
   8ded9:      	xorl	%edi, %edi
   8dedb:      	cmpq	%rcx, %r14
   8dede:      	sete	%dil
   8dee2:      	movq	-0x30(%rbp), %rax
   8dee6:      	movq	%rax, -0x38(%rbp)
   8deea:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8deef:      	movq	-0x38(%rbp), %rax
   8def3:      	movq	%rax, -0x30(%rbp)
   8def7:      	leaq	-0x30(%rbp), %r12
   8defb:      	movq	%r12, -0xf8(%rbp)
   8df02:      	leaq	0x16e2b7(%rip), %rax    # 0x1fc1c0 <scoop$1$bs$5a427326e8e8bc3c2113ad4359649ef96b2e4a77c8e1bcf61a98dc0c2f9b2467>
   8df09:      	movq	%rax, -0xf0(%rbp)
   8df10:      	xorps	%xmm0, %xmm0
   8df13:      	movups	%xmm0, -0x1f8(%rbp)
   8df1a:      	movq	$0x0, -0x1e8(%rbp)
   8df25:      	leaq	-0x1f8(%rbp), %rbx
   8df2c:      	leaq	-0xf8(%rbp), %rsi
   8df33:      	movl	$0x1, %edx
   8df38:      	movq	%rbx, %rdi
   8df3b:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8df40:      	xorps	%xmm0, %xmm0
   8df43:      	movups	%xmm0, -0x410(%rbp)
   8df4a:      	movups	%xmm0, -0x400(%rbp)
   8df51:      	movups	%xmm0, -0x3f0(%rbp)
   8df58:      	movups	%xmm0, -0x3e0(%rbp)
   8df5f:      	leaq	-0x410(%rbp), %r15
   8df66:      	movq	%r15, %rdi
   8df69:      	movq	%r15, %rsi
   8df6c:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8df71:      	callq	0xb4500 <m34_container_begin>
   8df76:      	movq	%r15, %rdi
   8df79:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8df7e:      	movq	-0x30(%rbp), %r15
   8df82:      	movq	%rbx, %rdi
   8df85:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8df8a:      	movq	%r15, -0x30(%rbp)
   8df8e:      	movq	%r15, -0x38(%rbp)
   8df92:      	movq	%r15, %rdi
   8df95:      	callq	0x7ff10 <scoop$1$cb$19e03488b99cf6692816ef3ef3377590a052bab440cde4d03fe6a32ee9cd6741>
   8df9a:      	movq	-0x38(%rbp), %rax
   8df9e:      	movq	%rax, -0x30(%rbp)
   8dfa2:      	movq	%rax, -0x130(%rbp)
   8dfa9:      	movq	%r12, -0x108(%rbp)
   8dfb0:      	leaq	0x16e219(%rip), %rax    # 0x1fc1d0 <scoop$1$bs$75e66033ce28459377674b8cff0c548e7c8ad30d9cadcc03ec3ed6986dbdb0a0>
   8dfb7:      	movq	%rax, -0x100(%rbp)
   8dfbe:      	xorps	%xmm0, %xmm0
   8dfc1:      	movups	%xmm0, -0x210(%rbp)
   8dfc8:      	movq	$0x0, -0x200(%rbp)
   8dfd3:      	leaq	-0x210(%rbp), %rbx
   8dfda:      	leaq	-0x108(%rbp), %rsi
   8dfe1:      	movl	$0x1, %edx
   8dfe6:      	movq	%rbx, %rdi
   8dfe9:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   8dfee:      	xorps	%xmm0, %xmm0
   8dff1:      	movups	%xmm0, -0x450(%rbp)
   8dff8:      	movups	%xmm0, -0x440(%rbp)
   8dfff:      	movups	%xmm0, -0x430(%rbp)
   8e006:      	movups	%xmm0, -0x420(%rbp)
   8e00d:      	leaq	-0x450(%rbp), %r15
   8e014:      	movq	%r15, %rdi
   8e017:      	movq	%r15, %rsi
   8e01a:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   8e01f:      	movl	$0x3, %edi
   8e024:      	callq	0xb45a0 <m34_container_end>
   8e029:      	movq	%r15, %rdi
   8e02c:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   8e031:      	movq	-0x30(%rbp), %r15
   8e035:      	movq	%rbx, %rdi
   8e038:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8e03d:      	movq	%r15, -0x30(%rbp)
   8e041:      	xorl	%edi, %edi
   8e043:      	cmpq	$0x0, 0x18(%r15)
   8e048:      	sete	%dil
   8e04c:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8e051:      	movq	%r14, %rax
   8e054:      	addq	$0x428, %rsp            # imm = 0x428
   8e05b:      	popq	%rbx
   8e05c:      	popq	%r12
   8e05e:      	popq	%r13
   8e060:      	popq	%r14
   8e062:      	popq	%r15
   8e064:      	popq	%rbp
   8e065:      	retq

; zeroSizeList, MIR fn6, LIR scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0

/home/chenxu/repos/scoop/tmp/m34/containers-off-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

0000000000080730 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0>:
   80730:      	pushq	%rbp
   80731:      	movq	%rsp, %rbp
   80734:      	pushq	%r15
   80736:      	pushq	%r14
   80738:      	pushq	%r13
   8073a:      	pushq	%r12
   8073c:      	pushq	%rbx
   8073d:      	subq	$0x318, %rsp            # imm = 0x318
   80744:      	movq	%rdi, %rbx
   80747:      	movq	%fs:0x0, %rax
   80750:      	leaq	-0x10(%rax), %rax
   80757:      	movq	(%rax), %r12
   8075a:      	leaq	0x1e3eaf(%rip), %rax    # 0x264610 <scoop_thread_gc_epoch>
   80761:      	movq	(%rax), %rax
   80764:      	leaq	0x1e3ead(%rip), %rcx    # 0x264618 <scoop_thread_world_phase>
   8076b:      	movl	(%rcx), %esi
   8076d:      	movl	(%r12), %edx
   80771:      	movq	0x8(%r12), %rcx
   80776:      	testl	%esi, %esi
   80778:      	jne	0x80784 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x54>
   8077a:      	cmpl	$0x1, %edx
   8077d:      	jne	0x80784 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x54>
   8077f:      	cmpq	%rax, %rcx
   80782:      	je	0x80789 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x59>
   80784:      	callq	0x9b0c0 <scoop_rt_safepoint>
   80789:      	callq	0x8f310 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   8078e:      	movq	0x1e3e0b(%rip), %rax    # 0x2645a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   80795:      	movq	$0x0, 0x10(%rax)
   8079d:      	xorps	%xmm0, %xmm0
   807a0:      	movups	%xmm0, -0x120(%rbp)
   807a7:      	movq	$0x0, -0x110(%rbp)
   807b2:      	leaq	-0x120(%rbp), %r14
   807b9:      	movq	%r14, %rdi
   807bc:      	xorl	%esi, %esi
   807be:      	xorl	%edx, %edx
   807c0:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   807c5:      	xorps	%xmm0, %xmm0
   807c8:      	movups	%xmm0, -0x1c0(%rbp)
   807cf:      	movups	%xmm0, -0x1b0(%rbp)
   807d6:      	movups	%xmm0, -0x1a0(%rbp)
   807dd:      	movups	%xmm0, -0x190(%rbp)
   807e4:      	leaq	-0x1c0(%rbp), %r15
   807eb:      	movq	%r15, %rdi
   807ee:      	movq	%r15, %rsi
   807f1:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   807f6:      	callq	0xb4500 <m34_container_begin>
   807fb:      	movq	%r15, %rdi
   807fe:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80803:      	movq	%r14, %rdi
   80806:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   8080b:      	movq	0x194b36(%rip), %r14    # 0x215348 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5+0x18>
   80812:      	movq	%r14, %r13
   80815:      	negq	%r13
   80818:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   80822:      	addq	%r14, %rax
   80825:      	cmpq	$-0x20, %rax
   80829:      	setae	-0x39(%rbp)
   8082d:      	leaq	0x1f(%r14), %r15
   80831:      	andq	%r13, %r15
   80834:      	movq	%fs:0x0, %rax
   8083d:      	leaq	-0x8(%rax), %rax
   80844:      	movq	(%rax), %rax
   80847:      	movq	(%rax), %rcx
   8084a:      	addq	%rcx, %r14
   8084d:      	decq	%r14
   80850:      	andq	%r13, %r14
   80853:      	leaq	(%r14,%r15), %rcx
   80857:      	testq	%r14, %r14
   8085a:      	sete	%dl
   8085d:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   80864:      	setae	%sil
   80868:      	cmpq	0x8(%rax), %rcx
   8086c:      	seta	%dil
   80870:      	cmpq	$0x0, 0x194b48(%rip)    # 0x2153c0 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5+0x90>
   80878:      	setne	%r8b
   8087c:      	orb	%sil, %r8b
   8087f:      	orb	%dl, %r8b
   80882:      	orb	-0x39(%rbp), %r8b
   80886:      	orb	%dil, %r8b
   80889:      	jne	0x808a2 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x172>
   8088b:      	movq	%rcx, (%rax)
   8088e:      	leaq	0x194a9b(%rip), %rsi    # 0x215330 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5>
   80895:      	movq	%r14, %rdi
   80898:      	movq	%r15, %rdx
   8089b:      	callq	0x9fe20 <scoop_runtime_finish_tlab_alloc>
   808a0:      	jmp	0x808b6 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x186>
   808a2:      	leaq	0x194a87(%rip), %rdi    # 0x215330 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5>
   808a9:      	movl	$0x20, %esi
   808ae:      	callq	0x9b100 <scoop_runtime_alloc_slow>
   808b3:      	movq	%rax, %r14
   808b6:      	movq	%r14, -0x38(%rbp)
   808ba:      	movq	%r14, %rdi
   808bd:      	movq	%rbx, %rsi
   808c0:      	callq	0x83fd0 <scoop$1$cb$bbf2b48d44d7bedf22a7d36826589ee9c8d458b9388e1546c16bcf3e309cc777>
   808c5:      	movq	-0x38(%rbp), %rax
   808c9:      	movq	%rax, -0x108(%rbp)
   808d0:      	movq	%rax, -0x30(%rbp)
   808d4:      	leaq	-0x30(%rbp), %r13
   808d8:      	movq	%r13, -0x68(%rbp)
   808dc:      	leaq	0x15a6ed(%rip), %rax    # 0x1dafd0 <scoop$1$bs$7d75ca21523a0411697697df983d83243c69e84e9136fc1a69a60a7e81dbd4b0>
   808e3:      	movq	%rax, -0x60(%rbp)
   808e7:      	xorps	%xmm0, %xmm0
   808ea:      	movups	%xmm0, -0xc0(%rbp)
   808f1:      	movq	$0x0, -0xb0(%rbp)
   808fc:      	leaq	-0xc0(%rbp), %rdi
   80903:      	leaq	-0x68(%rbp), %rsi
   80907:      	movl	$0x1, %edx
   8090c:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   80911:      	xorps	%xmm0, %xmm0
   80914:      	movups	%xmm0, -0x200(%rbp)
   8091b:      	movups	%xmm0, -0x1f0(%rbp)
   80922:      	movups	%xmm0, -0x1e0(%rbp)
   80929:      	movups	%xmm0, -0x1d0(%rbp)
   80930:      	leaq	-0x200(%rbp), %r15
   80937:      	movq	%r15, %rdi
   8093a:      	movq	%r15, %rsi
   8093d:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   80942:      	xorl	%r14d, %r14d
   80945:      	xorl	%edi, %edi
   80947:      	callq	0xb45a0 <m34_container_end>
   8094c:      	movq	%r15, %rdi
   8094f:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80954:      	movq	-0x30(%rbp), %r15
   80958:      	leaq	-0xc0(%rbp), %rdi
   8095f:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80964:      	movq	%r15, -0x30(%rbp)
   80968:      	movq	%r15, -0x50(%rbp)
   8096c:      	movq	%r13, -0xf8(%rbp)
   80973:      	leaq	0x15a666(%rip), %rax    # 0x1dafe0 <scoop$1$bs$f0296fc550f67d3cf183664287380d188ed2bd1c75965f8c503aa632c18ff3da>
   8097a:      	movq	%rax, -0xf0(%rbp)
   80981:      	leaq	-0x50(%rbp), %rax
   80985:      	movq	%rax, -0xe8(%rbp)
   8098c:      	leaq	0x15a65d(%rip), %rax    # 0x1daff0 <scoop$1$bs$4fa5314fc1a6bd85b6345a7964f95cd9eb9cb0eb910f0a7618c4c86eb13216e3>
   80993:      	movq	%rax, -0xe0(%rbp)
   8099a:      	xorps	%xmm0, %xmm0
   8099d:      	movups	%xmm0, -0xd8(%rbp)
   809a4:      	movq	$0x0, -0xc8(%rbp)
   809af:      	leaq	-0xd8(%rbp), %rdi
   809b6:      	leaq	-0xf8(%rbp), %rsi
   809bd:      	movl	$0x2, %edx
   809c2:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   809c7:      	xorps	%xmm0, %xmm0
   809ca:      	movups	%xmm0, -0x240(%rbp)
   809d1:      	movups	%xmm0, -0x230(%rbp)
   809d8:      	movups	%xmm0, -0x220(%rbp)
   809df:      	movups	%xmm0, -0x210(%rbp)
   809e6:      	leaq	-0x240(%rbp), %r15
   809ed:      	movq	%r15, %rdi
   809f0:      	movq	%r15, %rsi
   809f3:      	callq	0x9b2e0 <scoop_rt_enter_native_borrowed>
   809f8:      	movq	-0x50(%rbp), %rdi
   809fc:      	callq	0xb46d0 <m34_container_layout>
   80a01:      	movq	%r15, %rdi
   80a04:      	callq	0x9dc70 <scoop_rt_leave_native_borrowed>
   80a09:      	movq	-0x30(%rbp), %r15
   80a0d:      	movq	-0x50(%rbp), %r13
   80a11:      	leaq	-0xd8(%rbp), %rdi
   80a18:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80a1d:      	movq	%r15, -0x30(%rbp)
   80a21:      	movq	%r13, -0x50(%rbp)
   80a25:      	leaq	-0x30(%rbp), %rax
   80a29:      	movq	%rax, -0x78(%rbp)
   80a2d:      	leaq	0x15a5cc(%rip), %rax    # 0x1db000 <scoop$1$bs$288a2f95f5369e48a3a4bf0a80dd60640c9f5bb7412b16b7bffa8bed09136e14>
   80a34:      	movq	%rax, -0x70(%rbp)
   80a38:      	xorps	%xmm0, %xmm0
   80a3b:      	movups	%xmm0, -0x138(%rbp)
   80a42:      	movq	$0x0, -0x128(%rbp)
   80a4d:      	leaq	-0x138(%rbp), %r13
   80a54:      	leaq	-0x78(%rbp), %rsi
   80a58:      	movl	$0x1, %edx
   80a5d:      	movq	%r13, %rdi
   80a60:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   80a65:      	xorps	%xmm0, %xmm0
   80a68:      	movups	%xmm0, -0x280(%rbp)
   80a6f:      	movups	%xmm0, -0x270(%rbp)
   80a76:      	movups	%xmm0, -0x260(%rbp)
   80a7d:      	movups	%xmm0, -0x250(%rbp)
   80a84:      	leaq	-0x280(%rbp), %r15
   80a8b:      	movq	%r15, %rdi
   80a8e:      	movq	%r15, %rsi
   80a91:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   80a96:      	callq	0xb4500 <m34_container_begin>
   80a9b:      	movq	%r15, %rdi
   80a9e:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80aa3:      	movq	-0x30(%rbp), %r15
   80aa7:      	movq	%r13, %rdi
   80aaa:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80aaf:      	movq	%r15, -0x30(%rbp)
   80ab3:      	leaq	0x1e3b56(%rip), %r15    # 0x264610 <scoop_thread_gc_epoch>
   80aba:      	leaq	0x1e3b57(%rip), %r13    # 0x264618 <scoop_thread_world_phase>
   80ac1:      	nopw	%cs:(%rax,%rax)
   80ad0:      	movq	(%r15), %rax
   80ad3:      	movl	(%r13), %esi
   80ad7:      	movl	(%r12), %edx
   80adb:      	movq	0x8(%r12), %rcx
   80ae0:      	testl	%esi, %esi
   80ae2:      	jne	0x80aee <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3be>
   80ae4:      	cmpl	$0x1, %edx
   80ae7:      	jne	0x80aee <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3be>
   80ae9:      	cmpq	%rax, %rcx
   80aec:      	je	0x80b03 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3d3>
   80aee:      	movq	-0x30(%rbp), %rax
   80af2:      	movq	%rax, -0x38(%rbp)
   80af6:      	callq	0x9b0c0 <scoop_rt_safepoint>
   80afb:      	movq	-0x38(%rbp), %rax
   80aff:      	movq	%rax, -0x30(%rbp)
   80b03:      	cmpq	%rbx, %r14
   80b06:      	jge	0x80b65 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x435>
   80b08:      	movq	-0x30(%rbp), %rax
   80b0c:      	movq	%rax, -0x58(%rbp)
   80b10:      	movq	%rax, -0x38(%rbp)
   80b14:      	movq	%rax, -0x48(%rbp)
   80b18:      	callq	0x8f310 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   80b1d:      	movq	-0x48(%rbp), %rax
   80b21:      	movq	-0x38(%rbp), %rcx
   80b25:      	movq	%rcx, -0x30(%rbp)
   80b29:      	movq	%rax, -0x58(%rbp)
   80b2d:      	movq	0x1e3a6c(%rip), %rax    # 0x2645a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   80b34:      	incq	0x10(%rax)
   80b38:      	movq	-0x30(%rbp), %rax
   80b3c:      	movq	-0x58(%rbp), %rdi
   80b40:      	movq	%rax, -0x38(%rbp)
   80b44:      	movq	%rdi, -0x48(%rbp)
   80b48:      	callq	0x82bd0 <scoop$1$cb$8003ec207e511adbb97d22eb90432efef709dc7ae9e8ead9789fb9fb51659730>
   80b4d:      	movq	-0x48(%rbp), %rax
   80b51:      	movq	-0x38(%rbp), %rcx
   80b55:      	movq	%rcx, -0x30(%rbp)
   80b59:      	movq	%rax, -0x58(%rbp)
   80b5d:      	incq	%r14
   80b60:      	jmp	0x80ad0 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3a0>
   80b65:      	leaq	-0x30(%rbp), %r12
   80b69:      	movq	%r12, -0x88(%rbp)
   80b70:      	leaq	0x15a499(%rip), %rax    # 0x1db010 <scoop$1$bs$6fc0eb5b3ed64bb375b12533b5b96bdb30dd133bd066b2a1f2682261ae14df2f>
   80b77:      	movq	%rax, -0x80(%rbp)
   80b7b:      	xorps	%xmm0, %xmm0
   80b7e:      	movups	%xmm0, -0x150(%rbp)
   80b85:      	movq	$0x0, -0x140(%rbp)
   80b90:      	leaq	-0x150(%rbp), %r14
   80b97:      	leaq	-0x88(%rbp), %rsi
   80b9e:      	movl	$0x1, %edx
   80ba3:      	movq	%r14, %rdi
   80ba6:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   80bab:      	xorps	%xmm0, %xmm0
   80bae:      	movups	%xmm0, -0x2c0(%rbp)
   80bb5:      	movups	%xmm0, -0x2b0(%rbp)
   80bbc:      	movups	%xmm0, -0x2a0(%rbp)
   80bc3:      	movups	%xmm0, -0x290(%rbp)
   80bca:      	leaq	-0x2c0(%rbp), %r15
   80bd1:      	movq	%r15, %rdi
   80bd4:      	movq	%r15, %rsi
   80bd7:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   80bdc:      	movl	$0x1, %edi
   80be1:      	callq	0xb45a0 <m34_container_end>
   80be6:      	movq	%r15, %rdi
   80be9:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80bee:      	movq	-0x30(%rbp), %r15
   80bf2:      	movq	%r14, %rdi
   80bf5:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80bfa:      	movq	%r15, -0x30(%rbp)
   80bfe:      	cmpq	%rbx, 0x18(%r15)
   80c02:      	jne	0x80c25 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x4f5>
   80c04:      	movq	%r15, -0x38(%rbp)
   80c08:      	callq	0x8f310 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   80c0d:      	movq	-0x38(%rbp), %rax
   80c11:      	movq	%rax, -0x30(%rbp)
   80c15:      	movq	0x1e3984(%rip), %rax    # 0x2645a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   80c1c:      	cmpq	%rbx, 0x10(%rax)
   80c20:      	sete	%al
   80c23:      	jmp	0x80c27 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x4f7>
   80c25:      	xorl	%eax, %eax
   80c27:      	movq	-0x30(%rbp), %rcx
   80c2b:      	movq	%rcx, -0x38(%rbp)
   80c2f:      	movzbl	%al, %edi
   80c32:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   80c37:      	movq	-0x38(%rbp), %rax
   80c3b:      	movq	%rax, -0x30(%rbp)
   80c3f:      	movq	%r12, -0x98(%rbp)
   80c46:      	leaq	0x15a3d3(%rip), %rax    # 0x1db020 <scoop$1$bs$357f844c2360310b03a9ba68e1ec0f90aa3b34e8fe601e2d1b7840a869f85351>
   80c4d:      	movq	%rax, -0x90(%rbp)
   80c54:      	xorps	%xmm0, %xmm0
   80c57:      	movups	%xmm0, -0x168(%rbp)
   80c5e:      	movq	$0x0, -0x158(%rbp)
   80c69:      	leaq	-0x168(%rbp), %rbx
   80c70:      	leaq	-0x98(%rbp), %rsi
   80c77:      	movl	$0x1, %edx
   80c7c:      	movq	%rbx, %rdi
   80c7f:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   80c84:      	xorps	%xmm0, %xmm0
   80c87:      	movups	%xmm0, -0x300(%rbp)
   80c8e:      	movups	%xmm0, -0x2f0(%rbp)
   80c95:      	movups	%xmm0, -0x2e0(%rbp)
   80c9c:      	movups	%xmm0, -0x2d0(%rbp)
   80ca3:      	leaq	-0x300(%rbp), %r14
   80caa:      	movq	%r14, %rdi
   80cad:      	movq	%r14, %rsi
   80cb0:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   80cb5:      	callq	0xb4500 <m34_container_begin>
   80cba:      	movq	%r14, %rdi
   80cbd:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80cc2:      	movq	-0x30(%rbp), %r14
   80cc6:      	movq	%rbx, %rdi
   80cc9:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80cce:      	movq	%r14, -0x30(%rbp)
   80cd2:      	movq	%r14, -0x38(%rbp)
   80cd6:      	movq	%r14, %rdi
   80cd9:      	callq	0x826c0 <scoop$1$cb$c1d741454a2cf8c4cf47c4b7548c8fefcac986a4f724129eabf52dcdcb0ccd30>
   80cde:      	movq	-0x38(%rbp), %rax
   80ce2:      	movq	%rax, -0x30(%rbp)
   80ce6:      	movq	%rax, -0x100(%rbp)
   80ced:      	movq	%r12, -0xa8(%rbp)
   80cf4:      	leaq	0x15a335(%rip), %rax    # 0x1db030 <scoop$1$bs$cf9a169a8d86a7c86a7857448ab14c1b66534c2d7e7663b7b3eaba383056c69a>
   80cfb:      	movq	%rax, -0xa0(%rbp)
   80d02:      	xorps	%xmm0, %xmm0
   80d05:      	movups	%xmm0, -0x180(%rbp)
   80d0c:      	movq	$0x0, -0x170(%rbp)
   80d17:      	leaq	-0x180(%rbp), %rbx
   80d1e:      	leaq	-0xa8(%rbp), %rsi
   80d25:      	movl	$0x1, %edx
   80d2a:      	movq	%rbx, %rdi
   80d2d:      	callq	0xa9f40 <scoop_rt_push_caller_roots>
   80d32:      	xorps	%xmm0, %xmm0
   80d35:      	movups	%xmm0, -0x340(%rbp)
   80d3c:      	movups	%xmm0, -0x330(%rbp)
   80d43:      	movups	%xmm0, -0x320(%rbp)
   80d4a:      	movups	%xmm0, -0x310(%rbp)
   80d51:      	leaq	-0x340(%rbp), %r14
   80d58:      	movq	%r14, %rdi
   80d5b:      	movq	%r14, %rsi
   80d5e:      	callq	0x9b2c0 <scoop_rt_enter_native_safe>
   80d63:      	movl	$0x3, %edi
   80d68:      	callq	0xb45a0 <m34_container_end>
   80d6d:      	movq	%r14, %rdi
   80d70:      	callq	0x9dc30 <scoop_rt_leave_native_safe>
   80d75:      	movq	-0x30(%rbp), %r14
   80d79:      	movq	%rbx, %rdi
   80d7c:      	callq	0xa9ff0 <scoop_rt_pop_caller_roots>
   80d81:      	movq	%r14, -0x30(%rbp)
   80d85:      	xorl	%edi, %edi
   80d87:      	cmpq	$0x0, 0x18(%r14)
   80d8c:      	sete	%dil
   80d90:      	callq	0x843c0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   80d95:      	callq	0x8f310 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   80d9a:      	movq	0x1e37ff(%rip), %rax    # 0x2645a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   80da1:      	movq	0x10(%rax), %rax
   80da5:      	addq	$0x318, %rsp            # imm = 0x318
   80dac:      	popq	%rbx
   80dad:      	popq	%r12
   80daf:      	popq	%r13
   80db1:      	popq	%r14
   80db3:      	popq	%r15
   80db5:      	popq	%rbp
   80db6:      	retq
