
/home/chenxu/repos/scoop/tmp/m34/inline-on-linux-gnu/mir-inlining:	file format elf64-x86-64

Disassembly of section .text:

000000000004d240 <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d>:
   4d240:      	pushq	%rbp
   4d241:      	movq	%rsp, %rbp
   4d244:      	movl	$0x12345678, %edx       # imm = 0x12345678
   4d249:      	xorl	%eax, %eax
   4d24b:      	xorl	%ecx, %ecx
   4d24d:      	cmpl	%edi, %ecx
   4d24f:      	jge	0x4d284 <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x44>
   4d251:      	nopw	%cs:(%rax,%rax)
   4d260:      	incl	%edx
   4d262:      	movl	%edx, %esi
   4d264:      	shll	$0xd, %esi
   4d267:      	xorl	%edx, %esi
   4d269:      	movl	%esi, %r8d
   4d26c:      	shrl	$0x11, %r8d
   4d270:      	xorl	%esi, %r8d
   4d273:      	movl	%r8d, %edx
   4d276:      	shll	$0x5, %edx
   4d279:      	xorl	%r8d, %edx
   4d27c:      	xorl	%edx, %eax
   4d27e:      	incl	%ecx
   4d280:      	cmpl	%edi, %ecx
   4d282:      	jl	0x4d260 <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x20>
   4d284:      	popq	%rbp
   4d285:      	retq
