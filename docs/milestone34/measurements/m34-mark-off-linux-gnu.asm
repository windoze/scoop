
/home/chenxu/repos/scoop/tmp/m34/marker-off-linux-gnu/mark-graphs:	file format elf64-x86-64

Disassembly of section .text:

000000000005fc10 <scoop_gc_heap_plan_moving_locked>:
   5fc10:      	endbr64
   5fc14:      	pushq	%rbp
   5fc15:      	movq	%rsp, %rbp
   5fc18:      	pushq	%r15
   5fc1a:      	pushq	%r14
   5fc1c:      	pushq	%r13
   5fc1e:      	pushq	%r12
   5fc20:      	pushq	%rbx
   5fc21:      	subq	$0x48, %rsp
   5fc25:      	leaq	, %r12 <scoop_gc_heap_state>
   5fc2c:      	movb	%dil, -0x58(%rbp)
   5fc30:      	movzbl	0x85(%r12), %ebx
   5fc39:      	testb	%bl, %bl
   5fc3b:      	je	 <L30>
   5fc41:      	movl	%edi, %r14d
   5fc44:      	movzbl	%dil, %edi
   5fc48:      	callq	 <scoop_heap_select_evacuation_sources>
   5fc4d:      	movq	%rax, -0x70(%rbp)
   5fc51:      	testb	%r14b, %r14b
   5fc54:      	je	 <L17>
<L0>:
   5fc5a:      	movq	$0x0, -0x48(%rbp)
   5fc62:      	movq	$0x0, -0x40(%rbp)
   5fc6a:      	movq	$0x0, -0x38(%rbp)
   5fc72:      	callq	 <scoop_heap_first_block>
   5fc77:      	movl	$0x0, -0x60(%rbp)
   5fc7e:      	movq	%rax, %r14
   5fc81:      	testq	%rax, %rax
   5fc84:      	je	 <L27>
<L1>:
   5fc8a:      	leaq	-0x38(%rbp), %rax
   5fc8e:      	movq	%rax, -0x68(%rbp)
   5fc92:      	nopl	(%rax)
   5fc95:      	nopw	%cs:(%rax,%rax)
<L2>:
   5fca0:      	movl	%ebx, %r15d
   5fca3:      	cmpl	$0x3, 0x14(%r14)
   5fca8:      	jne	 <L6>
   5fcae:      	movl	$0x10, %r13d
   5fcb4:      	cmpl	$0x2, 0x18(%r14)
   5fcb9:      	jne	 <L5>
   5fcbb:      	jmp	 <L16>
<L3>:
   5fcc0:      	addq	$0x1, %r13
   5fcc4:      	movl	%ebx, %r15d
   5fcc7:      	cmpq	$0x1000, %r13           # imm = 0x1000
   5fcce:      	je	 <L6>
<L4>:
   5fcd0:      	testb	%r15b, %r15b
   5fcd3:      	je	 <L6>
<L5>:
   5fcd5:      	movq	0x20(%r14), %rdi
   5fcd9:      	movq	%r13, %rsi
   5fcdc:      	callq	 <scoop_heap_bit_test>
   5fce1:      	testb	%al, %al
   5fce3:      	je	 <L3>
   5fce5:      	movq	0x28(%r14), %rdi
   5fce9:      	movq	%r13, %rsi
   5fcec:      	callq	 <scoop_heap_bit_test>
   5fcf1:      	testb	%al, %al
   5fcf3:      	je	 <L3>
   5fcf5:      	movq	0x30(%r14), %rdi
   5fcf9:      	movq	%r13, %rsi
   5fcfc:      	callq	 <scoop_heap_bit_test>
   5fd01:      	testb	%al, %al
   5fd03:      	jne	 <L3>
   5fd05:      	movzbl	-0x58(%rbp), %eax
   5fd09:      	subq	$0x8, %rsp
   5fd0d:      	movl	-0x60(%rbp), %r9d
   5fd11:      	leaq	-0x40(%rbp), %rsi
   5fd15:      	movq	-0x68(%rbp), %rdx
   5fd19:      	leaq	-0x48(%rbp), %rdi
   5fd1d:      	movq	%r13, %r8
   5fd20:      	movq	%r14, %rcx
   5fd23:      	pushq	%rax
   5fd24:      	addq	$0x1, %r13
   5fd28:      	callq	 <append_move>
   5fd2d:      	popq	%rsi
   5fd2e:      	popq	%rdi
   5fd2f:      	movl	%eax, %r15d
   5fd32:      	cmpq	$0x1000, %r13           # imm = 0x1000
   5fd39:      	jne	 <L4>
   5fd3b:      	nopl	(%rax,%rax)
<L6>:
   5fd40:      	movq	%r14, %rdi
   5fd43:      	callq	 <scoop_heap_next_block>
   5fd48:      	movq	%rax, %r14
   5fd4b:      	testq	%rax, %rax
   5fd4e:      	je	 <L7>
   5fd50:      	testb	%r15b, %r15b
   5fd53:      	jne	 <L2>
<L7>:
   5fd59:      	movl	-0x60(%rbp), %ecx
   5fd5c:      	movq	-0x48(%rbp), %r14
   5fd60:      	testl	%ecx, %ecx
   5fd62:      	je	 <L11>
   5fd64:      	movq	-0x40(%rbp), %rsi
   5fd68:      	testq	%rsi, %rsi
   5fd6b:      	je	 <L10>
   5fd6d:      	leaq	(%rsi,%rsi,2), %rcx
   5fd71:      	leaq	0x28(%r14), %rax
   5fd75:      	xorl	%esi, %esi
   5fd77:      	shlq	$0x4, %rcx
   5fd7b:      	leaq	0x28(%r14,%rcx), %rdi
<L8>:
   5fd80:      	movq	(%rax), %rcx
   5fd83:      	movq	(%rcx), %rcx
   5fd86:      	cmpq	$0x0, 0x30(%rcx)
   5fd8b:      	jne	 <L9>
   5fd8d:      	cmpb	$0x0, 0x2c(%rcx)
   5fd91:      	jne	 <L9>
   5fd93:      	movb	$0x1, 0x2c(%rcx)
   5fd97:      	addq	$0x1, %rsi
<L9>:
   5fd9b:      	addq	$0x30, %rax
   5fd9f:      	cmpq	%rax, %rdi
   5fda2:      	jne	 <L8>
<L10>:
   5fda4:      	cmpq	-0x70(%rbp), %rsi
   5fda8:      	setb	%al
   5fdab:      	andl	%eax, %r15d
<L11>:
   5fdae:      	testb	%r15b, %r15b
   5fdb1:      	je	 <L19>
<L12>:
   5fdb7:      	movq	-0x40(%rbp), %rax
   5fdbb:      	movq	%r14, %r15
   5fdbe:      	xorl	%r13d, %r13d
   5fdc1:      	movq	%rax, -0x58(%rbp)
   5fdc5:      	testq	%rax, %rax
   5fdc8:      	jne	 <L15>
   5fdca:      	jmp	 <L28>
   5fdcf:      	nop
<L13>:
   5fdd0:      	movq	0x8(%r15), %rsi
   5fdd4:      	movl	$0x1, %ecx
   5fdd9:      	callq	 <scoop_heap_record_small_object>
   5fdde:      	movq	-0x60(%rbp), %rax
   5fde2:      	movq	0x8(%r15), %rcx
   5fde6:      	movq	0x18(%r15), %rdx
   5fdea:      	movq	0x58(%rax), %rax
   5fdee:      	movq	%rcx, (%rax,%rdx,8)
<L14>:
   5fdf2:      	addq	$0x1, 0x1c0(%r12)
   5fdfb:      	addq	$0x1, %r13
   5fdff:      	addq	$0x30, %r15
   5fe03:      	cmpq	%r13, -0x58(%rbp)
   5fe07:      	je	 <L28>
<L15>:
   5fe0d:      	movq	0x8(%r15), %rdi
   5fe11:      	movq	0x10(%r15), %rdx
   5fe15:      	movq	(%r15), %rsi
   5fe18:      	callq	 <memcpy@plt>
   5fe1d:      	movq	0x10(%r15), %rax
   5fe21:      	movq	0x28(%r15), %rdi
   5fe25:      	addq	%rax, 0x140(%r12)
   5fe2d:      	movq	0x20(%r15), %rax
   5fe31:      	movq	0x10(%r15), %rdx
   5fe35:      	movq	%rax, -0x60(%rbp)
   5fe39:      	cmpq	$0x7f80, %rdx           # imm = 0x7F80
   5fe40:      	jbe	 <L13>
   5fe42:      	movl	$0x1, %esi
   5fe47:      	callq	 <scoop_heap_publish_large_object>
   5fe4c:      	movq	0x8(%r15), %rdx
   5fe50:      	movq	-0x60(%rbp), %rax
   5fe54:      	movq	%rdx, 0x78(%rax)
   5fe58:      	jmp	 <L14>
   5fe5a:      	nopw	(%rax,%rax)
<L16>:
   5fe60:      	movzbl	-0x58(%rbp), %eax
   5fe64:      	subq	$0x8, %rsp
   5fe68:      	movl	-0x60(%rbp), %r9d
   5fe6c:      	leaq	-0x40(%rbp), %rsi
   5fe70:      	movq	-0x68(%rbp), %rdx
   5fe74:      	movl	$0x10, %r8d
   5fe7a:      	leaq	-0x48(%rbp), %rdi
   5fe7e:      	movq	%r14, %rcx
   5fe81:      	pushq	%rax
   5fe82:      	callq	 <append_move>
   5fe87:      	popq	%r8
   5fe89:      	popq	%r9
   5fe8b:      	movl	%eax, %r15d
   5fe8e:      	jmp	 <L6>
   5fe93:      	nopl	(%rax,%rax)
<L17>:
   5fe98:      	movzbl	0x81(%r12), %eax
   5fea1:      	movb	%al, -0x58(%rbp)
   5fea4:      	testb	%al, %al
   5fea6:      	jne	 <L0>
   5feac:      	cmpq	$0x0, -0x70(%rbp)
   5feb1:      	jne	 <L26>
<L18>:
   5feb7:      	leaq	-0x28(%rbp), %rsp
   5febb:      	movl	%ebx, %eax
   5febd:      	popq	%rbx
   5febe:      	popq	%r12
   5fec0:      	popq	%r13
   5fec2:      	popq	%r14
   5fec4:      	popq	%r15
   5fec6:      	popq	%rbp
   5fec7:      	retq
   5fec8:      	nopl	(%rax,%rax)
<L19>:
   5fed0:      	movq	%r14, %rdi
   5fed3:      	callq	 <free@plt>
   5fed8:      	callq	 <scoop_heap_first_block>
   5fedd:      	movq	%rax, %rbx
   5fee0:      	testq	%rax, %rax
   5fee3:      	jne	 <L25>
   5fee5:      	nopw	%cs:(%rax,%rax)
<L20>:
   5fef0:      	movq	0x28(%r12), %rax
   5fef5:      	testq	%rax, %rax
   5fef8:      	je	 <L22>
   5fefa:      	nopw	(%rax,%rax)
<L21>:
   5ff00:      	xorl	%edx, %edx
   5ff02:      	movw	%dx, 0x2b(%rax)
   5ff06:      	movq	0x18(%rax), %rax
   5ff0a:      	testq	%rax, %rax
   5ff0d:      	jne	 <L21>
<L22>:
   5ff0f:      	movzbl	0x81(%r12), %ebx
   5ff18:      	pxor	%xmm0, %xmm0
   5ff1c:      	movq	$0x0, 0x1b8(%r12)
   5ff28:      	movups	%xmm0, 0x1a8(%r12)
   5ff31:      	testb	%bl, %bl
   5ff33:      	je	 <L18>
   5ff35:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x322f>
   5ff3c:      	callq	 <scoop_heap_fatal>
   5ff41:      	nopl	(%rax)
<L23>:
   5ff48:      	cmpl	$0x3, %eax
   5ff4b:      	je	 <L29>
<L24>:
   5ff51:      	movq	%rbx, %rdi
   5ff54:      	callq	 <scoop_heap_next_block>
   5ff59:      	movq	%rax, %rbx
   5ff5c:      	testq	%rax, %rax
   5ff5f:      	je	 <L20>
<L25>:
   5ff61:      	movl	0x14(%rbx), %eax
   5ff64:      	cmpl	$0x4, %eax
   5ff67:      	jne	 <L23>
   5ff69:      	movq	%rbx, %rdi
   5ff6c:      	callq	 <scoop_heap_release_block>
   5ff71:      	jmp	 <L24>
<L26>:
   5ff73:      	callq	 <scoop_heap_prepare_evacuation_targets>
   5ff78:      	movq	$0x0, -0x48(%rbp)
   5ff80:      	movq	$0x0, -0x40(%rbp)
   5ff88:      	movq	$0x0, -0x38(%rbp)
   5ff90:      	callq	 <scoop_heap_first_block>
   5ff95:      	movq	%rax, %r14
   5ff98:      	testq	%rax, %rax
   5ff9b:      	je	 <L12>
   5ffa1:      	cmpq	$0x1, -0x70(%rbp)
   5ffa6:      	movl	$0x1, -0x60(%rbp)
   5ffad:      	setne	-0x58(%rbp)
   5ffb1:      	jmp	 <L1>
<L27>:
   5ffb6:      	xorl	%r14d, %r14d
   5ffb9:      	nopl	(%rax)
<L28>:
   5ffc0:      	movq	%r14, %rdi
   5ffc3:      	callq	 <free@plt>
   5ffc8:      	jmp	 <L18>
   5ffcd:      	nopl	(%rax)
<L29>:
   5ffd0:      	movq	%rbx, %rdi
   5ffd3:      	callq	 <scoop_heap_block_has_pins>
   5ffd8:      	movq	0x58(%rbx), %rdi
   5ffdc:      	cmpb	$0x1, %al
   5ffde:      	sbbl	%eax, %eax
   5ffe0:      	andl	$-0x3, %eax
   5ffe3:      	addl	$0x5, %eax
   5ffe6:      	movl	%eax, 0x14(%rbx)
   5ffe9:      	callq	 <free@plt>
   5ffee:      	movq	$0x0, 0x58(%rbx)
   5fff6:      	jmp	 <L24>
<L30>:
   5fffb:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x3207>
   60002:      	callq	 <scoop_heap_fatal>
   60007:      	nopw	(%rax,%rax)

0000000000065f80 <scoop_gc_mark_object_locked>:
   65f80:      	endbr64
   65f84:      	pushq	%rbp
   65f85:      	movq	%rsp, %rbp
   65f88:      	pushq	%r15
   65f8a:      	pushq	%r14
   65f8c:      	pushq	%r13
   65f8e:      	pushq	%r12
   65f90:      	pushq	%rbx
   65f91:      	subq	$0x28, %rsp
   65f95:      	leaq	, %rax <scoop_gc_heap_state>
   65f9c:      	cmpb	$0x0, 0x85(%rax)
   65fa3:      	je	 <L9>
   65fa9:      	leaq	-0x38(%rbp), %rdx
   65fad:      	leaq	-0x40(%rbp), %rsi
   65fb1:      	movq	%rdi, %r12
   65fb4:      	callq	 <scoop_heap_object_meta>
   65fb9:      	movl	%eax, %ebx
   65fbb:      	testb	%al, %al
   65fbd:      	je	 <L8>
   65fc3:      	movq	-0x40(%rbp), %r15
   65fc7:      	movq	-0x38(%rbp), %r13
   65fcb:      	cmpl	$0x2, 0x18(%r15)
   65fd0:      	jne	 <L2>
   65fd2:      	movzbl	0x81(%r15), %eax
   65fda:      	testb	%al, %al
   65fdc:      	je	 <L3>
<L0>:
   65fde:      	xorl	%ebx, %ebx
<L1>:
   65fe0:      	addq	$0x28, %rsp
   65fe4:      	movl	%ebx, %eax
   65fe6:      	popq	%rbx
   65fe7:      	popq	%r12
   65fe9:      	popq	%r13
   65feb:      	popq	%r14
   65fed:      	popq	%r15
   65fef:      	popq	%rbp
   65ff0:      	retq
   65ff1:      	nopl	(%rax)
<L2>:
   65ff8:      	movq	0x28(%r15), %rdi
   65ffc:      	movq	%r13, %rsi
   65fff:      	callq	 <scoop_heap_bit_test>
   66004:      	testb	%al, %al
   66006:      	jne	 <L0>
<L3>:
   66008:      	movq	%r12, %rdi
   6600b:      	callq	 <scoop_gc_object_size_locked>
   66010:      	movq	%rax, %r14
   66013:      	cmpl	$0x2, 0x18(%r15)
   66018:      	jne	 <L6>
   6601a:      	movzbl	0x82(%r15), %eax
   66022:      	movb	%al, -0x41(%rbp)
<L4>:
   66025:      	movb	$0x1, 0x81(%r15)
<L5>:
   6602d:      	addq	%r14, 0x68(%r15)
   66031:      	cmpb	$0x0, -0x41(%rbp)
   66035:      	jne	 <L1>
   66037:      	addq	%r14, 0x70(%r15)
   6603b:      	jmp	 <L1>
   6603d:      	nopl	(%rax)
<L6>:
   66040:      	movq	0x30(%r15), %rdi
   66044:      	movq	%r13, %rsi
   66047:      	callq	 <scoop_heap_bit_test>
   6604c:      	movb	%al, -0x41(%rbp)
   6604f:      	cmpl	$0x2, 0x18(%r15)
   66054:      	je	 <L4>
   66056:      	movq	0x28(%r15), %rdi
   6605a:      	movq	%r13, %rsi
   6605d:      	callq	 <scoop_heap_bit_set>
   66062:      	movq	%r15, %rdi
   66065:      	callq	 <scoop_heap_block_base>
   6606a:      	subq	%rax, %r12
   6606d:      	movq	%r12, %r13
   66070:      	leaq	-0x1(%r12,%r14), %rax
   66075:      	shrq	$0x7, %r13
   66079:      	shrq	$0x7, %rax
   6607d:      	cmpq	%r13, %rax
   66080:      	jb	 <L5>
   66082:      	leaq	0x1(%rax), %r12
   66086:      	nopw	%cs:(%rax,%rax)
<L7>:
   66090:      	movq	0x48(%r15), %rdi
   66094:      	movq	%r13, %rsi
   66097:      	addq	$0x1, %r13
   6609b:      	callq	 <scoop_heap_bit_set>
   660a0:      	cmpq	%r12, %r13
   660a3:      	jne	 <L7>
   660a5:      	jmp	 <L5>
<L8>:
   660a7:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x4797>
   660ae:      	callq	 <scoop_heap_fatal>
<L9>:
   660b3:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x476f>
   660ba:      	callq	 <scoop_heap_fatal>
   660bf:      	nop

000000000006d710 <visit_managed_slot>:
   6d710:      	endbr64
   6d714:      	pushq	%rbp
   6d715:      	movq	%rsp, %rbp
   6d718:      	pushq	%r14
   6d71a:      	pushq	%r12
   6d71c:      	pushq	%rbx
   6d71d:      	subq	$0x8, %rsp
   6d721:      	movl	(%rsi), %eax
   6d723:      	testl	%eax, %eax
   6d725:      	jne	 <L0>
   6d727:      	cmpb	$0x0, 0x5(%rsi)
   6d72b:      	je	 <L3>
   6d72d:      	leaq	, %rax <scoop_gc_heap_state>
   6d734:      	addq	$0x1, 0xd0(%rax)
<L0>:
   6d73c:      	movq	(%rdi), %rbx
   6d73f:      	testq	%rbx, %rbx
   6d742:      	je	 <L2>
   6d744:      	movq	%rdi, %r14
   6d747:      	movq	%rbx, %rdi
   6d74a:      	movq	%rsi, %r12
   6d74d:      	callq	 <scoop_gc_is_object_start_locked>
   6d752:      	testb	%al, %al
   6d754:      	je	 <L4>
   6d756:      	cmpb	$0x0, 0x4(%r12)
   6d75c:      	jne	 <L6>
   6d762:      	movl	(%r12), %eax
   6d766:      	cmpl	$0x1, %eax
   6d769:      	je	 <L7>
<L1>:
   6d76f:      	cmpl	$0x2, %eax
   6d772:      	je	 <L5>
   6d774:      	testl	%eax, %eax
   6d776:      	jne	 <L9>
   6d77c:      	movq	%rbx, %rdi
   6d77f:      	callq	 <scoop_gc_mark_object_locked>
   6d784:      	testb	%al, %al
   6d786:      	jne	 <L8>
<L2>:
   6d78c:      	addq	$0x8, %rsp
   6d790:      	popq	%rbx
   6d791:      	popq	%r12
   6d793:      	popq	%r14
   6d795:      	popq	%rbp
   6d796:      	retq
   6d797:      	nopw	(%rax,%rax)
<L3>:
   6d7a0:      	cmpb	$0x0, 0x6(%rsi)
   6d7a4:      	je	 <L0>
   6d7a6:      	leaq	, %rax <scoop_gc_heap_state>
   6d7ad:      	addq	$0x1, 0xc8(%rax)
   6d7b5:      	jmp	 <L0>
   6d7b7:      	nopw	(%rax,%rax)
<L4>:
   6d7c0:      	movq	%rbx, %rdi
   6d7c3:      	callq	 <scoop_gc_is_immortal_object_locked>
   6d7c8:      	testb	%al, %al
   6d7ca:      	jne	 <L2>
   6d7cc:      	movq	%rbx, %rdi
   6d7cf:      	callq	 <scoop_gc_is_external_object_locked>
   6d7d4:      	testb	%al, %al
   6d7d6:      	jne	 <L2>
   6d7d8:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x58cf>
   6d7df:      	callq	 <collector_fatal>
   6d7e4:      	nopl	(%rax)
<L5>:
   6d7e8:      	movq	%rbx, %rdi
   6d7eb:      	callq	 <scoop_gc_is_forwarded_old_locked>
   6d7f0:      	testb	%al, %al
   6d7f2:      	jne	 <L10>
   6d7f8:      	movq	%rbx, %rdi
   6d7fb:      	callq	 <scoop_gc_is_current_live_object_locked>
   6d800:      	testb	%al, %al
   6d802:      	jne	 <L2>
   6d804:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x594f>
   6d80b:      	callq	 <collector_fatal>
<L6>:
   6d810:      	movq	%rbx, %rdi
   6d813:      	callq	 <scoop_gc_is_young_object_locked>
   6d818:      	testb	%al, %al
   6d81a:      	je	 <L2>
   6d820:      	movl	(%r12), %eax
   6d824:      	cmpl	$0x1, %eax
   6d827:      	jne	 <L1>
<L7>:
   6d82d:      	movq	%rbx, %rdi
   6d830:      	callq	 <scoop_gc_forward_object_locked>
   6d835:      	movq	%rax, (%r14)
   6d838:      	movq	%rax, %rbx
   6d83b:      	cmpb	$0x0, 0x4(%r12)
   6d841:      	jne	 <L2>
   6d847:      	movq	%rax, %rdi
   6d84a:      	callq	 <scoop_gc_claim_object_scan_locked>
   6d84f:      	testb	%al, %al
   6d851:      	je	 <L2>
   6d857:      	movq	%rbx, %rdi
   6d85a:      	callq	 <work_push>
   6d85f:      	jmp	 <L2>
   6d864:      	nopl	(%rax)
<L8>:
   6d868:      	movq	%rbx, %rdi
   6d86b:      	addq	$0x1,  <marked_count>
   6d873:      	callq	 <work_push>
   6d878:      	jmp	 <L2>
<L9>:
   6d87d:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x597f>
   6d884:      	callq	 <collector_fatal>
<L10>:
   6d889:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x5917>
   6d890:      	callq	 <collector_fatal>
   6d895:      	nopw	%cs:(%rax,%rax)

000000000006d980 <collect.part.0>:
   6d980:      	pushq	%rbp
   6d981:      	movq	%rsp, %rbp
   6d984:      	pushq	%r15
   6d986:      	pushq	%r14
   6d988:      	leaq	-0xd0(%rbp), %rsi
   6d98f:      	pushq	%r13
   6d991:      	pushq	%r12
   6d993:      	movl	%edi, %r12d
   6d996:      	pushq	%rbx
   6d997:      	movl	%edi, %ebx
   6d999:      	movl	$0x1, %edi
   6d99e:      	subq	$0xe8, %rsp
   6d9a5:      	callq	 <clock_gettime@plt>
   6d9aa:      	testl	%eax, %eax
   6d9ac:      	jne	 <L22>
   6d9b2:      	movq	-0xd0(%rbp), %rax
   6d9b9:      	movq	%rax, -0x108(%rbp)
   6d9c0:      	movq	-0xc8(%rbp), %rax
   6d9c7:      	movq	%rax, -0x110(%rbp)
   6d9ce:      	callq	 <scoop_gc_heap_lock>
   6d9d3:      	callq	 <scoop_gc_roots_lock>
   6d9d8:      	movl	$0x1, %edi
   6d9dd:      	callq	 <scoop_gc_set_pin_frames_locked>
   6d9e2:      	callq	 <scoop_thread_collection_registry_head>
   6d9e7:      	pxor	%xmm0, %xmm0
   6d9eb:      	testq	%rax, %rax
   6d9ee:      	je	 <L1>
   6d9f0:      	nopl	(%rax,%rax)
   6d9f5:      	nopw	%cs:(%rax,%rax)
<L0>:
   6da00:      	movq	$0x0, 0xd0(%rax)
   6da0b:      	movaps	%xmm0, 0xc0(%rax)
   6da12:      	movq	0xe0(%rax), %rax
   6da19:      	testq	%rax, %rax
   6da1c:      	jne	 <L0>
<L1>:
   6da1e:      	movq	, %r14 <work_len>
   6da25:      	testq	%r14, %r14
   6da28:      	jne	 <L23>
   6da2e:      	movq	, %xmm3 <scoop_amd64_managed_frame_ops+0x50>
   6da36:      	leaq	, %rax <visit_external_root>
   6da3d:      	movzbl	%bl, %edi
   6da40:      	leaq	-0x50(%rbp), %rbx
   6da44:      	movq	%rax, %xmm4
   6da49:      	punpcklqdq	%xmm4, %xmm3    # xmm3 = xmm3[0],xmm4[0]
   6da4d:      	movaps	%xmm3, -0xf0(%rbp)
   6da54:      	callq	 <scoop_gc_heap_begin_collection_locked>
   6da59:      	leaq	, %r13 <scoop_gc_heap_state>
<L2>:
   6da60:      	movdqa	-0xf0(%rbp), %xmm1
   6da68:      	subq	$0x20, %rsp
   6da6c:      	pxor	%xmm0, %xmm0
   6da70:      	leaq	, %rax <visit_root_region>
   6da77:      	movaps	%xmm0, -0x50(%rbp)
   6da7b:      	movq	%rax, -0xa0(%rbp)
   6da82:      	movq	%rbx, -0x98(%rbp)
   6da89:      	movq	$0x0,  <marked_count>
   6da94:      	movq	$0x0,  <work_scanned>
   6da9f:      	movq	$0x0,  <work_len>
   6daaa:      	movb	%r12b, -0x4c(%rbp)
   6daae:      	movb	$0x1, -0x4b(%rbp)
   6dab2:      	movaps	%xmm1, -0xb0(%rbp)
   6dab9:      	movups	%xmm1, (%rsp)
   6dabd:      	movdqa	-0xa0(%rbp), %xmm0
   6dac5:      	movups	%xmm0, 0x10(%rsp)
   6daca:      	callq	 <scoop_gc_scan_roots>
   6dacf:      	movb	$0x0, -0x4b(%rbp)
   6dad3:      	addq	$0x20, %rsp
   6dad7:      	testb	%r12b, %r12b
   6dada:      	je	 <L4>
   6dadc:      	jmp	 <L5>
   6dade:      	nop
<L3>:
   6dae0:      	leaq	0x1(%rax), %rdx
   6dae4:      	movq	%rbx, %rsi
   6dae7:      	movq	%rdx,  <work_scanned>
   6daee:      	movq	, %rdx <work>
   6daf5:      	movq	(%rdx,%rax,8), %rdi
   6daf9:      	callq	 <visit_object>
<L4>:
   6dafe:      	movq	, %rax <work_scanned>
   6db05:      	cmpq	, %rax <work_len>
   6db0c:      	jb	 <L3>
   6db0e:      	movzbl	%r12b, %r15d
   6db12:      	movl	%r15d, %edi
   6db15:      	callq	 <scoop_gc_heap_plan_moving_locked>
   6db1a:      	testb	%al, %al
   6db1c:      	jne	 <L6>
   6db1e:      	testb	%r12b, %r12b
   6db21:      	je	 <L15>
   6db27:      	xorl	%edi, %edi
   6db29:      	addq	$0x1, 0xa0(%r13)
   6db31:      	xorl	%r12d, %r12d
   6db34:      	movb	$0x0, 0x85(%r13)
   6db3c:      	callq	 <scoop_gc_heap_begin_collection_locked>
   6db41:      	jmp	 <L2>
   6db46:      	nopw	%cs:(%rax,%rax)
<L5>:
   6db50:      	movl	$0x1, %edx
   6db55:      	movq	%rbx, %rsi
   6db58:      	leaq	, %rdi <visit_dirty_object>
   6db5f:      	callq	 <scoop_gc_scan_remembered>
   6db64:      	jmp	 <L4>
<L6>:
   6db66:      	movq	$0x0, -0xdb(%rbp)
   6db71:      	movl	$0x1, -0xe0(%rbp)
   6db7b:      	movl	$0x0, -0xd4(%rbp)
   6db85:      	movb	%r12b, -0xdc(%rbp)
   6db8c:      	testb	%r12b, %r12b
   6db8f:      	je	 <L16>
   6db95:      	movdqa	-0xf0(%rbp), %xmm5
   6db9d:      	subq	$0x20, %rsp
   6dba1:      	leaq	-0xe0(%rbp), %rbx
   6dba8:      	leaq	, %rax <visit_root_region>
   6dbaf:      	movq	%rax, -0x80(%rbp)
   6dbb3:      	movq	%rbx, -0x78(%rbp)
   6dbb7:      	movq	%rbx, -0xf8(%rbp)
   6dbbe:      	movaps	%xmm5, -0x90(%rbp)
   6dbc5:      	movb	$0x1, -0xdb(%rbp)
   6dbcc:      	movups	%xmm5, (%rsp)
   6dbd0:      	movdqa	-0x80(%rbp), %xmm0
   6dbd5:      	movups	%xmm0, 0x10(%rsp)
   6dbda:      	callq	 <scoop_gc_scan_roots>
   6dbdf:      	addq	$0x20, %rsp
   6dbe3:      	xorl	%edx, %edx
   6dbe5:      	movq	%rbx, %rsi
   6dbe8:      	leaq	, %rdi <visit_dirty_object>
   6dbef:      	movb	$0x0, -0xdb(%rbp)
   6dbf6:      	xorl	%ebx, %ebx
   6dbf8:      	callq	 <scoop_gc_scan_remembered>
   6dbfd:      	cmpq	$0x0,  <work_len>
   6dc05:      	je	 <L8>
   6dc07:      	nopw	(%rax,%rax)
<L7>:
   6dc10:      	movq	, %rax <work>
   6dc17:      	movq	(%rax,%rbx,8), %rdi
   6dc1b:      	addq	$0x1, %rbx
   6dc1f:      	callq	 <scoop_gc_forward_object_locked>
   6dc24:      	movq	%rax, %rdi
   6dc27:      	movq	%rax, -0x100(%rbp)
   6dc2e:      	callq	 <scoop_gc_claim_object_scan_locked>
   6dc33:      	movq	-0xf8(%rbp), %rsi
   6dc3a:      	movq	-0x100(%rbp), %rdi
   6dc41:      	callq	 <visit_object>
   6dc46:      	cmpq	, %rbx <work_len>
   6dc4d:      	jb	 <L7>
<L8>:
   6dc4f:      	addq	$0x1, 0x90(%r13)
   6dc57:      	leaq	-0xc0(%rbp), %rbx
   6dc5e:      	callq	 <scoop_gc_stress_move_enabled>
   6dc63:      	testb	%al, %al
   6dc65:      	jne	 <L19>
<L9>:
   6dc6b:      	callq	 <scoop_gc_stress_move_enabled>
   6dc70:      	testb	%al, %al
   6dc72:      	jne	 <L20>
<L10>:
   6dc78:      	movq	, %rdi <marked_count>
   6dc7f:      	movl	%r15d, %esi
   6dc82:      	movq	$0x0,  <work_scanned>
   6dc8d:      	movq	$0x0,  <work_len>
   6dc98:      	callq	 <scoop_gc_heap_finish_collection_locked>
   6dc9d:      	xorl	%edi, %edi
   6dc9f:      	callq	 <scoop_gc_set_pin_frames_locked>
   6dca4:      	movq	%rbx, %rsi
   6dca7:      	movl	$0x1, %edi
   6dcac:      	callq	 <clock_gettime@plt>
   6dcb1:      	testl	%eax, %eax
   6dcb3:      	jne	 <L22>
   6dcb9:      	imulq	$0x3b9aca00, -0x108(%rbp), %rdx # imm = 0x3B9ACA00
   6dcc4:      	imulq	$0x3b9aca00, -0xc0(%rbp), %rax # imm = 0x3B9ACA00
   6dccf:      	subq	-0x110(%rbp), %rax
   6dcd6:      	addq	-0xb8(%rbp), %rax
   6dcdd:      	subq	%rdx, %rax
   6dce0:      	addq	%rax, 0xe0(%r13)
   6dce7:      	cmpq	%rax, 0xe8(%r13)
   6dcee:      	jae	 <L11>
   6dcf0:      	movq	%rax, 0xe8(%r13)
<L11>:
   6dcf7:      	testb	%r12b, %r12b
   6dcfa:      	je	 <L21>
   6dd00:      	addq	%rax, 0x148(%r13)
<L12>:
   6dd07:      	leaq	, %rdx <bounds.0>
   6dd0e:      	nop
<L13>:
   6dd10:      	cmpq	%rax, (%rdx,%r14,8)
   6dd14:      	jae	 <L14>
   6dd16:      	addq	$0x1, %r14
   6dd1a:      	cmpq	$0x7, %r14
   6dd1e:      	jne	 <L13>
<L14>:
   6dd20:      	addq	$0x1, 0x158(%r13,%r14,8)
   6dd29:      	callq	 <scoop_gc_roots_unlock>
   6dd2e:      	callq	 <scoop_gc_heap_unlock>
   6dd33:      	callq	 <scoop_thread_end_collection>
   6dd38:      	leaq	-0x28(%rbp), %rsp
   6dd3c:      	movl	$0x1, %eax
   6dd41:      	popq	%rbx
   6dd42:      	popq	%r12
   6dd44:      	popq	%r13
   6dd46:      	popq	%r14
   6dd48:      	popq	%r15
   6dd4a:      	popq	%rbp
   6dd4b:      	retq
<L15>:
   6dd4c:      	xorl	%edx, %edx
   6dd4e:      	movb	$0x0, -0xdc(%rbp)
   6dd55:      	movq	$0x0, -0xda(%rbp)
   6dd60:      	movw	%dx, -0xd2(%rbp)
   6dd67:      	movl	$0x1, -0xe0(%rbp)
<L16>:
   6dd71:      	movdqa	-0xf0(%rbp), %xmm6
   6dd79:      	subq	$0x20, %rsp
   6dd7d:      	leaq	, %rax <visit_root_region>
   6dd84:      	movq	$0x0,  <work_scanned>
   6dd8f:      	movq	%rax, -0x60(%rbp)
   6dd93:      	leaq	-0xe0(%rbp), %rax
   6dd9a:      	movq	%rax, -0x58(%rbp)
   6dd9e:      	movq	%rax, -0xf8(%rbp)
   6dda5:      	movq	$0x0,  <work_len>
   6ddb0:      	movb	$0x1, -0xdb(%rbp)
   6ddb7:      	movaps	%xmm6, -0x70(%rbp)
   6ddbb:      	movups	%xmm6, (%rsp)
   6ddbf:      	movdqa	-0x60(%rbp), %xmm0
   6ddc4:      	movups	%xmm0, 0x10(%rsp)
   6ddc9:      	callq	 <scoop_gc_scan_roots>
   6ddce:      	movq	, %rax <work_scanned>
   6ddd5:      	addq	$0x20, %rsp
   6ddd9:      	cmpq	, %rax <work_len>
   6dde0:      	movb	$0x0, -0xdb(%rbp)
   6dde7:      	jae	 <L18>
   6dde9:      	nopl	(%rax)
<L17>:
   6ddf0:      	leaq	0x1(%rax), %rdx
   6ddf4:      	movq	-0xf8(%rbp), %rsi
   6ddfb:      	movq	%rdx,  <work_scanned>
   6de02:      	movq	, %rdx <work>
   6de09:      	movq	(%rdx,%rax,8), %rdi
   6de0d:      	callq	 <visit_object>
   6de12:      	movq	, %rax <work_scanned>
   6de19:      	cmpq	, %rax <work_len>
   6de20:      	jb	 <L17>
<L18>:
   6de22:      	addq	$0x1, 0x98(%r13)
   6de2a:      	xorl	%r15d, %r15d
   6de2d:      	xorl	%r12d, %r12d
   6de30:      	callq	 <scoop_gc_stress_move_enabled>
   6de35:      	leaq	-0xc0(%rbp), %rbx
   6de3c:      	testb	%al, %al
   6de3e:      	je	 <L9>
<L19>:
   6de44:      	movdqa	-0xf0(%rbp), %xmm7
   6de4c:      	xorl	%eax, %eax
   6de4e:      	subq	$0x20, %rsp
   6de52:      	movq	%rbx, -0x38(%rbp)
   6de56:      	movw	%ax, -0xb2(%rbp)
   6de5d:      	leaq	, %rax <visit_root_region>
   6de64:      	movq	%rax, -0x40(%rbp)
   6de68:      	movaps	%xmm7, -0x50(%rbp)
   6de6c:      	movq	$0x0, -0xba(%rbp)
   6de77:      	movl	$0x2, -0xc0(%rbp)
   6de81:      	movb	%r12b, -0xbc(%rbp)
   6de88:      	movb	$0x1, -0xbb(%rbp)
   6de8f:      	movups	%xmm7, (%rsp)
   6de93:      	movdqa	-0x40(%rbp), %xmm0
   6de98:      	movups	%xmm0, 0x10(%rsp)
   6de9d:      	callq	 <scoop_gc_scan_roots>
   6dea2:      	addq	$0x20, %rsp
   6dea6:      	movq	%rbx, %rsi
   6dea9:      	leaq	, %rdi <verify_heap_object>
   6deb0:      	movb	$0x0, -0xbb(%rbp)
   6deb7:      	callq	 <scoop_gc_visit_current_objects_locked>
   6debc:      	callq	 <scoop_gc_stress_move_enabled>
   6dec1:      	testb	%al, %al
   6dec3:      	je	 <L10>
<L20>:
   6dec9:      	callq	 <scoop_gc_heap_verify_stress_moved_locked>
   6dece:      	jmp	 <L10>
<L21>:
   6ded3:      	addq	%rax, 0x150(%r13)
   6deda:      	jmp	 <L12>
<L22>:
   6dedf:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x5a07>
   6dee6:      	callq	 <collector_fatal>
<L23>:
   6deeb:      	leaq	, %rdi <scoop$1$be$b68c7cb698da4c6c65064a9a94709f99e929bcb484fa0300df8693f3c1a1e0e3+0x5a27>
   6def2:      	callq	 <collector_fatal>
   6def7:      	nopw	(%rax,%rax)
