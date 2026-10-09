; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005c9d0 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
   5c9d0: 55                           	pushq	%rbp
   5c9d1: 48 89 e5                     	movq	%rsp, %rbp
   5c9d4: 41 56                        	pushq	%r14
   5c9d6: 53                           	pushq	%rbx
   5c9d7: 48 83 ec 10                  	subq	$0x10, %rsp
   5c9db: 89 f3                        	movl	%esi, %ebx
   5c9dd: 49 89 fe                     	movq	%rdi, %r14
   5c9e0: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c9e9: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5c9f0: 48 8b 08                     	movq	(%rax), %rcx
   5c9f3: 4c 89 75 e8                  	movq	%r14, -0x18(%rbp)
   5c9f7: 48 8d 05 f2 8b 13 00         	leaq	0x138bf2(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c9fe: 48 8b 00                     	movq	(%rax), %rax
   5ca01: 48 8d 15 f0 8b 13 00         	leaq	0x138bf0(%rip), %rdx    # 0x1955f8 <scoop_thread_world_phase>
   5ca08: 8b 32                        	movl	(%rdx), %esi
   5ca0a: 8b 11                        	movl	(%rcx), %edx
   5ca0c: 48 8b 49 08                  	movq	0x8(%rcx), %rcx
   5ca10: 85 f6                        	testl	%esi, %esi
   5ca12: 75 0a                        	jne	0x5ca1e <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x4e>
   5ca14: 83 fa 01                     	cmpl	$0x1, %edx
   5ca17: 75 05                        	jne	0x5ca1e <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x4e>
   5ca19: 48 39 c1                     	cmpq	%rax, %rcx
   5ca1c: 74 15                        	je	0x5ca33 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x63>
   5ca1e: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5ca22: 48 89 45 e0                  	movq	%rax, -0x20(%rbp)
   5ca26: e8 55 30 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5ca2b: 48 8b 45 e0                  	movq	-0x20(%rbp), %rax
   5ca2f: 48 89 45 e8                  	movq	%rax, -0x18(%rbp)
   5ca33: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5ca37: 89 d9                        	movl	%ebx, %ecx
   5ca39: c1 e1 0d                     	shll	$0xd, %ecx
   5ca3c: 33 48 10                     	xorl	0x10(%rax), %ecx
   5ca3f: 31 d9                        	xorl	%ebx, %ecx
   5ca41: 89 ca                        	movl	%ecx, %edx
   5ca43: c1 ea 11                     	shrl	$0x11, %edx
   5ca46: 31 ca                        	xorl	%ecx, %edx
   5ca48: 89 d0                        	movl	%edx, %eax
   5ca4a: c1 e0 05                     	shll	$0x5, %eax
   5ca4d: 31 d0                        	xorl	%edx, %eax
   5ca4f: 48 83 c4 10                  	addq	$0x10, %rsp
   5ca53: 5b                           	popq	%rbx
   5ca54: 41 5e                        	popq	%r14
   5ca56: 5d                           	popq	%rbp
   5ca57: c3                           	retq

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005cac0 <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
   5cac0: 55                           	pushq	%rbp
   5cac1: 48 89 e5                     	movq	%rsp, %rbp
   5cac4: 53                           	pushq	%rbx
   5cac5: 48 83 ec 18                  	subq	$0x18, %rsp
   5cac9: 48 89 fb                     	movq	%rdi, %rbx
   5cacc: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5cad5: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5cadc: 48 8b 08                     	movq	(%rax), %rcx
   5cadf: 48 89 5d f0                  	movq	%rbx, -0x10(%rbp)
   5cae3: 48 8d 05 06 8b 13 00         	leaq	0x138b06(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5caea: 48 8b 00                     	movq	(%rax), %rax
   5caed: 48 8d 15 04 8b 13 00         	leaq	0x138b04(%rip), %rdx    # 0x1955f8 <scoop_thread_world_phase>
   5caf4: 8b 32                        	movl	(%rdx), %esi
   5caf6: 8b 11                        	movl	(%rcx), %edx
   5caf8: 48 8b 49 08                  	movq	0x8(%rcx), %rcx
   5cafc: 85 f6                        	testl	%esi, %esi
   5cafe: 75 0a                        	jne	0x5cb0a <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x4a>
   5cb00: 83 fa 01                     	cmpl	$0x1, %edx
   5cb03: 75 05                        	jne	0x5cb0a <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x4a>
   5cb05: 48 39 c1                     	cmpq	%rax, %rcx
   5cb08: 74 15                        	je	0x5cb1f <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x5f>
   5cb0a: 48 8b 45 f0                  	movq	-0x10(%rbp), %rax
   5cb0e: 48 89 45 e8                  	movq	%rax, -0x18(%rbp)
   5cb12: e8 69 2f 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5cb17: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5cb1b: 48 89 45 f0                  	movq	%rax, -0x10(%rbp)
   5cb1f: 48 8b 45 f0                  	movq	-0x10(%rbp), %rax
   5cb23: 8b 40 10                     	movl	0x10(%rax), %eax
   5cb26: 48 83 c4 18                  	addq	$0x18, %rsp
   5cb2a: 5b                           	popq	%rbx
   5cb2b: 5d                           	popq	%rbp
   5cb2c: c3                           	retq

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005c8b0 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
   5c8b0: 55                           	pushq	%rbp
   5c8b1: 48 89 e5                     	movq	%rsp, %rbp
   5c8b4: 41 57                        	pushq	%r15
   5c8b6: 41 56                        	pushq	%r14
   5c8b8: 41 55                        	pushq	%r13
   5c8ba: 41 54                        	pushq	%r12
   5c8bc: 53                           	pushq	%rbx
   5c8bd: 48 83 ec 38                  	subq	$0x38, %rsp
   5c8c1: 89 f3                        	movl	%esi, %ebx
   5c8c3: 49 89 fe                     	movq	%rdi, %r14
   5c8c6: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c8cf: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5c8d6: 4c 8b 38                     	movq	(%rax), %r15
   5c8d9: 4c 89 75 c0                  	movq	%r14, -0x40(%rbp)
   5c8dd: 48 c7 45 b0 00 00 00 00      	movq	$0x0, -0x50(%rbp)
   5c8e5: 48 8d 05 04 8d 13 00         	leaq	0x138d04(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c8ec: 48 8b 00                     	movq	(%rax), %rax
   5c8ef: 4c 8d 2d 02 8d 13 00         	leaq	0x138d02(%rip), %r13    # 0x1955f8 <scoop_thread_world_phase>
   5c8f6: 41 8b 75 00                  	movl	(%r13), %esi
   5c8fa: 41 8b 17                     	movl	(%r15), %edx
   5c8fd: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5c901: 85 f6                        	testl	%esi, %esi
   5c903: 75 0a                        	jne	0x5c90f <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x5f>
   5c905: 83 fa 01                     	cmpl	$0x1, %edx
   5c908: 75 05                        	jne	0x5c90f <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x5f>
   5c90a: 48 39 c1                     	cmpq	%rax, %rcx
   5c90d: 74 15                        	je	0x5c924 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x74>
   5c90f: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5c913: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c917: e8 64 31 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c91c: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c920: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   5c924: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5c928: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c92c: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   5c932: 45 31 e4                     	xorl	%r12d, %r12d
   5c935: 66 66 2e 0f 1f 84 00 00 00 00 00     	nopw	%cs:(%rax,%rax)
   5c940: 48 8d 05 a9 8c 13 00         	leaq	0x138ca9(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c947: 48 8b 00                     	movq	(%rax), %rax
   5c94a: 41 8b 75 00                  	movl	(%r13), %esi
   5c94e: 41 8b 17                     	movl	(%r15), %edx
   5c951: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5c955: 85 f6                        	testl	%esi, %esi
   5c957: 75 0a                        	jne	0x5c963 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xb3>
   5c959: 83 fa 01                     	cmpl	$0x1, %edx
   5c95c: 75 05                        	jne	0x5c963 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xb3>
   5c95e: 48 39 c1                     	cmpq	%rax, %rcx
   5c961: 74 15                        	je	0x5c978 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xc8>
   5c963: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c967: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c96b: e8 10 31 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c970: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c974: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c978: 41 39 dc                     	cmpl	%ebx, %r12d
   5c97b: 7d 38                        	jge	0x5c9b5 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x105>
   5c97d: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c981: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5c985: 48 8b 7d a8                  	movq	-0x58(%rbp), %rdi
   5c989: 48 89 7d b0                  	movq	%rdi, -0x50(%rbp)
   5c98d: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c991: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5c995: 44 89 f6                     	movl	%r14d, %esi
   5c998: e8 33 00 00 00               	callq	0x5c9d0 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>
   5c99d: 41 89 c6                     	movl	%eax, %r14d
   5c9a0: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5c9a4: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5c9a8: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5c9ac: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5c9b0: 41 ff c4                     	incl	%r12d
   5c9b3: eb 8b                        	jmp	0x5c940 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x90>
   5c9b5: 44 89 f0                     	movl	%r14d, %eax
   5c9b8: 48 83 c4 38                  	addq	$0x38, %rsp
   5c9bc: 5b                           	popq	%rbx
   5c9bd: 41 5c                        	popq	%r12
   5c9bf: 41 5d                        	popq	%r13
   5c9c1: 41 5e                        	popq	%r14
   5c9c3: 41 5f                        	popq	%r15
   5c9c5: 5d                           	popq	%rbp
   5c9c6: c3                           	retq

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005c790 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
   5c790: 55                           	pushq	%rbp
   5c791: 48 89 e5                     	movq	%rsp, %rbp
   5c794: 41 57                        	pushq	%r15
   5c796: 41 56                        	pushq	%r14
   5c798: 41 55                        	pushq	%r13
   5c79a: 41 54                        	pushq	%r12
   5c79c: 53                           	pushq	%rbx
   5c79d: 48 83 ec 28                  	subq	$0x28, %rsp
   5c7a1: 89 d3                        	movl	%edx, %ebx
   5c7a3: 49 89 f6                     	movq	%rsi, %r14
   5c7a6: 49 89 ff                     	movq	%rdi, %r15
   5c7a9: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c7b2: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5c7b9: 4c 8b 20                     	movq	(%rax), %r12
   5c7bc: 4c 89 7d c8                  	movq	%r15, -0x38(%rbp)
   5c7c0: 48 c7 45 b0 00 00 00 00      	movq	$0x0, -0x50(%rbp)
   5c7c8: 48 8d 05 21 8e 13 00         	leaq	0x138e21(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c7cf: 48 8b 00                     	movq	(%rax), %rax
   5c7d2: 48 8d 0d 1f 8e 13 00         	leaq	0x138e1f(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5c7d9: 8b 31                        	movl	(%rcx), %esi
   5c7db: 41 8b 14 24                  	movl	(%r12), %edx
   5c7df: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5c7e4: 85 f6                        	testl	%esi, %esi
   5c7e6: 75 0a                        	jne	0x5c7f2 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x62>
   5c7e8: 83 fa 01                     	cmpl	$0x1, %edx
   5c7eb: 75 05                        	jne	0x5c7f2 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x62>
   5c7ed: 48 39 c1                     	cmpq	%rax, %rcx
   5c7f0: 74 15                        	je	0x5c807 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x77>
   5c7f2: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c7f6: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c7fa: e8 81 32 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c7ff: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c803: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c807: 41 bf 78 56 34 12            	movl	$0x12345678, %r15d      # imm = 0x12345678
   5c80d: 45 31 ed                     	xorl	%r13d, %r13d
   5c810: 48 8d 05 d9 8d 13 00         	leaq	0x138dd9(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c817: 48 8b 00                     	movq	(%rax), %rax
   5c81a: 48 8d 0d d7 8d 13 00         	leaq	0x138dd7(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5c821: 8b 31                        	movl	(%rcx), %esi
   5c823: 41 8b 14 24                  	movl	(%r12), %edx
   5c827: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5c82c: 85 f6                        	testl	%esi, %esi
   5c82e: 75 0a                        	jne	0x5c83a <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xaa>
   5c830: 83 fa 01                     	cmpl	$0x1, %edx
   5c833: 75 05                        	jne	0x5c83a <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xaa>
   5c835: 48 39 c1                     	cmpq	%rax, %rcx
   5c838: 74 15                        	je	0x5c84f <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xbf>
   5c83a: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c83e: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c842: e8 39 32 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c847: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c84b: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c84f: 41 39 dd                     	cmpl	%ebx, %r13d
   5c852: 7d 3c                        	jge	0x5c890 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x100>
   5c854: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c858: 48 89 45 b8                  	movq	%rax, -0x48(%rbp)
   5c85c: 48 8b 4d b8                  	movq	-0x48(%rbp), %rcx
   5c860: 48 8b 7d b8                  	movq	-0x48(%rbp), %rdi
   5c864: 48 89 7d b0                  	movq	%rdi, -0x50(%rbp)
   5c868: 49 8b 0e                     	movq	(%r14), %rcx
   5c86b: 48 89 7d c0                  	movq	%rdi, -0x40(%rbp)
   5c86f: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c873: 44 89 fe                     	movl	%r15d, %esi
   5c876: ff d1                        	callq	*%rcx
   5c878: 41 89 c7                     	movl	%eax, %r15d
   5c87b: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5c87f: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5c883: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5c887: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5c88b: 41 ff c5                     	incl	%r13d
   5c88e: eb 80                        	jmp	0x5c810 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x80>
   5c890: 44 89 f8                     	movl	%r15d, %eax
   5c893: 48 83 c4 28                  	addq	$0x28, %rsp
   5c897: 5b                           	popq	%rbx
   5c898: 41 5c                        	popq	%r12
   5c89a: 41 5d                        	popq	%r13
   5c89c: 41 5e                        	popq	%r14
   5c89e: 41 5f                        	popq	%r15
   5c8a0: 5d                           	popq	%rbp
   5c8a1: c3                           	retq

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b8e0 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
   5b8e0: 55                           	pushq	%rbp
   5b8e1: 48 89 e5                     	movq	%rsp, %rbp
   5b8e4: 41 57                        	pushq	%r15
   5b8e6: 41 56                        	pushq	%r14
   5b8e8: 41 55                        	pushq	%r13
   5b8ea: 41 54                        	pushq	%r12
   5b8ec: 53                           	pushq	%rbx
   5b8ed: 48 83 ec 38                  	subq	$0x38, %rsp
   5b8f1: 89 f3                        	movl	%esi, %ebx
   5b8f3: 49 89 fe                     	movq	%rdi, %r14
   5b8f6: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b8ff: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b906: 4c 8b 20                     	movq	(%rax), %r12
   5b909: 4c 89 75 c0                  	movq	%r14, -0x40(%rbp)
   5b90d: 48 c7 45 a8 00 00 00 00      	movq	$0x0, -0x58(%rbp)
   5b915: 48 8d 05 d4 9c 13 00         	leaq	0x139cd4(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5b91c: 48 8b 00                     	movq	(%rax), %rax
   5b91f: 48 8d 0d d2 9c 13 00         	leaq	0x139cd2(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5b926: 8b 31                        	movl	(%rcx), %esi
   5b928: 41 8b 14 24                  	movl	(%r12), %edx
   5b92c: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5b931: 85 f6                        	testl	%esi, %esi
   5b933: 75 0a                        	jne	0x5b93f <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x5f>
   5b935: 83 fa 01                     	cmpl	$0x1, %edx
   5b938: 75 05                        	jne	0x5b93f <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x5f>
   5b93a: 48 39 c1                     	cmpq	%rax, %rcx
   5b93d: 74 15                        	je	0x5b954 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x74>
   5b93f: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5b943: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b947: e8 34 41 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5b94c: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b950: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   5b954: 4c 8b 7d c0                  	movq	-0x40(%rbp), %r15
   5b958: 48 8d 35 d1 dd 10 00         	leaq	0x10ddd1(%rip), %rsi    # 0x169730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5b95f: 4c 89 ff                     	movq	%r15, %rdi
   5b962: e8 f9 3a 01 00               	callq	0x6f460 <scoop_rt_is_instance>
   5b967: a8 01                        	testb	$0x1, %al
   5b969: 0f 84 b3 00 00 00            	je	0x5ba22 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x142>
   5b96f: 49 8b 3f                     	movq	(%r15), %rdi
   5b972: 48 8d 35 b7 dd 10 00         	leaq	0x10ddb7(%rip), %rsi    # 0x169730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5b979: e8 22 3b 01 00               	callq	0x6f4a0 <scoop_rt_itable_lookup>
   5b97e: 49 89 c6                     	movq	%rax, %r14
   5b981: 4c 89 7d c8                  	movq	%r15, -0x38(%rbp)
   5b985: 41 bf 78 56 34 12            	movl	$0x12345678, %r15d      # imm = 0x12345678
   5b98b: 45 31 ed                     	xorl	%r13d, %r13d
   5b98e: 66 90                        	nop
   5b990: 48 8d 05 59 9c 13 00         	leaq	0x139c59(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5b997: 48 8b 00                     	movq	(%rax), %rax
   5b99a: 48 8d 0d 57 9c 13 00         	leaq	0x139c57(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5b9a1: 8b 31                        	movl	(%rcx), %esi
   5b9a3: 41 8b 14 24                  	movl	(%r12), %edx
   5b9a7: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5b9ac: 85 f6                        	testl	%esi, %esi
   5b9ae: 75 0a                        	jne	0x5b9ba <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xda>
   5b9b0: 83 fa 01                     	cmpl	$0x1, %edx
   5b9b3: 75 05                        	jne	0x5b9ba <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xda>
   5b9b5: 48 39 c1                     	cmpq	%rax, %rcx
   5b9b8: 74 15                        	je	0x5b9cf <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xef>
   5b9ba: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b9be: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b9c2: e8 b9 40 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5b9c7: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b9cb: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b9cf: 41 39 dd                     	cmpl	%ebx, %r13d
   5b9d2: 7d 3c                        	jge	0x5ba10 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x130>
   5b9d4: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b9d8: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5b9dc: 48 8b 4d b0                  	movq	-0x50(%rbp), %rcx
   5b9e0: 48 8b 7d b0                  	movq	-0x50(%rbp), %rdi
   5b9e4: 48 89 7d a8                  	movq	%rdi, -0x58(%rbp)
   5b9e8: 49 8b 0e                     	movq	(%r14), %rcx
   5b9eb: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5b9ef: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b9f3: 44 89 fe                     	movl	%r15d, %esi
   5b9f6: ff d1                        	callq	*%rcx
   5b9f8: 41 89 c7                     	movl	%eax, %r15d
   5b9fb: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5b9ff: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5ba03: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5ba07: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5ba0b: 41 ff c5                     	incl	%r13d
   5ba0e: eb 80                        	jmp	0x5b990 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xb0>
   5ba10: 44 89 f8                     	movl	%r15d, %eax
   5ba13: 48 83 c4 38                  	addq	$0x38, %rsp
   5ba17: 5b                           	popq	%rbx
   5ba18: 41 5c                        	popq	%r12
   5ba1a: 41 5d                        	popq	%r13
   5ba1c: 41 5e                        	popq	%r14
   5ba1e: 41 5f                        	popq	%r15
   5ba20: 5d                           	popq	%rbp
   5ba21: c3                           	retq
   5ba22: 48 8b 1d 3f b4 09 00         	movq	0x9b43f(%rip), %rbx     # 0xf6e68 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   5ba29: 49 89 df                     	movq	%rbx, %r15
   5ba2c: 49 f7 df                     	negq	%r15
   5ba2f: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5ba39: 48 01 d8                     	addq	%rbx, %rax
   5ba3c: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   5ba40: 41 0f 92 c4                  	setb	%r12b
   5ba44: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   5ba48: 4d 21 fe                     	andq	%r15, %r14
   5ba4b: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5ba54: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   5ba5b: 48 8b 00                     	movq	(%rax), %rax
   5ba5e: 48 8b 08                     	movq	(%rax), %rcx
   5ba61: 48 01 cb                     	addq	%rcx, %rbx
   5ba64: 48 ff cb                     	decq	%rbx
   5ba67: 4c 21 fb                     	andq	%r15, %rbx
   5ba6a: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   5ba6e: 48 85 db                     	testq	%rbx, %rbx
   5ba71: 0f 95 c2                     	setne	%dl
   5ba74: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5ba7b: 40 0f 92 c6                  	setb	%sil
   5ba7f: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   5ba83: 40 0f 96 c7                  	setbe	%dil
   5ba87: 48 83 3d 51 b4 09 00 00      	cmpq	$0x0, 0x9b451(%rip)     # 0xf6ee0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   5ba8f: 41 0f 94 c0                  	sete	%r8b
   5ba93: 41 20 f0                     	andb	%sil, %r8b
   5ba96: 41 20 d0                     	andb	%dl, %r8b
   5ba99: 41 20 f8                     	andb	%dil, %r8b
   5ba9c: 45 84 e0                     	testb	%r12b, %r8b
   5ba9f: 74 17                        	je	0x5bab8 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1d8>
   5baa1: 48 89 08                     	movq	%rcx, (%rax)
   5baa4: 48 8d 35 a5 b3 09 00         	leaq	0x9b3a5(%rip), %rsi     # 0xf6e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5baab: 48 89 df                     	movq	%rbx, %rdi
   5baae: 4c 89 f2                     	movq	%r14, %rdx
   5bab1: e8 ea 8e 00 00               	callq	0x649a0 <scoop_runtime_finish_tlab_alloc>
   5bab6: eb 14                        	jmp	0x5bacc <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1ec>
   5bab8: 48 8d 3d 91 b3 09 00         	leaq	0x9b391(%rip), %rdi     # 0xf6e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5babf: be 18 00 00 00               	movl	$0x18, %esi
   5bac4: e8 f7 3f 00 00               	callq	0x5fac0 <scoop_runtime_alloc_slow>
   5bac9: 48 89 c3                     	movq	%rax, %rbx
   5bacc: 48 89 5d d0                  	movq	%rbx, -0x30(%rbp)
   5bad0: 48 89 df                     	movq	%rbx, %rdi
   5bad3: e8 88 dd fd ff               	callq	0x39860 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   5bad8: 48 8b 7d d0                  	movq	-0x30(%rbp), %rdi
   5badc: 48 89 7d a0                  	movq	%rdi, -0x60(%rbp)
   5bae0: e8 ab ed 00 00               	callq	0x6a890 <scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/home/chenxu/repos/scoop/tmp/m34/verified-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005c580 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
   5c580: 55                           	pushq	%rbp
   5c581: 48 89 e5                     	movq	%rsp, %rbp
   5c584: 41 57                        	pushq	%r15
   5c586: 41 56                        	pushq	%r14
   5c588: 41 55                        	pushq	%r13
   5c58a: 41 54                        	pushq	%r12
   5c58c: 53                           	pushq	%rbx
   5c58d: 48 83 ec 48                  	subq	$0x48, %rsp
   5c591: 89 75 c4                     	movl	%esi, -0x3c(%rbp)
   5c594: 49 89 fe                     	movq	%rdi, %r14
   5c597: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c5a0: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5c5a7: 4c 8b 28                     	movq	(%rax), %r13
   5c5aa: 4c 89 75 c8                  	movq	%r14, -0x38(%rbp)
   5c5ae: 48 c7 45 a8 00 00 00 00      	movq	$0x0, -0x58(%rbp)
   5c5b6: 48 8d 05 33 90 13 00         	leaq	0x139033(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c5bd: 48 8b 00                     	movq	(%rax), %rax
   5c5c0: 48 8d 0d 31 90 13 00         	leaq	0x139031(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5c5c7: 8b 31                        	movl	(%rcx), %esi
   5c5c9: 41 8b 55 00                  	movl	(%r13), %edx
   5c5cd: 49 8b 4d 08                  	movq	0x8(%r13), %rcx
   5c5d1: 85 f6                        	testl	%esi, %esi
   5c5d3: 75 0a                        	jne	0x5c5df <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x5f>
   5c5d5: 83 fa 01                     	cmpl	$0x1, %edx
   5c5d8: 75 05                        	jne	0x5c5df <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x5f>
   5c5da: 48 39 c1                     	cmpq	%rax, %rcx
   5c5dd: 74 15                        	je	0x5c5f4 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x74>
   5c5df: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c5e3: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c5e7: e8 94 34 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c5ec: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c5f0: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c5f4: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   5c5fa: 31 db                        	xorl	%ebx, %ebx
   5c5fc: 4c 8d 3d 2d d1 10 00         	leaq	0x10d12d(%rip), %r15    # 0x169730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5c603: 66 66 66 66 2e 0f 1f 84 00 00 00 00 00       	nopw	%cs:(%rax,%rax)
   5c610: 48 8d 05 d9 8f 13 00         	leaq	0x138fd9(%rip), %rax    # 0x1955f0 <scoop_thread_gc_epoch>
   5c617: 48 8b 00                     	movq	(%rax), %rax
   5c61a: 48 8d 0d d7 8f 13 00         	leaq	0x138fd7(%rip), %rcx    # 0x1955f8 <scoop_thread_world_phase>
   5c621: 8b 31                        	movl	(%rcx), %esi
   5c623: 41 8b 55 00                  	movl	(%r13), %edx
   5c627: 49 8b 4d 08                  	movq	0x8(%r13), %rcx
   5c62b: 85 f6                        	testl	%esi, %esi
   5c62d: 75 0a                        	jne	0x5c639 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xb9>
   5c62f: 83 fa 01                     	cmpl	$0x1, %edx
   5c632: 75 05                        	jne	0x5c639 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xb9>
   5c634: 48 39 c1                     	cmpq	%rax, %rcx
   5c637: 74 15                        	je	0x5c64e <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xce>
   5c639: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5c63d: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5c641: e8 3a 34 00 00               	callq	0x5fa80 <scoop_rt_safepoint>
   5c646: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5c64a: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5c64e: 3b 5d c4                     	cmpl	-0x3c(%rbp), %ebx
   5c651: 7d 65                        	jge	0x5c6b8 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x138>
   5c653: 4c 8b 65 c8                  	movq	-0x38(%rbp), %r12
   5c657: 4c 89 e7                     	movq	%r12, %rdi
   5c65a: 4c 89 fe                     	movq	%r15, %rsi
   5c65d: e8 fe 2d 01 00               	callq	0x6f460 <scoop_rt_is_instance>
   5c662: a8 01                        	testb	$0x1, %al
   5c664: 74 64                        	je	0x5c6ca <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x14a>
   5c666: 49 8b 3c 24                  	movq	(%r12), %rdi
   5c66a: 4c 89 fe                     	movq	%r15, %rsi
   5c66d: e8 2e 2e 01 00               	callq	0x6f4a0 <scoop_rt_itable_lookup>
   5c672: 4c 89 65 a0                  	movq	%r12, -0x60(%rbp)
   5c676: 48 8b 4d a0                  	movq	-0x60(%rbp), %rcx
   5c67a: 48 89 4d b0                  	movq	%rcx, -0x50(%rbp)
   5c67e: 48 8b 4d b0                  	movq	-0x50(%rbp), %rcx
   5c682: 48 8b 7d b0                  	movq	-0x50(%rbp), %rdi
   5c686: 48 89 7d a8                  	movq	%rdi, -0x58(%rbp)
   5c68a: 48 8b 4d c8                  	movq	-0x38(%rbp), %rcx
   5c68e: 48 8b 00                     	movq	(%rax), %rax
   5c691: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5c695: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   5c699: 44 89 f6                     	movl	%r14d, %esi
   5c69c: ff d0                        	callq	*%rax
   5c69e: 41 89 c6                     	movl	%eax, %r14d
   5c6a1: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5c6a5: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5c6a9: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5c6ad: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5c6b1: ff c3                        	incl	%ebx
   5c6b3: e9 58 ff ff ff               	jmp	0x5c610 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x90>
   5c6b8: 44 89 f0                     	movl	%r14d, %eax
   5c6bb: 48 83 c4 48                  	addq	$0x48, %rsp
   5c6bf: 5b                           	popq	%rbx
   5c6c0: 41 5c                        	popq	%r12
   5c6c2: 41 5d                        	popq	%r13
   5c6c4: 41 5e                        	popq	%r14
   5c6c6: 41 5f                        	popq	%r15
   5c6c8: 5d                           	popq	%rbp
   5c6c9: c3                           	retq
   5c6ca: 48 8b 1d 97 a7 09 00         	movq	0x9a797(%rip), %rbx     # 0xf6e68 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   5c6d1: 49 89 df                     	movq	%rbx, %r15
   5c6d4: 49 f7 df                     	negq	%r15
   5c6d7: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5c6e1: 48 01 d8                     	addq	%rbx, %rax
   5c6e4: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   5c6e8: 41 0f 92 c4                  	setb	%r12b
   5c6ec: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   5c6f0: 4d 21 fe                     	andq	%r15, %r14
   5c6f3: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5c6fc: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   5c703: 48 8b 00                     	movq	(%rax), %rax
   5c706: 48 8b 08                     	movq	(%rax), %rcx
   5c709: 48 01 cb                     	addq	%rcx, %rbx
   5c70c: 48 ff cb                     	decq	%rbx
   5c70f: 4c 21 fb                     	andq	%r15, %rbx
   5c712: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   5c716: 48 85 db                     	testq	%rbx, %rbx
   5c719: 0f 95 c2                     	setne	%dl
   5c71c: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5c723: 40 0f 92 c6                  	setb	%sil
   5c727: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   5c72b: 40 0f 96 c7                  	setbe	%dil
   5c72f: 48 83 3d a9 a7 09 00 00      	cmpq	$0x0, 0x9a7a9(%rip)     # 0xf6ee0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   5c737: 41 0f 94 c0                  	sete	%r8b
   5c73b: 41 20 f0                     	andb	%sil, %r8b
   5c73e: 41 20 d0                     	andb	%dl, %r8b
   5c741: 41 20 f8                     	andb	%dil, %r8b
   5c744: 45 84 e0                     	testb	%r12b, %r8b
   5c747: 74 17                        	je	0x5c760 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1e0>
   5c749: 48 89 08                     	movq	%rcx, (%rax)
   5c74c: 48 8d 35 fd a6 09 00         	leaq	0x9a6fd(%rip), %rsi     # 0xf6e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5c753: 48 89 df                     	movq	%rbx, %rdi
   5c756: 4c 89 f2                     	movq	%r14, %rdx
   5c759: e8 42 82 00 00               	callq	0x649a0 <scoop_runtime_finish_tlab_alloc>
   5c75e: eb 14                        	jmp	0x5c774 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
   5c760: 48 8d 3d e9 a6 09 00         	leaq	0x9a6e9(%rip), %rdi     # 0xf6e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5c767: be 18 00 00 00               	movl	$0x18, %esi
   5c76c: e8 4f 33 00 00               	callq	0x5fac0 <scoop_runtime_alloc_slow>
   5c771: 48 89 c3                     	movq	%rax, %rbx
   5c774: 48 89 5d d0                  	movq	%rbx, -0x30(%rbp)
   5c778: 48 89 df                     	movq	%rbx, %rdi
   5c77b: e8 e0 d0 fd ff               	callq	0x39860 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   5c780: 48 8b 7d d0                  	movq	-0x30(%rbp), %rdi
   5c784: 48 89 7d 98                  	movq	%rdi, -0x68(%rbp)
   5c788: e8 03 e1 00 00               	callq	0x6a890 <scoop_rt_throw>
