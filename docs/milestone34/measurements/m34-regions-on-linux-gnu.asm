
/home/chenxu/repos/scoop/tmp/m34/regions-on-linux-gnu/region-stores:	file format elf64-x86-64

Disassembly of section .text:

000000000005b0e0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   5b0e0:      	pushq	%rbp
   5b0e1:      	movq	%rsp, %rbp
   5b0e4:      	pushq	%r15
   5b0e6:      	pushq	%r14
   5b0e8:      	pushq	%r13
   5b0ea:      	pushq	%r12
   5b0ec:      	pushq	%rbx
   5b0ed:      	subq	$0x68, %rsp
   5b0f1:      	movq	%fs:0x0, %rax
   5b0fa:      	leaq	-0x10(%rax), %rax
   5b101:      	movq	(%rax), %r15
   5b104:      	leaq	, %rax <scoop_thread_gc_epoch>
   5b10b:      	movq	(%rax), %rax
   5b10e:      	leaq	, %rcx <scoop_thread_world_phase>
   5b115:      	movl	(%rcx), %esi
   5b117:      	movl	(%r15), %edx
   5b11a:      	movq	0x8(%r15), %rcx
   5b11e:      	testl	%esi, %esi
   5b120:      	jne	 <L0>
   5b122:      	cmpl	$0x1, %edx
   5b125:      	jne	 <L0>
   5b127:      	cmpq	%rax, %rcx
   5b12a:      	je	 <L1>
<L0>:
   5b12c:      	callq	 <scoop_rt_safepoint>
<L1>:
   5b131:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5b13b:      	movq	$0x0, -0x80(%rbp)
   5b143:      	movq	, %r14 <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5b14a:      	movq	%r14, %r13
   5b14d:      	negq	%r13
   5b150:      	addq	%r14, %rax
   5b153:      	movq	%rax, -0x60(%rbp)
   5b157:      	cmpq	$-0x20, %rax
   5b15b:      	setae	%r12b
   5b15f:      	leaq	0x1f(%r14), %rbx
   5b163:      	andq	%r13, %rbx
   5b166:      	movq	%fs:0x0, %rax
   5b16f:      	leaq	-0x8(%rax), %rax
   5b176:      	movq	(%rax), %rax
   5b179:      	movq	(%rax), %rcx
   5b17c:      	addq	%rcx, %r14
   5b17f:      	decq	%r14
   5b182:      	movq	%r13, -0x68(%rbp)
   5b186:      	andq	%r13, %r14
   5b189:      	leaq	(%r14,%rbx), %rcx
   5b18d:      	testq	%r14, %r14
   5b190:      	sete	%dl
   5b193:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5b19a:      	setae	%sil
   5b19e:      	cmpq	0x8(%rax), %rcx
   5b1a2:      	seta	%dil
   5b1a6:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5b1ae:      	setne	%r8b
   5b1b2:      	orb	%sil, %r8b
   5b1b5:      	orb	%dl, %r8b
   5b1b8:      	orb	%r12b, %r8b
   5b1bb:      	orb	%dil, %r8b
   5b1be:      	jne	 <L2>
   5b1c0:      	movq	%rcx, (%rax)
   5b1c3:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b1ca:      	movq	%r14, %rdi
   5b1cd:      	movq	%rbx, %rdx
   5b1d0:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5b1d5:      	jmp	 <L3>
<L2>:
   5b1d7:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b1de:      	movl	$0x20, %esi
   5b1e3:      	callq	 <scoop_runtime_alloc_slow>
   5b1e8:      	movq	%rax, %r14
   5b1eb:      	xorl	%eax, %eax
   5b1ed:      	movq	$0x0, -0x80(%rbp)
<L3>:
   5b1f5:      	movq	-0x80(%rbp), %rcx
   5b1f9:      	movq	$0x0, 0x10(%r14)
   5b201:      	leaq	0x18(%r14), %rax
   5b205:      	movq	%rcx, 0x18(%r14)
   5b209:      	movq	%rax, %rcx
   5b20c:      	shrq	$0x34, %rcx
   5b210:      	leaq	, %r12 <scoop_gc_page_map>
   5b217:      	movq	(%r12,%rcx,8), %rcx
   5b21b:      	movq	%rax, %rdx
   5b21e:      	shrq	$0x28, %rdx
   5b222:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b228:      	movq	(%rcx,%rdx,8), %rcx
   5b22c:      	movq	%rax, %rdx
   5b22f:      	shrq	$0x1c, %rdx
   5b233:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b239:      	movq	(%rcx,%rdx,8), %rcx
   5b23d:      	movl	%eax, %edx
   5b23f:      	shrl	$0xd, %edx
   5b242:      	andl	$0x7ff8, %edx           # imm = 0x7FF8
   5b248:      	movq	(%rcx,%rdx), %rcx
   5b24c:      	movq	0x10(%rcx), %rdx
   5b250:      	subq	(%rcx), %rax
   5b253:      	shrq	$0x9, %rax
   5b257:      	lock
   5b258:      	orb	$0x1, (%rdx,%rax)
   5b25c:      	movq	%r14, -0x38(%rbp)
   5b260:      	movq	$0x0, -0x70(%rbp)
   5b268:      	cmpq	$-0x20, -0x60(%rbp)
   5b26d:      	setb	%r13b
   5b271:      	movq	%fs:0x0, %rax
   5b27a:      	leaq	-0x8(%rax), %rax
   5b281:      	movq	(%rax), %rax
   5b284:      	movq	(%rax), %rcx
   5b287:      	movq	, %rdx <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5b28e:      	leaq	(%rdx,%rcx), %r14
   5b292:      	decq	%r14
   5b295:      	andq	-0x68(%rbp), %r14
   5b299:      	leaq	(%r14,%rbx), %rcx
   5b29d:      	testq	%r14, %r14
   5b2a0:      	setne	%dl
   5b2a3:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5b2aa:      	setb	%sil
   5b2ae:      	cmpq	0x8(%rax), %rcx
   5b2b2:      	setbe	%dil
   5b2b6:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5b2be:      	sete	%r8b
   5b2c2:      	andb	%sil, %r8b
   5b2c5:      	andb	%dl, %r8b
   5b2c8:      	andb	%dil, %r8b
   5b2cb:      	testb	%r13b, %r8b
   5b2ce:      	je	 <L4>
   5b2d0:      	movq	%rcx, (%rax)
   5b2d3:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b2da:      	movq	%r14, %rdi
   5b2dd:      	movq	%rbx, %rdx
   5b2e0:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5b2e5:      	jmp	 <L5>
<L4>:
   5b2e7:      	movq	-0x38(%rbp), %rax
   5b2eb:      	movq	%rax, -0x30(%rbp)
   5b2ef:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b2f6:      	movl	$0x20, %esi
   5b2fb:      	callq	 <scoop_runtime_alloc_slow>
   5b300:      	movq	%rax, %r14
   5b303:      	movq	-0x30(%rbp), %rax
   5b307:      	movq	%rax, -0x38(%rbp)
   5b30b:      	xorl	%eax, %eax
   5b30d:      	movq	$0x0, -0x70(%rbp)
<L5>:
   5b315:      	movq	-0x70(%rbp), %rcx
   5b319:      	movq	$0x7, 0x10(%r14)
   5b321:      	leaq	0x18(%r14), %rax
   5b325:      	movq	%rcx, 0x18(%r14)
   5b329:      	movq	%rax, %rcx
   5b32c:      	shrq	$0x34, %rcx
   5b330:      	movq	(%r12,%rcx,8), %rcx
   5b334:      	movq	%rax, %rdx
   5b337:      	shrq	$0x28, %rdx
   5b33b:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b341:      	movq	(%rcx,%rdx,8), %rcx
   5b345:      	movq	%rax, %rdx
   5b348:      	shrq	$0x1c, %rdx
   5b34c:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b352:      	movq	(%rcx,%rdx,8), %rcx
   5b356:      	movl	%eax, %edx
   5b358:      	shrl	$0xd, %edx
   5b35b:      	andl	$0x7ff8, %edx           # imm = 0x7FF8
   5b361:      	movq	(%rcx,%rdx), %rcx
   5b365:      	movq	0x10(%rcx), %rdx
   5b369:      	subq	(%rcx), %rax
   5b36c:      	shrq	$0x9, %rax
   5b370:      	lock
   5b371:      	orb	$0x1, (%rdx,%rax)
   5b375:      	movq	%r14, -0x48(%rbp)
   5b379:      	movq	$0x0, -0x78(%rbp)
   5b381:      	cmpq	$-0x20, -0x60(%rbp)
   5b386:      	setb	%r13b
   5b38a:      	movq	%fs:0x0, %rax
   5b393:      	leaq	-0x8(%rax), %rax
   5b39a:      	movq	(%rax), %rax
   5b39d:      	movq	(%rax), %rcx
   5b3a0:      	movq	, %rdx <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5b3a7:      	leaq	(%rdx,%rcx), %r14
   5b3ab:      	decq	%r14
   5b3ae:      	andq	-0x68(%rbp), %r14
   5b3b2:      	leaq	(%r14,%rbx), %rcx
   5b3b6:      	testq	%r14, %r14
   5b3b9:      	setne	%dl
   5b3bc:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5b3c3:      	setb	%sil
   5b3c7:      	cmpq	0x8(%rax), %rcx
   5b3cb:      	setbe	%dil
   5b3cf:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5b3d7:      	sete	%r8b
   5b3db:      	andb	%sil, %r8b
   5b3de:      	andb	%dl, %r8b
   5b3e1:      	andb	%dil, %r8b
   5b3e4:      	testb	%r13b, %r8b
   5b3e7:      	je	 <L6>
   5b3e9:      	movq	%rcx, (%rax)
   5b3ec:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b3f3:      	movq	%r14, %rdi
   5b3f6:      	movq	%rbx, %rdx
   5b3f9:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5b3fe:      	jmp	 <L7>
<L6>:
   5b400:      	movq	-0x48(%rbp), %rax
   5b404:      	movq	-0x38(%rbp), %rcx
   5b408:      	movq	%rcx, -0x30(%rbp)
   5b40c:      	movq	%rax, -0x40(%rbp)
   5b410:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5b417:      	movl	$0x20, %esi
   5b41c:      	callq	 <scoop_runtime_alloc_slow>
   5b421:      	movq	%rax, %r14
   5b424:      	movq	-0x40(%rbp), %rax
   5b428:      	movq	-0x30(%rbp), %rcx
   5b42c:      	movq	%rcx, -0x38(%rbp)
   5b430:      	movq	%rax, -0x48(%rbp)
   5b434:      	xorl	%eax, %eax
   5b436:      	movq	$0x0, -0x78(%rbp)
<L7>:
   5b43e:      	leaq	, %r13 <scoop_thread_world_phase>
   5b445:      	movq	-0x78(%rbp), %rcx
   5b449:      	movq	$0x9, 0x10(%r14)
   5b451:      	leaq	0x18(%r14), %rax
   5b455:      	movq	%rcx, 0x18(%r14)
   5b459:      	movq	%rax, %rcx
   5b45c:      	shrq	$0x34, %rcx
   5b460:      	movq	(%r12,%rcx,8), %rcx
   5b464:      	movq	%rax, %rdx
   5b467:      	shrq	$0x28, %rdx
   5b46b:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b471:      	movq	(%rcx,%rdx,8), %rcx
   5b475:      	movq	%rax, %rdx
   5b478:      	shrq	$0x1c, %rdx
   5b47c:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b482:      	movq	(%rcx,%rdx,8), %rcx
   5b486:      	movl	%eax, %edx
   5b488:      	shrl	$0xd, %edx
   5b48b:      	andl	$0x7ff8, %edx           # imm = 0x7FF8
   5b491:      	movq	(%rcx,%rdx), %rcx
   5b495:      	movq	0x10(%rcx), %rdx
   5b499:      	subq	(%rcx), %rax
   5b49c:      	shrq	$0x9, %rax
   5b4a0:      	lock
   5b4a1:      	orb	$0x1, (%rdx,%rax)
   5b4a5:      	movq	-0x38(%rbp), %rax
   5b4a9:      	movq	-0x48(%rbp), %rcx
   5b4ad:      	movq	%r14, -0x50(%rbp)
   5b4b1:      	movq	%rax, -0x30(%rbp)
   5b4b5:      	movq	%rcx, -0x40(%rbp)
   5b4b9:      	callq	 <scoop_rt_gc_collect>
   5b4be:      	movq	-0x50(%rbp), %rax
   5b4c2:      	movq	-0x40(%rbp), %rcx
   5b4c6:      	movq	-0x30(%rbp), %rdx
   5b4ca:      	movq	%rdx, -0x38(%rbp)
   5b4ce:      	movq	%rcx, -0x48(%rbp)
   5b4d2:      	movq	%rax, -0x58(%rbp)
   5b4d6:      	xorl	%ebx, %ebx
   5b4d8:      	leaq	, %r14 <scoop_thread_gc_epoch>
   5b4df:      	jmp	 <L10>
   5b4e1:      	nopw	%cs:(%rax,%rax)
<L8>:
   5b4f0:      	movq	-0x58(%rbp), %rcx
<L9>:
   5b4f4:      	movq	-0x38(%rbp), %rax
   5b4f8:      	movq	%rcx, 0x18(%rax)
   5b4fc:      	addq	$0x18, %rax
   5b500:      	movq	%rax, %rcx
   5b503:      	shrq	$0x34, %rcx
   5b507:      	movq	(%r12,%rcx,8), %rcx
   5b50b:      	movq	%rax, %rdx
   5b50e:      	shrq	$0x28, %rdx
   5b512:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b518:      	movq	(%rcx,%rdx,8), %rcx
   5b51c:      	movq	%rax, %rdx
   5b51f:      	shrq	$0x1c, %rdx
   5b523:      	andl	$0xfff, %edx            # imm = 0xFFF
   5b529:      	movq	(%rcx,%rdx,8), %rcx
   5b52d:      	movl	%eax, %edx
   5b52f:      	shrl	$0xd, %edx
   5b532:      	andl	$0x7ff8, %edx           # imm = 0x7FF8
   5b538:      	movq	(%rcx,%rdx), %rcx
   5b53c:      	movq	0x10(%rcx), %rdx
   5b540:      	subq	(%rcx), %rax
   5b543:      	shrq	$0x9, %rax
   5b547:      	lock
   5b548:      	orb	$0x1, (%rdx,%rax)
   5b54c:      	incq	%rbx
<L10>:
   5b54f:      	movq	(%r14), %rax
   5b552:      	movl	(%r13), %esi
   5b556:      	movl	(%r15), %edx
   5b559:      	movq	0x8(%r15), %rcx
   5b55d:      	testl	%esi, %esi
   5b55f:      	jne	 <L11>
   5b561:      	cmpl	$0x1, %edx
   5b564:      	jne	 <L11>
   5b566:      	cmpq	%rax, %rcx
   5b569:      	je	 <L12>
<L11>:
   5b56b:      	movq	-0x38(%rbp), %rax
   5b56f:      	movq	-0x48(%rbp), %rcx
   5b573:      	movq	-0x58(%rbp), %rdx
   5b577:      	movq	%rax, -0x30(%rbp)
   5b57b:      	movq	%rcx, -0x40(%rbp)
   5b57f:      	movq	%rdx, -0x50(%rbp)
   5b583:      	callq	 <scoop_rt_safepoint>
   5b588:      	movq	-0x50(%rbp), %rax
   5b58c:      	movq	-0x40(%rbp), %rcx
   5b590:      	movq	-0x30(%rbp), %rdx
   5b594:      	movq	%rdx, -0x38(%rbp)
   5b598:      	movq	%rcx, -0x48(%rbp)
   5b59c:      	movq	%rax, -0x58(%rbp)
<L12>:
   5b5a0:      	cmpq	$0x7a1200, %rbx         # imm = 0x7A1200
   5b5a7:      	jge	 <L13>
   5b5a9:      	testb	$0x1, %bl
   5b5ac:      	jne	 <L8>
   5b5b2:      	movq	-0x48(%rbp), %rcx
   5b5b6:      	jmp	 <L9>
<L13>:
   5b5bb:      	movq	-0x38(%rbp), %rax
   5b5bf:      	movq	0x18(%rax), %rax
   5b5c3:      	testq	%rax, %rax
   5b5c6:      	je	 <L14>
   5b5c8:      	movq	0x10(%rax), %rdi
   5b5cc:      	callq	 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   5b5d1:      	addq	$0x68, %rsp
   5b5d5:      	popq	%rbx
   5b5d6:      	popq	%r12
   5b5d8:      	popq	%r13
   5b5da:      	popq	%r14
   5b5dc:      	popq	%r15
   5b5de:      	popq	%rbp
   5b5df:      	retq
<L14>:
   5b5e0:      	movq	, %rbx <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3+0x18>
   5b5e7:      	movq	%rbx, %r15
   5b5ea:      	negq	%r15
   5b5ed:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5b5f7:      	addq	%rbx, %rax
   5b5fa:      	cmpq	$-0x18, %rax
   5b5fe:      	setb	%r12b
   5b602:      	leaq	0x17(%rbx), %r14
   5b606:      	andq	%r15, %r14
   5b609:      	movq	%fs:0x0, %rax
   5b612:      	leaq	-0x8(%rax), %rax
   5b619:      	movq	(%rax), %rax
   5b61c:      	movq	(%rax), %rcx
   5b61f:      	addq	%rcx, %rbx
   5b622:      	decq	%rbx
   5b625:      	andq	%r15, %rbx
   5b628:      	leaq	(%rbx,%r14), %rcx
   5b62c:      	testq	%rbx, %rbx
   5b62f:      	setne	%dl
   5b632:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5b639:      	setb	%sil
   5b63d:      	cmpq	0x8(%rax), %rcx
   5b641:      	setbe	%dil
   5b645:      	cmpq	$0x0,  <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3+0x90>
   5b64d:      	sete	%r8b
   5b651:      	andb	%sil, %r8b
   5b654:      	andb	%dl, %r8b
   5b657:      	andb	%dil, %r8b
   5b65a:      	testb	%r12b, %r8b
   5b65d:      	je	 <L15>
   5b65f:      	movq	%rcx, (%rax)
   5b662:      	leaq	, %rsi <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3>
   5b669:      	movq	%rbx, %rdi
   5b66c:      	movq	%r14, %rdx
   5b66f:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5b674:      	jmp	 <L16>
<L15>:
   5b676:      	leaq	, %rdi <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3>
   5b67d:      	movl	$0x18, %esi
   5b682:      	callq	 <scoop_runtime_alloc_slow>
   5b687:      	movq	%rax, %rbx
<L16>:
   5b68a:      	movq	%rbx, -0x30(%rbp)
   5b68e:      	movq	%rbx, %rdi
   5b691:      	callq	 <scoop$1$cb$a743fee7ba2831ad1852cbb01a2d484559872cb6aa9aa7bc66d5e03510bf8243>
   5b696:      	movq	-0x30(%rbp), %rdi
   5b69a:      	movq	%rdi, -0x88(%rbp)
   5b6a1:      	callq	 <scoop_rt_throw>

0000000000062960 <scoop_rt_gc_write_barrier>:
   62960:      	endbr64
   62964:      	pushq	%rbp
   62965:      	movq	%rdi, %rax
   62968:      	addq	%rsi, %rax
   6296b:      	movq	%rsp, %rbp
   6296e:      	pushq	%r12
   62970:      	pushq	%rbx
   62971:      	jb	 <L4>
   62973:      	movq	%rsi, %rbx
   62976:      	movq	%rdi, %r12
   62979:      	testq	%rsi, %rsi
   6297c:      	je	 <L3>
   6297e:      	nop
<L0>:
   62980:      	movq	%r12, %rdi
   62983:      	callq	 <scoop_heap_region_for_address>
   62988:      	movq	%rax, %rsi
   6298b:      	testq	%rax, %rax
   6298e:      	je	 <L5>
   62990:      	movq	(%rax), %rcx
   62993:      	movq	%r12, %rdx
   62996:      	subq	%rcx, %rdx
   62999:      	addq	0x8(%rax), %rcx
   6299d:      	subq	%r12, %rcx
   629a0:      	movq	%rdx, %rax
   629a3:      	cmpq	%rbx, %rcx
   629a6:      	cmovaq	%rbx, %rcx
   629aa:      	shrq	$0x9, %rax
   629ae:      	leaq	-0x1(%rdx,%rcx), %rdx
   629b3:      	shrq	$0x9, %rdx
   629b7:      	cmpq	%rax, %rdx
   629ba:      	jb	 <L2>
   629bc:      	leaq	0x1(%rdx), %rdi
<L1>:
   629c0:      	movq	0x10(%rsi), %rdx
   629c4:      	lock
   629c5:      	orb	$0x1, (%rdx,%rax)
   629c9:      	addq	$0x1, %rax
   629cd:      	cmpq	%rax, %rdi
   629d0:      	jne	 <L1>
<L2>:
   629d2:      	addq	%rcx, %r12
   629d5:      	subq	%rcx, %rbx
   629d8:      	jne	 <L0>
<L3>:
   629da:      	popq	%rbx
   629db:      	popq	%r12
   629dd:      	popq	%rbp
   629de:      	retq
<L4>:
   629df:      	leaq	, %rdi <scoop$1$be$1b425876b90f5e9644c00e51dce63e5157003d91225fec53eea0e4265f53f890+0x49f>
   629e6:      	callq	 <scoop_heap_fatal>
   629eb:      	nopl	(%rax,%rax)
<L5>:
   629f0:      	leaq	, %rdi <scoop$1$be$1b425876b90f5e9644c00e51dce63e5157003d91225fec53eea0e4265f53f890+0x46b0>
   629f7:      	callq	 <scoop_heap_fatal>
   629fc:      	nopl	(%rax)

0000000000067a80 <scoop_runtime_finish_tlab_alloc>:
   67a80:      	endbr64
   67a84:      	pushq	%rbp
   67a85:      	movq	%rsp, %rbp
   67a88:      	pushq	%r13
   67a8a:      	movq	%rdx, %r13
   67a8d:      	pushq	%r12
   67a8f:      	movq	%rdi, %r12
   67a92:      	pushq	%rbx
   67a93:      	movq	%rsi, %rbx
   67a96:      	subq	$0x8, %rsp
   67a9a:      	callq	 <scoop_gc_stress_move_enabled>
   67a9f:      	testb	%al, %al
   67aa1:      	jne	 <L0>
   67aa3:      	cmpq	$0x0, 0x90(%rbx)
   67aab:      	jne	 <L0>
   67aad:      	movq	%r13, %rsi
   67ab0:      	movq	%rbx, %rdi
   67ab3:      	callq	 <scoop_shape_normalize_allocation>
   67ab8:      	movq	%rax, %r13
   67abb:      	callq	 <scoop_thread_current_required>
   67ac0:      	movq	%r13, %rcx
   67ac3:      	movq	%rbx, %rdx
   67ac6:      	movq	%r12, %rsi
   67ac9:      	movq	%rax, %rdi
   67acc:      	callq	 <finish_small_allocation>
   67ad1:      	addq	$0x8, %rsp
   67ad5:      	popq	%rbx
   67ad6:      	popq	%r12
   67ad8:      	popq	%r13
   67ada:      	popq	%rbp
   67adb:      	retq
<L0>:
   67adc:      	leaq	, %rdi <scoop$1$be$1b425876b90f5e9644c00e51dce63e5157003d91225fec53eea0e4265f53f890+0x5058>
   67ae3:      	callq	 <scoop_heap_fatal>
   67ae8:      	nopl	(%rax,%rax)

0000000000073a90 <scoop_heap_region_for_address>:
   73a90:      	endbr64
   73a94:      	movl	$0x34, %ecx
   73a99:      	leaq	, %rax <scoop_gc_page_map>
<L0>:
   73aa0:      	movq	%rdi, %rdx
   73aa3:      	shrq	%cl, %rdx
   73aa6:      	andl	$0xfff, %edx            # imm = 0xFFF
   73aac:      	movq	(%rax,%rdx,8), %rax
   73ab0:      	testq	%rax, %rax
   73ab3:      	je	 <L1>
   73ab5:      	subl	$0xc, %ecx
   73ab8:      	cmpl	$0x10, %ecx
   73abb:      	jne	 <L0>
   73abd:      	shrq	$0x10, %rdi
   73ac1:      	andl	$0xfff, %edi            # imm = 0xFFF
   73ac7:      	movq	(%rax,%rdi,8), %rax
   73acb:      	retq
   73acc:      	nopl	(%rax)
<L1>:
   73ad0:      	xorl	%eax, %eax
   73ad2:      	retq
   73ad3:      	nop
   73ad5:      	nopw	%cs:(%rax,%rax)
