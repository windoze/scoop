; compute, MIR fn5, LIR scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c

/home/chenxu/repos/scoop/tmp/m34/verified-poll-on-linux-gnu/poll:	file format elf64-x86-64

Disassembly of section .text:

000000000005c6f0 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c>:
   5c6f0: 55                           	pushq	%rbp
   5c6f1: 48 89 e5                     	movq	%rsp, %rbp
   5c6f4: 41 57                        	pushq	%r15
   5c6f6: 41 56                        	pushq	%r14
   5c6f8: 41 55                        	pushq	%r13
   5c6fa: 41 54                        	pushq	%r12
   5c6fc: 53                           	pushq	%rbx
   5c6fd: 50                           	pushq	%rax
   5c6fe: 89 fb                        	movl	%edi, %ebx
   5c700: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c709: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5c710: 4c 8b 38                     	movq	(%rax), %r15
   5c713: 48 8d 05 b6 5e 13 00         	leaq	0x135eb6(%rip), %rax    # 0x1925d0 <scoop_thread_gc_epoch>
   5c71a: 48 8b 00                     	movq	(%rax), %rax
   5c71d: 4c 8d 2d b4 5e 13 00         	leaq	0x135eb4(%rip), %r13    # 0x1925d8 <scoop_thread_world_phase>
   5c724: 41 8b 75 00                  	movl	(%r13), %esi
   5c728: 41 8b 17                     	movl	(%r15), %edx
   5c72b: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5c72f: 85 f6                        	testl	%esi, %esi
   5c731: 75 0a                        	jne	0x5c73d <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x4d>
   5c733: 83 fa 01                     	cmpl	$0x1, %edx
   5c736: 75 05                        	jne	0x5c73d <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x4d>
   5c738: 48 39 c1                     	cmpq	%rax, %rcx
   5c73b: 74 05                        	je	0x5c742 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x52>
   5c73d: e8 be 2d 00 00               	callq	0x5f500 <scoop_rt_safepoint>
   5c742: 41 bc 01 00 00 00            	movl	$0x1, %r12d
   5c748: 45 31 f6                     	xorl	%r14d, %r14d
   5c74b: 0f 1f 44 00 00               	nopl	(%rax,%rax)
   5c750: 48 8d 05 79 5e 13 00         	leaq	0x135e79(%rip), %rax    # 0x1925d0 <scoop_thread_gc_epoch>
   5c757: 48 8b 00                     	movq	(%rax), %rax
   5c75a: 41 8b 75 00                  	movl	(%r13), %esi
   5c75e: 41 8b 17                     	movl	(%r15), %edx
   5c761: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5c765: 85 f6                        	testl	%esi, %esi
   5c767: 75 0a                        	jne	0x5c773 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x83>
   5c769: 83 fa 01                     	cmpl	$0x1, %edx
   5c76c: 75 05                        	jne	0x5c773 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x83>
   5c76e: 48 39 c1                     	cmpq	%rax, %rcx
   5c771: 74 05                        	je	0x5c778 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x88>
   5c773: e8 88 2d 00 00               	callq	0x5f500 <scoop_rt_safepoint>
   5c778: 49 8d 44 24 ff               	leaq	-0x1(%r12), %rax
   5c77d: 39 d8                        	cmpl	%ebx, %eax
   5c77f: 7d 08                        	jge	0x5c789 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x99>
   5c781: 4d 01 e6                     	addq	%r12, %r14
   5c784: 49 ff c4                     	incq	%r12
   5c787: eb c7                        	jmp	0x5c750 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x60>
   5c789: 4c 89 f0                     	movq	%r14, %rax
   5c78c: 48 83 c4 08                  	addq	$0x8, %rsp
   5c790: 5b                           	popq	%rbx
   5c791: 41 5c                        	popq	%r12
   5c793: 41 5d                        	popq	%r13
   5c795: 41 5e                        	popq	%r14
   5c797: 41 5f                        	popq	%r15
   5c799: 5d                           	popq	%rbp
   5c79a: c3                           	retq
