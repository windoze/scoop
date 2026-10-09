
/home/chenxu/repos/scoop/tmp/m34/marker-on-linux-gnu/mark-graphs:	file format elf64-x86-64

Disassembly of section .text:

0000000000063940 <scoop_gc_mark_slot>:
   63940:      	endbr64
   63944:      	pushq	%rbp
   63945:      	movq	%rsp, %rbp
   63948:      	pushq	%r15
   6394a:      	pushq	%r14
   6394c:      	pushq	%r13
   6394e:      	pushq	%r12
   63950:      	pushq	%rbx
   63951:      	subq	$0x68, %rsp
   63955:      	movl	0x8(%rsi), %eax
   63958:      	movq	(%rsi), %rbx
   6395b:      	testl	%eax, %eax
   6395d:      	jne	 <L3>
   6395f:      	addq	$0x1, 0xe8(%rbx)
<L0>:
   63967:      	movq	(%rdi), %r12
   6396a:      	testq	%r12, %r12
   6396d:      	je	 <L2>
   6396f:      	leaq	-0x68(%rbp), %rdx
   63973:      	leaq	-0x70(%rbp), %rsi
   63977:      	movq	%r12, %rdi
   6397a:      	callq	 <scoop_heap_object_meta>
   6397f:      	testb	%al, %al
   63981:      	je	 <L4>
   63983:      	movq	-0x70(%rbp), %rax
   63987:      	cmpb	$0x0, 0x98(%rbx)
   6398e:      	je	 <L1>
   63990:      	movl	0x1c(%rax), %edx
   63993:      	testl	%edx, %edx
   63995:      	jne	 <L2>
<L1>:
   63997:      	cmpl	$0x2, 0x18(%rax)
   6399b:      	je	 <L20>
   639a1:      	movq	-0x68(%rbp), %rdx
   639a5:      	movq	0x28(%rax), %rax
   639a9:      	movq	%rdx, %rcx
   639ac:      	shrq	$0x6, %rcx
   639b0:      	leaq	(%rax,%rcx,8), %rax
   639b4:      	movq	(%rax), %rcx
   639b7:      	btq	%rdx, %rcx
   639bb:      	jae	 <L6>
<L2>:
   639bd:      	leaq	-0x28(%rbp), %rsp
   639c1:      	popq	%rbx
   639c2:      	popq	%r12
   639c4:      	popq	%r13
   639c6:      	popq	%r14
   639c8:      	popq	%r15
   639ca:      	popq	%rbp
   639cb:      	retq
   639cc:      	nopl	(%rax)
<L3>:
   639d0:      	cmpl	$0x1, %eax
   639d3:      	je	 <L5>
   639d5:      	addq	$0x1, 0xf8(%rbx)
   639dd:      	jmp	 <L0>
   639df:      	nop
<L4>:
   639e0:      	movq	%r12, %rdi
   639e3:      	callq	 <scoop_gc_stw_is_stable_object>
   639e8:      	testb	%al, %al
   639ea:      	jne	 <L2>
   639ec:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x3fa7>
   639f3:      	callq	 <scoop_heap_fatal>
   639f8:      	nopl	(%rax,%rax)
<L5>:
   63a00:      	addq	$0x1, 0xf0(%rbx)
   63a08:      	jmp	 <L0>
   63a0d:      	nopl	(%rax)
<L6>:
   63a10:      	andl	$0x3f, %edx
   63a13:      	lock
   63a14:      	btsq	%rdx, (%rax)
   63a18:      	setae	%r15b
<L7>:
   63a1c:      	testb	%r15b, %r15b
   63a1f:      	je	 <L2>
   63a21:      	movq	-0x70(%rbp), %rax
   63a25:      	cmpl	$0x2, 0x18(%rax)
   63a29:      	je	 <L19>
   63a2f:      	movq	0x50(%rax), %rax
   63a33:      	movq	-0x68(%rbp), %rdx
   63a37:      	movzwl	(%rax,%rdx,2), %eax
   63a3b:      	leaq	(,%rax,8), %r14
<L8>:
   63a43:      	movq	%r12, %rdi
   63a46:      	movq	%r14, %rsi
   63a49:      	callq	 <scoop_shape_validate_object>
   63a4e:      	movq	-0x70(%rbp), %rdi
   63a52:      	movq	-0x68(%rbp), %rax
   63a56:      	cmpl	$0x1, 0x18(%rdi)
   63a5a:      	je	 <L25>
<L9>:
   63a60:      	movq	0x78(%rdi), %r13
   63a64:      	movq	%rax, %rsi
   63a67:      	shlq	$0x4, %r13
   63a6b:      	addq	0xa0(%rbx), %r13
   63a72:      	addq	%r14, (%r13)
   63a76:      	callq	 <scoop_heap_object_pinned>
   63a7b:      	testb	%al, %al
   63a7d:      	jne	 <L10>
   63a7f:      	addq	%r14, 0x8(%r13)
<L10>:
   63a83:      	movl	$0x1, %eax
   63a88:      	movdqu	0xd8(%rbx), %xmm2
   63a90:      	xorl	%r8d, %r8d
   63a93:      	movq	%rax, %xmm0
   63a98:      	movq	(%r12), %rax
   63a9c:      	punpcklqdq	%xmm0, %xmm0    # xmm0 = xmm0[0,0]
   63aa0:      	movq	0x48(%rax), %r13
   63aa4:      	paddq	%xmm2, %xmm0
   63aa8:      	movups	%xmm0, 0xd8(%rbx)
   63aaf:      	testq	%r13, %r13
   63ab2:      	je	 <L11>
   63ab4:      	xorl	%r8d, %r8d
   63ab7:      	cmpq	$-0x1, (%r13)
   63abc:      	sete	%r8b
   63ac0:      	setne	%r15b
<L11>:
   63ac4:      	movq	0xb8(%rbx), %rax
   63acb:      	testq	%rax, %rax
   63ace:      	je	 <L16>
   63ad4:      	movq	0x8(%rax), %rdx
   63ad8:      	cmpq	$0x400, %rdx            # imm = 0x400
   63adf:      	je	 <L16>
   63ae5:      	leaq	0x1(%rdx), %rcx
   63ae9:      	addq	$0x2, %rdx
   63aed:      	leaq	(,%rdx,8), %rdi
<L12>:
   63af5:      	movq	%rcx, 0x8(%rax)
   63af9:      	testq	%r13, %r13
   63afc:      	movq	0xc8(%rbx), %rcx
   63b03:      	setne	%sil
   63b07:      	movq	%r12, (%rax,%rdx,8)
   63b0b:      	andl	%r15d, %esi
   63b0e:      	testq	%rcx, %rcx
   63b11:      	jne	 <L13>
   63b13:      	addq	%rdi, %rax
   63b16:      	movq	%rax, 0xc0(%rbx)
   63b1d:      	testb	%sil, %sil
   63b20:      	je	 <L24>
   63b26:      	orq	$0x1, 0xd0(%rbx)
   63b2e:      	movq	$0x1, 0xc8(%rbx)
   63b39:      	jmp	 <L2>
   63b3e:      	nop
<L13>:
   63b40:      	leaq	0x1(%rcx), %rdx
   63b44:      	testb	%sil, %sil
   63b47:      	jne	 <L21>
   63b4d:      	movq	%rdx, 0xc8(%rbx)
   63b54:      	cmpq	$0x40, %rdx
   63b58:      	je	 <L30>
<L14>:
   63b5e:      	testl	%r8d, %r8d
   63b61:      	je	 <L2>
   63b67:      	movq	0x8(%r13), %rax
   63b6b:      	movq	(%r12,%rax), %rsi
   63b6f:      	testq	%rsi, %rsi
   63b72:      	je	 <L2>
   63b78:      	movq	%r12, %xmm3
   63b7d:      	movq	0x10(%r13), %r8
   63b81:      	movq	%r13, %xmm4
   63b86:      	punpcklqdq	%xmm4, %xmm3    # xmm3 = xmm3[0],xmm4[0]
   63b8a:      	addq	%r12, %r8
   63b8d:      	movaps	%xmm3, -0x90(%rbp)
   63b94:      	xorl	%r12d, %r12d
   63b97:      	nopw	(%rax,%rax)
<L15>:
   63ba0:      	movq	0x18(%r13), %rax
   63ba4:      	movl	$0x400, %ecx            # imm = 0x400
   63ba9:      	movq	%rsi, -0x80(%rbp)
   63bad:      	movq	%rbx, %rdi
   63bb0:      	movdqa	-0x90(%rbp), %xmm1
   63bb8:      	movq	$0x1, -0x60(%rbp)
   63bc0:      	movq	%rax, %rdx
   63bc3:      	movq	%r8, -0x78(%rbp)
   63bc7:      	imulq	%r12, %rdx
   63bcb:      	movups	%xmm1, -0x58(%rbp)
   63bcf:      	movdqa	-0x60(%rbp), %xmm0
   63bd4:      	addq	%r8, %rdx
   63bd7:      	movq	%rdx, -0x48(%rbp)
   63bdb:      	movq	%rsi, %rdx
   63bde:      	subq	%r12, %rdx
   63be1:      	cmpq	%rcx, %rdx
   63be4:      	cmovaq	%rcx, %rdx
   63be8:      	subq	$0x30, %rsp
   63bec:      	addq	%rdx, %r12
   63bef:      	imulq	%r12, %rax
   63bf3:      	addq	%r8, %rax
   63bf6:      	movq	%rax, -0x40(%rbp)
   63bfa:      	movups	%xmm0, (%rsp)
   63bfe:      	movdqa	-0x50(%rbp), %xmm0
   63c03:      	movq	%rax, 0x20(%rsp)
   63c08:      	movups	%xmm0, 0x10(%rsp)
   63c0d:      	callq	 <scoop_gc_mark_queue_push>
   63c12:      	movq	-0x80(%rbp), %rsi
   63c16:      	addq	$0x30, %rsp
   63c1a:      	movq	-0x78(%rbp), %r8
   63c1e:      	cmpq	%rsi, %r12
   63c21:      	jb	 <L15>
   63c27:      	jmp	 <L2>
   63c2c:      	nopl	(%rax)
<L16>:
   63c30:      	movq	0xd0(%rbx), %rax
   63c37:      	testq	%rax, %rax
   63c3a:      	jne	 <L28>
<L17>:
   63c40:      	pxor	%xmm0, %xmm0
   63c44:      	movl	$0x2010, %edi           # imm = 0x2010
   63c49:      	movl	%r8d, -0x78(%rbp)
   63c4d:      	movq	$0x0, 0xc0(%rbx)
   63c58:      	movups	%xmm0, 0xc8(%rbx)
   63c5f:      	callq	 <malloc@plt>
   63c64:      	movl	-0x78(%rbp), %r8d
   63c68:      	testq	%rax, %rax
   63c6b:      	je	 <L31>
   63c71:      	movq	$0x0, (%rax)
   63c78:      	movq	0xb8(%rbx), %rdx
   63c7f:      	movq	$0x0, 0x8(%rax)
   63c87:      	testq	%rdx, %rdx
   63c8a:      	je	 <L29>
   63c90:      	movq	%rax, (%rdx)
<L18>:
   63c93:      	movl	$0x10, %edi
   63c98:      	movl	$0x1, %ecx
   63c9d:      	movl	$0x2, %edx
   63ca2:      	movq	%rax, 0xb8(%rbx)
   63ca9:      	jmp	 <L12>
   63cae:      	nop
<L19>:
   63cb0:      	movq	0x60(%rax), %r14
   63cb4:      	jmp	 <L8>
   63cb9:      	nopl	(%rax)
<L20>:
   63cc0:      	movl	$0x1, %edx
   63cc5:      	xchgb	%dl, 0x89(%rax)
   63ccb:      	testb	%dl, %dl
   63ccd:      	sete	%r15b
   63cd1:      	jmp	 <L7>
   63cd6:      	nopw	%cs:(%rax,%rax)
<L21>:
   63ce0:      	movl	$0x1, %eax
   63ce5:      	movq	%rdx, 0xc8(%rbx)
   63cec:      	shlq	%cl, %rax
   63cef:      	orq	0xd0(%rbx), %rax
   63cf6:      	movq	%rax, 0xd0(%rbx)
   63cfd:      	cmpq	$0x40, %rdx
   63d01:      	jne	 <L2>
<L22>:
   63d07:      	pxor	%xmm0, %xmm0
   63d0b:      	subq	$0x30, %rsp
   63d0f:      	movl	%r8d, -0x78(%rbp)
   63d13:      	movq	%rbx, %rdi
   63d16:      	movaps	%xmm0, -0x50(%rbp)
   63d1a:      	movaps	%xmm0, -0x60(%rbp)
   63d1e:      	movq	$0x0, -0x40(%rbp)
   63d26:      	movq	0xc0(%rbx), %rdx
   63d2d:      	movq	%rax, -0x50(%rbp)
   63d31:      	movq	%rdx, -0x58(%rbp)
   63d35:      	movdqa	-0x60(%rbp), %xmm0
   63d3a:      	movups	%xmm0, (%rsp)
   63d3e:      	movdqa	-0x50(%rbp), %xmm0
   63d43:      	movq	$0x0, 0x20(%rsp)
   63d4c:      	movups	%xmm0, 0x10(%rsp)
   63d51:      	callq	 <scoop_gc_mark_queue_push>
   63d56:      	movl	-0x78(%rbp), %r8d
   63d5a:      	addq	$0x30, %rsp
<L23>:
   63d5e:      	movq	$0x0, 0xc0(%rbx)
   63d69:      	pxor	%xmm0, %xmm0
   63d6d:      	movups	%xmm0, 0xc8(%rbx)
   63d74:      	jmp	 <L14>
   63d79:      	nopl	(%rax)
<L24>:
   63d80:      	movq	$0x1, 0xc8(%rbx)
   63d8b:      	jmp	 <L14>
<L25>:
   63d90:      	leaq	(,%rax,8), %rdx
   63d98:      	leaq	-0x1(%rdx,%r14), %rcx
   63d9d:      	movq	%rdx, %r8
   63da0:      	movq	%rcx, %rsi
   63da3:      	shrq	$0xd, %r8
   63da7:      	shrq	$0xd, %rsi
   63dab:      	cmpq	%r8, %rsi
   63dae:      	jb	 <L9>
   63db4:      	shrq	$0x7, %rcx
   63db8:      	movq	$-0x1, %r9
   63dbf:      	shrq	$0x7, %rdx
   63dc3:      	movq	%r8, %rax
   63dc6:      	movq	%rcx, %r11
   63dc9:      	movq	%r9, %r10
   63dcc:      	movl	%edx, %ecx
   63dce:      	shlq	%cl, %r10
   63dd1:      	movl	%r11d, %ecx
   63dd4:      	movq	%r9, %r11
   63dd7:      	notl	%ecx
   63dd9:      	shrq	%cl, %r11
   63ddc:      	jmp	 <L27>
   63dde:      	nop
   63ddf:      	nopw	%cs:(%rax,%rax)
   63dea:      	nopw	%cs:(%rax,%rax)
   63df5:      	nopw	%cs:(%rax,%rax)
<L26>:
   63e00:      	addq	$0x1, %rax
<L27>:
   63e04:      	cmpq	%rax, %r8
   63e07:      	movq	%r9, %rdx
   63e0a:      	cmoveq	%r10, %rdx
   63e0e:      	movq	%rdx, %rcx
   63e11:      	andq	%r11, %rdx
   63e14:      	cmpq	%rax, %rsi
   63e17:      	cmovneq	%rcx, %rdx
   63e1b:      	movq	0x48(%rdi), %rcx
   63e1f:      	lock
   63e20:      	orq	%rdx, (%rcx,%rax,8)
   63e24:      	cmpq	%rax, %rsi
   63e27:      	jne	 <L26>
   63e29:      	movq	-0x70(%rbp), %rdi
   63e2d:      	movq	-0x68(%rbp), %rax
   63e31:      	jmp	 <L9>
   63e36:      	nopw	%cs:(%rax,%rax)
<L28>:
   63e40:      	pxor	%xmm0, %xmm0
   63e44:      	subq	$0x30, %rsp
   63e48:      	movl	%r8d, -0x78(%rbp)
   63e4c:      	movq	%rbx, %rdi
   63e4f:      	movaps	%xmm0, -0x50(%rbp)
   63e53:      	movaps	%xmm0, -0x60(%rbp)
   63e57:      	movq	$0x0, -0x40(%rbp)
   63e5f:      	movq	0xc0(%rbx), %rdx
   63e66:      	movq	%rax, -0x50(%rbp)
   63e6a:      	movq	%rdx, -0x58(%rbp)
   63e6e:      	movdqa	-0x60(%rbp), %xmm0
   63e73:      	movups	%xmm0, (%rsp)
   63e77:      	movdqa	-0x50(%rbp), %xmm0
   63e7c:      	movq	$0x0, 0x20(%rsp)
   63e85:      	movups	%xmm0, 0x10(%rsp)
   63e8a:      	callq	 <scoop_gc_mark_queue_push>
   63e8f:      	movl	-0x78(%rbp), %r8d
   63e93:      	addq	$0x30, %rsp
   63e97:      	jmp	 <L17>
   63e9c:      	nopl	(%rax)
<L29>:
   63ea0:      	movq	%rax, 0xb0(%rbx)
   63ea7:      	jmp	 <L18>
   63eac:      	nopl	(%rax)
<L30>:
   63eb0:      	movq	0xd0(%rbx), %rax
   63eb7:      	testq	%rax, %rax
   63eba:      	je	 <L23>
   63ec0:      	jmp	 <L22>
<L31>:
   63ec5:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x3fef>
   63ecc:      	callq	 <scoop_heap_fatal>
   63ed1:      	nopl	(%rax)
   63ed5:      	nopw	%cs:(%rax,%rax)

0000000000063f60 <scoop_gc_mark_run_task>:
   63f60:      	endbr64
   63f64:      	pushq	%rbp
   63f65:      	movq	%rsp, %rbp
   63f68:      	pushq	%r15
   63f6a:      	pushq	%r14
   63f6c:      	pushq	%r13
   63f6e:      	movq	%rdi, %r13
   63f71:      	pushq	%r12
   63f73:      	pushq	%rbx
   63f74:      	subq	$0x18, %rsp
   63f78:      	movq	0x18(%rbp), %r14
   63f7c:      	movq	0x20(%rbp), %r12
   63f80:      	movq	%rdi, -0x40(%rbp)
   63f84:      	addq	$0x1, 0x100(%rdi)
   63f8c:      	movq	$0x2, -0x38(%rbp)
   63f94:      	cmpb	$0x0, 0x10(%rbp)
   63f98:      	jne	 <L3>
   63f9e:      	xorl	%ebx, %ebx
   63fa0:      	testq	%r12, %r12
   63fa3:      	je	 <L1>
   63fa5:      	nopw	%cs:(%rax,%rax)
<L0>:
   63fb0:      	xorl	%eax, %eax
   63fb2:      	movq	$-0x1, %r9
   63fb9:      	xorl	%r8d, %r8d
   63fbc:      	addq	$0x1, %rbx
   63fc0:      	tzcntq	%r12, %rax
   63fc5:      	leaq	-0x40(%rbp), %rcx
   63fc9:      	leaq	, %rdx <scoop_gc_mark_slot>
   63fd0:      	cltq
   63fd2:      	movq	(%r14,%rax,8), %rdi
   63fd6:      	movq	(%rdi), %rax
   63fd9:      	movq	0x48(%rax), %rsi
   63fdd:      	callq	 <scoop_gc_scan_descriptor>
   63fe2:      	leaq	-0x1(%r12), %rax
   63fe7:      	andq	%rax, %r12
   63fea:      	jne	 <L0>
   63fec:      	cmpq	$0x40, %rbx
   63ff0:      	je	 <L2>
   63ff2:      	nopl	(%rax)
   63ff5:      	nopw	%cs:(%rax,%rax)
<L1>:
   64000:      	movq	0xd0(%r13), %rax
   64007:      	testq	%rax, %rax
   6400a:      	je	 <L2>
   6400c:      	xorl	%edx, %edx
   6400e:      	movq	0xc0(%r13), %rcx
   64015:      	xorl	%r8d, %r8d
   64018:      	addq	$0x1, %rbx
   6401c:      	tzcntq	%rax, %rdx
   64021:      	movq	$-0x1, %r9
   64028:      	movslq	%edx, %rdx
   6402b:      	movq	(%rcx,%rdx,8), %rdi
   6402f:      	leaq	-0x1(%rax), %rdx
   64033:      	leaq	-0x40(%rbp), %rcx
   64037:      	andq	%rdx, %rax
   6403a:      	leaq	, %rdx <scoop_gc_mark_slot>
   64041:      	movq	%rax, 0xd0(%r13)
   64048:      	movq	(%rdi), %rax
   6404b:      	movq	0x48(%rax), %rsi
   6404f:      	callq	 <scoop_gc_scan_descriptor>
   64054:      	cmpq	$0x40, %rbx
   64058:      	jne	 <L1>
<L2>:
   6405a:      	addq	$0x18, %rsp
   6405e:      	popq	%rbx
   6405f:      	popq	%r12
   64061:      	popq	%r13
   64063:      	popq	%r14
   64065:      	popq	%r15
   64067:      	popq	%rbp
   64068:      	retq
   64069:      	nopl	(%rax)
<L3>:
   64070:      	movq	0x30(%rbp), %r9
   64074:      	movq	0x28(%rbp), %r8
   64078:      	leaq	-0x40(%rbp), %rcx
   6407c:      	movq	%r12, %rsi
   6407f:      	addq	$0x1, 0x108(%rdi)
   64087:      	leaq	, %rdx <scoop_gc_mark_slot>
   6408e:      	movq	%r14, %rdi
   64091:      	callq	 <scoop_gc_scan_descriptor>
   64096:      	addq	$0x18, %rsp
   6409a:      	popq	%rbx
   6409b:      	popq	%r12
   6409d:      	popq	%r13
   6409f:      	popq	%r14
   640a1:      	popq	%r15
   640a3:      	popq	%rbp
   640a4:      	retq
   640a5:      	nopw	%cs:(%rax,%rax)
   640af:      	nop

000000000006d280 <scoop_gc_mark_queue_push>:
   6d280:      	endbr64
   6d284:      	pushq	%rbp
   6d285:      	movq	%rsp, %rbp
   6d288:      	pushq	%r12
   6d28a:      	movl	$0x1, %r12d
   6d290:      	pushq	%rbx
   6d291:      	movq	%rdi, %rbx
   6d294:      	subq	$0x10, %rsp
   6d298:      	leaq	, %rax <scoop_gc_marker>
   6d29f:      	lock
   6d2a0:      	xaddq	%r12, 0xb8(%rax)
   6d2a8:      	callq	 <pthread_mutex_lock@plt>
   6d2ad:      	movq	0x40(%rbx), %rax
   6d2b1:      	movq	0x30(%rbx), %r8
   6d2b5:      	cmpq	%r8, %rax
   6d2b8:      	je	 <L2>
   6d2ba:      	movq	0x28(%rbx), %r9
   6d2be:      	addq	0x38(%rbx), %rax
<L0>:
   6d2c2:      	xorl	%edx, %edx
   6d2c4:      	movdqu	0x10(%rbp), %xmm0
   6d2c9:      	movq	%rbx, %rdi
   6d2cc:      	divq	%r8
   6d2cf:      	leaq	(%rdx,%rdx,4), %rax
   6d2d3:      	movq	0x30(%rbp), %rdx
   6d2d7:      	movups	%xmm0, (%r9,%rax,8)
   6d2dc:      	movdqu	0x20(%rbp), %xmm0
   6d2e1:      	movq	%rdx, 0x20(%r9,%rax,8)
   6d2e6:      	movups	%xmm0, 0x10(%r9,%rax,8)
   6d2ec:      	addq	$0x1, 0x40(%rbx)
   6d2f1:      	callq	 <pthread_mutex_unlock@plt>
   6d2f6:      	cmpq	$0x1, %r12
   6d2fa:      	jbe	 <L1>
   6d2fc:      	callq	 <scoop_gc_mark_notify_work>
<L1>:
   6d301:      	addq	$0x10, %rsp
   6d305:      	popq	%rbx
   6d306:      	popq	%r12
   6d308:      	popq	%rbp
   6d309:      	retq
   6d30a:      	nopw	(%rax,%rax)
<L2>:
   6d310:      	testq	%r8, %r8
   6d313:      	jne	 <L6>
   6d319:      	movl	$0x500, %edi            # imm = 0x500
   6d31e:      	movl	$0x20, %r8d
<L3>:
   6d324:      	movq	%r8, -0x18(%rbp)
   6d328:      	callq	 <malloc@plt>
   6d32d:      	movq	%rax, %r9
   6d330:      	testq	%rax, %rax
   6d333:      	je	 <L8>
   6d339:      	movq	0x40(%rbx), %r10
   6d33d:      	movq	0x28(%rbx), %r11
   6d341:      	movq	-0x18(%rbp), %r8
   6d345:      	testq	%r10, %r10
   6d348:      	je	 <L5>
   6d34a:      	movq	0x38(%rbx), %rcx
   6d34e:      	movq	0x30(%rbx), %rdi
   6d352:      	movq	%rax, %rsi
   6d355:      	addq	%rcx, %r10
   6d358:      	nopl	(%rax)
   6d35f:      	nopw	%cs:(%rax,%rax)
   6d36a:      	nopw	%cs:(%rax,%rax)
   6d375:      	nopw	%cs:(%rax,%rax)
<L4>:
   6d380:      	movq	%rcx, %rax
   6d383:      	xorl	%edx, %edx
   6d385:      	addq	$0x1, %rcx
   6d389:      	addq	$0x28, %rsi
   6d38d:      	divq	%rdi
   6d390:      	leaq	(%rdx,%rdx,4), %rax
   6d394:      	leaq	(%r11,%rax,8), %rax
   6d398:      	movdqu	(%rax), %xmm0
   6d39c:      	movups	%xmm0, -0x28(%rsi)
   6d3a0:      	movdqu	0x10(%rax), %xmm0
   6d3a5:      	movups	%xmm0, -0x18(%rsi)
   6d3a9:      	movq	0x20(%rax), %rax
   6d3ad:      	movq	%rax, -0x8(%rsi)
   6d3b1:      	cmpq	%rcx, %r10
   6d3b4:      	jne	 <L4>
<L5>:
   6d3b6:      	movq	%r11, %rdi
   6d3b9:      	movq	%r9, -0x20(%rbp)
   6d3bd:      	movq	%r8, -0x18(%rbp)
   6d3c1:      	callq	 <free@plt>
   6d3c6:      	movq	-0x20(%rbp), %r9
   6d3ca:      	movq	-0x18(%rbp), %r8
   6d3ce:      	movq	$0x0, 0x38(%rbx)
   6d3d6:      	movq	0x40(%rbx), %rax
   6d3da:      	movq	%r9, 0x28(%rbx)
   6d3de:      	movq	%r8, 0x30(%rbx)
   6d3e2:      	jmp	 <L0>
   6d3e7:      	nopw	(%rax,%rax)
<L6>:
   6d3f0:      	leaq	(%r8,%r8), %rax
   6d3f4:      	cmpq	%r8, %rax
   6d3f7:      	jb	 <L7>
   6d3f9:      	movabsq	$0x666666666666666, %rdx # imm = 0x666666666666666
   6d403:      	cmpq	%rax, %rdx
   6d406:      	jb	 <L7>
   6d408:      	leaq	(%r8,%r8,4), %rdi
   6d40c:      	movq	%rax, %r8
   6d40f:      	shlq	$0x4, %rdi
   6d413:      	jmp	 <L3>
<L7>:
   6d418:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x75c>
   6d41f:      	callq	 <scoop_heap_fatal>
<L8>:
   6d424:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x58ef>
   6d42b:      	callq	 <scoop_heap_fatal>

000000000006d430 <scoop_gc_mark_queue_take>:
   6d430:      	endbr64
   6d434:      	pushq	%rbp
   6d435:      	movq	%rsi, %rdx
   6d438:      	movq	%rsp, %rbp
   6d43b:      	pushq	%r15
   6d43d:      	pushq	%r14
   6d43f:      	pushq	%r13
   6d441:      	movq	%rsi, %r13
   6d444:      	xorl	%esi, %esi
   6d446:      	pushq	%r12
   6d448:      	movq	%rdi, %r12
   6d44b:      	pushq	%rbx
   6d44c:      	subq	$0x8, %rsp
   6d450:      	callq	 <take>
   6d455:      	testb	%al, %al
   6d457:      	jne	 <L2>
   6d459:      	leaq	, %rbx <scoop_gc_marker>
   6d460:      	movl	%eax, %r14d
   6d463:      	movl	$0x1, %r15d
   6d469:      	movq	0xc8(%rbx), %rcx
   6d470:      	cmpq	$0x1, %rcx
   6d474:      	ja	 <L1>
   6d476:      	jmp	 <L3>
   6d478:      	nopl	(%rax,%rax)
<L0>:
   6d480:      	movq	0xc8(%rbx), %rcx
   6d487:      	addq	$0x1, %r15
   6d48b:      	cmpq	%rcx, %r15
   6d48e:      	jae	 <L3>
<L1>:
   6d490:      	movq	0x80(%r12), %rax
   6d498:      	xorl	%edx, %edx
   6d49a:      	movl	$0x1, %esi
   6d49f:      	addq	%r15, %rax
   6d4a2:      	divq	%rcx
   6d4a5:      	leaq	(%rdx,%rdx,2), %rax
   6d4a9:      	movq	%r13, %rdx
   6d4ac:      	shlq	$0x7, %rax
   6d4b0:      	leaq	0x100(%rax,%rbx), %rdi
   6d4b8:      	callq	 <take>
   6d4bd:      	testb	%al, %al
   6d4bf:      	je	 <L0>
   6d4c1:      	addq	$0x1, 0x110(%r12)
<L2>:
   6d4ca:      	movl	$0x1, %r14d
<L3>:
   6d4d0:      	addq	$0x8, %rsp
   6d4d4:      	movl	%r14d, %eax
   6d4d7:      	popq	%rbx
   6d4d8:      	popq	%r12
   6d4da:      	popq	%r13
   6d4dc:      	popq	%r14
   6d4de:      	popq	%r15
   6d4e0:      	popq	%rbp
   6d4e1:      	retq
   6d4e2:      	nopl	(%rax)
   6d4e5:      	nopw	%cs:(%rax,%rax)

000000000006d4f0 <scoop_gc_mark_task_done>:
   6d4f0:      	endbr64
   6d4f4:      	pushq	%rbp
   6d4f5:      	leaq	, %rax <scoop_gc_marker>
   6d4fc:      	movq	$-0x1, %rdx
   6d503:      	movq	%rsp, %rbp
   6d506:      	lock
   6d507:      	xaddq	%rdx, 0xb8(%rax)
   6d50f:      	testq	%rdx, %rdx
   6d512:      	je	 <L1>
   6d514:      	cmpq	$0x1, %rdx
   6d518:      	je	 <L0>
   6d51a:      	popq	%rbp
   6d51b:      	retq
   6d51c:      	nopl	(%rax)
<L0>:
   6d520:      	callq	 <scoop_gc_mark_notify_work>
   6d525:      	popq	%rbp
   6d526:      	retq
<L1>:
   6d527:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x5917>
   6d52e:      	callq	 <scoop_heap_fatal>
   6d533:      	nopw	%cs:(%rax,%rax)
   6d53d:      	nopl	(%rax)

0000000000071500 <run_worker>:
   71500:      	pushq	%rbp
   71501:      	movq	%rsp, %rbp
   71504:      	pushq	%r13
   71506:      	pushq	%r12
   71508:      	pushq	%rbx
   71509:      	movq	%rdi, %rbx
   7150c:      	subq	$0x38, %rsp
   71510:      	callq	 <scoop_gc_thread_cpu_ns>
   71515:      	movq	%rax, %r13
   71518:      	nopl	(%rax,%rax)
<L0>:
   71520:      	leaq	-0x50(%rbp), %rsi
   71524:      	movq	%rbx, %rdi
   71527:      	callq	 <scoop_gc_mark_queue_take>
   7152c:      	testb	%al, %al
   7152e:      	jne	 <L4>
   71530:      	leaq	, %rdi <scoop_gc_marker>
   71537:      	callq	 <pthread_mutex_lock@plt>
   7153c:      	jmp	 <L2>
   7153e:      	nop
<L1>:
   71540:      	leaq	-0x50(%rbp), %rsi
   71544:      	movq	%rbx, %rdi
   71547:      	callq	 <scoop_gc_mark_queue_take>
   7154c:      	testb	%al, %al
   7154e:      	jne	 <L3>
   71550:      	leaq	, %rsi <scoop_gc_marker>
   71557:      	leaq	0x58(%rsi), %rdi
   7155b:      	callq	 <pthread_cond_wait@plt>
<L2>:
   71560:      	movq	, %rax <scoop_gc_marker+0xb8>
   71567:      	testq	%rax, %rax
   7156a:      	jne	 <L1>
   7156c:      	leaq	, %rdi <scoop_gc_marker>
   71573:      	callq	 <pthread_mutex_unlock@plt>
   71578:      	callq	 <scoop_gc_thread_cpu_ns>
   7157d:      	movq	%rax, %rdx
   71580:      	movq	0x118(%rbx), %rax
   71587:      	subq	%r13, %rax
   7158a:      	addq	%rdx, %rax
   7158d:      	movq	%rax, 0x118(%rbx)
   71594:      	leaq	-0x18(%rbp), %rsp
   71598:      	popq	%rbx
   71599:      	popq	%r12
   7159b:      	popq	%r13
   7159d:      	popq	%rbp
   7159e:      	retq
   7159f:      	nop
<L3>:
   715a0:      	leaq	, %rdi <scoop_gc_marker>
   715a7:      	callq	 <pthread_mutex_unlock@plt>
<L4>:
   715ac:      	movdqa	-0x50(%rbp), %xmm0
   715b1:      	subq	$0x30, %rsp
   715b5:      	movq	%rbx, %rdi
   715b8:      	movups	%xmm0, (%rsp)
   715bc:      	movdqa	-0x40(%rbp), %xmm0
   715c1:      	movups	%xmm0, 0x10(%rsp)
   715c6:      	movq	-0x30(%rbp), %rax
   715ca:      	movq	%rax, 0x20(%rsp)
   715cf:      	callq	 <scoop_gc_mark_run_task>
   715d4:      	addq	$0x30, %rsp
   715d8:      	movq	%rbx, %rdi
   715db:      	callq	 <scoop_gc_mark_flush>
   715e0:      	callq	 <scoop_gc_mark_task_done>
   715e5:      	jmp	 <L0>
   715ea:      	nopw	(%rax,%rax)

00000000000717a0 <scoop_gc_mark_pool_prepare>:
   717a0:      	endbr64
   717a4:      	pushq	%rbp
   717a5:      	movq	%rsp, %rbp
   717a8:      	pushq	%r14
   717aa:      	movl	%edi, %r14d
   717ad:      	pushq	%r13
   717af:      	pushq	%r12
   717b1:      	pushq	%rbx
   717b2:      	subq	$0x10, %rsp
   717b6:      	cmpb	$0x0,  <scoop_gc_marker+0xe8>
   717bd:      	je	 <L7>
   717c3:      	movq	, %r13 <scoop_gc_marker+0xc0>
<L0>:
   717ca:      	cmpb	$0x0,  <scoop_gc_marker+0xe9>
   717d1:      	jne	 <L5>
   717d3:      	testb	%r14b, %r14b
   717d6:      	je	 <L4>
<L1>:
   717d8:      	leaq	, %r12 <scoop_gc_marker>
<L2>:
   717df:      	movl	$0x1, %r13d
<L3>:
   717e5:      	movq	%r12, %rdi
   717e8:      	callq	 <pthread_mutex_lock@plt>
   717ed:      	movq	%r12, %rdi
   717f0:      	movq	%r13,  <scoop_gc_marker+0xc8>
   717f7:      	callq	 <pthread_mutex_unlock@plt>
   717fc:      	addq	$0x10, %rsp
   71800:      	popq	%rbx
   71801:      	popq	%r12
   71803:      	popq	%r13
   71805:      	popq	%r14
   71807:      	popq	%rbp
   71808:      	retq
   71809:      	nopl	(%rax)
<L4>:
   71810:      	leaq	, %rax <scoop_gc_heap_state>
   71817:      	cmpq	$0x3fffff, 0x48(%rax)   # imm = 0x3FFFFF
   7181f:      	jbe	 <L1>
   71821:      	nopl	(%rax)
   71825:      	nopw	%cs:(%rax,%rax)
<L5>:
   71830:      	leaq	, %r12 <scoop_gc_marker>
   71837:      	cmpq	$0x1, %r13
   7183b:      	jbe	 <L3>
   7183d:      	cmpq	$0x0,  <scoop_gc_marker+0xd0>
   71845:      	jne	 <L3>
   71847:      	leaq	0x308(%r12), %rbx
   7184f:      	movl	$0x1, %r14d
   71855:      	nopw	%cs:(%rax,%rax)
<L6>:
   71860:      	movq	0xe0(%r12), %rax
   71868:      	xorl	%esi, %esi
   7186a:      	movq	%rbx, %rdi
   7186d:      	leaq	-0x88(%rbx), %rcx
   71874:      	leaq	, %rdx <worker_main>
   7187b:      	movq	%rax, 0x8(%rbx)
   7187f:      	callq	 <pthread_create@plt>
   71884:      	testl	%eax, %eax
   71886:      	jne	 <L11>
   7188c:      	addq	$0x1, %r14
   71890:      	addq	$0x180, %rbx            # imm = 0x180
   71897:      	addq	$0x1, 0xd0(%r12)
   718a0:      	cmpq	%r13, %r14
   718a3:      	jne	 <L6>
   718a5:      	jmp	 <L3>
   718aa:      	nopw	(%rax,%rax)
<L7>:
   718b0:      	callq	 <scoop_platform_bundle>
   718b5:      	movq	0x8(%rax), %rax
   718b9:      	callq	*0x18(%rax)
   718bc:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x9fe>
   718c3:      	movq	%rax, %r13
   718c6:      	callq	 <getenv@plt>
   718cb:      	movq	%rax, %rbx
   718ce:      	testq	%rax, %rax
   718d1:      	je	 <L10>
   718d7:      	leaq	-0x28(%rbp), %rsi
   718db:      	movl	$0xa, %edx
   718e0:      	movq	%rax, %rdi
   718e3:      	movq	$0x0, -0x28(%rbp)
   718eb:      	callq	 <strtoul@plt>
   718f0:      	movq	%rax, %r13
   718f3:      	movq	-0x28(%rbp), %rax
   718f7:      	cmpq	%rax, %rbx
   718fa:      	je	 <L13>
   71900:      	cmpb	$0x0, (%rax)
   71903:      	jne	 <L13>
   71909:      	leaq	-0x1(%r13), %rax
   7190d:      	cmpq	$0x7, %rax
   71911:      	ja	 <L13>
   71917:      	movb	$0x1,  <scoop_gc_marker+0xe9>
<L8>:
   7191e:      	leaq	, %r12 <scoop_gc_marker+0x100>
   71925:      	xorl	%ebx, %ebx
   71927:      	nopw	(%rax,%rax)
<L9>:
   71930:      	movq	%rbx, 0x80(%r12)
   71938:      	xorl	%esi, %esi
   7193a:      	movq	%r12, %rdi
   7193d:      	callq	 <pthread_mutex_init@plt>
   71942:      	testl	%eax, %eax
   71944:      	jne	 <L12>
   71946:      	addq	$0x1, %rbx
   7194a:      	addq	$0x180, %r12            # imm = 0x180
   71951:      	cmpq	$0x8, %rbx
   71955:      	jne	 <L9>
   71957:      	movq	%r13,  <scoop_gc_marker+0xc0>
   7195e:      	movb	$0x1,  <scoop_gc_marker+0xe8>
   71965:      	jmp	 <L0>
   7196a:      	nopw	(%rax,%rax)
<L10>:
   71970:      	movl	$0x4, %eax
   71975:      	cmpq	%rax, %r13
   71978:      	cmovaq	%rax, %r13
   7197c:      	jmp	 <L8>
   7197e:      	nop
<L11>:
   71980:      	callq	 <stop_threads>
   71985:      	leaq	, %rax <scoop_gc_heap_state>
   7198c:      	movq	$0x1,  <scoop_gc_marker+0xc0>
   71997:      	addq	$0x1, 0x198(%rax)
   7199f:      	jmp	 <L2>
<L12>:
   719a4:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x65bf>
   719ab:      	callq	 <scoop_heap_fatal>
<L13>:
   719b0:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x658f>
   719b7:      	callq	 <scoop_heap_fatal>
   719bc:      	nopl	(%rax)

00000000000770b0 <scoop_gc_heap_plan_moving_locked>:
   770b0:      	endbr64
   770b4:      	pushq	%rbp
   770b5:      	movq	%rsp, %rbp
   770b8:      	pushq	%r15
   770ba:      	pushq	%r14
   770bc:      	pushq	%r13
   770be:      	pushq	%r12
   770c0:      	pushq	%rbx
   770c1:      	subq	$0x38, %rsp
   770c5:      	leaq	, %rbx <scoop_gc_heap_state>
   770cc:      	movzbl	0x85(%rbx), %eax
   770d3:      	movb	%al, -0x51(%rbp)
   770d6:      	testb	%al, %al
   770d8:      	je	 <L20>
   770de:      	movl	%edi, %r12d
   770e1:      	movzbl	%dil, %edi
   770e5:      	callq	 <scoop_heap_select_evacuation_sources>
   770ea:      	movq	%rax, %r13
   770ed:      	testb	%r12b, %r12b
   770f0:      	je	 <L5>
<L0>:
   770f6:      	pxor	%xmm0, %xmm0
   770fa:      	movl	$0x101, %eax            # imm = 0x101
   770ff:      	leaq	-0x50(%rbp), %rsi
   77103:      	leaq	, %rdi <append_move>
   7710a:      	movaps	%xmm0, -0x40(%rbp)
   7710e:      	movaps	%xmm0, -0x50(%rbp)
   77112:      	movw	%ax, -0x37(%rbp)
   77116:      	callq	 <scoop_gc_mark_visit_live>
   7711b:      	cmpb	$0x0, -0x36(%rbp)
   7711f:      	je	 <L11>
<L1>:
   77125:      	callq	 <scoop_gc_monotonic_ns>
   7712a:      	xorl	%r13d, %r13d
   7712d:      	xorl	%r12d, %r12d
   77130:      	movq	%rax, -0x60(%rbp)
   77134:      	cmpq	$0x0, -0x48(%rbp)
   77139:      	jne	 <L4>
   7713b:      	jmp	 <L9>
<L2>:
   77140:      	movq	0x8(%r15), %rsi
   77144:      	movl	$0x1, %ecx
   77149:      	callq	 <scoop_heap_record_small_object>
   7714e:      	movq	0x8(%r15), %rcx
   77152:      	movq	0x18(%r15), %rdx
   77156:      	movq	0x58(%r14), %rax
   7715a:      	movq	%rcx, (%rax,%rdx,8)
<L3>:
   7715e:      	addq	$0x1, 0x2e8(%rbx)
   77166:      	addq	$0x1, %r12
   7716a:      	addq	$0x30, %r13
   7716e:      	cmpq	-0x48(%rbp), %r12
   77172:      	jae	 <L9>
<L4>:
   77178:      	movq	-0x50(%rbp), %r15
   7717c:      	addq	%r13, %r15
   7717f:      	movq	0x8(%r15), %rdi
   77183:      	movq	0x10(%r15), %rdx
   77187:      	movq	(%r15), %rsi
   7718a:      	callq	 <memcpy@plt>
   7718f:      	movq	0x10(%r15), %rax
   77193:      	addq	%rax, 0x268(%rbx)
   7719a:      	movq	0x10(%r15), %rdx
   7719e:      	movq	0x20(%r15), %r14
   771a2:      	movq	0x28(%r15), %rdi
   771a6:      	cmpq	$0x7f80, %rdx           # imm = 0x7F80
   771ad:      	jbe	 <L2>
   771af:      	movl	$0x1, %esi
   771b4:      	callq	 <scoop_heap_publish_large_object>
   771b9:      	movq	0x8(%r15), %rdx
   771bd:      	movq	%rdx, 0x80(%r14)
   771c4:      	jmp	 <L3>
   771c6:      	nopw	%cs:(%rax,%rax)
<L5>:
   771d0:      	cmpb	$0x0, 0x81(%rbx)
   771d7:      	jne	 <L0>
   771dd:      	pxor	%xmm0, %xmm0
   771e1:      	movups	%xmm0, 0x250(%rbx)
   771e8:      	testq	%rax, %rax
   771eb:      	je	 <L10>
   771f1:      	callq	 <scoop_heap_prepare_evacuation_targets>
   771f6:      	pxor	%xmm0, %xmm0
   771fa:      	cmpq	$0x1, %r13
   771fe:      	leaq	-0x50(%rbp), %rsi
   77202:      	leaq	, %rdi <append_move>
   77209:      	movaps	%xmm0, -0x40(%rbp)
   7720d:      	movaps	%xmm0, -0x50(%rbp)
   77211:      	movb	$0x1, -0x38(%rbp)
   77215:      	movb	$0x1, -0x36(%rbp)
   77219:      	setne	-0x37(%rbp)
   7721d:      	callq	 <scoop_gc_mark_visit_live>
   77222:      	movq	-0x48(%rbp), %rcx
   77226:      	testq	%rcx, %rcx
   77229:      	je	 <L19>
   7722f:      	movq	-0x50(%rbp), %rsi
   77233:      	leaq	(%rcx,%rcx,2), %rdx
   77237:      	xorl	%ecx, %ecx
   77239:      	shlq	$0x4, %rdx
   7723d:      	leaq	0x28(%rsi), %rax
   77241:      	leaq	0x28(%rsi,%rdx), %rdi
   77246:      	xorl	%esi, %esi
   77248:      	nop
   77249:      	nopw	%cs:(%rax,%rax)
   77254:      	nopw	%cs:(%rax,%rax)
   7725f:      	nopw	%cs:(%rax,%rax)
   7726a:      	nopw	%cs:(%rax,%rax)
   77275:      	nopw	%cs:(%rax,%rax)
<L6>:
   77280:      	movq	(%rax), %rdx
   77283:      	movq	(%rdx), %rdx
   77286:      	cmpb	$0x0, 0x2c(%rdx)
   7728a:      	jne	 <L7>
   7728c:      	movb	$0x1, 0x2c(%rdx)
   77290:      	addq	$0x1, %rcx
   77294:      	cmpq	$0x1, 0x30(%rdx)
   77299:      	adcq	$0x0, %rsi
<L7>:
   7729d:      	addq	$0x30, %rax
   772a1:      	cmpq	%rax, %rdi
   772a4:      	jne	 <L6>
<L8>:
   772a6:      	cmpq	%r13, %rsi
   772a9:      	setb	%al
   772ac:      	andb	%al, -0x36(%rbp)
   772af:      	je	 <L11>
   772b1:      	movq	%r13, 0x250(%rbx)
   772b8:      	movq	%rcx, 0x258(%rbx)
   772bf:      	jmp	 <L1>
   772c4:      	nopl	(%rax)
<L9>:
   772c8:      	callq	 <scoop_gc_monotonic_ns>
   772cd:      	movq	-0x50(%rbp), %rdi
   772d1:      	movq	%rax, %rdx
   772d4:      	movq	0x168(%rbx), %rax
   772db:      	subq	-0x60(%rbp), %rax
   772df:      	addq	%rdx, %rax
   772e2:      	movq	%rax, 0x168(%rbx)
   772e9:      	callq	 <free@plt>
<L10>:
   772ee:      	movzbl	-0x51(%rbp), %eax
   772f2:      	addq	$0x38, %rsp
   772f6:      	popq	%rbx
   772f7:      	popq	%r12
   772f9:      	popq	%r13
   772fb:      	popq	%r14
   772fd:      	popq	%r15
   772ff:      	popq	%rbp
   77300:      	retq
   77301:      	nopl	(%rax)
<L11>:
   77308:      	movq	-0x50(%rbp), %rdi
   7730c:      	callq	 <free@plt>
   77311:      	callq	 <scoop_heap_first_block>
   77316:      	movq	%rax, %r12
   77319:      	testq	%rax, %rax
   7731c:      	jne	 <L17>
   7731e:      	nop
<L12>:
   77320:      	movq	0x28(%rbx), %rax
   77324:      	testq	%rax, %rax
   77327:      	je	 <L14>
   77329:      	nopl	(%rax)
<L13>:
   77330:      	xorl	%edx, %edx
   77332:      	movw	%dx, 0x2b(%rax)
   77336:      	movq	0x18(%rax), %rax
   7733a:      	testq	%rax, %rax
   7733d:      	jne	 <L13>
<L14>:
   7733f:      	movzbl	0x81(%rbx), %eax
   77346:      	pxor	%xmm0, %xmm0
   7734a:      	movq	$0x0, 0x2e0(%rbx)
   77355:      	movups	%xmm0, 0x2d0(%rbx)
   7735c:      	movb	%al, -0x51(%rbp)
   7735f:      	testb	%al, %al
   77361:      	je	 <L10>
   77363:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x6eb7>
   7736a:      	callq	 <scoop_heap_fatal>
   7736f:      	nop
<L15>:
   77370:      	cmpl	$0x3, %eax
   77373:      	je	 <L18>
<L16>:
   77375:      	movq	%r12, %rdi
   77378:      	callq	 <scoop_heap_next_block>
   7737d:      	movq	%rax, %r12
   77380:      	testq	%rax, %rax
   77383:      	je	 <L12>
<L17>:
   77385:      	movl	0x14(%r12), %eax
   7738a:      	cmpl	$0x4, %eax
   7738d:      	jne	 <L15>
   7738f:      	movq	%r12, %rdi
   77392:      	callq	 <scoop_heap_release_block>
   77397:      	jmp	 <L16>
   77399:      	nopl	(%rax)
<L18>:
   773a0:      	movq	%r12, %rdi
   773a3:      	callq	 <scoop_heap_block_has_pins>
   773a8:      	movq	0x58(%r12), %rdi
   773ad:      	cmpb	$0x1, %al
   773af:      	sbbl	%eax, %eax
   773b1:      	andl	$-0x3, %eax
   773b4:      	addl	$0x5, %eax
   773b7:      	movl	%eax, 0x14(%r12)
   773bc:      	callq	 <free@plt>
   773c1:      	movq	$0x0, 0x58(%r12)
   773ca:      	jmp	 <L16>
<L19>:
   773cc:      	xorl	%esi, %esi
   773ce:      	jmp	 <L8>
<L20>:
   773d3:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x6e8f>
   773da:      	callq	 <scoop_heap_fatal>
   773df:      	nop

0000000000078350 <scoop_gc_mark_visit_live>:
   78350:      	endbr64
   78354:      	leaq	, %rax <scoop_gc_marker>
   7835b:      	cmpq	$0x0, 0xc8(%rax)
   78363:      	je	 <L5>
   78369:      	pushq	%rbp
   7836a:      	movq	%rsp, %rbp
   7836d:      	pushq	%r15
   7836f:      	pushq	%r14
   78371:      	movq	%rax, %r14
   78374:      	pushq	%r13
   78376:      	pushq	%r12
   78378:      	movq	%rdi, %r12
   7837b:      	pushq	%rbx
   7837c:      	movq	%rsi, %rbx
   7837f:      	subq	$0x18, %rsp
   78383:      	movq	$0x0, -0x38(%rbp)
   7838b:      	nopl	(%rax,%rax)
<L0>:
   78390:      	movq	0x1b0(%r14), %r15
   78397:      	testq	%r15, %r15
   7839a:      	je	 <L4>
   7839c:      	nopl	(%rax)
<L1>:
   783a0:      	xorl	%r13d, %r13d
   783a3:      	cmpq	$0x0, 0x8(%r15)
   783a8:      	je	 <L3>
   783aa:      	nopw	(%rax,%rax)
<L2>:
   783b0:      	movq	0x10(%r15,%r13,8), %rdi
   783b5:      	movq	%rbx, %rsi
   783b8:      	addq	$0x1, %r13
   783bc:      	callq	*%r12
   783bf:      	cmpq	0x8(%r15), %r13
   783c3:      	jb	 <L2>
<L3>:
   783c5:      	movq	(%r15), %r15
   783c8:      	testq	%r15, %r15
   783cb:      	jne	 <L1>
<L4>:
   783cd:      	leaq	, %rdx <scoop_gc_marker>
   783d4:      	addq	$0x1, -0x38(%rbp)
   783d9:      	addq	$0x180, %r14            # imm = 0x180
   783e0:      	movq	-0x38(%rbp), %rax
   783e4:      	cmpq	0xc8(%rdx), %rax
   783eb:      	jb	 <L0>
   783ed:      	addq	$0x18, %rsp
   783f1:      	popq	%rbx
   783f2:      	popq	%r12
   783f4:      	popq	%r13
   783f6:      	popq	%r14
   783f8:      	popq	%r15
   783fa:      	popq	%rbp
   783fb:      	retq
<L5>:
   783fc:      	retq
   783fd:      	nopl	(%rax)

0000000000079830 <collect>:
   79830:      	pushq	%rbp
   79831:      	movq	%rsp, %rbp
   79834:      	pushq	%r15
   79836:      	pushq	%r14
   79838:      	pushq	%r13
   7983a:      	movl	%edi, %r13d
   7983d:      	pushq	%r12
   7983f:      	pushq	%rbx
   79840:      	subq	$0x68, %rsp
   79844:      	callq	 <scoop_gc_monotonic_ns>
   79849:      	movq	%rax, %r12
   7984c:      	callq	 <scoop_thread_begin_collection>
   79851:      	movl	%eax, %ebx
   79853:      	testb	%al, %al
   79855:      	jne	 <L1>
<L0>:
   79857:      	leaq	-0x28(%rbp), %rsp
   7985b:      	movl	%ebx, %eax
   7985d:      	popq	%rbx
   7985e:      	popq	%r12
   79860:      	popq	%r13
   79862:      	popq	%r14
   79864:      	popq	%r15
   79866:      	popq	%rbp
   79867:      	retq
   79868:      	nopl	(%rax,%rax)
<L1>:
   79870:      	movq	, %xmm1 <scoop_linux_thread_vm_ops+0x40>
   79878:      	leaq	, %rax <visit_external_root>
   7987f:      	movq	%rax, %xmm3
   79884:      	punpcklqdq	%xmm3, %xmm1    # xmm1 = xmm1[0],xmm3[0]
   79888:      	movaps	%xmm1, -0x80(%rbp)
   7988c:      	callq	 <scoop_gc_monotonic_ns>
   79891:      	movq	%rax, %rbx
   79894:      	movq	%rax, -0x88(%rbp)
   7989b:      	callq	 <scoop_gc_heap_lock>
   798a0:      	callq	 <scoop_gc_roots_lock>
   798a5:      	leaq	, %r15 <scoop_gc_heap_state>
   798ac:      	movl	$0x1, %edi
   798b1:      	movq	0x140(%r15), %rax
   798b8:      	subq	%r12, %rax
   798bb:      	addq	%rbx, %rax
   798be:      	movq	%rax, 0x140(%r15)
   798c5:      	callq	 <scoop_gc_set_pin_frames_locked>
   798ca:      	callq	 <scoop_thread_collection_registry_head>
   798cf:      	testq	%rax, %rax
   798d2:      	je	 <L5>
   798d8:      	pxor	%xmm0, %xmm0
   798dc:      	nopl	(%rax)
<L2>:
   798e0:      	movq	$0x0, 0xd0(%rax)
   798eb:      	movaps	%xmm0, 0xc0(%rax)
   798f2:      	movq	0xe0(%rax), %rax
   798f9:      	testq	%rax, %rax
   798fc:      	jne	 <L2>
   798fe:      	jmp	 <L5>
   79903:      	nopl	(%rax,%rax)
<L3>:
   79908:      	callq	 <scoop_gc_monotonic_ns>
   7990d:      	movq	%rax, %rbx
   79910:      	callq	 <scoop_gc_mark_remembered>
   79915:      	callq	 <scoop_gc_monotonic_ns>
   7991a:      	subq	%rbx, %rax
   7991d:      	addq	%rax, 0x150(%r15)
<L4>:
   79924:      	callq	 <scoop_gc_monotonic_ns>
   79929:      	movl	%r12d, %edi
   7992c:      	movq	%rax, %rbx
   7992f:      	callq	 <scoop_gc_mark_finish>
   79934:      	movq	%rax, -0x70(%rbp)
   79938:      	callq	 <scoop_gc_monotonic_ns>
   7993d:      	subq	%rbx, %rax
   79940:      	addq	%rax, 0x158(%r15)
   79947:      	callq	 <scoop_gc_monotonic_ns>
   7994c:      	movq	0x168(%r15), %rdx
   79953:      	movl	%r12d, %edi
   79956:      	movq	%rax, %r14
   79959:      	movq	%rdx, -0x68(%rbp)
   7995d:      	callq	 <scoop_gc_heap_plan_moving_locked>
   79962:      	movl	%eax, %ebx
   79964:      	callq	 <scoop_gc_monotonic_ns>
   79969:      	movq	-0x68(%rbp), %rdx
   7996d:      	addq	0x160(%r15), %rdx
   79974:      	subq	0x168(%r15), %rdx
   7997b:      	addq	%rax, %rdx
   7997e:      	subq	%r14, %rdx
   79981:      	movl	%r13d, %r14d
   79984:      	xorl	$0x1, %r14d
   79988:      	movq	%rdx, 0x160(%r15)
   7998f:      	orb	%r14b, %bl
   79992:      	jne	 <L6>
   79994:      	callq	 <scoop_gc_mark_dispose>
   79999:      	addq	$0x1, 0xa0(%r15)
   799a1:      	xorl	%r13d, %r13d
   799a4:      	movb	$0x0, 0x85(%r15)
<L5>:
   799ac:      	movzbl	%r13b, %r12d
   799b0:      	movl	%r12d, %edi
   799b3:      	callq	 <scoop_gc_heap_begin_collection_locked>
   799b8:      	movl	%r12d, %edi
   799bb:      	movq	%rax, %rsi
   799be:      	callq	 <scoop_gc_mark_begin>
   799c3:      	callq	 <scoop_gc_monotonic_ns>
   799c8:      	movq	%rax, %rbx
   799cb:      	callq	 <scoop_gc_mark_roots>
   799d0:      	callq	 <scoop_gc_monotonic_ns>
   799d5:      	subq	%rbx, %rax
   799d8:      	addq	%rax, 0x148(%r15)
   799df:      	testb	%r13b, %r13b
   799e2:      	je	 <L4>
   799e8:      	jmp	 <L3>
   799ed:      	nopl	(%rax)
<L6>:
   799f0:      	callq	 <scoop_gc_monotonic_ns>
   799f5:      	movdqa	-0x80(%rbp), %xmm2
   799fa:      	subq	$0x20, %rsp
   799fe:      	leaq	-0x54(%rbp), %rsi
   79a02:      	movq	%rax, -0x68(%rbp)
   79a06:      	leaq	, %rax <visit_root_region>
   79a0d:      	movq	%rsi, -0x38(%rbp)
   79a11:      	movq	%rax, -0x40(%rbp)
   79a15:      	movq	%rsi, -0x90(%rbp)
   79a1c:      	movb	$0x0, -0x53(%rbp)
   79a20:      	movb	%r13b, -0x54(%rbp)
   79a24:      	movaps	%xmm2, -0x50(%rbp)
   79a28:      	movups	%xmm2, (%rsp)
   79a2c:      	movdqa	-0x40(%rbp), %xmm0
   79a31:      	movups	%xmm0, 0x10(%rsp)
   79a36:      	callq	 <scoop_gc_scan_roots>
   79a3b:      	addq	$0x20, %rsp
   79a3f:      	testb	%r13b, %r13b
   79a42:      	movq	-0x90(%rbp), %rsi
   79a49:      	jne	 <L15>
<L7>:
   79a4f:      	leaq	, %rdi <relocate_live_object>
   79a56:      	callq	 <scoop_gc_mark_visit_live>
   79a5b:      	callq	 <scoop_gc_monotonic_ns>
   79a60:      	movzbl	%r14b, %r8d
   79a64:      	movdqu	0x90(%r15), %xmm6
   79a6d:      	movq	%rax, %rdx
   79a70:      	movq	0x170(%r15), %rax
   79a77:      	subq	-0x68(%rbp), %rax
   79a7b:      	movq	%r8, %xmm5
   79a80:      	addq	%rdx, %rax
   79a83:      	movq	%rax, 0x170(%r15)
   79a8a:      	movzbl	%r13b, %eax
   79a8e:      	movq	%rax, %xmm0
   79a93:      	punpcklqdq	%xmm5, %xmm0    # xmm0 = xmm0[0],xmm5[0]
   79a97:      	paddq	%xmm6, %xmm0
   79a9b:      	movups	%xmm0, 0x90(%r15)
   79aa3:      	callq	 <scoop_gc_stress_move_enabled>
   79aa8:      	testb	%al, %al
   79aaa:      	jne	 <L17>
<L8>:
   79ab0:      	callq	 <scoop_gc_stress_move_enabled>
   79ab5:      	testb	%al, %al
   79ab7:      	jne	 <L16>
<L9>:
   79abd:      	callq	 <scoop_gc_mark_dispose>
   79ac2:      	callq	 <scoop_gc_monotonic_ns>
   79ac7:      	movq	0x180(%r15), %rdx
   79ace:      	movq	-0x70(%rbp), %rdi
   79ad2:      	movl	%r12d, %esi
   79ad5:      	movq	%rax, %r14
   79ad8:      	movq	%rdx, -0x68(%rbp)
   79adc:      	callq	 <scoop_gc_heap_finish_collection_locked>
   79ae1:      	callq	 <scoop_gc_monotonic_ns>
   79ae6:      	movq	-0x68(%rbp), %rdx
   79aea:      	addq	0x178(%r15), %rdx
   79af1:      	xorl	%edi, %edi
   79af3:      	subq	0x180(%r15), %rdx
   79afa:      	addq	%rax, %rdx
   79afd:      	subq	%r14, %rdx
   79b00:      	movq	%rdx, 0x178(%r15)
   79b07:      	callq	 <scoop_gc_set_pin_frames_locked>
   79b0c:      	callq	 <scoop_gc_monotonic_ns>
   79b11:      	subq	-0x88(%rbp), %rax
   79b18:      	addq	%rax, 0xe0(%r15)
   79b1f:      	cmpq	%rax, 0xe8(%r15)
   79b26:      	jae	 <L10>
   79b28:      	movq	%rax, 0xe8(%r15)
<L10>:
   79b2f:      	testb	%r13b, %r13b
   79b32:      	je	 <L14>
   79b34:      	addq	%rax, 0x270(%r15)
<L11>:
   79b3b:      	xorl	%edx, %edx
   79b3d:      	leaq	, %rsi <bounds.0>
   79b44:      	nop
   79b45:      	nopw	%cs:(%rax,%rax)
<L12>:
   79b50:      	cmpq	%rax, (%rsi,%rdx,8)
   79b54:      	jae	 <L13>
   79b56:      	addq	$0x1, %rdx
   79b5a:      	cmpq	$0x7, %rdx
   79b5e:      	jne	 <L12>
<L13>:
   79b60:      	addq	$0x1, 0x280(%r15,%rdx,8)
   79b69:      	callq	 <scoop_gc_roots_unlock>
   79b6e:      	callq	 <scoop_gc_heap_unlock>
   79b73:      	callq	 <scoop_thread_end_collection>
   79b78:      	jmp	 <L0>
   79b7d:      	nopl	(%rax)
<L14>:
   79b80:      	addq	%rax, 0x278(%r15)
   79b87:      	jmp	 <L11>
   79b89:      	nopl	(%rax)
<L15>:
   79b90:      	xorl	%edx, %edx
   79b92:      	leaq	, %rdi <visit_dirty_object>
   79b99:      	callq	 <scoop_gc_scan_remembered>
   79b9e:      	movq	-0x90(%rbp), %rsi
   79ba5:      	jmp	 <L7>
   79baa:      	nopw	(%rax,%rax)
<L16>:
   79bb0:      	callq	 <scoop_gc_heap_verify_stress_moved_locked>
   79bb5:      	jmp	 <L9>
   79bba:      	nopw	(%rax,%rax)
<L17>:
   79bc0:      	movdqa	-0x80(%rbp), %xmm7
   79bc5:      	subq	$0x20, %rsp
   79bc9:      	leaq	-0x52(%rbp), %r14
   79bcd:      	movb	%r13b, -0x52(%rbp)
   79bd1:      	leaq	, %rax <visit_root_region>
   79bd8:      	movq	%r14, -0x38(%rbp)
   79bdc:      	movq	%rax, -0x40(%rbp)
   79be0:      	movb	$0x1, -0x51(%rbp)
   79be4:      	movaps	%xmm7, -0x50(%rbp)
   79be8:      	movups	%xmm7, (%rsp)
   79bec:      	movdqa	-0x40(%rbp), %xmm0
   79bf1:      	movups	%xmm0, 0x10(%rsp)
   79bf6:      	callq	 <scoop_gc_scan_roots>
   79bfb:      	addq	$0x20, %rsp
   79bff:      	movq	%r14, %rsi
   79c02:      	leaq	, %rdi <verify_heap_object>
   79c09:      	callq	 <scoop_gc_visit_current_objects_locked>
   79c0e:      	jmp	 <L8>
   79c13:      	nop
   79c15:      	nopw	%cs:(%rax,%rax)
