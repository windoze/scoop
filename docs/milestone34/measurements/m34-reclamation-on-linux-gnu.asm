
/home/chenxu/repos/scoop/tmp/m34/reclamation-on-linux-gnu/region-churn:	file format elf64-x86-64

Disassembly of section .text:

000000000005c970 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   5c970:      	pushq	%rbp
   5c971:      	movq	%rsp, %rbp
   5c974:      	pushq	%r15
   5c976:      	pushq	%r14
   5c978:      	pushq	%r13
   5c97a:      	pushq	%r12
   5c97c:      	pushq	%rbx
   5c97d:      	subq	$0xc8, %rsp
   5c984:      	movq	%fs:0x0, %rax
   5c98d:      	leaq	-0x10(%rax), %rax
   5c994:      	movq	(%rax), %rdi
   5c997:      	leaq	, %rax <scoop_thread_gc_epoch>
   5c99e:      	movq	(%rax), %rax
   5c9a1:      	leaq	, %rcx <scoop_thread_world_phase>
   5c9a8:      	movl	(%rcx), %esi
   5c9aa:      	movl	(%rdi), %edx
   5c9ac:      	movq	%rdi, -0x38(%rbp)
   5c9b0:      	movq	0x8(%rdi), %rcx
   5c9b4:      	testl	%esi, %esi
   5c9b6:      	jne	 <L0>
   5c9b8:      	cmpl	$0x1, %edx
   5c9bb:      	jne	 <L0>
   5c9bd:      	cmpq	%rax, %rcx
   5c9c0:      	je	 <L1>
<L0>:
   5c9c2:      	callq	 <scoop_rt_safepoint>
<L1>:
   5c9c7:      	xorps	%xmm0, %xmm0
   5c9ca:      	movups	%xmm0, -0x50(%rbp)
   5c9ce:      	movq	$0x0, -0x40(%rbp)
   5c9d6:      	xorl	%ebx, %ebx
   5c9d8:      	leaq	-0x50(%rbp), %r14
   5c9dc:      	movq	%r14, %rdi
   5c9df:      	xorl	%esi, %esi
   5c9e1:      	xorl	%edx, %edx
   5c9e3:      	callq	 <scoop_rt_push_caller_roots>
   5c9e8:      	xorps	%xmm0, %xmm0
   5c9eb:      	movups	%xmm0, -0xa8(%rbp)
   5c9f2:      	movups	%xmm0, -0x98(%rbp)
   5c9f9:      	movups	%xmm0, -0x88(%rbp)
   5ca00:      	movups	%xmm0, -0x78(%rbp)
   5ca04:      	leaq	-0xa8(%rbp), %r13
   5ca0b:      	movq	%r13, %rdi
   5ca0e:      	movq	%r13, %rsi
   5ca11:      	callq	 <scoop_rt_enter_native_safe>
   5ca16:      	callq	 <m34_memory_pins>
   5ca1b:      	movl	%eax, %r12d
   5ca1e:      	movq	%r13, %rdi
   5ca21:      	callq	 <scoop_rt_leave_native_safe>
   5ca26:      	movq	%r14, %rdi
   5ca29:      	callq	 <scoop_rt_pop_caller_roots>
   5ca2e:      	movl	$0x2, %r14d
   5ca34:      	movzbl	%r12b, %eax
   5ca38:      	movl	%eax, -0x2c(%rbp)
   5ca3b:      	leaq	-0xe8(%rbp), %r13
   5ca42:      	xorl	%r12d, %r12d
   5ca45:      	nopw	%cs:(%rax,%rax)
<L2>:
   5ca50:      	leaq	, %rax <scoop_thread_gc_epoch>
   5ca57:      	movq	(%rax), %rax
   5ca5a:      	leaq	, %rcx <scoop_thread_world_phase>
   5ca61:      	movl	(%rcx), %esi
   5ca63:      	movq	-0x38(%rbp), %rcx
   5ca67:      	movl	(%rcx), %edx
   5ca69:      	movq	0x8(%rcx), %rcx
   5ca6d:      	testl	%esi, %esi
   5ca6f:      	jne	 <L3>
   5ca71:      	cmpl	$0x1, %edx
   5ca74:      	jne	 <L3>
   5ca76:      	cmpq	%rax, %rcx
   5ca79:      	je	 <L4>
<L3>:
   5ca7b:      	callq	 <scoop_rt_safepoint>
<L4>:
   5ca80:      	cmpl	$0x2, %ebx
   5ca83:      	jge	 <L5>
   5ca85:      	movl	%ebx, %edi
   5ca87:      	movl	-0x2c(%rbp), %esi
   5ca8a:      	callq	 <scoop$1$cb$9fcafbb211d8f976a0bd7b23d8482b48f61bd5b743ce18a7cd5abcb7f9166e4d>
   5ca8f:      	addq	%rax, %r12
   5ca92:      	callq	 <scoop_rt_gc_collect>
   5ca97:      	xorps	%xmm0, %xmm0
   5ca9a:      	movups	%xmm0, -0x68(%rbp)
   5ca9e:      	movq	$0x0, -0x58(%rbp)
   5caa6:      	leaq	-0x68(%rbp), %r15
   5caaa:      	movq	%r15, %rdi
   5caad:      	xorl	%esi, %esi
   5caaf:      	xorl	%edx, %edx
   5cab1:      	callq	 <scoop_rt_push_caller_roots>
   5cab6:      	xorps	%xmm0, %xmm0
   5cab9:      	movups	%xmm0, -0xe8(%rbp)
   5cac0:      	movups	%xmm0, -0xd8(%rbp)
   5cac7:      	movups	%xmm0, -0xc8(%rbp)
   5cace:      	movups	%xmm0, -0xb8(%rbp)
   5cad5:      	movq	%r13, %rdi
   5cad8:      	movq	%r13, %rsi
   5cadb:      	callq	 <scoop_rt_enter_native_safe>
   5cae0:      	movl	%r14d, %edi
   5cae3:      	callq	 <m34_memory_sample>
   5cae8:      	movq	%r13, %rdi
   5caeb:      	callq	 <scoop_rt_leave_native_safe>
   5caf0:      	movq	%r15, %rdi
   5caf3:      	callq	 <scoop_rt_pop_caller_roots>
   5caf8:      	incl	%ebx
   5cafa:      	addl	$0x3, %r14d
   5cafe:      	jmp	 <L2>
<L5>:
   5cb03:      	movq	%r12, %rdi
   5cb06:      	callq	 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   5cb0b:      	addq	$0xc8, %rsp
   5cb12:      	popq	%rbx
   5cb13:      	popq	%r12
   5cb15:      	popq	%r13
   5cb17:      	popq	%r14
   5cb19:      	popq	%r15
   5cb1b:      	popq	%rbp
   5cb1c:      	retq

000000000005db10 <linux_discard_pages>:
   5db10:      	endbr64
   5db14:      	testq	%rdx, %rdx
   5db17:      	je	 <L2>
   5db19:      	movq	%rdx, %rcx
   5db1c:      	testq	%rdi, %rdi
   5db1f:      	je	 <L1>
   5db21:      	testq	%rsi, %rsi
   5db24:      	je	 <L1>
   5db26:      	pushq	%rbp
   5db27:      	movq	%rsp, %rbp
   5db2a:      	subq	$0x10, %rsp
   5db2e:      	movq	%rdx, -0x8(%rbp)
   5db32:      	movl	$0x4, %edx
   5db37:      	callq	 <madvise@plt>
   5db3c:      	movq	-0x8(%rbp), %rcx
   5db40:      	testl	%eax, %eax
   5db42:      	jne	 <L0>
   5db44:      	leave
   5db45:      	movl	$0x1, %eax
   5db4a:      	retq
   5db4b:      	nopl	(%rax,%rax)
<L0>:
   5db50:      	movl	$0x7, (%rcx)
   5db56:      	xorl	%eax, %eax
   5db58:      	leave
   5db59:      	retq
   5db5a:      	nopw	(%rax,%rax)
<L1>:
   5db60:      	movl	$0x7, (%rcx)
<L2>:
   5db66:      	xorl	%eax, %eax
   5db68:      	retq
   5db69:      	nopl	(%rax)

000000000005ed10 <scoop_gc_heap_plan_moving_locked>:
   5ed10:      	endbr64
   5ed14:      	pushq	%rbp
   5ed15:      	movq	%rsp, %rbp
   5ed18:      	pushq	%r15
   5ed1a:      	pushq	%r14
   5ed1c:      	pushq	%r13
   5ed1e:      	pushq	%r12
   5ed20:      	pushq	%rbx
   5ed21:      	subq	$0x48, %rsp
   5ed25:      	leaq	, %r12 <scoop_gc_heap_state>
   5ed2c:      	movb	%dil, -0x58(%rbp)
   5ed30:      	movzbl	0x85(%r12), %ebx
   5ed39:      	testb	%bl, %bl
   5ed3b:      	je	 <L30>
   5ed41:      	movl	%edi, %r14d
   5ed44:      	movzbl	%dil, %edi
   5ed48:      	callq	 <scoop_heap_select_evacuation_sources>
   5ed4d:      	movq	%rax, -0x70(%rbp)
   5ed51:      	testb	%r14b, %r14b
   5ed54:      	je	 <L17>
<L0>:
   5ed5a:      	movq	$0x0, -0x48(%rbp)
   5ed62:      	movq	$0x0, -0x40(%rbp)
   5ed6a:      	movq	$0x0, -0x38(%rbp)
   5ed72:      	callq	 <scoop_heap_first_block>
   5ed77:      	movl	$0x0, -0x60(%rbp)
   5ed7e:      	movq	%rax, %r14
   5ed81:      	testq	%rax, %rax
   5ed84:      	je	 <L27>
<L1>:
   5ed8a:      	leaq	-0x38(%rbp), %rax
   5ed8e:      	movq	%rax, -0x68(%rbp)
   5ed92:      	nopl	(%rax)
   5ed95:      	nopw	%cs:(%rax,%rax)
<L2>:
   5eda0:      	movl	%ebx, %r15d
   5eda3:      	cmpl	$0x3, 0x14(%r14)
   5eda8:      	jne	 <L6>
   5edae:      	movl	$0x10, %r13d
   5edb4:      	cmpl	$0x2, 0x18(%r14)
   5edb9:      	jne	 <L5>
   5edbb:      	jmp	 <L16>
<L3>:
   5edc0:      	addq	$0x1, %r13
   5edc4:      	movl	%ebx, %r15d
   5edc7:      	cmpq	$0x1000, %r13           # imm = 0x1000
   5edce:      	je	 <L6>
<L4>:
   5edd0:      	testb	%r15b, %r15b
   5edd3:      	je	 <L6>
<L5>:
   5edd5:      	movq	0x20(%r14), %rdi
   5edd9:      	movq	%r13, %rsi
   5eddc:      	callq	 <scoop_heap_bit_test>
   5ede1:      	testb	%al, %al
   5ede3:      	je	 <L3>
   5ede5:      	movq	0x28(%r14), %rdi
   5ede9:      	movq	%r13, %rsi
   5edec:      	callq	 <scoop_heap_bit_test>
   5edf1:      	testb	%al, %al
   5edf3:      	je	 <L3>
   5edf5:      	movq	0x30(%r14), %rdi
   5edf9:      	movq	%r13, %rsi
   5edfc:      	callq	 <scoop_heap_bit_test>
   5ee01:      	testb	%al, %al
   5ee03:      	jne	 <L3>
   5ee05:      	movzbl	-0x58(%rbp), %eax
   5ee09:      	subq	$0x8, %rsp
   5ee0d:      	movl	-0x60(%rbp), %r9d
   5ee11:      	leaq	-0x40(%rbp), %rsi
   5ee15:      	movq	-0x68(%rbp), %rdx
   5ee19:      	leaq	-0x48(%rbp), %rdi
   5ee1d:      	movq	%r13, %r8
   5ee20:      	movq	%r14, %rcx
   5ee23:      	pushq	%rax
   5ee24:      	addq	$0x1, %r13
   5ee28:      	callq	 <append_move>
   5ee2d:      	popq	%rsi
   5ee2e:      	popq	%rdi
   5ee2f:      	movl	%eax, %r15d
   5ee32:      	cmpq	$0x1000, %r13           # imm = 0x1000
   5ee39:      	jne	 <L4>
   5ee3b:      	nopl	(%rax,%rax)
<L6>:
   5ee40:      	movq	%r14, %rdi
   5ee43:      	callq	 <scoop_heap_next_block>
   5ee48:      	movq	%rax, %r14
   5ee4b:      	testq	%rax, %rax
   5ee4e:      	je	 <L7>
   5ee50:      	testb	%r15b, %r15b
   5ee53:      	jne	 <L2>
<L7>:
   5ee59:      	movl	-0x60(%rbp), %ecx
   5ee5c:      	movq	-0x48(%rbp), %r14
   5ee60:      	testl	%ecx, %ecx
   5ee62:      	je	 <L11>
   5ee64:      	movq	-0x40(%rbp), %rsi
   5ee68:      	testq	%rsi, %rsi
   5ee6b:      	je	 <L10>
   5ee6d:      	leaq	(%rsi,%rsi,2), %rcx
   5ee71:      	leaq	0x28(%r14), %rax
   5ee75:      	xorl	%esi, %esi
   5ee77:      	shlq	$0x4, %rcx
   5ee7b:      	leaq	0x28(%r14,%rcx), %rdi
<L8>:
   5ee80:      	movq	(%rax), %rcx
   5ee83:      	movq	(%rcx), %rcx
   5ee86:      	cmpq	$0x0, 0x30(%rcx)
   5ee8b:      	jne	 <L9>
   5ee8d:      	cmpb	$0x0, 0x2c(%rcx)
   5ee91:      	jne	 <L9>
   5ee93:      	movb	$0x1, 0x2c(%rcx)
   5ee97:      	addq	$0x1, %rsi
<L9>:
   5ee9b:      	addq	$0x30, %rax
   5ee9f:      	cmpq	%rax, %rdi
   5eea2:      	jne	 <L8>
<L10>:
   5eea4:      	cmpq	-0x70(%rbp), %rsi
   5eea8:      	setb	%al
   5eeab:      	andl	%eax, %r15d
<L11>:
   5eeae:      	testb	%r15b, %r15b
   5eeb1:      	je	 <L19>
<L12>:
   5eeb7:      	movq	-0x40(%rbp), %rax
   5eebb:      	movq	%r14, %r15
   5eebe:      	xorl	%r13d, %r13d
   5eec1:      	movq	%rax, -0x58(%rbp)
   5eec5:      	testq	%rax, %rax
   5eec8:      	jne	 <L15>
   5eeca:      	jmp	 <L28>
   5eecf:      	nop
<L13>:
   5eed0:      	movq	0x8(%r15), %rsi
   5eed4:      	movl	$0x1, %ecx
   5eed9:      	callq	 <scoop_heap_record_small_object>
   5eede:      	movq	-0x60(%rbp), %rax
   5eee2:      	movq	0x8(%r15), %rcx
   5eee6:      	movq	0x18(%r15), %rdx
   5eeea:      	movq	0x58(%rax), %rax
   5eeee:      	movq	%rcx, (%rax,%rdx,8)
<L14>:
   5eef2:      	addq	$0x1, 0x1c0(%r12)
   5eefb:      	addq	$0x1, %r13
   5eeff:      	addq	$0x30, %r15
   5ef03:      	cmpq	%r13, -0x58(%rbp)
   5ef07:      	je	 <L28>
<L15>:
   5ef0d:      	movq	0x8(%r15), %rdi
   5ef11:      	movq	0x10(%r15), %rdx
   5ef15:      	movq	(%r15), %rsi
   5ef18:      	callq	 <memcpy@plt>
   5ef1d:      	movq	0x10(%r15), %rax
   5ef21:      	movq	0x28(%r15), %rdi
   5ef25:      	addq	%rax, 0x140(%r12)
   5ef2d:      	movq	0x20(%r15), %rax
   5ef31:      	movq	0x10(%r15), %rdx
   5ef35:      	movq	%rax, -0x60(%rbp)
   5ef39:      	cmpq	$0x7f80, %rdx           # imm = 0x7F80
   5ef40:      	jbe	 <L13>
   5ef42:      	movl	$0x1, %esi
   5ef47:      	callq	 <scoop_heap_publish_large_object>
   5ef4c:      	movq	0x8(%r15), %rdx
   5ef50:      	movq	-0x60(%rbp), %rax
   5ef54:      	movq	%rdx, 0x78(%rax)
   5ef58:      	jmp	 <L14>
   5ef5a:      	nopw	(%rax,%rax)
<L16>:
   5ef60:      	movzbl	-0x58(%rbp), %eax
   5ef64:      	subq	$0x8, %rsp
   5ef68:      	movl	-0x60(%rbp), %r9d
   5ef6c:      	leaq	-0x40(%rbp), %rsi
   5ef70:      	movq	-0x68(%rbp), %rdx
   5ef74:      	movl	$0x10, %r8d
   5ef7a:      	leaq	-0x48(%rbp), %rdi
   5ef7e:      	movq	%r14, %rcx
   5ef81:      	pushq	%rax
   5ef82:      	callq	 <append_move>
   5ef87:      	popq	%r8
   5ef89:      	popq	%r9
   5ef8b:      	movl	%eax, %r15d
   5ef8e:      	jmp	 <L6>
   5ef93:      	nopl	(%rax,%rax)
<L17>:
   5ef98:      	movzbl	0x81(%r12), %eax
   5efa1:      	movb	%al, -0x58(%rbp)
   5efa4:      	testb	%al, %al
   5efa6:      	jne	 <L0>
   5efac:      	cmpq	$0x0, -0x70(%rbp)
   5efb1:      	jne	 <L26>
<L18>:
   5efb7:      	leaq	-0x28(%rbp), %rsp
   5efbb:      	movl	%ebx, %eax
   5efbd:      	popq	%rbx
   5efbe:      	popq	%r12
   5efc0:      	popq	%r13
   5efc2:      	popq	%r14
   5efc4:      	popq	%r15
   5efc6:      	popq	%rbp
   5efc7:      	retq
   5efc8:      	nopl	(%rax,%rax)
<L19>:
   5efd0:      	movq	%r14, %rdi
   5efd3:      	callq	 <free@plt>
   5efd8:      	callq	 <scoop_heap_first_block>
   5efdd:      	movq	%rax, %rbx
   5efe0:      	testq	%rax, %rax
   5efe3:      	jne	 <L25>
   5efe5:      	nopw	%cs:(%rax,%rax)
<L20>:
   5eff0:      	movq	0x28(%r12), %rax
   5eff5:      	testq	%rax, %rax
   5eff8:      	je	 <L22>
   5effa:      	nopw	(%rax,%rax)
<L21>:
   5f000:      	xorl	%edx, %edx
   5f002:      	movw	%dx, 0x2b(%rax)
   5f006:      	movq	0x18(%rax), %rax
   5f00a:      	testq	%rax, %rax
   5f00d:      	jne	 <L21>
<L22>:
   5f00f:      	movzbl	0x81(%r12), %ebx
   5f018:      	pxor	%xmm0, %xmm0
   5f01c:      	movq	$0x0, 0x1b8(%r12)
   5f028:      	movups	%xmm0, 0x1a8(%r12)
   5f031:      	testb	%bl, %bl
   5f033:      	je	 <L18>
   5f035:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x3204>
   5f03c:      	callq	 <scoop_heap_fatal>
   5f041:      	nopl	(%rax)
<L23>:
   5f048:      	cmpl	$0x3, %eax
   5f04b:      	je	 <L29>
<L24>:
   5f051:      	movq	%rbx, %rdi
   5f054:      	callq	 <scoop_heap_next_block>
   5f059:      	movq	%rax, %rbx
   5f05c:      	testq	%rax, %rax
   5f05f:      	je	 <L20>
<L25>:
   5f061:      	movl	0x14(%rbx), %eax
   5f064:      	cmpl	$0x4, %eax
   5f067:      	jne	 <L23>
   5f069:      	movq	%rbx, %rdi
   5f06c:      	callq	 <scoop_heap_release_block>
   5f071:      	jmp	 <L24>
<L26>:
   5f073:      	callq	 <scoop_heap_prepare_evacuation_targets>
   5f078:      	movq	$0x0, -0x48(%rbp)
   5f080:      	movq	$0x0, -0x40(%rbp)
   5f088:      	movq	$0x0, -0x38(%rbp)
   5f090:      	callq	 <scoop_heap_first_block>
   5f095:      	movq	%rax, %r14
   5f098:      	testq	%rax, %rax
   5f09b:      	je	 <L12>
   5f0a1:      	cmpq	$0x1, -0x70(%rbp)
   5f0a6:      	movl	$0x1, -0x60(%rbp)
   5f0ad:      	setne	-0x58(%rbp)
   5f0b1:      	jmp	 <L1>
<L27>:
   5f0b6:      	xorl	%r14d, %r14d
   5f0b9:      	nopl	(%rax)
<L28>:
   5f0c0:      	movq	%r14, %rdi
   5f0c3:      	callq	 <free@plt>
   5f0c8:      	jmp	 <L18>
   5f0cd:      	nopl	(%rax)
<L29>:
   5f0d0:      	movq	%rbx, %rdi
   5f0d3:      	callq	 <scoop_heap_block_has_pins>
   5f0d8:      	movq	0x58(%rbx), %rdi
   5f0dc:      	cmpb	$0x1, %al
   5f0de:      	sbbl	%eax, %eax
   5f0e0:      	andl	$-0x3, %eax
   5f0e3:      	addl	$0x5, %eax
   5f0e6:      	movl	%eax, 0x14(%rbx)
   5f0e9:      	callq	 <free@plt>
   5f0ee:      	movq	$0x0, 0x58(%rbx)
   5f0f6:      	jmp	 <L24>
<L30>:
   5f0fb:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x31dc>
   5f102:      	callq	 <scoop_heap_fatal>
   5f107:      	nopw	(%rax,%rax)

0000000000063c10 <scoop_heap_region_destroy>:
   63c10:      	endbr64
   63c14:      	pushq	%rbp
   63c15:      	movq	%rsp, %rbp
   63c18:      	pushq	%r14
   63c1a:      	movl	$0x88, %r14d
   63c20:      	pushq	%r13
   63c22:      	movq	%rdi, %r13
   63c25:      	pushq	%r12
   63c27:      	pushq	%rbx
   63c28:      	callq	 <scoop_heap_page_map_remove>
   63c2d:      	movq	0x8(%r13), %rsi
   63c31:      	movq	(%r13), %rdi
   63c35:      	callq	 <release_mapping>
   63c3a:      	leaq	, %rax <scoop_gc_heap_state>
   63c41:      	movq	0x8(%r13), %rdx
   63c45:      	addq	%rdx, 0x128(%rax)
   63c4c:      	cmpb	$0x1, 0x2a(%r13)
   63c51:      	sbbq	%rax, %rax
   63c54:      	testl	$0x1ff, %eax            # imm = 0x1FF
   63c59:      	movl	$0x11000, %eax          # imm = 0x11000
   63c5e:      	cmovneq	%rax, %r14
   63c62:      	xorl	%r12d, %r12d
   63c65:      	nopw	%cs:(%rax,%rax)
<L0>:
   63c70:      	movq	0x20(%r13), %rbx
   63c74:      	addq	%r12, %rbx
   63c77:      	addq	$0x88, %r12
   63c7e:      	movq	0x20(%rbx), %rdi
   63c82:      	callq	 <free@plt>
   63c87:      	movq	0x28(%rbx), %rdi
   63c8b:      	callq	 <free@plt>
   63c90:      	movq	0x30(%rbx), %rdi
   63c94:      	callq	 <free@plt>
   63c99:      	movq	0x38(%rbx), %rdi
   63c9d:      	callq	 <free@plt>
   63ca2:      	movq	0x40(%rbx), %rdi
   63ca6:      	callq	 <free@plt>
   63cab:      	movq	0x48(%rbx), %rdi
   63caf:      	callq	 <free@plt>
   63cb4:      	movq	0x50(%rbx), %rdi
   63cb8:      	callq	 <free@plt>
   63cbd:      	movq	0x58(%rbx), %rdi
   63cc1:      	callq	 <free@plt>
   63cc6:      	cmpq	%r12, %r14
   63cc9:      	jne	 <L0>
   63ccb:      	movq	0x20(%r13), %rdi
   63ccf:      	callq	 <free@plt>
   63cd4:      	movq	0x10(%r13), %rdi
   63cd8:      	callq	 <free@plt>
   63cdd:      	movq	%r13, %rdi
   63ce0:      	callq	 <free@plt>
   63ce5:      	popq	%rbx
   63ce6:      	popq	%r12
   63ce8:      	popq	%r13
   63cea:      	popq	%r14
   63cec:      	popq	%rbp
   63ced:      	retq
   63cee:      	nop

0000000000066950 <scoop_gc_heap_finish_collection_locked>:
   66950:      	endbr64
   66954:      	pushq	%rbp
   66955:      	movq	%rsp, %rbp
   66958:      	pushq	%r15
   6695a:      	pushq	%r14
   6695c:      	pushq	%r13
   6695e:      	pushq	%r12
   66960:      	pushq	%rbx
   66961:      	subq	$0x38, %rsp
   66965:      	leaq	, %rbx <scoop_gc_heap_state>
   6696c:      	cmpb	$0x0, 0x85(%rbx)
   66973:      	je	 <L12>
   66979:      	movq	%rdi, %r13
   6697c:      	leaq	-0x50(%rbp), %rdi
   66980:      	movl	%esi, %r12d
   66983:      	callq	 <scoop_thread_allocation_totals_locked>
   66988:      	movq	-0x50(%rbp), %rax
   6698c:      	movq	-0x48(%rbp), %r14
   66990:      	movq	%rax, -0x60(%rbp)
   66994:      	testb	%r12b, %r12b
   66997:      	je	 <L10>
   6699d:      	addq	0x70(%rbx), %rax
   669a1:      	addq	0x60(%rbx), %rax
   669a5:      	subq	0x68(%rbx), %rax
   669a9:      	subq	%r14, %rax
   669ac:      	addq	%rax, %r13
   669af:      	callq	 <scoop_heap_first_block>
   669b4:      	movq	%rax, %r15
   669b7:      	testq	%rax, %rax
   669ba:      	jne	 <L3>
   669bc:      	jmp	 <L11>
   669c1:      	nopl	(%rax)
<L0>:
   669c8:      	testl	%eax, %eax
   669ca:      	jne	 <L1>
   669cc:      	movq	0x68(%r15), %rax
   669d0:      	addq	%rax, 0xb8(%rbx)
<L1>:
   669d7:      	movl	$0x1, 0x1c(%r15)
   669df:      	movzbl	0x81(%rbx), %esi
   669e6:      	movq	%r15, %rdi
   669e9:      	callq	 <scoop_heap_finish_block>
<L2>:
   669ee:      	movq	%r15, %rdi
   669f1:      	callq	 <scoop_heap_next_block>
   669f6:      	movq	%rax, %r15
   669f9:      	testq	%rax, %rax
   669fc:      	je	 <L4>
<L3>:
   669fe:      	movq	%r15, %rdi
   66a01:      	callq	 <scoop_heap_active_head>
   66a06:      	testb	%al, %al
   66a08:      	je	 <L2>
   66a0a:      	movl	0x1c(%r15), %eax
   66a0e:      	testb	%r12b, %r12b
   66a11:      	je	 <L0>
   66a13:      	cmpl	$0x1, %eax
   66a16:      	jne	 <L0>
   66a18:      	cmpl	$0x4, 0x14(%r15)
   66a1d:      	je	 <L1>
   66a1f:      	movq	%r15, %rdi
   66a22:      	callq	 <scoop_heap_next_block>
   66a27:      	movq	%rax, %r15
   66a2a:      	testq	%rax, %rax
   66a2d:      	jne	 <L3>
   66a2f:      	nop
<L4>:
   66a30:      	pxor	%xmm0, %xmm0
   66a34:      	xorl	%edi, %edi
   66a36:      	movq	$0x0, 0x1b8(%rbx)
   66a41:      	movups	%xmm0, 0x1a8(%rbx)
   66a48:      	testb	%r12b, %r12b
   66a4b:      	jne	 <L6>
<L5>:
   66a4d:      	movzbl	0x81(%rbx), %edi
   66a54:      	xorl	$0x1, %edi
   66a57:      	movzbl	%dil, %edi
<L6>:
   66a5b:      	movq	%r13, %xmm1
   66a60:      	movhps	-0x60(%rbp), %xmm1      # xmm1 = xmm1[0,1],mem[0,1]
   66a64:      	movaps	%xmm1, -0x60(%rbp)
   66a68:      	callq	 <scoop_heap_reclaim_regions>
   66a6d:      	movq	0x28(%rbx), %r13
   66a71:      	testq	%r13, %r13
   66a74:      	je	 <L8>
   66a76:      	nopw	%cs:(%rax,%rax)
<L7>:
   66a80:      	movq	0x8(%r13), %rdx
   66a84:      	movq	0x10(%r13), %rdi
   66a88:      	xorl	%esi, %esi
   66a8a:      	shrq	$0x9, %rdx
   66a8e:      	callq	 <memset@plt>
   66a93:      	xorl	%eax, %eax
   66a95:      	movw	%ax, 0x2b(%r13)
   66a9a:      	movq	0x18(%r13), %r13
   66a9e:      	testq	%r13, %r13
   66aa1:      	jne	 <L7>
<L8>:
   66aa3:      	movdqa	-0x60(%rbp), %xmm2
   66aa8:      	movq	%r14, 0x70(%rbx)
   66aac:      	movq	$0x0, 0x88(%rbx)
   66ab7:      	movups	%xmm2, 0x60(%rbx)
   66abb:      	movq	0x1c0(%rbx), %rax
   66ac2:      	movq	%rax, 0x78(%rbx)
   66ac6:      	testb	%r12b, %r12b
   66ac9:      	jne	 <L9>
   66acb:      	movq	0x48(%rbx), %rcx
   66acf:      	movq	$-0x1, %rax
   66ad6:      	leaq	(%rcx,%rcx), %rdx
   66ada:      	testq	%rcx, %rcx
   66add:      	cmovnsq	%rdx, %rax
   66ae1:      	movl	$0x1000000, %edx        # imm = 0x1000000
   66ae6:      	cmpq	%rdx, %rax
   66ae9:      	cmovaeq	%rax, %rdx
   66aed:      	testq	%rcx, %rcx
   66af0:      	cmovnsq	%rdx, %rax
   66af4:      	movq	%rax, 0x50(%rbx)
<L9>:
   66af8:      	movb	$0x0, 0x85(%rbx)
   66aff:      	addq	$0x38, %rsp
   66b03:      	popq	%rbx
   66b04:      	popq	%r12
   66b06:      	popq	%r13
   66b08:      	popq	%r14
   66b0a:      	popq	%r15
   66b0c:      	popq	%rbp
   66b0d:      	retq
   66b0e:      	nop
<L10>:
   66b10:      	callq	 <scoop_heap_free_run_nodes>
   66b15:      	callq	 <scoop_heap_first_block>
   66b1a:      	movq	%rax, %r15
   66b1d:      	testq	%rax, %rax
   66b20:      	jne	 <L3>
   66b26:      	movq	$0x0, 0x1b8(%rbx)
   66b31:      	pxor	%xmm0, %xmm0
   66b35:      	movups	%xmm0, 0x1a8(%rbx)
   66b3c:      	jmp	 <L5>
<L11>:
   66b41:      	pxor	%xmm0, %xmm0
   66b45:      	xorl	%edi, %edi
   66b47:      	movq	$0x0, 0x1b8(%rbx)
   66b52:      	movups	%xmm0, 0x1a8(%rbx)
   66b59:      	jmp	 <L6>
<L12>:
   66b5e:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x49e4>
   66b65:      	callq	 <scoop_heap_fatal>
   66b6a:      	nopw	%cs:(%rax,%rax)
   66b74:      	nopw	%cs:(%rax,%rax)
   66b7e:      	nop

0000000000066c00 <scoop_heap_reclaim_regions>:
   66c00:      	endbr64
   66c04:      	pushq	%rbp
   66c05:      	movq	%rsp, %rbp
   66c08:      	pushq	%r15
   66c0a:      	pushq	%r14
   66c0c:      	pushq	%r13
   66c0e:      	pushq	%r12
   66c10:      	pushq	%rbx
   66c11:      	movl	%edi, %ebx
   66c13:      	subq	$0x58, %rsp
   66c17:      	movl	%edi, -0x68(%rbp)
   66c1a:      	callq	 <scoop_platform_bundle>
   66c1f:      	movq	0x8(%rax), %rax
   66c23:      	movq	%rax, -0x60(%rbp)
   66c27:      	testb	%bl, %bl
   66c29:      	je	 <L37>
   66c2f:      	callq	*0x10(%rax)
   66c32:      	movq	%rax, -0x38(%rbp)
   66c36:      	movq	%rax, %rcx
   66c39:      	testq	%rax, %rax
   66c3c:      	je	 <L42>
   66c42:      	testb	$0x7f, %al
   66c44:      	jne	 <L42>
   66c4a:      	movl	$0x8000, %eax           # imm = 0x8000
   66c4f:      	xorl	%edx, %edx
   66c51:      	divq	%rcx
   66c54:      	testq	%rdx, %rdx
   66c57:      	jne	 <L42>
   66c5d:      	leaq	, %r13 <scoop_gc_heap_state>
   66c64:      	pxor	%xmm0, %xmm0
   66c68:      	movq	0x28(%r13), %r8
   66c6c:      	movups	%xmm0, 0x30(%r13)
   66c71:      	testq	%r8, %r8
   66c74:      	je	 <L8>
<L0>:
   66c7a:      	movq	$0x0, -0x78(%rbp)
   66c82:      	leaq	0x28(%r13), %rbx
   66c86:      	xorl	%r12d, %r12d
   66c89:      	nopl	(%rax)
<L1>:
   66c90:      	cmpb	$0x0, 0x2a(%r8)
   66c95:      	jne	 <L3>
   66c97:      	cmpb	$0x0, -0x68(%rbp)
   66c9b:      	jne	 <L11>
<L2>:
   66ca1:      	movq	0x18(%r8), %rax
   66ca5:      	testq	%rax, %rax
   66ca8:      	je	 <L7>
   66caa:      	cmpb	$0x0, 0x2a(%rax)
   66cae:      	jne	 <L10>
   66cb0:      	movq	0x18(%rax), %r8
   66cb4:      	testq	%r8, %r8
   66cb7:      	je	 <L6>
   66cb9:      	cmpb	$0x0, 0x2a(%r8)
   66cbe:      	je	 <L2>
   66cc0:      	leaq	0x18(%rax), %rbx
   66cc4:      	nop
   66cc5:      	nopw	%cs:(%rax,%rax)
<L3>:
   66cd0:      	movq	0x20(%r8), %rax
   66cd4:      	cmpl	$0x1, 0x14(%rax)
   66cd8:      	jne	 <L9>
<L4>:
   66cda:      	movq	0x18(%r8), %rax
   66cde:      	movq	%r8, %rdi
   66ce1:      	movl	$0x1, %r12d
   66ce7:      	movq	%rax, (%rbx)
   66cea:      	callq	 <scoop_heap_region_destroy>
<L5>:
   66cef:      	movq	(%rbx), %r8
   66cf2:      	testq	%r8, %r8
   66cf5:      	jne	 <L1>
<L6>:
   66cf7:      	cmpb	$0x0, -0x68(%rbp)
   66cfb:      	je	 <L7>
   66cfd:      	movq	-0x78(%rbp), %rax
   66d01:      	movq	%rax, 0x30(%r13)
<L7>:
   66d05:      	testb	%r12b, %r12b
   66d08:      	je	 <L8>
   66d0a:      	callq	 <scoop_heap_page_map_prune>
<L8>:
   66d0f:      	addq	$0x58, %rsp
   66d13:      	popq	%rbx
   66d14:      	popq	%r12
   66d16:      	popq	%r13
   66d18:      	popq	%r14
   66d1a:      	popq	%r15
   66d1c:      	popq	%rbp
   66d1d:      	retq
   66d1e:      	nop
<L9>:
   66d20:      	leaq	0x18(%r8), %rbx
   66d24:      	jmp	 <L5>
<L10>:
   66d26:      	leaq	0x18(%r8), %rbx
   66d2a:      	movq	%rax, %r8
   66d2d:      	jmp	 <L3>
<L11>:
   66d2f:      	movb	%r12b, -0x62(%rbp)
   66d33:      	movq	%r13, %r12
<L12>:
   66d36:      	movzwl	0x28(%r8), %esi
   66d3b:      	testw	%si, %si
   66d3e:      	je	 <L39>
   66d44:      	movq	0x20(%r8), %rcx
   66d48:      	xorl	%edx, %edx
   66d4a:      	leaq	0x14(%rcx), %rax
   66d4e:      	jmp	 <L14>
   66d50:      	nopl	(%rax,%rax)
   66d55:      	nopw	%cs:(%rax,%rax)
<L13>:
   66d60:      	addq	$0x1, %rdx
   66d64:      	addq	$0x88, %rax
   66d6a:      	cmpq	%rdx, %rsi
   66d6d:      	je	 <L25>
<L14>:
   66d73:      	cmpl	$0x1, (%rax)
   66d76:      	jbe	 <L13>
<L15>:
   66d78:      	movzbl	-0x68(%rbp), %edx
   66d7c:      	movq	(%r8), %rsi
   66d7f:      	movq	%r12, -0x80(%rbp)
   66d83:      	xorl	%r12d, %r12d
   66d86:      	movb	%dl, -0x61(%rbp)
   66d89:      	movq	%rsi, %rax
   66d8c:      	movq	%rsi, %r15
   66d8f:      	movq	%r8, %rsi
   66d92:      	movq	%rax, %r8
   66d95:      	nopw	%cs:(%rax,%rax)
<L16>:
   66da0:      	imulq	$0x88, %r12, %rdx
   66da7:      	movq	%rsi, -0x48(%rbp)
   66dab:      	movq	%rax, %r13
   66dae:      	movq	%r12, -0x70(%rbp)
   66db2:      	leaq	(%rcx,%rdx), %rbx
   66db6:      	movq	%r12, %rcx
   66db9:      	xorl	%edx, %edx
   66dbb:      	shlq	$0xf, %rcx
   66dbf:      	movq	%rbx, %r12
   66dc2:      	movq	%r15, %rbx
   66dc5:      	movq	%rcx, -0x40(%rbp)
   66dc9:      	movq	%r8, %rcx
   66dcc:      	nopl	(%rax)
<L17>:
   66dd0:      	movq	-0x38(%rbp), %rax
   66dd4:      	addq	%rdx, %r13
   66dd7:      	addq	-0x40(%rbp), %r13
   66ddb:      	leaq	(%rax,%rdx), %r14
   66ddf:      	cmpb	$0x0, 0x84(%r12)
   66de8:      	je	 <L20>
   66dee:      	cmpl	$0x6, 0x14(%r12)
   66df4:      	je	 <L20>
   66df6:      	movq	%r12, %rdi
   66df9:      	movq	%rcx, -0x50(%rbp)
   66dfd:      	movq	%rdx, -0x58(%rbp)
   66e01:      	callq	 <scoop_heap_active_head>
   66e06:      	movq	-0x50(%rbp), %rcx
   66e0a:      	movq	-0x58(%rbp), %rdx
   66e0e:      	testb	%al, %al
   66e10:      	je	 <L23>
   66e16:      	movq	%r14, %r10
   66e19:      	shrq	$0x7, %rdx
   66e1d:      	shrq	$0x7, %r10
   66e21:      	cmpq	%r10, %rdx
   66e24:      	jae	 <L28>
   66e2a:      	movq	%rbx, -0x58(%rbp)
   66e2e:      	movq	%r10, %r15
   66e31:      	movq	%r12, %rbx
   66e34:      	movq	%rdx, %r12
   66e37:      	movq	%rcx, -0x50(%rbp)
   66e3b:      	jmp	 <L19>
   66e3d:      	nopl	(%rax)
<L18>:
   66e40:      	addq	$0x1, %r12
   66e44:      	cmpq	%r12, %r15
   66e47:      	je	 <L22>
<L19>:
   66e49:      	movq	0x40(%rbx), %rdi
   66e4d:      	movq	%r12, %rsi
   66e50:      	callq	 <scoop_heap_bit_test>
   66e55:      	testb	%al, %al
   66e57:      	je	 <L18>
   66e59:      	movq	%rbx, %r12
   66e5c:      	movq	-0x50(%rbp), %rcx
   66e60:      	movq	-0x58(%rbp), %rbx
   66e64:      	nop
   66e65:      	nopw	%cs:(%rax,%rax)
<L20>:
   66e70:      	movq	-0x60(%rbp), %rdi
   66e74:      	movq	%rcx, %rdx
   66e77:      	movq	%rbx, %rsi
   66e7a:      	callq	 <discard_range>
   66e7f:      	andb	%al, -0x61(%rbp)
   66e82:      	movq	-0x38(%rbp), %rax
   66e86:      	leaq	(%r13,%rax), %rbx
   66e8b:      	movq	%rbx, %rcx
   66e8e:      	cmpq	$0x7fff, %r14           # imm = 0x7FFF
   66e95:      	ja	 <L24>
<L21>:
   66e97:      	movq	-0x48(%rbp), %rax
   66e9b:      	movq	%r14, %rdx
   66e9e:      	movq	(%rax), %r13
   66ea1:      	jmp	 <L17>
   66ea6:      	nopw	%cs:(%rax,%rax)
<L22>:
   66eb0:      	movq	%rbx, %r12
   66eb3:      	movq	-0x50(%rbp), %rcx
   66eb7:      	movq	-0x58(%rbp), %rbx
<L23>:
   66ebb:      	movq	-0x38(%rbp), %rax
   66ebf:      	cmpq	%rcx, %rbx
   66ec2:      	cmoveq	%r13, %rbx
   66ec6:      	leaq	(%r13,%rax), %rcx
   66ecb:      	cmpq	$0x7fff, %r14           # imm = 0x7FFF
   66ed2:      	jbe	 <L21>
<L24>:
   66ed4:      	movq	-0x48(%rbp), %rsi
   66ed8:      	movq	-0x70(%rbp), %r12
   66edc:      	movq	%rcx, %r8
   66edf:      	movq	%rbx, %r15
   66ee2:      	movzwl	0x28(%rsi), %eax
   66ee6:      	addq	$0x1, %r12
   66eea:      	cmpq	%rax, %r12
   66eed:      	jae	 <L29>
   66eef:      	movq	0x20(%rsi), %rcx
   66ef3:      	movq	(%rsi), %rax
   66ef6:      	jmp	 <L16>
<L25>:
   66efb:      	cmpq	$0x0, -0x78(%rbp)
   66f00:      	je	 <L27>
<L26>:
   66f02:      	movq	%r12, %r13
   66f05:      	jmp	 <L4>
<L27>:
   66f0a:      	movq	%r8, -0x78(%rbp)
   66f0e:      	jmp	 <L15>
<L28>:
   66f13:      	movq	-0x38(%rbp), %rax
   66f17:      	cmpq	%rcx, %rbx
   66f1a:      	leaq	(%r13,%rax), %rcx
   66f1f:      	jne	 <L21>
   66f25:      	movq	%r13, %rbx
   66f28:      	jmp	 <L21>
<L29>:
   66f2d:      	movq	-0x80(%rbp), %r12
   66f31:      	movq	%rsi, %r8
   66f34:      	movq	%rcx, %r9
   66f37:      	movq	%rbx, %rsi
<L30>:
   66f3a:      	movq	-0x60(%rbp), %rdi
   66f3e:      	movq	%r9, %rdx
   66f41:      	movq	%r8, -0x40(%rbp)
   66f45:      	callq	 <discard_range>
   66f4a:      	movq	-0x40(%rbp), %r8
   66f4e:      	testb	%al, %al
   66f50:      	movzwl	0x28(%r8), %esi
   66f55:      	je	 <L38>
   66f5b:      	cmpb	$0x0, -0x61(%rbp)
   66f5f:      	je	 <L38>
   66f65:      	testw	%si, %si
   66f68:      	je	 <L36>
   66f6e:      	imulq	$0x88, %rsi, %rdi
   66f75:      	movq	0x20(%r8), %rax
   66f79:      	leaq	0x84(%rax), %rdx
   66f80:      	leaq	(%rdi,%rdx), %rcx
   66f84:      	andl	$0x8, %edi
   66f87:      	je	 <L31>
   66f89:      	leaq	0x10c(%rax), %rdx
   66f90:      	movb	$0x0, 0x84(%rax)
   66f97:      	cmpq	%rdx, %rcx
   66f9a:      	je	 <L32>
   66f9c:      	nopl	(%rax)
<L31>:
   66fa0:      	movb	$0x0, (%rdx)
   66fa3:      	addq	$0x110, %rdx            # imm = 0x110
   66faa:      	movb	$0x0, -0x88(%rdx)
   66fb1:      	cmpq	%rdx, %rcx
   66fb4:      	jne	 <L31>
<L32>:
   66fb6:      	xorl	%ecx, %ecx
   66fb8:      	xorl	%edx, %edx
   66fba:      	jmp	 <L34>
   66fbc:      	nopl	(%rax)
<L33>:
   66fc0:      	movq	0x20(%r8), %rax
<L34>:
   66fc4:      	addq	%rcx, %rax
   66fc7:      	cmpl	$0x1, 0x14(%rax)
   66fcb:      	jne	 <L35>
   66fcd:      	movq	0x38(%r12), %rdi
   66fd2:      	movq	%rdi, 0x8(%rax)
   66fd6:      	movq	%rax, 0x38(%r12)
<L35>:
   66fdb:      	addq	$0x1, %rdx
   66fdf:      	addq	$0x88, %rcx
   66fe6:      	cmpq	%rsi, %rdx
   66fe9:      	jne	 <L33>
<L36>:
   66feb:      	movq	0x18(%r8), %rax
   66fef:      	testq	%rax, %rax
   66ff2:      	je	 <L41>
   66ff4:      	leaq	0x18(%r8), %rbx
   66ff8:      	cmpb	$0x0, 0x2a(%rax)
   66ffc:      	je	 <L40>
   66ffe:      	movq	%r12, %r13
   67001:      	movq	%rax, %r8
   67004:      	movzbl	-0x62(%rbp), %r12d
   67009:      	jmp	 <L3>
   6700e:      	nop
<L37>:
   67010:      	leaq	, %r13 <scoop_gc_heap_state>
   67017:      	movq	$0x8000, -0x38(%rbp)    # imm = 0x8000
   6701f:      	movq	0x28(%r13), %r8
   67023:      	testq	%r8, %r8
   67026:      	jne	 <L0>
   6702c:      	jmp	 <L8>
<L38>:
   67031:      	testq	%rsi, %rsi
   67034:      	je	 <L36>
   67036:      	movq	0x20(%r8), %rax
   6703a:      	jmp	 <L32>
<L39>:
   6703f:      	cmpq	$0x0, -0x78(%rbp)
   67044:      	jne	 <L26>
   6704a:      	movq	(%r8), %rsi
   6704d:      	movzbl	-0x68(%rbp), %eax
   67051:      	movq	%r8, -0x78(%rbp)
   67055:      	movb	%al, -0x61(%rbp)
   67058:      	movq	%rsi, %r9
   6705b:      	jmp	 <L30>
<L40>:
   67060:      	movq	%rax, %r8
   67063:      	jmp	 <L12>
<L41>:
   67068:      	movq	%r12, %r13
   6706b:      	movzbl	-0x62(%rbp), %r12d
   67070:      	jmp	 <L6>
<L42>:
   67075:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x4a0c>
   6707c:      	callq	 <scoop_heap_fatal>
   67081:      	nopw	%cs:(%rax,%rax)
   6708b:      	nopw	%cs:(%rax,%rax)
   67095:      	nopw	%cs:(%rax,%rax)
   6709f:      	nopw	%cs:(%rax,%rax)
   670a9:      	nopw	%cs:(%rax,%rax)
   670b3:      	nopw	%cs:(%rax,%rax)
   670bd:      	nopl	(%rax)

00000000000719d0 <scoop_heap_select_evacuation_sources>:
   719d0:      	endbr64
   719d4:      	pushq	%rbp
   719d5:      	movq	%rsp, %rbp
   719d8:      	pushq	%r15
   719da:      	pushq	%r14
   719dc:      	pushq	%r13
   719de:      	pushq	%r12
   719e0:      	pushq	%rbx
   719e1:      	subq	$0x38, %rsp
   719e5:      	movl	%edi, -0x34(%rbp)
   719e8:      	movq	$0x0, -0x48(%rbp)
   719f0:      	testb	%dil, %dil
   719f3:      	jne	 <L0>
   719f5:      	leaq	, %rax <scoop_gc_heap_state>
   719fc:      	cmpb	$0x0, 0x81(%rax)
   71a03:      	je	 <L9>
<L0>:
   71a09:      	callq	 <scoop_heap_first_block>
   71a0e:      	movq	%rax, %rbx
   71a11:      	testq	%rax, %rax
   71a14:      	je	 <L5>
   71a16:      	nopw	%cs:(%rax,%rax)
<L1>:
   71a20:      	movq	%rbx, %rdi
   71a23:      	callq	 <scoop_heap_active_head>
   71a28:      	testb	%al, %al
   71a2a:      	je	 <L4>
   71a2c:      	cmpq	$0x0, 0x70(%rbx)
   71a31:      	je	 <L4>
   71a33:      	cmpb	$0x0, -0x34(%rbp)
   71a37:      	je	 <L6>
   71a39:      	movl	0x1c(%rbx), %eax
   71a3c:      	testl	%eax, %eax
   71a3e:      	jne	 <L4>
   71a40:      	movl	0x18(%rbx), %ecx
   71a43:      	cmpl	$0x2, %ecx
   71a46:      	je	 <L21>
   71a4c:      	movq	0x30(%rbx), %rax
   71a50:      	leaq	0x200(%rax), %rdx
   71a57:      	jmp	 <L3>
   71a59:      	nopl	(%rax)
<L2>:
   71a60:      	addq	$0x8, %rax
   71a64:      	cmpq	%rdx, %rax
   71a67:      	je	 <L8>
<L3>:
   71a69:      	cmpq	$0x0, (%rax)
   71a6d:      	je	 <L2>
   71a6f:      	nop
<L4>:
   71a70:      	movq	%rbx, %rdi
   71a73:      	callq	 <scoop_heap_next_block>
   71a78:      	movq	%rax, %rbx
   71a7b:      	testq	%rax, %rax
   71a7e:      	jne	 <L1>
<L5>:
   71a80:      	movq	-0x48(%rbp), %rax
   71a84:      	addq	$0x38, %rsp
   71a88:      	popq	%rbx
   71a89:      	popq	%r12
   71a8b:      	popq	%r13
   71a8d:      	popq	%r14
   71a8f:      	popq	%r15
   71a91:      	popq	%rbp
   71a92:      	retq
   71a93:      	nopl	(%rax,%rax)
<L6>:
   71a98:      	leaq	, %rax <scoop_gc_heap_state>
   71a9f:      	cmpb	$0x0, 0x81(%rax)
   71aa6:      	jne	 <L7>
   71aa8:      	movq	(%rbx), %rax
   71aab:      	cmpb	$0x0, 0x2b(%rax)
   71aaf:      	je	 <L4>
<L7>:
   71ab1:      	movl	0x18(%rbx), %ecx
<L8>:
   71ab4:      	movl	$0x3, 0x14(%rbx)
   71abb:      	cmpl	$0x1, %ecx
   71abe:      	jne	 <L4>
   71ac0:      	movl	$0x8, %esi
   71ac5:      	movl	$0x1000, %edi           # imm = 0x1000
   71aca:      	callq	 <calloc@plt>
   71acf:      	movq	%rax, 0x58(%rbx)
   71ad3:      	testq	%rax, %rax
   71ad6:      	jne	 <L4>
   71ad8:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6a2c>
   71adf:      	callq	 <scoop_heap_fatal>
   71ae4:      	nopl	(%rax)
<L9>:
   71ae8:      	movq	0x28(%rax), %r15
   71aec:      	testq	%r15, %r15
   71aef:      	je	 <L0>
   71af5:      	movq	$0x0, -0x50(%rbp)
   71afd:      	movq	$0x0, -0x58(%rbp)
   71b05:      	jmp	 <L11>
   71b07:      	nopl	(%rax)
   71b0a:      	nopw	%cs:(%rax,%rax)
   71b15:      	nopw	%cs:(%rax,%rax)
<L10>:
   71b20:      	movq	0x18(%r15), %r15
   71b24:      	testq	%r15, %r15
   71b27:      	je	 <L18>
<L11>:
   71b2d:      	movzbl	0x2a(%r15), %ebx
   71b32:      	testb	%bl, %bl
   71b34:      	jne	 <L10>
   71b36:      	cmpw	$0x0, 0x28(%r15)
   71b3c:      	je	 <L22>
   71b42:      	movq	$0x0, -0x40(%rbp)
   71b4a:      	xorl	%r13d, %r13d
   71b4d:      	xorl	%r14d, %r14d
<L12>:
   71b50:      	movq	0x20(%r15), %r12
   71b54:      	addq	%r13, %r12
   71b57:      	movq	%r12, %rdi
   71b5a:      	callq	 <scoop_heap_active_head>
   71b5f:      	testb	%al, %al
   71b61:      	je	 <L16>
   71b63:      	movq	0x68(%r12), %rdi
   71b68:      	addq	%rdi, -0x40(%rbp)
   71b6c:      	cmpl	$0x2, 0x18(%r12)
   71b72:      	je	 <L19>
   71b78:      	movq	0x30(%r12), %rdx
   71b7d:      	leaq	0x200(%rdx), %rcx
   71b84:      	jmp	 <L14>
   71b86:      	nopw	%cs:(%rax,%rax)
<L13>:
   71b90:      	addq	$0x8, %rdx
   71b94:      	cmpq	%rdx, %rcx
   71b97:      	je	 <L20>
<L14>:
   71b9d:      	cmpq	$0x0, (%rdx)
   71ba1:      	je	 <L13>
<L15>:
   71ba3:      	movl	%eax, %ebx
<L16>:
   71ba5:      	movzwl	0x28(%r15), %eax
   71baa:      	addq	$0x1, %r14
   71bae:      	addq	$0x88, %r13
   71bb5:      	cmpq	%rax, %r14
   71bb8:      	jb	 <L12>
   71bba:      	movq	-0x40(%rbp), %rsi
   71bbe:      	leaq	-0x1(%rsi), %rax
   71bc2:      	cmpq	$0x3fffff, %rax         # imm = 0x3FFFFF
   71bc8:      	seta	%al
   71bcb:      	orl	%ebx, %eax
   71bcd:      	xorl	$0x1, %eax
   71bd0:      	movl	%eax, %ebx
   71bd2:      	movzbl	%al, %eax
   71bd5:      	addq	%rax, -0x48(%rbp)
   71bd9:      	movq	-0x58(%rbp), %rax
   71bdd:      	cmpq	%rsi, %rax
   71be0:      	cmovbq	%rsi, %rax
   71be4:      	movq	%rax, -0x58(%rbp)
   71be8:      	movq	-0x50(%rbp), %rax
   71bec:      	cmovbq	%r15, %rax
   71bf0:      	movq	%rax, -0x50(%rbp)
<L17>:
   71bf4:      	movq	-0x40(%rbp), %rax
   71bf8:      	movb	%bl, 0x2b(%r15)
   71bfc:      	movq	%rax, 0x30(%r15)
   71c00:      	movq	0x18(%r15), %r15
   71c04:      	testq	%r15, %r15
   71c07:      	jne	 <L11>
<L18>:
   71c0d:      	movq	-0x50(%rbp), %rax
   71c11:      	testq	%rax, %rax
   71c14:      	je	 <L0>
   71c1a:      	cmpb	$0x0, 0x2b(%rax)
   71c1e:      	je	 <L0>
   71c24:      	subq	$0x1, -0x48(%rbp)
   71c29:      	movb	$0x0, 0x2b(%rax)
   71c2d:      	jmp	 <L0>
   71c32:      	nopw	(%rax,%rax)
<L19>:
   71c38:      	movzbl	0x82(%r12), %eax
   71c41:      	orl	%ebx, %eax
   71c43:      	jmp	 <L15>
   71c48:      	nopl	(%rax,%rax)
<L20>:
   71c50:      	movl	%ebx, %eax
   71c52:      	jmp	 <L15>
<L21>:
   71c57:      	cmpb	$0x0, 0x82(%rbx)
   71c5e:      	jne	 <L4>
   71c64:      	movl	$0x3, 0x14(%rbx)
   71c6b:      	jmp	 <L4>
<L22>:
   71c70:      	movq	$0x0, -0x40(%rbp)
   71c78:      	jmp	 <L17>
   71c7d:      	nopl	(%rax)

0000000000075000 <scoop_heap_prepare_evacuation_targets>:
   75000:      	endbr64
   75004:      	pushq	%rbp
   75005:      	movq	%rsp, %rbp
   75008:      	pushq	%r12
   7500a:      	pushq	%rbx
   7500b:      	callq	 <scoop_heap_first_block>
   75010:      	testq	%rax, %rax
   75013:      	je	 <L6>
   75019:      	movq	%rax, %r12
   7501c:      	jmp	 <L1>
   7501e:      	nop
<L0>:
   75020:      	movq	%r12, %rdi
   75023:      	callq	 <scoop_heap_next_block>
   75028:      	movq	%rax, %r12
   7502b:      	testq	%rax, %rax
   7502e:      	je	 <L6>
<L1>:
   75034:      	movq	%r12, %rdi
   75037:      	callq	 <scoop_heap_active_head>
   7503c:      	testb	%al, %al
   7503e:      	je	 <L0>
   75040:      	cmpl	$0x1, 0x18(%r12)
   75046:      	jne	 <L0>
   75048:      	movq	(%r12), %rax
   7504c:      	cmpb	$0x0, 0x2b(%rax)
   75050:      	jne	 <L0>
   75052:      	movl	0x1c(%r12), %ecx
   75057:      	testl	%ecx, %ecx
   75059:      	jne	 <L2>
   7505b:      	cmpq	$0x0, 0x68(%r12)
   75061:      	jne	 <L0>
<L2>:
   75063:      	movl	$0x10, %ebx
   75068:      	jmp	 <L4>
   7506a:      	nopw	(%rax,%rax)
<L3>:
   75070:      	addq	$0x1, %rbx
   75074:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   7507b:      	je	 <L5>
<L4>:
   7507d:      	movq	0x20(%r12), %rdi
   75082:      	movq	%rbx, %rsi
   75085:      	callq	 <scoop_heap_bit_test>
   7508a:      	testb	%al, %al
   7508c:      	je	 <L3>
   7508e:      	movq	0x28(%r12), %rdi
   75093:      	movq	%rbx, %rsi
   75096:      	callq	 <scoop_heap_bit_test>
   7509b:      	testb	%al, %al
   7509d:      	jne	 <L3>
   7509f:      	movq	0x50(%r12), %rax
   750a4:      	cmpw	$0x0, (%rax,%rbx,2)
   750a9:      	je	 <L8>
   750af:      	movq	%r12, %rdi
   750b2:      	callq	 <scoop_heap_block_base>
   750b7:      	leaq	(%rax,%rbx,8), %rdi
   750bb:      	callq	 <release_dead_object>
   750c0:      	movq	0x20(%r12), %rdi
   750c5:      	movq	%rbx, %rsi
   750c8:      	callq	 <scoop_heap_bit_clear>
   750cd:      	movq	0x30(%r12), %rdi
   750d2:      	movq	%rbx, %rsi
   750d5:      	callq	 <scoop_heap_bit_clear>
   750da:      	movq	0x50(%r12), %rax
   750df:      	xorl	%edx, %edx
   750e1:      	movw	%dx, (%rax,%rbx,2)
   750e5:      	addq	$0x1, %rbx
   750e9:      	movb	$0x1, 0x84(%r12)
   750f2:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   750f9:      	jne	 <L4>
<L5>:
   750fb:      	cmpq	$0x0, 0x68(%r12)
   75101:      	jne	 <L7>
   75103:      	movq	%r12, %rdi
   75106:      	callq	 <scoop_heap_release_block>
   7510b:      	movq	%r12, %rdi
   7510e:      	callq	 <scoop_heap_next_block>
   75113:      	movq	%rax, %r12
   75116:      	testq	%rax, %rax
   75119:      	jne	 <L1>
<L6>:
   7511f:      	popq	%rbx
   75120:      	popq	%r12
   75122:      	popq	%rbp
   75123:      	retq
<L7>:
   75124:      	movq	0x48(%r12), %rsi
   75129:      	movq	%r12, %rdi
   7512c:      	callq	 <record_free_runs>
   75131:      	jmp	 <L0>
<L8>:
   75136:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x703c>
   7513d:      	callq	 <scoop_heap_fatal>
   75142:      	nopl	(%rax)
   75145:      	nopw	%cs:(%rax,%rax)

0000000000075150 <scoop_heap_finish_block>:
   75150:      	endbr64
   75154:      	pushq	%rbp
   75155:      	movq	%rsp, %rbp
   75158:      	pushq	%r15
   7515a:      	movq	%rdi, %r15
   7515d:      	pushq	%r14
   7515f:      	pushq	%r13
   75161:      	pushq	%r12
   75163:      	pushq	%rbx
   75164:      	subq	$0x18, %rsp
   75168:      	movl	0x18(%rdi), %eax
   7516b:      	movl	%esi, -0x40(%rbp)
   7516e:      	cmpl	$0x1, %eax
   75171:      	je	 <L3>
   75177:      	cmpl	$0x2, %eax
   7517a:      	jne	 <L25>
   75180:      	movzbl	0x81(%rdi), %edx
   75187:      	cmpl	$0x3, 0x14(%rdi)
   7518b:      	je	 <L2>
   7518d:      	testb	%dl, %dl
   7518f:      	je	 <L18>
   75195:      	cmpb	$0x0, 0x82(%rdi)
   7519c:      	movl	$0x5, %edx
   751a1:      	cmovnel	%edx, %eax
<L0>:
   751a4:      	movl	%eax, 0x14(%r15)
   751a8:      	pxor	%xmm0, %xmm0
   751ac:      	movq	$0x0, 0x78(%r15)
   751b4:      	movb	$0x0, 0x81(%r15)
   751bc:      	movb	$0x0, 0x83(%r15)
   751c4:      	movups	%xmm0, 0x68(%r15)
<L1>:
   751c9:      	addq	$0x18, %rsp
   751cd:      	popq	%rbx
   751ce:      	popq	%r12
   751d0:      	popq	%r13
   751d2:      	popq	%r14
   751d4:      	popq	%r15
   751d6:      	popq	%rbp
   751d7:      	retq
   751d8:      	nopl	(%rax,%rax)
<L2>:
   751e0:      	cmpb	$0x0, 0x82(%rdi)
   751e7:      	jne	 <L17>
   751ed:      	testb	%dl, %dl
   751ef:      	je	 <L18>
   751f5:      	cmpq	$0x0, 0x78(%rdi)
   751fa:      	jne	 <L19>
   75200:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x70b4>
   75207:      	callq	 <scoop_heap_fatal>
   7520c:      	nopl	(%rax)
<L3>:
   75210:      	movq	0x40(%rdi), %rax
   75214:      	movb	$0x0, -0x3a(%rbp)
   75218:      	pxor	%xmm0, %xmm0
   7521c:      	movl	$0x80, %r12d
   75222:      	movb	$0x0, -0x3b(%rbp)
   75226:      	movl	$0x10, %ebx
   7522b:      	movups	%xmm0, (%rax)
   7522e:      	movups	%xmm0, 0x10(%rax)
   75232:      	nopl	(%rax)
   75235:      	nopw	%cs:(%rax,%rax)
<L4>:
   75240:      	movq	0x20(%r15), %rdi
   75244:      	movq	%rbx, %rsi
   75247:      	callq	 <scoop_heap_bit_test>
   7524c:      	testb	%al, %al
   7524e:      	je	 <L11>
   75254:      	movq	0x30(%r15), %rdi
   75258:      	movq	%rbx, %rsi
   7525b:      	callq	 <scoop_heap_bit_test>
   75260:      	movq	0x28(%r15), %rdi
   75264:      	movq	%rbx, %rsi
   75267:      	movb	%al, -0x39(%rbp)
   7526a:      	movl	%eax, %r14d
   7526d:      	callq	 <scoop_heap_bit_test>
   75272:      	movb	%al, -0x38(%rbp)
   75275:      	cmpl	$0x3, 0x14(%r15)
   7527a:      	jne	 <L5>
   7527c:      	testb	%r14b, %r14b
   7527f:      	je	 <L16>
<L5>:
   75285:      	cmpb	$0x0, -0x38(%rbp)
   75289:      	jne	 <L12>
<L6>:
   7528f:      	movq	0x50(%r15), %rax
   75293:      	leaq	(%rbx,%rbx), %r13
   75297:      	movzwl	(%rax,%rbx,2), %edx
   7529b:      	shlq	$0x3, %rdx
   7529f:      	movq	%rdx, %r14
   752a2:      	jne	 <L8>
<L7>:
   752a4:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x703c>
   752ab:      	callq	 <scoop_heap_fatal>
<L8>:
   752b0:      	movq	%r15, %rdi
   752b3:      	callq	 <scoop_heap_block_base>
   752b8:      	leaq	(%rax,%r12), %rdi
   752bc:      	movq	%rdi, -0x38(%rbp)
   752c0:      	callq	 <release_dead_object>
   752c5:      	movq	-0x38(%rbp), %rdi
<L9>:
   752c9:      	cmpb	$0x0, -0x40(%rbp)
   752cd:      	jne	 <L15>
<L10>:
   752d3:      	movq	0x20(%r15), %rdi
   752d7:      	movq	%rbx, %rsi
   752da:      	callq	 <scoop_heap_bit_clear>
   752df:      	movq	0x30(%r15), %rdi
   752e3:      	movq	%rbx, %rsi
   752e6:      	callq	 <scoop_heap_bit_clear>
   752eb:      	movq	0x50(%r15), %rax
   752ef:      	xorl	%edx, %edx
   752f1:      	movw	%dx, (%rax,%r13)
   752f6:      	movb	$0x1, 0x84(%r15)
<L11>:
   752fe:      	addq	$0x1, %rbx
   75302:      	addq	$0x8, %r12
   75306:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   7530d:      	jne	 <L4>
   75313:      	movq	0x28(%r15), %rdx
   75317:      	xorl	%eax, %eax
   75319:      	pxor	%xmm0, %xmm0
   7531d:      	leaq	0x8(%rdx), %rdi
   75321:      	movq	$0x0, (%rdx)
   75328:      	movq	$0x0, 0x1f8(%rdx)
   75333:      	andq	$-0x8, %rdi
   75337:      	subq	%rdi, %rdx
   7533a:      	leal	0x200(%rdx), %ecx
   75340:      	shrl	$0x3, %ecx
   75343:      	rep		stosq	%rax, %es:(%rdi)
   75346:      	movq	0x38(%r15), %rdx
   7534a:      	leaq	0x8(%rdx), %rdi
   7534e:      	movq	$0x0, (%rdx)
   75355:      	movq	$0x0, 0x1f8(%rdx)
   75360:      	andq	$-0x8, %rdi
   75364:      	subq	%rdi, %rdx
   75367:      	leal	0x200(%rdx), %ecx
   7536d:      	shrl	$0x3, %ecx
   75370:      	rep		stosq	%rax, %es:(%rdi)
   75373:      	movq	0x48(%r15), %rax
   75377:      	movups	%xmm0, (%rax)
   7537a:      	movups	%xmm0, 0x10(%rax)
   7537e:      	movq	0x58(%r15), %rdi
   75382:      	callq	 <free@plt>
   75387:      	pxor	%xmm0, %xmm0
   7538b:      	movq	$0x0, 0x58(%r15)
   75393:      	movups	%xmm0, 0x68(%r15)
   75398:      	cmpb	$0x0, -0x3a(%rbp)
   7539c:      	je	 <L21>
   753a2:      	cmpb	$0x1, -0x3b(%rbp)
   753a6:      	sbbl	%eax, %eax
   753a8:      	andl	$-0x3, %eax
   753ab:      	addl	$0x5, %eax
   753ae:      	movl	%eax, 0x14(%r15)
   753b2:      	cmpb	$0x0, -0x40(%rbp)
   753b6:      	jne	 <L1>
   753bc:      	movq	0x40(%r15), %rsi
   753c0:      	movq	%r15, %rdi
   753c3:      	callq	 <record_free_runs>
   753c8:      	jmp	 <L1>
   753cd:      	nopl	(%rax)
<L12>:
   753d0:      	movq	0x50(%r15), %rax
   753d4:      	movzwl	(%rax,%rbx,2), %eax
   753d8:      	shlq	$0x3, %rax
   753dc:      	je	 <L23>
   753e2:      	movq	%r12, %r14
   753e5:      	leaq	-0x1(%rax,%r12), %rdx
   753ea:      	shrq	$0x7, %rdx
   753ee:      	shrq	$0x7, %r14
   753f2:      	leaq	0x1(%rdx), %r13
   753f6:      	cmpq	%r14, %rdx
   753f9:      	jb	 <L14>
   753fb:      	nopl	(%rax,%rax)
<L13>:
   75400:      	movq	0x40(%r15), %rdi
   75404:      	movq	%r14, %rsi
   75407:      	addq	$0x1, %r14
   7540b:      	callq	 <scoop_heap_bit_set>
   75410:      	cmpq	%r13, %r14
   75413:      	jne	 <L13>
<L14>:
   75415:      	movzbl	-0x38(%rbp), %eax
   75419:      	movzbl	-0x39(%rbp), %ecx
   7541d:      	orb	%cl, -0x3b(%rbp)
   75420:      	movb	%al, -0x3a(%rbp)
   75423:      	jmp	 <L11>
   75428:      	nopl	(%rax,%rax)
<L15>:
   75430:      	movq	%r14, %rdx
   75433:      	movl	$0xa5, %esi
   75438:      	callq	 <memset@plt>
   7543d:      	jmp	 <L10>
   75442:      	nopw	(%rax,%rax)
<L16>:
   75448:      	testb	%al, %al
   7544a:      	je	 <L6>
   75450:      	movq	0x58(%r15), %rax
   75454:      	cmpq	$0x0, (%rax,%r12)
   75459:      	je	 <L24>
   7545f:      	movq	0x50(%r15), %rax
   75463:      	leaq	(%rbx,%rbx), %r13
   75467:      	movzwl	(%rax,%rbx,2), %edx
   7546b:      	shlq	$0x3, %rdx
   7546f:      	movq	%rdx, %r14
   75472:      	je	 <L7>
   75478:      	movq	%r15, %rdi
   7547b:      	callq	 <scoop_heap_block_base>
   75480:      	leaq	(%rax,%r12), %rdi
   75484:      	jmp	 <L9>
<L17>:
   75489:      	movl	$0x5, %eax
   7548e:      	testb	%dl, %dl
   75490:      	jne	 <L0>
   75496:      	nopw	%cs:(%rax,%rax)
<L18>:
   754a0:      	movq	%r15, %rdi
   754a3:      	callq	 <scoop_heap_block_base>
   754a8:      	leaq	0x80(%rax), %rdi
   754af:      	callq	 <release_dead_object>
<L19>:
   754b4:      	cmpb	$0x0, -0x40(%rbp)
   754b8:      	je	 <L22>
   754ba:      	movq	0x60(%r15), %rbx
   754be:      	movq	%r15, %rdi
   754c1:      	callq	 <scoop_heap_block_base>
   754c6:      	movl	$0xa5, %esi
   754cb:      	leaq	0x80(%rax), %rdi
   754d2:      	movq	%rbx, %rdx
   754d5:      	callq	 <memset@plt>
<L20>:
   754da:      	movq	%r15, %rdi
   754dd:      	callq	 <scoop_heap_quarantine_block>
   754e2:      	addq	$0x18, %rsp
   754e6:      	popq	%rbx
   754e7:      	popq	%r12
   754e9:      	popq	%r13
   754eb:      	popq	%r14
   754ed:      	popq	%r15
   754ef:      	popq	%rbp
   754f0:      	retq
<L21>:
   754f1:      	cmpb	$0x0, -0x40(%rbp)
   754f5:      	jne	 <L20>
<L22>:
   754f7:      	movq	%r15, %rdi
   754fa:      	callq	 <scoop_heap_release_block>
   754ff:      	addq	$0x18, %rsp
   75503:      	popq	%rbx
   75504:      	popq	%r12
   75506:      	popq	%r13
   75508:      	popq	%r14
   7550a:      	popq	%r15
   7550c:      	popq	%rbp
   7550d:      	retq
<L23>:
   7550e:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x7094>
   75515:      	callq	 <scoop_heap_fatal>
<L24>:
   7551a:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x7064>
   75521:      	callq	 <scoop_heap_fatal>
<L25>:
   75526:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x70ec>
   7552d:      	callq	 <scoop_heap_fatal>
   75532:      	nopw	%cs:(%rax,%rax)
   7553c:      	nopl	(%rax)
