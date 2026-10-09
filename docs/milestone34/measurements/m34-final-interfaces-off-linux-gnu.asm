; Cell.next, MIR fn0, LIR scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004c230 <scoop$1$cb$d6d44c12518bb1e3171fe4235a04708304936c6d7b2f9b60aec3050058d674b0>:
   4c230: 55                           	pushq	%rbp
   4c231: 48 89 e5                     	movq	%rsp, %rbp
   4c234: 53                           	pushq	%rbx
   4c235: 48 83 ec 18                  	subq	$0x18, %rsp
   4c239: 89 f3                        	movl	%esi, %ebx
   4c23b: 48 89 7d f0                  	movq	%rdi, -0x10(%rbp)
   4c23f: e8 9c 33 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c244: 48 8b 7d f0                  	movq	-0x10(%rbp), %rdi
   4c248: 48 89 7d e8                  	movq	%rdi, -0x18(%rbp)
   4c24c: e8 8f 06 00 00               	callq	0x4c8e0 <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>
   4c251: 48 8b 4d f0                  	movq	-0x10(%rbp), %rcx
   4c255: 48 89 4d e8                  	movq	%rcx, -0x18(%rbp)
   4c259: 89 d9                        	movl	%ebx, %ecx
   4c25b: c1 e1 0d                     	shll	$0xd, %ecx
   4c25e: 31 d9                        	xorl	%ebx, %ecx
   4c260: 31 c8                        	xorl	%ecx, %eax
   4c262: 89 c1                        	movl	%eax, %ecx
   4c264: c1 e9 11                     	shrl	$0x11, %ecx
   4c267: 31 c8                        	xorl	%ecx, %eax
   4c269: 89 c1                        	movl	%eax, %ecx
   4c26b: c1 e1 05                     	shll	$0x5, %ecx
   4c26e: 31 c8                        	xorl	%ecx, %eax
   4c270: 48 83 c4 18                  	addq	$0x18, %rsp
   4c274: 5b                           	popq	%rbx
   4c275: 5d                           	popq	%rbp
   4c276: c3                           	retq

; Cell.$get$salt, MIR fn1, LIR scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004c8e0 <scoop$1$cb$073cd401d310770fd6c547809a2719a257086eb47c666e34c45a2ae41a89e0be>:
   4c8e0: 55                           	pushq	%rbp
   4c8e1: 48 89 e5                     	movq	%rsp, %rbp
   4c8e4: 48 83 ec 10                  	subq	$0x10, %rsp
   4c8e8: 48 89 7d f8                  	movq	%rdi, -0x8(%rbp)
   4c8ec: e8 ef 2c 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c8f1: 48 8b 45 f8                  	movq	-0x8(%rbp), %rax
   4c8f5: 48 89 45 f0                  	movq	%rax, -0x10(%rbp)
   4c8f9: 8b 40 10                     	movl	0x10(%rax), %eax
   4c8fc: 48 83 c4 10                  	addq	$0x10, %rsp
   4c900: 5d                           	popq	%rbp
   4c901: c3                           	retq

; known, MIR fn2, LIR scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004c810 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb>:
   4c810: 55                           	pushq	%rbp
   4c811: 48 89 e5                     	movq	%rsp, %rbp
   4c814: 41 57                        	pushq	%r15
   4c816: 41 56                        	pushq	%r14
   4c818: 41 54                        	pushq	%r12
   4c81a: 53                           	pushq	%rbx
   4c81b: 48 83 ec 30                  	subq	$0x30, %rsp
   4c81f: 89 f3                        	movl	%esi, %ebx
   4c821: 48 89 7d d8                  	movq	%rdi, -0x28(%rbp)
   4c825: e8 b6 2d 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c82a: 48 8b 45 d8                  	movq	-0x28(%rbp), %rax
   4c82e: 48 89 45 b8                  	movq	%rax, -0x48(%rbp)
   4c832: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4c836: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   4c83c: 45 31 e4                     	xorl	%r12d, %r12d
   4c83f: 4c 8d 3d ba f7 10 00         	leaq	0x10f7ba(%rip), %r15    # 0x15c000 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   4c846: 66 2e 0f 1f 84 00 00 00 00 00	nopw	%cs:(%rax,%rax)
   4c850: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   4c854: 48 89 45 d8                  	movq	%rax, -0x28(%rbp)
   4c858: e8 83 2d 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c85d: 48 8b 45 d8                  	movq	-0x28(%rbp), %rax
   4c861: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4c865: 41 39 dc                     	cmpl	%ebx, %r12d
   4c868: 7d 3f                        	jge	0x4c8a9 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x99>
   4c86a: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4c86e: 48 8b 38                     	movq	(%rax), %rdi
   4c871: 4c 89 fe                     	movq	%r15, %rsi
   4c874: e8 47 35 01 00               	callq	0x5fdc0 <scoop_rt_itable_lookup>
   4c879: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   4c87d: 48 8b 7d c0                  	movq	-0x40(%rbp), %rdi
   4c881: 48 8b 00                     	movq	(%rax), %rax
   4c884: 48 89 7d c8                  	movq	%rdi, -0x38(%rbp)
   4c888: 48 89 4d d8                  	movq	%rcx, -0x28(%rbp)
   4c88c: 44 89 f6                     	movl	%r14d, %esi
   4c88f: ff d0                        	callq	*%rax
   4c891: 41 89 c6                     	movl	%eax, %r14d
   4c894: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   4c898: 48 8b 4d d8                  	movq	-0x28(%rbp), %rcx
   4c89c: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   4c8a0: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4c8a4: 41 ff c4                     	incl	%r12d
   4c8a7: eb a7                        	jmp	0x4c850 <scoop$1$cb$89d463c70b3b9092140dcfc3a35d7372a7af6539922bbbaaa5b3ecc51369ffeb+0x40>
   4c8a9: 44 89 f0                     	movl	%r14d, %eax
   4c8ac: 48 83 c4 30                  	addq	$0x30, %rsp
   4c8b0: 5b                           	popq	%rbx
   4c8b1: 41 5c                        	popq	%r12
   4c8b3: 41 5e                        	popq	%r14
   4c8b5: 41 5f                        	popq	%r15
   4c8b7: 5d                           	popq	%rbp
   4c8b8: c3                           	retq

; unknown, MIR fn3, LIR scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004c760 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0>:
   4c760: 55                           	pushq	%rbp
   4c761: 48 89 e5                     	movq	%rsp, %rbp
   4c764: 41 57                        	pushq	%r15
   4c766: 41 56                        	pushq	%r14
   4c768: 41 54                        	pushq	%r12
   4c76a: 53                           	pushq	%rbx
   4c76b: 48 83 ec 20                  	subq	$0x20, %rsp
   4c76f: 89 f3                        	movl	%esi, %ebx
   4c771: 48 89 7d d8                  	movq	%rdi, -0x28(%rbp)
   4c775: e8 66 2e 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c77a: 48 8b 45 d8                  	movq	-0x28(%rbp), %rax
   4c77e: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4c782: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   4c788: 45 31 e4                     	xorl	%r12d, %r12d
   4c78b: 4c 8d 3d 6e f8 10 00         	leaq	0x10f86e(%rip), %r15    # 0x15c000 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   4c792: 66 66 66 66 66 2e 0f 1f 84 00 00 00 00 00    	nopw	%cs:(%rax,%rax)
   4c7a0: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   4c7a4: 48 89 45 d8                  	movq	%rax, -0x28(%rbp)
   4c7a8: e8 33 2e 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c7ad: 48 8b 45 d8                  	movq	-0x28(%rbp), %rax
   4c7b1: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4c7b5: 41 39 dc                     	cmpl	%ebx, %r12d
   4c7b8: 7d 3f                        	jge	0x4c7f9 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x99>
   4c7ba: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4c7be: 48 8b 38                     	movq	(%rax), %rdi
   4c7c1: 4c 89 fe                     	movq	%r15, %rsi
   4c7c4: e8 f7 35 01 00               	callq	0x5fdc0 <scoop_rt_itable_lookup>
   4c7c9: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   4c7cd: 48 8b 7d c0                  	movq	-0x40(%rbp), %rdi
   4c7d1: 48 8b 00                     	movq	(%rax), %rax
   4c7d4: 48 89 7d c8                  	movq	%rdi, -0x38(%rbp)
   4c7d8: 48 89 4d d8                  	movq	%rcx, -0x28(%rbp)
   4c7dc: 44 89 f6                     	movl	%r14d, %esi
   4c7df: ff d0                        	callq	*%rax
   4c7e1: 41 89 c6                     	movl	%eax, %r14d
   4c7e4: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   4c7e8: 48 8b 4d d8                  	movq	-0x28(%rbp), %rcx
   4c7ec: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   4c7f0: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4c7f4: 41 ff c4                     	incl	%r12d
   4c7f7: eb a7                        	jmp	0x4c7a0 <scoop$1$cb$803f972bd8a37e762b321c8df27119f059d46b85f2f6aefcb5acb6f300a859c0+0x40>
   4c7f9: 44 89 f0                     	movl	%r14d, %eax
   4c7fc: 48 83 c4 20                  	addq	$0x20, %rsp
   4c800: 5b                           	popq	%rbx
   4c801: 41 5c                        	popq	%r12
   4c803: 41 5e                        	popq	%r14
   4c805: 41 5f                        	popq	%r15
   4c807: 5d                           	popq	%rbp
   4c808: c3                           	retq

; convertedOnce, MIR fn4, LIR scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004bca0 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd>:
   4bca0: 55                           	pushq	%rbp
   4bca1: 48 89 e5                     	movq	%rsp, %rbp
   4bca4: 41 57                        	pushq	%r15
   4bca6: 41 56                        	pushq	%r14
   4bca8: 41 54                        	pushq	%r12
   4bcaa: 53                           	pushq	%rbx
   4bcab: 48 83 ec 30                  	subq	$0x30, %rsp
   4bcaf: 89 f3                        	movl	%esi, %ebx
   4bcb1: 48 89 7d d8                  	movq	%rdi, -0x28(%rbp)
   4bcb5: e8 26 39 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4bcba: 4c 8b 75 d8                  	movq	-0x28(%rbp), %r14
   4bcbe: 4c 89 75 b0                  	movq	%r14, -0x50(%rbp)
   4bcc2: 48 8d 35 37 03 11 00         	leaq	0x110337(%rip), %rsi    # 0x15c000 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   4bcc9: 4c 89 f7                     	movq	%r14, %rdi
   4bccc: e8 af 40 01 00               	callq	0x5fd80 <scoop_rt_is_instance>
   4bcd1: a8 01                        	testb	$0x1, %al
   4bcd3: 0f 84 80 00 00 00            	je	0x4bd59 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xb9>
   4bcd9: 4c 89 75 d0                  	movq	%r14, -0x30(%rbp)
   4bcdd: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   4bce3: 45 31 e4                     	xorl	%r12d, %r12d
   4bce6: 4c 8d 3d 13 03 11 00         	leaq	0x110313(%rip), %r15    # 0x15c000 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   4bced: 0f 1f 00                     	nopl	(%rax)
   4bcf0: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   4bcf4: 48 89 45 d8                  	movq	%rax, -0x28(%rbp)
   4bcf8: e8 e3 38 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4bcfd: 48 8b 45 d8                  	movq	-0x28(%rbp), %rax
   4bd01: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4bd05: 41 39 dc                     	cmpl	%ebx, %r12d
   4bd08: 7d 3f                        	jge	0x4bd49 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0xa9>
   4bd0a: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4bd0e: 48 8b 38                     	movq	(%rax), %rdi
   4bd11: 4c 89 fe                     	movq	%r15, %rsi
   4bd14: e8 a7 40 01 00               	callq	0x5fdc0 <scoop_rt_itable_lookup>
   4bd19: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   4bd1d: 48 8b 7d c0                  	movq	-0x40(%rbp), %rdi
   4bd21: 48 8b 00                     	movq	(%rax), %rax
   4bd24: 48 89 7d c8                  	movq	%rdi, -0x38(%rbp)
   4bd28: 48 89 4d d8                  	movq	%rcx, -0x28(%rbp)
   4bd2c: 44 89 f6                     	movl	%r14d, %esi
   4bd2f: ff d0                        	callq	*%rax
   4bd31: 41 89 c6                     	movl	%eax, %r14d
   4bd34: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   4bd38: 48 8b 4d d8                  	movq	-0x28(%rbp), %rcx
   4bd3c: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   4bd40: 48 89 45 c0                  	movq	%rax, -0x40(%rbp)
   4bd44: 41 ff c4                     	incl	%r12d
   4bd47: eb a7                        	jmp	0x4bcf0 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x50>
   4bd49: 44 89 f0                     	movl	%r14d, %eax
   4bd4c: 48 83 c4 30                  	addq	$0x30, %rsp
   4bd50: 5b                           	popq	%rbx
   4bd51: 41 5c                        	popq	%r12
   4bd53: 41 5e                        	popq	%r14
   4bd55: 41 5f                        	popq	%r15
   4bd57: 5d                           	popq	%rbp
   4bd58: c3                           	retq
   4bd59: 48 8b 1d f8 b5 09 00         	movq	0x9b5f8(%rip), %rbx     # 0xe7358 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   4bd60: 49 89 df                     	movq	%rbx, %r15
   4bd63: 49 f7 df                     	negq	%r15
   4bd66: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   4bd70: 48 01 d8                     	addq	%rbx, %rax
   4bd73: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   4bd77: 41 0f 92 c4                  	setb	%r12b
   4bd7b: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   4bd7f: 4d 21 fe                     	andq	%r15, %r14
   4bd82: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   4bd8b: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   4bd92: 48 8b 00                     	movq	(%rax), %rax
   4bd95: 48 8b 08                     	movq	(%rax), %rcx
   4bd98: 48 01 cb                     	addq	%rcx, %rbx
   4bd9b: 48 ff cb                     	decq	%rbx
   4bd9e: 4c 21 fb                     	andq	%r15, %rbx
   4bda1: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   4bda5: 48 85 db                     	testq	%rbx, %rbx
   4bda8: 0f 95 c2                     	setne	%dl
   4bdab: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   4bdb2: 40 0f 92 c6                  	setb	%sil
   4bdb6: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   4bdba: 40 0f 96 c7                  	setbe	%dil
   4bdbe: 48 83 3d 0a b6 09 00 00      	cmpq	$0x0, 0x9b60a(%rip)     # 0xe73d0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   4bdc6: 41 0f 94 c0                  	sete	%r8b
   4bdca: 41 20 f0                     	andb	%sil, %r8b
   4bdcd: 41 20 d0                     	andb	%dl, %r8b
   4bdd0: 41 20 f8                     	andb	%dil, %r8b
   4bdd3: 45 84 e0                     	testb	%r12b, %r8b
   4bdd6: 74 17                        	je	0x4bdef <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x14f>
   4bdd8: 48 89 08                     	movq	%rcx, (%rax)
   4bddb: 48 8d 35 5e b5 09 00         	leaq	0x9b55e(%rip), %rsi     # 0xe7340 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   4bde2: 48 89 df                     	movq	%rbx, %rdi
   4bde5: 4c 89 f2                     	movq	%r14, %rdx
   4bde8: e8 c3 50 00 00               	callq	0x50eb0 <scoop_runtime_finish_tlab_alloc>
   4bded: eb 14                        	jmp	0x4be03 <scoop$1$cb$d96b4b18d972ec178c732a7b98ed8faf23417a50a315c26392b65225672007cd+0x163>
   4bdef: 48 8d 3d 4a b5 09 00         	leaq	0x9b54a(%rip), %rdi     # 0xe7340 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   4bdf6: be 18 00 00 00               	movl	$0x18, %esi
   4bdfb: e8 20 38 00 00               	callq	0x4f620 <scoop_runtime_alloc_slow>
   4be00: 48 89 c3                     	movq	%rax, %rbx
   4be03: 48 89 5d d8                  	movq	%rbx, -0x28(%rbp)
   4be07: 48 89 df                     	movq	%rbx, %rdi
   4be0a: e8 41 3b fe ff               	callq	0x2f950 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   4be0f: 48 8b 7d d8                  	movq	-0x28(%rbp), %rdi
   4be13: 48 89 7d b8                  	movq	%rdi, -0x48(%rbp)
   4be17: e8 24 dc 00 00               	callq	0x59a40 <scoop_rt_throw>

; convertedEach, MIR fn5, LIR scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b

/home/chenxu/repos/scoop/tmp/m34/final-interfaces-off-linux-gnu/interfaces:	file format elf64-x86-64

Disassembly of section .text:

000000000004c5e0 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b>:
   4c5e0: 55                           	pushq	%rbp
   4c5e1: 48 89 e5                     	movq	%rsp, %rbp
   4c5e4: 41 57                        	pushq	%r15
   4c5e6: 41 56                        	pushq	%r14
   4c5e8: 41 55                        	pushq	%r13
   4c5ea: 41 54                        	pushq	%r12
   4c5ec: 53                           	pushq	%rbx
   4c5ed: 48 83 ec 28                  	subq	$0x28, %rsp
   4c5f1: 89 f3                        	movl	%esi, %ebx
   4c5f3: 48 89 7d d0                  	movq	%rdi, -0x30(%rbp)
   4c5f7: e8 e4 2f 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c5fc: 48 8b 45 d0                  	movq	-0x30(%rbp), %rax
   4c600: 48 89 45 c8                  	movq	%rax, -0x38(%rbp)
   4c604: 41 be 78 56 34 12            	movl	$0x12345678, %r14d      # imm = 0x12345678
   4c60a: 45 31 ed                     	xorl	%r13d, %r13d
   4c60d: 4c 8d 3d ec f9 10 00         	leaq	0x10f9ec(%rip), %r15    # 0x15c000 <scoop$1$td$48835de14069ed024217487d0dad48c60d171573b56cb2aa3b46a21999386bb3>
   4c614: 66 66 66 2e 0f 1f 84 00 00 00 00 00  	nopw	%cs:(%rax,%rax)
   4c620: 48 8b 45 c8                  	movq	-0x38(%rbp), %rax
   4c624: 48 89 45 d0                  	movq	%rax, -0x30(%rbp)
   4c628: e8 b3 2f 00 00               	callq	0x4f5e0 <scoop_rt_safepoint>
   4c62d: 4c 8b 65 d0                  	movq	-0x30(%rbp), %r12
   4c631: 4c 89 65 c8                  	movq	%r12, -0x38(%rbp)
   4c635: 41 39 dd                     	cmpl	%ebx, %r13d
   4c638: 7d 4f                        	jge	0x4c689 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xa9>
   4c63a: 4c 89 e7                     	movq	%r12, %rdi
   4c63d: 4c 89 fe                     	movq	%r15, %rsi
   4c640: e8 3b 37 01 00               	callq	0x5fd80 <scoop_rt_is_instance>
   4c645: a8 01                        	testb	$0x1, %al
   4c647: 74 52                        	je	0x4c69b <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0xbb>
   4c649: 4c 89 65 b8                  	movq	%r12, -0x48(%rbp)
   4c64d: 49 8b 3c 24                  	movq	(%r12), %rdi
   4c651: 4c 89 fe                     	movq	%r15, %rsi
   4c654: e8 67 37 01 00               	callq	0x5fdc0 <scoop_rt_itable_lookup>
   4c659: 48 8b 4d c8                  	movq	-0x38(%rbp), %rcx
   4c65d: 48 8b 7d b8                  	movq	-0x48(%rbp), %rdi
   4c661: 48 8b 00                     	movq	(%rax), %rax
   4c664: 48 89 7d c0                  	movq	%rdi, -0x40(%rbp)
   4c668: 48 89 4d d0                  	movq	%rcx, -0x30(%rbp)
   4c66c: 44 89 f6                     	movl	%r14d, %esi
   4c66f: ff d0                        	callq	*%rax
   4c671: 41 89 c6                     	movl	%eax, %r14d
   4c674: 48 8b 45 c0                  	movq	-0x40(%rbp), %rax
   4c678: 48 8b 4d d0                  	movq	-0x30(%rbp), %rcx
   4c67c: 48 89 4d c8                  	movq	%rcx, -0x38(%rbp)
   4c680: 48 89 45 b8                  	movq	%rax, -0x48(%rbp)
   4c684: 41 ff c5                     	incl	%r13d
   4c687: eb 97                        	jmp	0x4c620 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x40>
   4c689: 44 89 f0                     	movl	%r14d, %eax
   4c68c: 48 83 c4 28                  	addq	$0x28, %rsp
   4c690: 5b                           	popq	%rbx
   4c691: 41 5c                        	popq	%r12
   4c693: 41 5d                        	popq	%r13
   4c695: 41 5e                        	popq	%r14
   4c697: 41 5f                        	popq	%r15
   4c699: 5d                           	popq	%rbp
   4c69a: c3                           	retq
   4c69b: 48 8b 1d b6 ac 09 00         	movq	0x9acb6(%rip), %rbx     # 0xe7358 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x18>
   4c6a2: 49 89 df                     	movq	%rbx, %r15
   4c6a5: 49 f7 df                     	negq	%r15
   4c6a8: 48 b8 ff ff ff ff ff ff ff 7f	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   4c6b2: 48 01 d8                     	addq	%rbx, %rax
   4c6b5: 48 83 f8 e8                  	cmpq	$-0x18, %rax
   4c6b9: 41 0f 92 c4                  	setb	%r12b
   4c6bd: 4c 8d 73 17                  	leaq	0x17(%rbx), %r14
   4c6c1: 4d 21 fe                     	andq	%r15, %r14
   4c6c4: 64 48 8b 04 25 00 00 00 00   	movq	%fs:0x0, %rax
   4c6cd: 48 8d 80 f8 ff ff ff         	leaq	-0x8(%rax), %rax
   4c6d4: 48 8b 00                     	movq	(%rax), %rax
   4c6d7: 48 8b 08                     	movq	(%rax), %rcx
   4c6da: 48 01 cb                     	addq	%rcx, %rbx
   4c6dd: 48 ff cb                     	decq	%rbx
   4c6e0: 4c 21 fb                     	andq	%r15, %rbx
   4c6e3: 4a 8d 0c 33                  	leaq	(%rbx,%r14), %rcx
   4c6e7: 48 85 db                     	testq	%rbx, %rbx
   4c6ea: 0f 95 c2                     	setne	%dl
   4c6ed: 49 81 fe 81 7f 00 00         	cmpq	$0x7f81, %r14           # imm = 0x7F81
   4c6f4: 40 0f 92 c6                  	setb	%sil
   4c6f8: 48 3b 48 08                  	cmpq	0x8(%rax), %rcx
   4c6fc: 40 0f 96 c7                  	setbe	%dil
   4c700: 48 83 3d c8 ac 09 00 00      	cmpq	$0x0, 0x9acc8(%rip)     # 0xe73d0 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b+0x90>
   4c708: 41 0f 94 c0                  	sete	%r8b
   4c70c: 41 20 f0                     	andb	%sil, %r8b
   4c70f: 41 20 d0                     	andb	%dl, %r8b
   4c712: 41 20 f8                     	andb	%dil, %r8b
   4c715: 45 84 e0                     	testb	%r12b, %r8b
   4c718: 74 17                        	je	0x4c731 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x151>
   4c71a: 48 89 08                     	movq	%rcx, (%rax)
   4c71d: 48 8d 35 1c ac 09 00         	leaq	0x9ac1c(%rip), %rsi     # 0xe7340 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   4c724: 48 89 df                     	movq	%rbx, %rdi
   4c727: 4c 89 f2                     	movq	%r14, %rdx
   4c72a: e8 81 47 00 00               	callq	0x50eb0 <scoop_runtime_finish_tlab_alloc>
   4c72f: eb 14                        	jmp	0x4c745 <scoop$1$cb$4012dab5872149ded60b8a912e8966def1aeeee9dd24a0444293c16878ed786b+0x165>
   4c731: 48 8d 3d 08 ac 09 00         	leaq	0x9ac08(%rip), %rdi     # 0xe7340 <scoop$1$td$f247e0162cdacf44b62850481f79fa6702d74919636bd6e6963c4aa99f2a463b>
   4c738: be 18 00 00 00               	movl	$0x18, %esi
   4c73d: e8 de 2e 00 00               	callq	0x4f620 <scoop_runtime_alloc_slow>
   4c742: 48 89 c3                     	movq	%rax, %rbx
   4c745: 48 89 5d d0                  	movq	%rbx, -0x30(%rbp)
   4c749: 48 89 df                     	movq	%rbx, %rdi
   4c74c: e8 ff 31 fe ff               	callq	0x2f950 <scoop$1$cb$a3b0b3b444da4d318c9f0b55ebec41b6158ef922455d3603034f013debec7aa8>
   4c751: 48 8b 7d d0                  	movq	-0x30(%rbp), %rdi
   4c755: 48 89 7d b0                  	movq	%rdi, -0x50(%rbp)
   4c759: e8 e2 d2 00 00               	callq	0x59a40 <scoop_rt_throw>
