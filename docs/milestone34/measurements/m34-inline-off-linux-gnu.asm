
/home/chenxu/repos/scoop/tmp/m34/inline-off-linux-gnu/mir-inlining:	file format elf64-x86-64

Disassembly of section .text:

000000000004be20 <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d>:
   4be20:      	pushq	%rbp
   4be21:      	movq	%rsp, %rbp
   4be24:      	pushq	%r15
   4be26:      	pushq	%r14
   4be28:      	pushq	%rbx
   4be29:      	pushq	%rax
   4be2a:      	movl	%edi, %ebx
   4be2c:      	movl	$0x12345678, %eax       # imm = 0x12345678
   4be31:      	xorl	%r14d, %r14d
   4be34:      	xorl	%r15d, %r15d
   4be37:      	cmpl	%ebx, %r15d
   4be3a:      	jge	0x4be5b <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x3b>
   4be3c:      	nopl	(%rax)
   4be40:      	movl	%eax, %edi
   4be42:      	xorl	%esi, %esi
   4be44:      	callq	0x4bd30 <scoop$1$cb$282a33668916ebe7a926c8f62abf2ad7d3253d12099213678cdee754a7cefd97>
   4be49:      	movl	%eax, %edi
   4be4b:      	callq	0x4bd10 <scoop$1$cb$c56cd1861f9adfdb1366c5782dbbdf9045f283ad72cd878590a00b5e3a8a082c>
   4be50:      	xorl	%eax, %r14d
   4be53:      	incl	%r15d
   4be56:      	cmpl	%ebx, %r15d
   4be59:      	jl	0x4be40 <scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x20>
   4be5b:      	movl	%r14d, %eax
   4be5e:      	addq	$0x8, %rsp
   4be62:      	popq	%rbx
   4be63:      	popq	%r14
   4be65:      	popq	%r15
   4be67:      	popq	%rbp
   4be68:      	retq
