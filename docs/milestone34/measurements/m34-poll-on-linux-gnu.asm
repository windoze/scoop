
tmp/m34/poll-cached-linux-gnu/primitive:	file format elf64-x86-64

Disassembly of section .text:

0000000000059c70 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   59c70:      	pushq	%rbp
   59c71:      	movq	%rsp, %rbp
   59c74:      	pushq	%r15
   59c76:      	pushq	%r14
   59c78:      	pushq	%r13
   59c7a:      	pushq	%r12
   59c7c:      	pushq	%rbx
   59c7d:      	pushq	%rax
   59c7e:      	movq	%fs:0x0, %rax
   59c87:      	leaq	-0x10(%rax), %rax
   59c8e:      	movq	(%rax), %r12
   59c91:      	leaq	0x12f8b8(%rip), %rax    # 0x189550 <scoop_thread_gc_epoch>
   59c98:      	movq	(%rax), %rax
   59c9b:      	leaq	0x12f8b6(%rip), %rcx    # 0x189558 <scoop_thread_world_phase>
   59ca2:      	movl	(%rcx), %esi
   59ca4:      	movl	(%r12), %edx
   59ca8:      	movq	0x8(%r12), %rcx
   59cad:      	testl	%esi, %esi
   59caf:      	jne	0x59cbb <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x4b>
   59cb1:      	cmpl	$0x1, %edx
   59cb4:      	jne	0x59cbb <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x4b>
   59cb6:      	cmpq	%rax, %rcx
   59cb9:      	je	0x59cc0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x50>
   59cbb:      	callq	0x5c4a0 <scoop_rt_safepoint>
   59cc0:      	movl	$0x1, %ebx
   59cc5:      	xorl	%r14d, %r14d
   59cc8:      	movabsq	$-0xf0f0f0f0f0f0f0f, %r13 # imm = 0xF0F0F0F0F0F0F0F1
   59cd2:      	nopw	%cs:(%rax,%rax)
   59ce0:      	movq	%r14, %rax
   59ce3:      	mulq	%r13
   59ce6:      	movq	%rdx, %r15
   59ce9:      	leaq	0x12f860(%rip), %rax    # 0x189550 <scoop_thread_gc_epoch>
   59cf0:      	movq	(%rax), %rax
   59cf3:      	leaq	0x12f85e(%rip), %rcx    # 0x189558 <scoop_thread_world_phase>
   59cfa:      	movl	(%rcx), %esi
   59cfc:      	movl	(%r12), %edx
   59d00:      	movq	0x8(%r12), %rcx
   59d05:      	testl	%esi, %esi
   59d07:      	jne	0x59d13 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa3>
   59d09:      	cmpl	$0x1, %edx
   59d0c:      	jne	0x59d13 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa3>
   59d0e:      	cmpq	%rax, %rcx
   59d11:      	je	0x59d18 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xa8>
   59d13:      	callq	0x5c4a0 <scoop_rt_safepoint>
   59d18:      	cmpq	$0x989680, %r14         # imm = 0x989680
   59d1f:      	jge	0x59d3d <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0xcd>
   59d21:      	movq	%r15, %rax
   59d24:      	andq	$-0x10, %rax
   59d28:      	shrq	$0x4, %r15
   59d2c:      	addq	%rax, %r15
   59d2f:      	movq	%r14, %rax
   59d32:      	subq	%r15, %rax
   59d35:      	addq	%rax, %rbx
   59d38:      	incq	%r14
   59d3b:      	jmp	0x59ce0 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca+0x70>
   59d3d:      	movq	%rbx, %rdi
   59d40:      	callq	0x59d60 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   59d45:      	addq	$0x8, %rsp
   59d49:      	popq	%rbx
   59d4a:      	popq	%r12
   59d4c:      	popq	%r13
   59d4e:      	popq	%r14
   59d50:      	popq	%r15
   59d52:      	popq	%rbp
   59d53:      	retq
