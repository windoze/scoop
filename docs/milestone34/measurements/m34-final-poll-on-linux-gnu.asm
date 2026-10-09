; compute, MIR fn5, LIR scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c

/home/chenxu/repos/scoop/tmp/m34/final-poll-on-linux-gnu/poll:	file format elf64-x86-64

Disassembly of section .text:

000000000005b330 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c>:
   5b330: 55                           	pushq	%rbp
   5b331: 48 89 e5                     	movq	%rsp, %rbp
   5b334: 41 57                        	pushq	%r15
   5b336: 41 56                        	pushq	%r14
   5b338: 41 55                        	pushq	%r13
   5b33a: 41 54                        	pushq	%r12
   5b33c: 53                           	pushq	%rbx
   5b33d: 50                           	pushq	%rax
   5b33e: 89 fb                        	movl	%edi, %ebx
   5b340: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b349: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b350: 4c 8b 38                     	movq	(%rax), %r15
   5b353: 48 8d 05 76 62 13 00         	leaq	0x136276(%rip), %rax    # 0x1915d0 <scoop_thread_gc_epoch>
   5b35a: 48 8b 00                     	movq	(%rax), %rax
   5b35d: 4c 8d 2d 74 62 13 00         	leaq	0x136274(%rip), %r13    # 0x1915d8 <scoop_thread_world_phase>
   5b364: 41 8b 75 00                  	movl	(%r13), %esi
   5b368: 41 8b 17                     	movl	(%r15), %edx
   5b36b: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5b36f: 85 f6                        	testl	%esi, %esi
   5b371: 75 0a                        	jne	0x5b37d <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x4d>
   5b373: 83 fa 01                     	cmpl	$0x1, %edx
   5b376: 75 05                        	jne	0x5b37d <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x4d>
   5b378: 48 39 c1                     	cmpq	%rax, %rcx
   5b37b: 74 05                        	je	0x5b382 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x52>
   5b37d: e8 be 2d 00 00               	callq	0x5e140 <scoop_rt_safepoint>
   5b382: 41 bc 01 00 00 00            	movl	$0x1, %r12d
   5b388: 45 31 f6                     	xorl	%r14d, %r14d
   5b38b: 0f 1f 44 00 00               	nopl	(%rax,%rax)
   5b390: 48 8d 05 39 62 13 00         	leaq	0x136239(%rip), %rax    # 0x1915d0 <scoop_thread_gc_epoch>
   5b397: 48 8b 00                     	movq	(%rax), %rax
   5b39a: 41 8b 75 00                  	movl	(%r13), %esi
   5b39e: 41 8b 17                     	movl	(%r15), %edx
   5b3a1: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5b3a5: 85 f6                        	testl	%esi, %esi
   5b3a7: 75 0a                        	jne	0x5b3b3 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x83>
   5b3a9: 83 fa 01                     	cmpl	$0x1, %edx
   5b3ac: 75 05                        	jne	0x5b3b3 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x83>
   5b3ae: 48 39 c1                     	cmpq	%rax, %rcx
   5b3b1: 74 05                        	je	0x5b3b8 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x88>
   5b3b3: e8 88 2d 00 00               	callq	0x5e140 <scoop_rt_safepoint>
   5b3b8: 49 8d 44 24 ff               	leaq	-0x1(%r12), %rax
   5b3bd: 39 d8                        	cmpl	%ebx, %eax
   5b3bf: 7d 08                        	jge	0x5b3c9 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x99>
   5b3c1: 4d 01 e6                     	addq	%r12, %r14
   5b3c4: 49 ff c4                     	incq	%r12
   5b3c7: eb c7                        	jmp	0x5b390 <scoop$1$cb$1fae31b5ae643617acccfbe5d51c772901b38ef70423d570500ef8eb3981e50c+0x60>
   5b3c9: 4c 89 f0                     	movq	%r14, %rax
   5b3cc: 48 83 c4 08                  	addq	$0x8, %rsp
   5b3d0: 5b                           	popq	%rbx
   5b3d1: 41 5c                        	popq	%r12
   5b3d3: 41 5d                        	popq	%r13
   5b3d5: 41 5e                        	popq	%r14
   5b3d7: 41 5f                        	popq	%r15
   5b3d9: 5d                           	popq	%rbp
   5b3da: c3                           	retq
