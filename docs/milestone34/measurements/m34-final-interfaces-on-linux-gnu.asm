; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b5a0 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
   5b5a0: 55                           	pushq	%rbp
   5b5a1: 48 89 e5                     	movq	%rsp, %rbp
   5b5a4: 41 56                        	pushq	%r14
   5b5a6: 53                           	pushq	%rbx
   5b5a7: 48 83 ec 10                  	subq	$0x10, %rsp
   5b5ab: 89 f3                        	movl	%esi, %ebx
   5b5ad: 49 89 fe                     	movq	%rdi, %r14
   5b5b0: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b5b9: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b5c0: 48 8b 08                     	movq	(%rax), %rcx
   5b5c3: 4c 89 75 e8                  	movq	%r14, -0x18(%rbp)
   5b5c7: 48 8d 05 22 90 13 00         	leaq	0x139022(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b5ce: 48 8b 00                     	movq	(%rax), %rax
   5b5d1: 48 8d 15 20 90 13 00         	leaq	0x139020(%rip), %rdx    # 0x1945f8 <scoop_thread_world_phase>
   5b5d8: 8b 32                        	movl	(%rdx), %esi
   5b5da: 8b 11                        	movl	(%rcx), %edx
   5b5dc: 48 8b 49 08                  	movq	0x8(%rcx), %rcx
   5b5e0: 85 f6                        	testl	%esi, %esi
   5b5e2: 75 0a                        	jne	0x5b5ee <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x4e>
   5b5e4: 83 fa 01                     	cmpl	$0x1, %edx
   5b5e7: 75 05                        	jne	0x5b5ee <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x4e>
   5b5e9: 48 39 c1                     	cmpq	%rax, %rcx
   5b5ec: 74 15                        	je	0x5b603 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0+0x63>
   5b5ee: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5b5f2: 48 89 45 e0                  	movq	%rax, -0x20(%rbp)
   5b5f6: e8 45 30 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b5fb: 48 8b 45 e0                  	movq	-0x20(%rbp), %rax
   5b5ff: 48 89 45 e8                  	movq	%rax, -0x18(%rbp)
   5b603: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5b607: 89 d9                        	movl	%ebx, %ecx
   5b609: c1 e1 0d                     	shll	$0xd, %ecx
   5b60c: 33 48 10                     	xorl	0x10(%rax), %ecx
   5b60f: 31 d9                        	xorl	%ebx, %ecx
   5b611: 89 ca                        	movl	%ecx, %edx
   5b613: c1 ea 11                     	shrl	$0x11, %edx
   5b616: 31 ca                        	xorl	%ecx, %edx
   5b618: 89 d0                        	movl	%edx, %eax
   5b61a: c1 e0 05                     	shll	$0x5, %eax
   5b61d: 31 d0                        	xorl	%edx, %eax
   5b61f: 48 83 c4 10                  	addq	$0x10, %rsp
   5b623: 5b                           	popq	%rbx
   5b624: 41 5e                        	popq	%r14
   5b626: 5d                           	popq	%rbp
   5b627: c3                           	retq

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b680 <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
   5b680: 55                           	pushq	%rbp
   5b681: 48 89 e5                     	movq	%rsp, %rbp
   5b684: 53                           	pushq	%rbx
   5b685: 48 83 ec 18                  	subq	$0x18, %rsp
   5b689: 48 89 fb                     	movq	%rdi, %rbx
   5b68c: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b695: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b69c: 48 8b 08                     	movq	(%rax), %rcx
   5b69f: 48 89 5d f0                  	movq	%rbx, -0x10(%rbp)
   5b6a3: 48 8d 05 46 8f 13 00         	leaq	0x138f46(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b6aa: 48 8b 00                     	movq	(%rax), %rax
   5b6ad: 48 8d 15 44 8f 13 00         	leaq	0x138f44(%rip), %rdx    # 0x1945f8 <scoop_thread_world_phase>
   5b6b4: 8b 32                        	movl	(%rdx), %esi
   5b6b6: 8b 11                        	movl	(%rcx), %edx
   5b6b8: 48 8b 49 08                  	movq	0x8(%rcx), %rcx
   5b6bc: 85 f6                        	testl	%esi, %esi
   5b6be: 75 0a                        	jne	0x5b6ca <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x4a>
   5b6c0: 83 fa 01                     	cmpl	$0x1, %edx
   5b6c3: 75 05                        	jne	0x5b6ca <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x4a>
   5b6c5: 48 39 c1                     	cmpq	%rax, %rcx
   5b6c8: 74 15                        	je	0x5b6df <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be+0x5f>
   5b6ca: 48 8b 45 f0                  	movq	-0x10(%rbp), %rax
   5b6ce: 48 89 45 e8                  	movq	%rax, -0x18(%rbp)
   5b6d2: e8 69 2f 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b6d7: 48 8b 45 e8                  	movq	-0x18(%rbp), %rax
   5b6db: 48 89 45 f0                  	movq	%rax, -0x10(%rbp)
   5b6df: 48 8b 45 f0                  	movq	-0x10(%rbp), %rax
   5b6e3: 8b 40 10                     	movl	0x10(%rax), %eax
   5b6e6: 48 83 c4 18                  	addq	$0x18, %rsp
   5b6ea: 5b                           	popq	%rbx
   5b6eb: 5d                           	popq	%rbp
   5b6ec: c3                           	retq

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b480 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
   5b480: 55                           	pushq	%rbp
   5b481: 48 89 e5                     	movq	%rsp, %rbp
   5b484: 41 57                        	pushq	%r15
   5b486: 41 56                        	pushq	%r14
   5b488: 41 55                        	pushq	%r13
   5b48a: 41 54                        	pushq	%r12
   5b48c: 53                           	pushq	%rbx
   5b48d: 48 83 ec 38                  	subq	$0x38, %rsp
   5b491: 89 f3                        	movl	%esi, %ebx
   5b493: 49 89 fe                     	movq	%rdi, %r14
   5b496: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b49f: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b4a6: 4c 8b 38                     	movq	(%rax), %r15
   5b4a9: 4c 89 75 c0                  	movq	%r14, -0x40(%rbp)
   5b4ad: 48 c7 45 b0 00 00 00 00      	movq	$0x0, -0x50(%rbp)
   5b4b5: 48 8d 05 34 91 13 00         	leaq	0x139134(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b4bc: 48 8b 00                     	movq	(%rax), %rax
   5b4bf: 4c 8d 2d 32 91 13 00         	leaq	0x139132(%rip), %r13    # 0x1945f8 <scoop_thread_world_phase>
   5b4c6: 41 8b 75 00                  	movl	(%r13), %esi
   5b4ca: 41 8b 17                     	movl	(%r15), %edx
   5b4cd: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5b4d1: 85 f6                        	testl	%esi, %esi
   5b4d3: 75 0a                        	jne	0x5b4df <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x5f>
   5b4d5: 83 fa 01                     	cmpl	$0x1, %edx
   5b4d8: 75 05                        	jne	0x5b4df <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x5f>
   5b4da: 48 39 c1                     	cmpq	%rax, %rcx
   5b4dd: 74 15                        	je	0x5b4f4 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x74>
   5b4df: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5b4e3: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b4e7: e8 54 31 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b4ec: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b4f0: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   5b4f4: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5b4f8: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b4fc: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   5b502: 45 31 e4                     	xorl	%r12d, %r12d
   5b505: 66 66 2e 0f 1f 84 00 00 00 00 00     	nopw	%cs:(%rax,%rax)
   5b510: 48 8d 05 d9 90 13 00         	leaq	0x1390d9(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b517: 48 8b 00                     	movq	(%rax), %rax
   5b51a: 41 8b 75 00                  	movl	(%r13), %esi
   5b51e: 41 8b 17                     	movl	(%r15), %edx
   5b521: 49 8b 4f 08                  	movq	0x8(%r15), %rcx
   5b525: 85 f6                        	testl	%esi, %esi
   5b527: 75 0a                        	jne	0x5b533 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xb3>
   5b529: 83 fa 01                     	cmpl	$0x1, %edx
   5b52c: 75 05                        	jne	0x5b533 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xb3>
   5b52e: 48 39 c1                     	cmpq	%rax, %rcx
   5b531: 74 15                        	je	0x5b548 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0xc8>
   5b533: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b537: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b53b: e8 00 31 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b540: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b544: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b548: 41 39 dc                     	cmpl	%ebx, %r12d
   5b54b: 7d 38                        	jge	0x5b585 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x105>
   5b54d: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b551: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5b555: 48 8b 7d a8                  	movq	-0x58(%rbp), %rdi
   5b559: 48 89 7d b0                  	movq	%rdi, -0x50(%rbp)
   5b55d: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b561: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5b565: 44 89 f6                     	movl	%r14d, %esi
   5b568: e8 33 00 00 00               	callq	0x5b5a0 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>
   5b56d: 41 89 c6                     	movl	%eax, %r14d
   5b570: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5b574: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5b578: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5b57c: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5b580: 41 ff c4                     	incl	%r12d
   5b583: eb 8b                        	jmp	0x5b510 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x90>
   5b585: 44 89 f0                     	movl	%r14d, %eax
   5b588: 48 83 c4 38                  	addq	$0x38, %rsp
   5b58c: 5b                           	popq	%rbx
   5b58d: 41 5c                        	popq	%r12
   5b58f: 41 5d                        	popq	%r13
   5b591: 41 5e                        	popq	%r14
   5b593: 41 5f                        	popq	%r15
   5b595: 5d                           	popq	%rbp
   5b596: c3                           	retq

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b360 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
   5b360: 55                           	pushq	%rbp
   5b361: 48 89 e5                     	movq	%rsp, %rbp
   5b364: 41 57                        	pushq	%r15
   5b366: 41 56                        	pushq	%r14
   5b368: 41 55                        	pushq	%r13
   5b36a: 41 54                        	pushq	%r12
   5b36c: 53                           	pushq	%rbx
   5b36d: 48 83 ec 28                  	subq	$0x28, %rsp
   5b371: 89 d3                        	movl	%edx, %ebx
   5b373: 49 89 f6                     	movq	%rsi, %r14
   5b376: 49 89 ff                     	movq	%rdi, %r15
   5b379: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b382: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b389: 4c 8b 20                     	movq	(%rax), %r12
   5b38c: 4c 89 7d c8                  	movq	%r15, -0x38(%rbp)
   5b390: 48 c7 45 b0 00 00 00 00      	movq	$0x0, -0x50(%rbp)
   5b398: 48 8d 05 51 92 13 00         	leaq	0x139251(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b39f: 48 8b 00                     	movq	(%rax), %rax
   5b3a2: 48 8d 0d 4f 92 13 00         	leaq	0x13924f(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5b3a9: 8b 31                        	movl	(%rcx), %esi
   5b3ab: 41 8b 14 24                  	movl	(%r12), %edx
   5b3af: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5b3b4: 85 f6                        	testl	%esi, %esi
   5b3b6: 75 0a                        	jne	0x5b3c2 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x62>
   5b3b8: 83 fa 01                     	cmpl	$0x1, %edx
   5b3bb: 75 05                        	jne	0x5b3c2 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x62>
   5b3bd: 48 39 c1                     	cmpq	%rax, %rcx
   5b3c0: 74 15                        	je	0x5b3d7 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x77>
   5b3c2: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b3c6: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b3ca: e8 71 32 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b3cf: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b3d3: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b3d7: 41 bf 78 56 34 12            	movl	$0x12345678, %r15d      # imm = 0x12345678
   5b3dd: 45 31 ed                     	xorl	%r13d, %r13d
   5b3e0: 48 8d 05 09 92 13 00         	leaq	0x139209(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b3e7: 48 8b 00                     	movq	(%rax), %rax
   5b3ea: 48 8d 0d 07 92 13 00         	leaq	0x139207(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5b3f1: 8b 31                        	movl	(%rcx), %esi
   5b3f3: 41 8b 14 24                  	movl	(%r12), %edx
   5b3f7: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5b3fc: 85 f6                        	testl	%esi, %esi
   5b3fe: 75 0a                        	jne	0x5b40a <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xaa>
   5b400: 83 fa 01                     	cmpl	$0x1, %edx
   5b403: 75 05                        	jne	0x5b40a <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xaa>
   5b405: 48 39 c1                     	cmpq	%rax, %rcx
   5b408: 74 15                        	je	0x5b41f <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0xbf>
   5b40a: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b40e: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b412: e8 29 32 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b417: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b41b: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b41f: 41 39 dd                     	cmpl	%ebx, %r13d
   5b422: 7d 3c                        	jge	0x5b460 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x100>
   5b424: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b428: 48 89 45 b8                  	movq	%rax, -0x48(%rbp)
   5b42c: 48 8b 4d b8                  	movq	-0x48(%rbp), %rcx
   5b430: 48 8b 7d b8                  	movq	-0x48(%rbp), %rdi
   5b434: 48 89 7d b0                  	movq	%rdi, -0x50(%rbp)
   5b438: 49 8b 0e                     	movq	(%r14), %rcx
   5b43b: 48 89 7d c0                  	movq	%rdi, -0x40(%rbp)
   5b43f: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b443: 44 89 fe                     	movl	%r15d, %esi
   5b446: ff d1                        	callq	*%rcx
   5b448: 41 89 c7                     	movl	%eax, %r15d
   5b44b: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5b44f: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5b453: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5b457: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5b45b: 41 ff c5                     	incl	%r13d
   5b45e: eb 80                        	jmp	0x5b3e0 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x80>
   5b460: 44 89 f8                     	movl	%r15d, %eax
   5b463: 48 83 c4 28                  	addq	$0x28, %rsp
   5b467: 5b                           	popq	%rbx
   5b468: 41 5c                        	popq	%r12
   5b46a: 41 5d                        	popq	%r13
   5b46c: 41 5e                        	popq	%r14
   5b46e: 41 5f                        	popq	%r15
   5b470: 5d                           	popq	%rbp
   5b471: c3                           	retq

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005a510 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
   5a510: 55                           	pushq	%rbp
   5a511: 48 89 e5                     	movq	%rsp, %rbp
   5a514: 41 57                        	pushq	%r15
   5a516: 41 56                        	pushq	%r14
   5a518: 41 55                        	pushq	%r13
   5a51a: 41 54                        	pushq	%r12
   5a51c: 53                           	pushq	%rbx
   5a51d: 48 83 ec 38                  	subq	$0x38, %rsp
   5a521: 89 f3                        	movl	%esi, %ebx
   5a523: 49 89 fe                     	movq	%rdi, %r14
   5a526: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5a52f: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5a536: 4c 8b 20                     	movq	(%rax), %r12
   5a539: 4c 89 75 c0                  	movq	%r14, -0x40(%rbp)
   5a53d: 48 c7 45 a8 00 00 00 00      	movq	$0x0, -0x58(%rbp)
   5a545: 48 8d 05 a4 a0 13 00         	leaq	0x13a0a4(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5a54c: 48 8b 00                     	movq	(%rax), %rax
   5a54f: 48 8d 0d a2 a0 13 00         	leaq	0x13a0a2(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5a556: 8b 31                        	movl	(%rcx), %esi
   5a558: 41 8b 14 24                  	movl	(%r12), %edx
   5a55c: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5a561: 85 f6                        	testl	%esi, %esi
   5a563: 75 0a                        	jne	0x5a56f <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x5f>
   5a565: 83 fa 01                     	cmpl	$0x1, %edx
   5a568: 75 05                        	jne	0x5a56f <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x5f>
   5a56a: 48 39 c1                     	cmpq	%rax, %rcx
   5a56d: 74 15                        	je	0x5a584 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x74>
   5a56f: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   5a573: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5a577: e8 c4 40 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5a57c: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5a580: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   5a584: 4c 8b 7d c0                  	movq	-0x40(%rbp), %r15
   5a588: 48 8d 35 a1 e1 10 00         	leaq	0x10e1a1(%rip), %rsi    # 0x168730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5a58f: 4c 89 ff                     	movq	%r15, %rdi
   5a592: e8 89 3a 01 00               	callq	0x6e020 <scoop_rt_is_instance>
   5a597: a8 01                        	testb	$0x1, %al
   5a599: 0f 84 b3 00 00 00            	je	0x5a652 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x142>
   5a59f: 49 8b 3f                     	movq	(%r15), %rdi
   5a5a2: 48 8d 35 87 e1 10 00         	leaq	0x10e187(%rip), %rsi    # 0x168730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5a5a9: e8 b2 3a 01 00               	callq	0x6e060 <scoop_rt_itable_lookup>
   5a5ae: 49 89 c6                     	movq	%rax, %r14
   5a5b1: 4c 89 7d c8                  	movq	%r15, -0x38(%rbp)
   5a5b5: 41 bf 78 56 34 12            	movl	$0x12345678, %r15d      # imm = 0x12345678
   5a5bb: 45 31 ed                     	xorl	%r13d, %r13d
   5a5be: 66 90                        	nop
   5a5c0: 48 8d 05 29 a0 13 00         	leaq	0x13a029(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5a5c7: 48 8b 00                     	movq	(%rax), %rax
   5a5ca: 48 8d 0d 27 a0 13 00         	leaq	0x13a027(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5a5d1: 8b 31                        	movl	(%rcx), %esi
   5a5d3: 41 8b 14 24                  	movl	(%r12), %edx
   5a5d7: 49 8b 4c 24 08               	movq	0x8(%r12), %rcx
   5a5dc: 85 f6                        	testl	%esi, %esi
   5a5de: 75 0a                        	jne	0x5a5ea <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xda>
   5a5e0: 83 fa 01                     	cmpl	$0x1, %edx
   5a5e3: 75 05                        	jne	0x5a5ea <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xda>
   5a5e5: 48 39 c1                     	cmpq	%rax, %rcx
   5a5e8: 74 15                        	je	0x5a5ff <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xef>
   5a5ea: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5a5ee: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5a5f2: e8 49 40 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5a5f7: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5a5fb: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5a5ff: 41 39 dd                     	cmpl	%ebx, %r13d
   5a602: 7d 3c                        	jge	0x5a640 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x130>
   5a604: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5a608: 48 89 45 b0                  	movq	%rax, -0x50(%rbp)
   5a60c: 48 8b 4d b0                  	movq	-0x50(%rbp), %rcx
   5a610: 48 8b 7d b0                  	movq	-0x50(%rbp), %rdi
   5a614: 48 89 7d a8                  	movq	%rdi, -0x58(%rbp)
   5a618: 49 8b 0e                     	movq	(%r14), %rcx
   5a61b: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5a61f: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5a623: 44 89 fe                     	movl	%r15d, %esi
   5a626: ff d1                        	callq	*%rcx
   5a628: 41 89 c7                     	movl	%eax, %r15d
   5a62b: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5a62f: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5a633: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5a637: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5a63b: 41 ff c5                     	incl	%r13d
   5a63e: eb 80                        	jmp	0x5a5c0 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xb0>
   5a640: 44 89 f8                     	movl	%r15d, %eax
   5a643: 48 83 c4 38                  	addq	$0x38, %rsp
   5a647: 5b                           	popq	%rbx
   5a648: 41 5c                        	popq	%r12
   5a64a: 41 5d                        	popq	%r13
   5a64c: 41 5e                        	popq	%r14
   5a64e: 41 5f                        	popq	%r15
   5a650: 5d                           	popq	%rbp
   5a651: c3                           	retq
   5a652: 48 8b 1d 0f b8 09 00         	movq	0x9b80f(%rip), %rbx     # 0xf5e68 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   5a659: 49 89 df                     	movq	%rbx, %r15
   5a65c: 49 f7 df                     	negq	%r15
   5a65f: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5a669: 48 01 d8                     	addq	%rbx, %rax
   5a66c: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   5a670: 41 0f 92 c4                  	setb	%r12b
   5a674: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   5a678: 4d 21 fe                     	andq	%r15, %r14
   5a67b: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5a684: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   5a68b: 48 8b 00                     	movq	(%rax), %rax
   5a68e: 48 8b 08                     	movq	(%rax), %rcx
   5a691: 48 01 cb                     	addq	%rcx, %rbx
   5a694: 48 ff cb                     	decq	%rbx
   5a697: 4c 21 fb                     	andq	%r15, %rbx
   5a69a: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   5a69e: 48 85 db                     	testq	%rbx, %rbx
   5a6a1: 0f 95 c2                     	setne	%dl
   5a6a4: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5a6ab: 40 0f 92 c6                  	setb	%sil
   5a6af: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   5a6b3: 40 0f 96 c7                  	setbe	%dil
   5a6b7: 48 83 3d 21 b8 09 00 00      	cmpq	$0x0, 0x9b821(%rip)     # 0xf5ee0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   5a6bf: 41 0f 94 c0                  	sete	%r8b
   5a6c3: 41 20 f0                     	andb	%sil, %r8b
   5a6c6: 41 20 d0                     	andb	%dl, %r8b
   5a6c9: 41 20 f8                     	andb	%dil, %r8b
   5a6cc: 45 84 e0                     	testb	%r12b, %r8b
   5a6cf: 74 17                        	je	0x5a6e8 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1d8>
   5a6d1: 48 89 08                     	movq	%rcx, (%rax)
   5a6d4: 48 8d 35 75 b7 09 00         	leaq	0x9b775(%rip), %rsi     # 0xf5e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5a6db: 48 89 df                     	movq	%rbx, %rdi
   5a6de: 4c 89 f2                     	movq	%r14, %rdx
   5a6e1: e8 7a 8e 00 00               	callq	0x63560 <scoop_runtime_finish_tlab_alloc>
   5a6e6: eb 14                        	jmp	0x5a6fc <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x1ec>
   5a6e8: 48 8d 3d 61 b7 09 00         	leaq	0x9b761(%rip), %rdi     # 0xf5e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5a6ef: be 18 00 00 00               	movl	$0x18, %esi
   5a6f4: e8 87 3f 00 00               	callq	0x5e680 <scoop_runtime_alloc_slow>
   5a6f9: 48 89 c3                     	movq	%rax, %rbx
   5a6fc: 48 89 5d d0                  	movq	%rbx, -0x30(%rbp)
   5a700: 48 89 df                     	movq	%rbx, %rdi
   5a703: e8 28 eb fd ff               	callq	0x39230 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   5a708: 48 8b 7d d0                  	movq	-0x30(%rbp), %rdi
   5a70c: 48 89 7d a0                  	movq	%rdi, -0x60(%rbp)
   5a710: e8 3b ed 00 00               	callq	0x69450 <scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-on-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000005b150 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
   5b150: 55                           	pushq	%rbp
   5b151: 48 89 e5                     	movq	%rsp, %rbp
   5b154: 41 57                        	pushq	%r15
   5b156: 41 56                        	pushq	%r14
   5b158: 41 55                        	pushq	%r13
   5b15a: 41 54                        	pushq	%r12
   5b15c: 53                           	pushq	%rbx
   5b15d: 48 83 ec 48                  	subq	$0x48, %rsp
   5b161: 89 75 c4                     	movl	%esi, -0x3c(%rbp)
   5b164: 49 89 fe                     	movq	%rdi, %r14
   5b167: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b170: 48 8d 80 f0 ff ff ff         	leaq	-0x10(%rax), %rax
   5b177: 4c 8b 28                     	movq	(%rax), %r13
   5b17a: 4c 89 75 c8                  	movq	%r14, -0x38(%rbp)
   5b17e: 48 c7 45 a8 00 00 00 00      	movq	$0x0, -0x58(%rbp)
   5b186: 48 8d 05 63 94 13 00         	leaq	0x139463(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b18d: 48 8b 00                     	movq	(%rax), %rax
   5b190: 48 8d 0d 61 94 13 00         	leaq	0x139461(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5b197: 8b 31                        	movl	(%rcx), %esi
   5b199: 41 8b 55 00                  	movl	(%r13), %edx
   5b19d: 49 8b 4d 08                  	movq	0x8(%r13), %rcx
   5b1a1: 85 f6                        	testl	%esi, %esi
   5b1a3: 75 0a                        	jne	0x5b1af <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x5f>
   5b1a5: 83 fa 01                     	cmpl	$0x1, %edx
   5b1a8: 75 05                        	jne	0x5b1af <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x5f>
   5b1aa: 48 39 c1                     	cmpq	%rax, %rcx
   5b1ad: 74 15                        	je	0x5b1c4 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x74>
   5b1af: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b1b3: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b1b7: e8 84 34 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b1bc: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b1c0: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b1c4: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   5b1ca: 31 db                        	xorl	%ebx, %ebx
   5b1cc: 4c 8d 3d 5d d5 10 00         	leaq	0x10d55d(%rip), %r15    # 0x168730 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   5b1d3: 66 66 66 66 2e 0f 1f 84 00 00 00 00 00       	nopw	%cs:(%rax,%rax)
   5b1e0: 48 8d 05 09 94 13 00         	leaq	0x139409(%rip), %rax    # 0x1945f0 <scoop_thread_gc_epoch>
   5b1e7: 48 8b 00                     	movq	(%rax), %rax
   5b1ea: 48 8d 0d 07 94 13 00         	leaq	0x139407(%rip), %rcx    # 0x1945f8 <scoop_thread_world_phase>
   5b1f1: 8b 31                        	movl	(%rcx), %esi
   5b1f3: 41 8b 55 00                  	movl	(%r13), %edx
   5b1f7: 49 8b 4d 08                  	movq	0x8(%r13), %rcx
   5b1fb: 85 f6                        	testl	%esi, %esi
   5b1fd: 75 0a                        	jne	0x5b209 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xb9>
   5b1ff: 83 fa 01                     	cmpl	$0x1, %edx
   5b202: 75 05                        	jne	0x5b209 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xb9>
   5b204: 48 39 c1                     	cmpq	%rax, %rcx
   5b207: 74 15                        	je	0x5b21e <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xce>
   5b209: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   5b20d: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   5b211: e8 2a 34 00 00               	callq	0x5e640 <scoop_rt_safepoint>
   5b216: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   5b21a: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   5b21e: 3b 5d c4                     	cmpl	-0x3c(%rbp), %ebx
   5b221: 7d 65                        	jge	0x5b288 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x138>
   5b223: 4c 8b 65 c8                  	movq	-0x38(%rbp), %r12
   5b227: 4c 89 e7                     	movq	%r12, %rdi
   5b22a: 4c 89 fe                     	movq	%r15, %rsi
   5b22d: e8 ee 2d 01 00               	callq	0x6e020 <scoop_rt_is_instance>
   5b232: a8 01                        	testb	$0x1, %al
   5b234: 74 64                        	je	0x5b29a <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x14a>
   5b236: 49 8b 3c 24                  	movq	(%r12), %rdi
   5b23a: 4c 89 fe                     	movq	%r15, %rsi
   5b23d: e8 1e 2e 01 00               	callq	0x6e060 <scoop_rt_itable_lookup>
   5b242: 4c 89 65 a0                  	movq	%r12, -0x60(%rbp)
   5b246: 48 8b 4d a0                  	movq	-0x60(%rbp), %rcx
   5b24a: 48 89 4d b0                  	movq	%rcx, -0x50(%rbp)
   5b24e: 48 8b 4d b0                  	movq	-0x50(%rbp), %rcx
   5b252: 48 8b 7d b0                  	movq	-0x50(%rbp), %rdi
   5b256: 48 89 7d a8                  	movq	%rdi, -0x58(%rbp)
   5b25a: 48 8b 4d c8                  	movq	-0x38(%rbp), %rcx
   5b25e: 48 8b 00                     	movq	(%rax), %rax
   5b261: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   5b265: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   5b269: 44 89 f6                     	movl	%r14d, %esi
   5b26c: ff d0                        	callq	*%rax
   5b26e: 41 89 c6                     	movl	%eax, %r14d
   5b271: 48 8b 45 b8                  	movq	-0x48(%rbp), %rax
   5b275: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   5b279: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   5b27d: 48 89 45 a8                  	movq	%rax, -0x58(%rbp)
   5b281: ff c3                        	incl	%ebx
   5b283: e9 58 ff ff ff               	jmp	0x5b1e0 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x90>
   5b288: 44 89 f0                     	movl	%r14d, %eax
   5b28b: 48 83 c4 48                  	addq	$0x48, %rsp
   5b28f: 5b                           	popq	%rbx
   5b290: 41 5c                        	popq	%r12
   5b292: 41 5d                        	popq	%r13
   5b294: 41 5e                        	popq	%r14
   5b296: 41 5f                        	popq	%r15
   5b298: 5d                           	popq	%rbp
   5b299: c3                           	retq
   5b29a: 48 8b 1d c7 ab 09 00         	movq	0x9abc7(%rip), %rbx     # 0xf5e68 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   5b2a1: 49 89 df                     	movq	%rbx, %r15
   5b2a4: 49 f7 df                     	negq	%r15
   5b2a7: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   5b2b1: 48 01 d8                     	addq	%rbx, %rax
   5b2b4: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   5b2b8: 41 0f 92 c4                  	setb	%r12b
   5b2bc: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   5b2c0: 4d 21 fe                     	andq	%r15, %r14
   5b2c3: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   5b2cc: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   5b2d3: 48 8b 00                     	movq	(%rax), %rax
   5b2d6: 48 8b 08                     	movq	(%rax), %rcx
   5b2d9: 48 01 cb                     	addq	%rcx, %rbx
   5b2dc: 48 ff cb                     	decq	%rbx
   5b2df: 4c 21 fb                     	andq	%r15, %rbx
   5b2e2: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   5b2e6: 48 85 db                     	testq	%rbx, %rbx
   5b2e9: 0f 95 c2                     	setne	%dl
   5b2ec: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   5b2f3: 40 0f 92 c6                  	setb	%sil
   5b2f7: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   5b2fb: 40 0f 96 c7                  	setbe	%dil
   5b2ff: 48 83 3d d9 ab 09 00 00      	cmpq	$0x0, 0x9abd9(%rip)     # 0xf5ee0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   5b307: 41 0f 94 c0                  	sete	%r8b
   5b30b: 41 20 f0                     	andb	%sil, %r8b
   5b30e: 41 20 d0                     	andb	%dl, %r8b
   5b311: 41 20 f8                     	andb	%dil, %r8b
   5b314: 45 84 e0                     	testb	%r12b, %r8b
   5b317: 74 17                        	je	0x5b330 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1e0>
   5b319: 48 89 08                     	movq	%rcx, (%rax)
   5b31c: 48 8d 35 2d ab 09 00         	leaq	0x9ab2d(%rip), %rsi     # 0xf5e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5b323: 48 89 df                     	movq	%rbx, %rdi
   5b326: 4c 89 f2                     	movq	%r14, %rdx
   5b329: e8 32 82 00 00               	callq	0x63560 <scoop_runtime_finish_tlab_alloc>
   5b32e: eb 14                        	jmp	0x5b344 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x1f4>
   5b330: 48 8d 3d 19 ab 09 00         	leaq	0x9ab19(%rip), %rdi     # 0xf5e50 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   5b337: be 18 00 00 00               	movl	$0x18, %esi
   5b33c: e8 3f 33 00 00               	callq	0x5e680 <scoop_runtime_alloc_slow>
   5b341: 48 89 c3                     	movq	%rax, %rbx
   5b344: 48 89 5d d0                  	movq	%rbx, -0x30(%rbp)
   5b348: 48 89 df                     	movq	%rbx, %rdi
   5b34b: e8 e0 de fd ff               	callq	0x39230 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   5b350: 48 8b 7d d0                  	movq	-0x30(%rbp), %rdi
   5b354: 48 89 7d 98                  	movq	%rdi, -0x68(%rbp)
   5b358: e8 f3 e0 00 00               	callq	0x69450 <scoop_rt_throw>
