
tmp/m34/poll-off-linux-gnu/primitive:	file format elf64-x86-64

Disassembly of section .text:

000000000004d1b0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   4d1b0:      	pushq	%rbp
   4d1b1:      	movq	%rsp, %rbp
   4d1b4:      	pushq	%r15
   4d1b6:      	pushq	%r14
   4d1b8:      	pushq	%r12
   4d1ba:      	pushq	%rbx
   4d1bb:      	callq	0x4f920 <scoop_rt_safepoint>
   4d1c0:      	movl	$0x1, %ebx
   4d1c5:      	xorl	%r14d, %r14d
   4d1c8:      	movabsq	$-0xf0f0f0f0f0f0f0f, %r12 # imm = 0xF0F0F0F0F0F0F0F1
   4d1d2:      	nopw	%cs:(%rax,%rax)
   4d1e0:      	movq	%r14, %rax
   4d1e3:      	mulq	%r12
   4d1e6:      	movq	%rdx, %r15
   4d1e9:      	callq	0x4f920 <scoop_rt_safepoint>
   4d1ee:      	cmpq	$0x989680, %r14         # imm = 0x989680
   4d1f5:      	jge	0x4d213 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x63>
   4d1f7:      	movq	%r15, %rax
   4d1fa:      	andq	$-0x10, %rax
   4d1fe:      	shrq	$0x4, %r15
   4d202:      	addq	%rax, %r15
   4d205:      	movq	%r14, %rax
   4d208:      	subq	%r15, %rax
   4d20b:      	addq	%rax, %rbx
   4d20e:      	incq	%r14
   4d211:      	jmp	0x4d1e0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x30>
   4d213:      	movq	%rbx, %rdi
   4d216:      	callq	0x4d230 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   4d21b:      	popq	%rbx
   4d21c:      	popq	%r12
   4d21e:      	popq	%r14
   4d220:      	popq	%r15
   4d222:      	popq	%rbp
   4d223:      	retq
