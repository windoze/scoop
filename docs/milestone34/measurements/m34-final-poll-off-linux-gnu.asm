; compute, MIR fn5, LIR scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c

/home/chenxu/repos/scoop/tmp/m34/final-poll-off-linux-gnu/poll:	file format elf64-x86-64

Disassembly of section .text:

000000000004c770 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c>:
   4c770: 55                           	pushq	%rbp
   4c771: 48 89 e5                     	movq	%rsp, %rbp
   4c774: 41 57                        	pushq	%r15
   4c776: 41 56                        	pushq	%r14
   4c778: 41 54                        	pushq	%r12
   4c77a: 53                           	pushq	%rbx
   4c77b: 89 fb                        	movl	%edi, %ebx
   4c77d: e8 de 2b 00 00               	callq	0x4f360 <scoop_rt_safepoint>
   4c782: 41 bf 01 00 00 00            	movl	$0x1, %r15d
   4c788: 45 31 f6                     	xorl	%r14d, %r14d
   4c78b: 0f 1f 44 00 00               	nopl	(%rax,%rax)
   4c790: 45 8d 67 ff                  	leal	-0x1(%r15), %r12d
   4c794: e8 c7 2b 00 00               	callq	0x4f360 <scoop_rt_safepoint>
   4c799: 41 39 dc                     	cmpl	%ebx, %r12d
   4c79c: 7d 08                        	jge	0x4c7a6 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x36>
   4c79e: 4d 01 fe                     	addq	%r15, %r14
   4c7a1: 49 ff c7                     	incq	%r15
   4c7a4: eb ea                        	jmp	0x4c790 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x20>
   4c7a6: 4c 89 f0                     	movq	%r14, %rax
   4c7a9: 5b                           	popq	%rbx
   4c7aa: 41 5c                        	popq	%r12
   4c7ac: 41 5e                        	popq	%r14
   4c7ae: 41 5f                        	popq	%r15
   4c7b0: 5d                           	popq	%rbp
   4c7b1: c3                           	retq
