
/home/chenxu/repos/scoop/tmp/m34/regions-off-linux-gnu/region-stores:	file format elf64-x86-64

Disassembly of section .text:

000000000005a090 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   5a090:      	pushq	%rbp
   5a091:      	movq	%rsp, %rbp
   5a094:      	pushq	%r15
   5a096:      	pushq	%r14
   5a098:      	pushq	%r13
   5a09a:      	pushq	%r12
   5a09c:      	pushq	%rbx
   5a09d:      	subq	$0x68, %rsp
   5a0a1:      	movq	%fs:0x0, %rax
   5a0aa:      	leaq	-0x10(%rax), %rax
   5a0b1:      	movq	(%rax), %r15
   5a0b4:      	leaq	, %rax <scoop_thread_gc_epoch>
   5a0bb:      	movq	(%rax), %rax
   5a0be:      	leaq	, %r13 <scoop_thread_world_phase>
   5a0c5:      	movl	(%r13), %esi
   5a0c9:      	movl	(%r15), %edx
   5a0cc:      	movq	0x8(%r15), %rcx
   5a0d0:      	testl	%esi, %esi
   5a0d2:      	jne	 <L0>
   5a0d4:      	cmpl	$0x1, %edx
   5a0d7:      	jne	 <L0>
   5a0d9:      	cmpq	%rax, %rcx
   5a0dc:      	je	 <L1>
<L0>:
   5a0de:      	callq	 <scoop_rt_safepoint>
<L1>:
   5a0e3:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5a0ed:      	movq	$0x0, -0x88(%rbp)
   5a0f8:      	movq	, %r14 <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5a0ff:      	movq	%r14, %r12
   5a102:      	negq	%r12
   5a105:      	addq	%r14, %rax
   5a108:      	movq	%rax, -0x60(%rbp)
   5a10c:      	cmpq	$-0x20, %rax
   5a110:      	setae	-0x29(%rbp)
   5a114:      	leaq	0x1f(%r14), %rbx
   5a118:      	andq	%r12, %rbx
   5a11b:      	movq	%fs:0x0, %rax
   5a124:      	leaq	-0x8(%rax), %rax
   5a12b:      	movq	(%rax), %rax
   5a12e:      	movq	(%rax), %rcx
   5a131:      	addq	%rcx, %r14
   5a134:      	decq	%r14
   5a137:      	movq	%r12, -0x70(%rbp)
   5a13b:      	andq	%r12, %r14
   5a13e:      	leaq	(%r14,%rbx), %rcx
   5a142:      	testq	%r14, %r14
   5a145:      	sete	%dl
   5a148:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5a14f:      	setae	%sil
   5a153:      	cmpq	0x8(%rax), %rcx
   5a157:      	seta	%dil
   5a15b:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5a163:      	setne	%r8b
   5a167:      	orb	%sil, %r8b
   5a16a:      	orb	%dl, %r8b
   5a16d:      	orb	-0x29(%rbp), %r8b
   5a171:      	orb	%dil, %r8b
   5a174:      	jne	 <L2>
   5a176:      	movq	%rcx, (%rax)
   5a179:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a180:      	movq	%r14, %rdi
   5a183:      	movq	%rbx, %rdx
   5a186:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5a18b:      	jmp	 <L3>
<L2>:
   5a18d:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a194:      	movl	$0x20, %esi
   5a199:      	callq	 <scoop_runtime_alloc_slow>
   5a19e:      	movq	%rax, %r14
   5a1a1:      	xorl	%eax, %eax
   5a1a3:      	movq	$0x0, -0x88(%rbp)
<L3>:
   5a1ae:      	movq	-0x88(%rbp), %rax
   5a1b5:      	movq	$0x0, 0x10(%r14)
   5a1bd:      	leaq	0x18(%r14), %rcx
   5a1c1:      	movq	%rax, 0x18(%r14)
   5a1c5:      	leaq	, %r12 <scoop_gc_card_table>
   5a1cc:      	movq	(%r12), %rax
   5a1d0:      	shrq	$0x9, %rcx
   5a1d4:      	lock
   5a1d5:      	orb	$0x1, (%rax,%rcx)
   5a1d9:      	movq	%r14, -0x40(%rbp)
   5a1dd:      	movq	$0x0, -0x78(%rbp)
   5a1e5:      	cmpq	$-0x20, -0x60(%rbp)
   5a1ea:      	setb	-0x29(%rbp)
   5a1ee:      	movq	%fs:0x0, %rax
   5a1f7:      	leaq	-0x8(%rax), %rax
   5a1fe:      	movq	(%rax), %rax
   5a201:      	movq	(%rax), %rcx
   5a204:      	movq	, %rdx <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5a20b:      	leaq	(%rdx,%rcx), %r14
   5a20f:      	decq	%r14
   5a212:      	andq	-0x70(%rbp), %r14
   5a216:      	leaq	(%r14,%rbx), %rcx
   5a21a:      	testq	%r14, %r14
   5a21d:      	setne	%dl
   5a220:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5a227:      	setb	%sil
   5a22b:      	cmpq	0x8(%rax), %rcx
   5a22f:      	setbe	%dil
   5a233:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5a23b:      	sete	%r8b
   5a23f:      	andb	%sil, %r8b
   5a242:      	andb	%dl, %r8b
   5a245:      	andb	%dil, %r8b
   5a248:      	testb	%r8b, -0x29(%rbp)
   5a24c:      	je	 <L4>
   5a24e:      	movq	%rcx, (%rax)
   5a251:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a258:      	movq	%r14, %rdi
   5a25b:      	movq	%rbx, %rdx
   5a25e:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5a263:      	jmp	 <L5>
<L4>:
   5a265:      	movq	-0x40(%rbp), %rax
   5a269:      	movq	%rax, -0x38(%rbp)
   5a26d:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a274:      	movl	$0x20, %esi
   5a279:      	callq	 <scoop_runtime_alloc_slow>
   5a27e:      	movq	%rax, %r14
   5a281:      	movq	-0x38(%rbp), %rax
   5a285:      	movq	%rax, -0x40(%rbp)
   5a289:      	xorl	%eax, %eax
   5a28b:      	movq	$0x0, -0x78(%rbp)
<L5>:
   5a293:      	movq	-0x78(%rbp), %rax
   5a297:      	movq	$0x7, 0x10(%r14)
   5a29f:      	leaq	0x18(%r14), %rcx
   5a2a3:      	movq	%rax, 0x18(%r14)
   5a2a7:      	movq	(%r12), %rax
   5a2ab:      	shrq	$0x9, %rcx
   5a2af:      	lock
   5a2b0:      	orb	$0x1, (%rax,%rcx)
   5a2b4:      	movq	%r14, -0x50(%rbp)
   5a2b8:      	movq	$0x0, -0x80(%rbp)
   5a2c0:      	cmpq	$-0x20, -0x60(%rbp)
   5a2c5:      	setb	-0x60(%rbp)
   5a2c9:      	movq	%fs:0x0, %rax
   5a2d2:      	leaq	-0x8(%rax), %rax
   5a2d9:      	movq	(%rax), %rax
   5a2dc:      	movq	(%rax), %rcx
   5a2df:      	movq	, %rdx <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x18>
   5a2e6:      	leaq	(%rdx,%rcx), %r14
   5a2ea:      	decq	%r14
   5a2ed:      	andq	-0x70(%rbp), %r14
   5a2f1:      	leaq	(%r14,%rbx), %rcx
   5a2f5:      	testq	%r14, %r14
   5a2f8:      	setne	%dl
   5a2fb:      	cmpq	$0x7f81, %rbx           # imm = 0x7F81
   5a302:      	setb	%sil
   5a306:      	cmpq	0x8(%rax), %rcx
   5a30a:      	setbe	%dil
   5a30e:      	cmpq	$0x0,  <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3+0x90>
   5a316:      	sete	%r8b
   5a31a:      	andb	%sil, %r8b
   5a31d:      	andb	%dl, %r8b
   5a320:      	andb	%dil, %r8b
   5a323:      	testb	%r8b, -0x60(%rbp)
   5a327:      	je	 <L6>
   5a329:      	movq	%rcx, (%rax)
   5a32c:      	leaq	, %rsi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a333:      	movq	%r14, %rdi
   5a336:      	movq	%rbx, %rdx
   5a339:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5a33e:      	jmp	 <L7>
<L6>:
   5a340:      	movq	-0x50(%rbp), %rax
   5a344:      	movq	-0x40(%rbp), %rcx
   5a348:      	movq	%rcx, -0x38(%rbp)
   5a34c:      	movq	%rax, -0x48(%rbp)
   5a350:      	leaq	, %rdi <scoop$1$td$7cd58eee51e8ebae7d0ccc2db6ff6a7686829c87e2fecdd9d32e46965c4938c3>
   5a357:      	movl	$0x20, %esi
   5a35c:      	callq	 <scoop_runtime_alloc_slow>
   5a361:      	movq	%rax, %r14
   5a364:      	movq	-0x48(%rbp), %rax
   5a368:      	movq	-0x38(%rbp), %rcx
   5a36c:      	movq	%rcx, -0x40(%rbp)
   5a370:      	movq	%rax, -0x50(%rbp)
   5a374:      	xorl	%eax, %eax
   5a376:      	movq	$0x0, -0x80(%rbp)
<L7>:
   5a37e:      	movq	-0x80(%rbp), %rax
   5a382:      	movq	$0x9, 0x10(%r14)
   5a38a:      	leaq	0x18(%r14), %rcx
   5a38e:      	movq	%rax, 0x18(%r14)
   5a392:      	movq	(%r12), %rax
   5a396:      	shrq	$0x9, %rcx
   5a39a:      	lock
   5a39b:      	orb	$0x1, (%rax,%rcx)
   5a39f:      	movq	-0x40(%rbp), %rax
   5a3a3:      	movq	-0x50(%rbp), %rcx
   5a3a7:      	movq	%r14, -0x58(%rbp)
   5a3ab:      	movq	%rax, -0x38(%rbp)
   5a3af:      	movq	%rcx, -0x48(%rbp)
   5a3b3:      	callq	 <scoop_rt_gc_collect>
   5a3b8:      	movq	-0x58(%rbp), %rax
   5a3bc:      	movq	-0x48(%rbp), %rcx
   5a3c0:      	movq	-0x38(%rbp), %rdx
   5a3c4:      	movq	%rdx, -0x40(%rbp)
   5a3c8:      	movq	%rcx, -0x50(%rbp)
   5a3cc:      	movq	%rax, -0x68(%rbp)
   5a3d0:      	xorl	%ebx, %ebx
   5a3d2:      	leaq	, %r14 <scoop_thread_gc_epoch>
   5a3d9:      	jmp	 <L10>
   5a3db:      	nopl	(%rax,%rax)
<L8>:
   5a3e0:      	movq	-0x68(%rbp), %rax
<L9>:
   5a3e4:      	movq	-0x40(%rbp), %rcx
   5a3e8:      	movq	%rax, 0x18(%rcx)
   5a3ec:      	addq	$0x18, %rcx
   5a3f0:      	movq	(%r12), %rax
   5a3f4:      	shrq	$0x9, %rcx
   5a3f8:      	lock
   5a3f9:      	orb	$0x1, (%rax,%rcx)
   5a3fd:      	incq	%rbx
<L10>:
   5a400:      	movq	(%r14), %rax
   5a403:      	movl	(%r13), %esi
   5a407:      	movl	(%r15), %edx
   5a40a:      	movq	0x8(%r15), %rcx
   5a40e:      	testl	%esi, %esi
   5a410:      	jne	 <L11>
   5a412:      	cmpl	$0x1, %edx
   5a415:      	jne	 <L11>
   5a417:      	cmpq	%rax, %rcx
   5a41a:      	je	 <L12>
<L11>:
   5a41c:      	movq	-0x40(%rbp), %rax
   5a420:      	movq	-0x50(%rbp), %rcx
   5a424:      	movq	-0x68(%rbp), %rdx
   5a428:      	movq	%rax, -0x38(%rbp)
   5a42c:      	movq	%rcx, -0x48(%rbp)
   5a430:      	movq	%rdx, -0x58(%rbp)
   5a434:      	callq	 <scoop_rt_safepoint>
   5a439:      	movq	-0x58(%rbp), %rax
   5a43d:      	movq	-0x48(%rbp), %rcx
   5a441:      	movq	-0x38(%rbp), %rdx
   5a445:      	movq	%rdx, -0x40(%rbp)
   5a449:      	movq	%rcx, -0x50(%rbp)
   5a44d:      	movq	%rax, -0x68(%rbp)
<L12>:
   5a451:      	cmpq	$0x7a1200, %rbx         # imm = 0x7A1200
   5a458:      	jge	 <L13>
   5a45a:      	testb	$0x1, %bl
   5a45d:      	jne	 <L8>
   5a45f:      	movq	-0x50(%rbp), %rax
   5a463:      	jmp	 <L9>
<L13>:
   5a468:      	movq	-0x40(%rbp), %rax
   5a46c:      	movq	0x18(%rax), %rax
   5a470:      	testq	%rax, %rax
   5a473:      	je	 <L14>
   5a475:      	movq	0x10(%rax), %rdi
   5a479:      	callq	 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   5a47e:      	addq	$0x68, %rsp
   5a482:      	popq	%rbx
   5a483:      	popq	%r12
   5a485:      	popq	%r13
   5a487:      	popq	%r14
   5a489:      	popq	%r15
   5a48b:      	popq	%rbp
   5a48c:      	retq
<L14>:
   5a48d:      	movq	, %rbx <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3+0x18>
   5a494:      	movq	%rbx, %r15
   5a497:      	negq	%r15
   5a49a:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5a4a4:      	addq	%rbx, %rax
   5a4a7:      	cmpq	$-0x18, %rax
   5a4ab:      	setb	%r12b
   5a4af:      	leaq	0x17(%rbx), %r14
   5a4b3:      	andq	%r15, %r14
   5a4b6:      	movq	%fs:0x0, %rax
   5a4bf:      	leaq	-0x8(%rax), %rax
   5a4c6:      	movq	(%rax), %rax
   5a4c9:      	movq	(%rax), %rcx
   5a4cc:      	addq	%rcx, %rbx
   5a4cf:      	decq	%rbx
   5a4d2:      	andq	%r15, %rbx
   5a4d5:      	leaq	(%rbx,%r14), %rcx
   5a4d9:      	testq	%rbx, %rbx
   5a4dc:      	setne	%dl
   5a4df:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5a4e6:      	setb	%sil
   5a4ea:      	cmpq	0x8(%rax), %rcx
   5a4ee:      	setbe	%dil
   5a4f2:      	cmpq	$0x0,  <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3+0x90>
   5a4fa:      	sete	%r8b
   5a4fe:      	andb	%sil, %r8b
   5a501:      	andb	%dl, %r8b
   5a504:      	andb	%dil, %r8b
   5a507:      	testb	%r12b, %r8b
   5a50a:      	je	 <L15>
   5a50c:      	movq	%rcx, (%rax)
   5a50f:      	leaq	, %rsi <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3>
   5a516:      	movq	%rbx, %rdi
   5a519:      	movq	%r14, %rdx
   5a51c:      	callq	 <scoop_runtime_finish_tlab_alloc>
   5a521:      	jmp	 <L16>
<L15>:
   5a523:      	leaq	, %rdi <scoop$1$td$3e5281134cfedec025420fd79f1c4748321f748c77f8755860359f9bcd4fb6e3>
   5a52a:      	movl	$0x18, %esi
   5a52f:      	callq	 <scoop_runtime_alloc_slow>
   5a534:      	movq	%rax, %rbx
<L16>:
   5a537:      	movq	%rbx, -0x38(%rbp)
   5a53b:      	movq	%rbx, %rdi
   5a53e:      	callq	 <scoop$1$cb$a743fee7ba2831ad1852cbb01a2d484559872cb6aa9aa7bc66d5e03510bf8243>
   5a543:      	movq	-0x38(%rbp), %rdi
   5a547:      	movq	%rdi, -0x90(%rbp)
   5a54e:      	callq	 <scoop_rt_throw>

000000000005df10 <scoop_runtime_finish_tlab_alloc>:
   5df10:      	endbr64
   5df14:      	pushq	%rbp
   5df15:      	movq	%rsp, %rbp
   5df18:      	pushq	%r13
   5df1a:      	movq	%rdx, %r13
   5df1d:      	pushq	%r12
   5df1f:      	movq	%rdi, %r12
   5df22:      	pushq	%rbx
   5df23:      	movq	%rsi, %rbx
   5df26:      	subq	$0x8, %rsp
   5df2a:      	callq	 <scoop_gc_stress_move_enabled>
   5df2f:      	testb	%al, %al
   5df31:      	jne	 <L0>
   5df33:      	cmpq	$0x0, 0x90(%rbx)
   5df3b:      	jne	 <L0>
   5df3d:      	movq	%r13, %rsi
   5df40:      	movq	%rbx, %rdi
   5df43:      	callq	 <scoop_shape_normalize_allocation>
   5df48:      	movq	%rax, %r13
   5df4b:      	callq	 <scoop_thread_current_required>
   5df50:      	movq	%r13, %rcx
   5df53:      	movq	%rbx, %rdx
   5df56:      	movq	%r12, %rsi
   5df59:      	movq	%rax, %rdi
   5df5c:      	callq	 <finish_small_allocation>
   5df61:      	addq	$0x8, %rsp
   5df65:      	popq	%rbx
   5df66:      	popq	%r12
   5df68:      	popq	%r13
   5df6a:      	popq	%rbp
   5df6b:      	retq
<L0>:
   5df6c:      	leaq	, %rdi <scoop$1$be$1b425876b90f5e9644c00e51dce63e5157003d91225fec53eea0e4265f53f890+0x3b08>
   5df73:      	callq	 <scoop_heap_fatal>
   5df78:      	nopl	(%rax,%rax)

0000000000070470 <scoop_rt_gc_write_barrier>:
   70470:      	endbr64
   70474:      	testq	%rsi, %rsi
   70477:      	je	 <L1>
   70479:      	cmpb	$0x0,  <scoop_gc_heap_state+0x98>
   70480:      	je	 <L2>
   70482:      	movq	, %rdx <scoop_gc_heap_state+0x28>
   70489:      	cmpq	%rdx, %rdi
   7048c:      	jb	 <L2>
   7048e:      	movq	, %rax <scoop_gc_heap_state+0x30>
   70495:      	cmpq	%rax, %rdi
   70498:      	jae	 <L2>
   7049a:      	subq	%rdi, %rax
   7049d:      	cmpq	%rsi, %rax
   704a0:      	jb	 <L2>
   704a2:      	subq	%rdx, %rdi
   704a5:      	movq	%rdi, %rax
   704a8:      	leaq	-0x1(%rdi,%rsi), %rdx
   704ad:      	shrq	$0x9, %rax
   704b1:      	shrq	$0x9, %rdx
   704b5:      	cmpq	%rax, %rdx
   704b8:      	jb	 <L1>
   704ba:      	leaq	, %rsi <scoop_gc_heap_state>
   704c1:      	leaq	0x1(%rdx), %rcx
   704c5:      	nopw	%cs:(%rax,%rax)
<L0>:
   704d0:      	movq	0x58(%rsi), %rdx
   704d4:      	lock
   704d5:      	orb	$0x1, (%rdx,%rax)
   704d9:      	addq	$0x1, %rax
   704dd:      	cmpq	%rax, %rcx
   704e0:      	jne	 <L0>
<L1>:
   704e2:      	retq
<L2>:
   704e3:      	pushq	%rbp
   704e4:      	leaq	, %rdi <scoop$1$be$1b425876b90f5e9644c00e51dce63e5157003d91225fec53eea0e4265f53f890+0x6cb8>
   704eb:      	movq	%rsp, %rbp
   704ee:      	callq	 <scoop_heap_fatal>
   704f3:      	nop
   704f5:      	nopw	%cs:(%rax,%rax)
