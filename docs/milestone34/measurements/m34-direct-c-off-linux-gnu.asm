000000000005ba60 <scoop$1$br$8e9b1811ab9831eeec4c68e4feb97cf9e0eb47737a73d7e4f89ee5f6a434a664>:
   5ba60:      	endbr64
   5ba64:      	pushq	%rbx
   5ba65:      	movq	%rdi, %rbx
   5ba68:      	movq	(%rsi), %rdi
   5ba6b:      	movq	0x8(%rsi), %rsi
   5ba6f:      	callq	0x762c0 <bench_pair>
   5ba74:      	movq	%rax, (%rbx)
   5ba77:      	movq	%rdx, 0x8(%rbx)
   5ba7b:      	popq	%rbx
   5ba7c:      	retq


0000000000076300 <run_worker>:
   76300:      	endbr64
   76304:      	pushq	%rbp
   76305:      	movq	%rsp, %rbp
   76308:      	pushq	%r13
   7630a:      	pushq	%r12
   7630c:      	pushq	%rbx
   7630d:      	subq	$0x8, %rsp
   76311:      	lock
   76312:      	addl	$0x1, 0x2a2db7(%rip)    # 0x3190d0 <ready>
   76319:      	jmp	0x76325 <run_worker+0x25>
   7631b:      	nopl	(%rax,%rax)
   76320:      	callq	0x2a2f0 <sched_yield@plt>
   76325:      	movzbl	0x2a2d9c(%rip), %eax    # 0x3190c8 <start>
   7632c:      	testb	%al, %al
   7632e:      	je	0x76320 <run_worker+0x20>
   76330:      	movl	0x2a2d9e(%rip), %eax    # 0x3190d4 <selected>
   76336:      	movslq	0x2a2d9b(%rip), %rdi    # 0x3190d8 <iterations>
   7633d:      	testl	%eax, %eax
   7633f:      	jne	0x763a0 <run_worker+0xa0>
   76341:      	testl	%edi, %edi
   76343:      	jle	0x76425 <run_worker+0x125>
   76349:      	xorl	%ebx, %ebx
   7634b:      	xorl	%r12d, %r12d
   7634e:      	nop
   76350:      	movq	%rbx, %rdi
   76353:      	addq	$0x1, %rbx
   76357:      	callq	0x762b0 <bench_scalar>
   7635c:      	movslq	0x2a2d75(%rip), %rdi    # 0x3190d8 <iterations>
   76363:      	addq	%rax, %r12
   76366:      	cmpl	%ebx, %edi
   76368:      	jg	0x76350 <run_worker+0x50>
   7636a:      	movl	0x2a2d64(%rip), %eax    # 0x3190d4 <selected>
   76370:      	subl	$0x3, %eax
   76373:      	cmpl	$0x2, %eax
   76376:      	ja	0x763d3 <run_worker+0xd3>
   76378:      	leaq	0x2(%rdi), %rax
   7637c:      	imulq	%rdi, %rax
   76380:      	cmpq	%r12, %rax
   76383:      	jne	0x2a480 <run_worker.cold>
   76389:      	lock
   7638a:      	addl	$0x1, 0x2a2d3b(%rip)    # 0x3190cc <finished>
   76391:      	xorl	%eax, %eax
   76393:      	addq	$0x8, %rsp
   76397:      	popq	%rbx
   76398:      	popq	%r12
   7639a:      	popq	%r13
   7639c:      	popq	%rbp
   7639d:      	retq
   7639e:      	nop
   763a0:      	cmpl	$0x3, %eax
   763a3:      	je	0x763f0 <run_worker+0xf0>
   763a5:      	cmpl	$0x6, %eax
   763a8:      	je	0x76430 <run_worker+0x130>
   763ae:      	movq	0x2a2d33(%rip), %rsi    # 0x3190e8 <worker_context>
   763b5:      	callq	*0x2a2d3d(%rip)         # 0x3190f8 <worker>
   763bb:      	movslq	0x2a2d16(%rip), %rdi    # 0x3190d8 <iterations>
   763c2:      	movq	%rax, %r12
   763c5:      	movl	0x2a2d09(%rip), %eax    # 0x3190d4 <selected>
   763cb:      	subl	$0x3, %eax
   763ce:      	cmpl	$0x2, %eax
   763d1:      	jbe	0x76378 <run_worker+0x78>
   763d3:      	leaq	0x1(%rdi), %rdx
   763d7:      	imulq	%rdi, %rdx
   763db:      	movq	%rdx, %rax
   763de:      	shrq	$0x3f, %rax
   763e2:      	addq	%rdx, %rax
   763e5:      	sarq	%rax
   763e8:      	jmp	0x76380 <run_worker+0x80>
   763ea:      	nopw	(%rax,%rax)
   763f0:      	testl	%edi, %edi
   763f2:      	jle	0x76481 <run_worker+0x181>
   763f8:      	xorl	%ebx, %ebx
   763fa:      	xorl	%r12d, %r12d
   763fd:      	nopl	(%rax)
   76400:      	movq	%rbx, %rdi
   76403:      	movq	%rbx, %rsi
   76406:      	addq	$0x1, %rbx
   7640a:      	callq	0x762c0 <bench_pair>
   7640f:      	movslq	0x2a2cc2(%rip), %rdi    # 0x3190d8 <iterations>
   76416:      	addq	%rdx, %rax
   76419:      	addq	%rax, %r12
   7641c:      	cmpl	%ebx, %edi
   7641e:      	jg	0x76400 <run_worker+0x100>
   76420:      	jmp	0x7636a <run_worker+0x6a>
   76425:      	xorl	%r12d, %r12d
   76428:      	jmp	0x763d3 <run_worker+0xd3>
   7642a:      	nopw	(%rax,%rax)
   76430:      	testl	%edi, %edi
   76432:      	jle	0x76425 <run_worker+0x125>
   76434:      	callq	0x2a040 <__errno_location@plt>
   76439:      	xorl	%ebx, %ebx
   7643b:      	xorl	%r12d, %r12d
   7643e:      	movq	%rax, %r13
   76441:      	nopl	(%rax)
   76445:      	nopw	%cs:(%rax,%rax)
   76450:      	movl	$0x0, (%r13)
   76458:      	movq	%rbx, %rdi
   7645b:      	addq	$0x1, %rbx
   7645f:      	callq	0x762b0 <bench_scalar>
   76464:      	movslq	0x2a2c6d(%rip), %rdi    # 0x3190d8 <iterations>
   7646b:      	movq	%rax, %rdx
   7646e:      	movslq	(%r13), %rax
   76472:      	addq	%rdx, %rax
   76475:      	addq	%rax, %r12
   76478:      	cmpl	%ebx, %edi
   7647a:      	jg	0x76450 <run_worker+0x150>
   7647c:      	jmp	0x7636a <run_worker+0x6a>
   76481:      	xorl	%r12d, %r12d
   76484:      	jmp	0x76378 <run_worker+0x78>
   76489:      	nopl	(%rax)


/home/chenxu/repos/scoop/tmp/m34/direct-c-before-gnu/leaf.o:	file format elf64-x86-64

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
