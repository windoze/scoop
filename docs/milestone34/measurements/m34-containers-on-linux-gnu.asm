; lowOccupancy, MIR fn1, LIR scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000008bde0 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2>:
   8bde0:      	pushq	%rbp
   8bde1:      	movq	%rsp, %rbp
   8bde4:      	pushq	%r15
   8bde6:      	pushq	%r14
   8bde8:      	pushq	%r13
   8bdea:      	pushq	%r12
   8bdec:      	pushq	%rbx
   8bded:      	subq	$0x268, %rsp            # imm = 0x268
   8bdf4:      	movq	%rdi, %rbx
   8bdf7:      	movq	%fs:0x0, %rax
   8be00:      	leaq	-0x10(%rax), %rax
   8be07:      	movq	(%rax), %rcx
   8be0a:      	leaq	0x1d67ff(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8be11:      	movq	(%rax), %rax
   8be14:      	leaq	0x1d67fd(%rip), %rdx    # 0x262618 <scoop_thread_world_phase>
   8be1b:      	movl	(%rdx), %esi
   8be1d:      	movl	(%rcx), %edx
   8be1f:      	movq	%rcx, -0x60(%rbp)
   8be23:      	movq	0x8(%rcx), %rcx
   8be27:      	testl	%esi, %esi
   8be29:      	jne	0x8be35 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x55>
   8be2b:      	cmpl	$0x1, %edx
   8be2e:      	jne	0x8be35 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x55>
   8be30:      	cmpq	%rax, %rcx
   8be33:      	je	0x8be3a <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5a>
   8be35:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8be3a:      	movabsq	$0x7fffffffffffffff, %r13 # imm = 0x7FFFFFFFFFFFFFFF
   8be44:      	xorps	%xmm0, %xmm0
   8be47:      	movups	%xmm0, -0xe8(%rbp)
   8be4e:      	movq	$0x0, -0xd8(%rbp)
   8be59:      	leaq	-0xe8(%rbp), %r14
   8be60:      	movq	%r14, %rdi
   8be63:      	xorl	%esi, %esi
   8be65:      	xorl	%edx, %edx
   8be67:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8be6c:      	xorps	%xmm0, %xmm0
   8be6f:      	movups	%xmm0, -0x188(%rbp)
   8be76:      	movups	%xmm0, -0x178(%rbp)
   8be7d:      	movups	%xmm0, -0x168(%rbp)
   8be84:      	movups	%xmm0, -0x158(%rbp)
   8be8b:      	leaq	-0x188(%rbp), %r15
   8be92:      	movq	%r15, %rdi
   8be95:      	movq	%r15, %rsi
   8be98:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8be9d:      	callq	0xb3800 <m34_container_begin>
   8bea2:      	movq	%r15, %rdi
   8bea5:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8beaa:      	movq	%r14, %rdi
   8bead:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8beb2:      	movq	0x18341f(%rip), %r14    # 0x20f2d8 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb+0x18>
   8beb9:      	movq	%r14, %r12
   8bebc:      	negq	%r12
   8bebf:      	leaq	(%r14,%r13), %rax
   8bec3:      	cmpq	$-0x20, %rax
   8bec7:      	setae	%r13b
   8becb:      	leaq	0x1f(%r14), %r15
   8becf:      	andq	%r12, %r15
   8bed2:      	movq	%fs:0x0, %rax
   8bedb:      	leaq	-0x8(%rax), %rax
   8bee2:      	movq	(%rax), %rax
   8bee5:      	movq	(%rax), %rcx
   8bee8:      	addq	%rcx, %r14
   8beeb:      	decq	%r14
   8beee:      	andq	%r12, %r14
   8bef1:      	leaq	(%r14,%r15), %rcx
   8bef5:      	testq	%r14, %r14
   8bef8:      	sete	%dl
   8befb:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   8bf02:      	setae	%sil
   8bf06:      	cmpq	0x8(%rax), %rcx
   8bf0a:      	seta	%dil
   8bf0e:      	cmpq	$0x0, 0x18343a(%rip)    # 0x20f350 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb+0x90>
   8bf16:      	setne	%r8b
   8bf1a:      	orb	%sil, %r8b
   8bf1d:      	orb	%dl, %r8b
   8bf20:      	orb	%r13b, %r8b
   8bf23:      	orb	%dil, %r8b
   8bf26:      	jne	0x8bf3f <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x15f>
   8bf28:      	movq	%rcx, (%rax)
   8bf2b:      	leaq	0x18338e(%rip), %rsi    # 0x20f2c0 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb>
   8bf32:      	movq	%r14, %rdi
   8bf35:      	movq	%r15, %rdx
   8bf38:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8bf3d:      	jmp	0x8bf53 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x173>
   8bf3f:      	leaq	0x18337a(%rip), %rdi    # 0x20f2c0 <scoop$1$td$8acb9318ae639413abfbb0ea848bc137c666b08432b50d15279ee926e424bbbb>
   8bf46:      	movl	$0x20, %esi
   8bf4b:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8bf50:      	movq	%rax, %r14
   8bf53:      	movq	%r14, -0x30(%rbp)
   8bf57:      	movq	%r14, %rdi
   8bf5a:      	movq	%rbx, %rsi
   8bf5d:      	callq	0x95680 <scoop$1$cb$8492f4f1e40312d9a91dc01340f047a06736ed40daece5034713c3ce75f36d3a>
   8bf62:      	movq	-0x30(%rbp), %rax
   8bf66:      	movq	%rax, -0xd0(%rbp)
   8bf6d:      	movq	%rax, -0x38(%rbp)
   8bf71:      	leaq	-0x38(%rbp), %rax
   8bf75:      	movq	%rax, -0x70(%rbp)
   8bf79:      	leaq	0x16bd40(%rip), %rax    # 0x1f7cc0 <scoop$1$bs$8e0c9f84dcb272dd6e27fb1eb4673c930eeecf18c61bbc1619e4cd7998fb3608>
   8bf80:      	movq	%rax, -0x68(%rbp)
   8bf84:      	xorps	%xmm0, %xmm0
   8bf87:      	movups	%xmm0, -0x100(%rbp)
   8bf8e:      	movq	$0x0, -0xf0(%rbp)
   8bf99:      	leaq	-0x100(%rbp), %rbx
   8bfa0:      	leaq	-0x70(%rbp), %rsi
   8bfa4:      	movl	$0x1, %edx
   8bfa9:      	movq	%rbx, %rdi
   8bfac:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8bfb1:      	xorps	%xmm0, %xmm0
   8bfb4:      	movups	%xmm0, -0x1c8(%rbp)
   8bfbb:      	movups	%xmm0, -0x1b8(%rbp)
   8bfc2:      	movups	%xmm0, -0x1a8(%rbp)
   8bfc9:      	movups	%xmm0, -0x198(%rbp)
   8bfd0:      	leaq	-0x1c8(%rbp), %r14
   8bfd7:      	movq	%r14, %rdi
   8bfda:      	movq	%r14, %rsi
   8bfdd:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8bfe2:      	xorl	%edi, %edi
   8bfe4:      	callq	0xb38a0 <m34_container_end>
   8bfe9:      	movq	%r14, %rdi
   8bfec:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8bff1:      	movq	-0x38(%rbp), %r15
   8bff5:      	movq	%rbx, %rdi
   8bff8:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8bffd:      	movq	%r15, -0x38(%rbp)
   8c001:      	movq	%r15, -0x50(%rbp)
   8c005:      	movq	0x1852dc(%rip), %rbx    # 0x2112e8 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x18>
   8c00c:      	movq	%rbx, %r12
   8c00f:      	negq	%r12
   8c012:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8c01c:      	addq	%rbx, %rax
   8c01f:      	cmpq	$-0x18, %rax
   8c023:      	setae	%r13b
   8c027:      	leaq	0x17(%rbx), %r14
   8c02b:      	andq	%r12, %r14
   8c02e:      	movq	%fs:0x0, %rax
   8c037:      	leaq	-0x8(%rax), %rax
   8c03e:      	movq	(%rax), %rax
   8c041:      	movq	(%rax), %rcx
   8c044:      	addq	%rcx, %rbx
   8c047:      	decq	%rbx
   8c04a:      	andq	%r12, %rbx
   8c04d:      	leaq	(%rbx,%r14), %rcx
   8c051:      	testq	%rbx, %rbx
   8c054:      	sete	%dl
   8c057:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   8c05e:      	setae	%sil
   8c062:      	cmpq	0x8(%rax), %rcx
   8c066:      	seta	%dil
   8c06a:      	cmpq	$0x0, 0x1852ee(%rip)    # 0x211360 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x90>
   8c072:      	setne	%r8b
   8c076:      	orb	%sil, %r8b
   8c079:      	orb	%dl, %r8b
   8c07c:      	orb	%r13b, %r8b
   8c07f:      	orb	%dil, %r8b
   8c082:      	jne	0x8c09b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x2bb>
   8c084:      	movq	%rcx, (%rax)
   8c087:      	leaq	0x185242(%rip), %rsi    # 0x2112d0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8c08e:      	movq	%rbx, %rdi
   8c091:      	movq	%r14, %rdx
   8c094:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8c099:      	jmp	0x8c0cb <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x2eb>
   8c09b:      	movq	-0x38(%rbp), %rax
   8c09f:      	movq	%rax, -0x30(%rbp)
   8c0a3:      	movq	%r15, -0x40(%rbp)
   8c0a7:      	leaq	0x185222(%rip), %rdi    # 0x2112d0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8c0ae:      	movl	$0x18, %esi
   8c0b3:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8c0b8:      	movq	%rax, %rbx
   8c0bb:      	movq	-0x40(%rbp), %rax
   8c0bf:      	movq	-0x30(%rbp), %rcx
   8c0c3:      	movq	%rcx, -0x38(%rbp)
   8c0c7:      	movq	%rax, -0x50(%rbp)
   8c0cb:      	movl	$0x7, 0x10(%rbx)
   8c0d2:      	movq	-0x38(%rbp), %rax
   8c0d6:      	movq	-0x50(%rbp), %rdi
   8c0da:      	movq	%rbx, -0x40(%rbp)
   8c0de:      	movq	%rdi, -0x30(%rbp)
   8c0e2:      	movq	%rax, -0x58(%rbp)
   8c0e6:      	movq	%rbx, %rsi
   8c0e9:      	callq	0x90ea0 <scoop$1$cb$0f1041b9eb91408efa4bc77ccb6f369d37f6c7070c63fb1f153f47cdf3e294f8>
   8c0ee:      	movq	-0x58(%rbp), %rax
   8c0f2:      	movq	-0x40(%rbp), %rcx
   8c0f6:      	movq	-0x30(%rbp), %rdx
   8c0fa:      	movq	%rax, -0x38(%rbp)
   8c0fe:      	movq	%rdx, -0x50(%rbp)
   8c102:      	movq	%rcx, -0xc8(%rbp)
   8c109:      	movq	%rax, -0x48(%rbp)
   8c10d:      	leaq	-0x38(%rbp), %rax
   8c111:      	movq	%rax, -0xb0(%rbp)
   8c118:      	leaq	0x16bbd1(%rip), %rax    # 0x1f7cf0 <scoop$1$bs$c09917c0399bcaa0cd2d4061806fd6aad7b455d45380d1e931b4622784220a26>
   8c11f:      	movq	%rax, -0xa8(%rbp)
   8c126:      	leaq	-0x48(%rbp), %rax
   8c12a:      	movq	%rax, -0xa0(%rbp)
   8c131:      	leaq	0x16bbc8(%rip), %rax    # 0x1f7d00 <scoop$1$bs$dd63e8327353c664d559879a00238d151851f41dd9ba9521794aa396f7f4d1e8>
   8c138:      	movq	%rax, -0x98(%rbp)
   8c13f:      	xorps	%xmm0, %xmm0
   8c142:      	movups	%xmm0, -0x148(%rbp)
   8c149:      	movq	$0x0, -0x138(%rbp)
   8c154:      	leaq	-0x148(%rbp), %rbx
   8c15b:      	leaq	-0xb0(%rbp), %rsi
   8c162:      	movl	$0x2, %edx
   8c167:      	movq	%rbx, %rdi
   8c16a:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8c16f:      	xorps	%xmm0, %xmm0
   8c172:      	movups	%xmm0, -0x288(%rbp)
   8c179:      	movups	%xmm0, -0x278(%rbp)
   8c180:      	movups	%xmm0, -0x268(%rbp)
   8c187:      	movups	%xmm0, -0x258(%rbp)
   8c18e:      	leaq	-0x288(%rbp), %r14
   8c195:      	movq	%r14, %rdi
   8c198:      	movq	%r14, %rsi
   8c19b:      	callq	0x9a5e0 <scoop_rt_enter_native_borrowed>
   8c1a0:      	movq	-0x48(%rbp), %rdi
   8c1a4:      	callq	0xb39d0 <m34_container_layout>
   8c1a9:      	movq	%r14, %rdi
   8c1ac:      	callq	0x9d130 <scoop_rt_leave_native_borrowed>
   8c1b1:      	movq	-0x38(%rbp), %r14
   8c1b5:      	movq	-0x48(%rbp), %r15
   8c1b9:      	movq	%rbx, %rdi
   8c1bc:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8c1c1:      	movq	%r14, -0x38(%rbp)
   8c1c5:      	movq	%r15, -0x48(%rbp)
   8c1c9:      	movq	%r14, -0x30(%rbp)
   8c1cd:      	callq	0x9a3e0 <scoop_rt_gc_collect>
   8c1d2:      	movq	-0x30(%rbp), %rax
   8c1d6:      	movq	%rax, -0x38(%rbp)
   8c1da:      	xorl	%ebx, %ebx
   8c1dc:      	xorl	%r14d, %r14d
   8c1df:      	nop
   8c1e0:      	leaq	0x1d6429(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8c1e7:      	movq	(%rax), %rax
   8c1ea:      	leaq	0x1d6427(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8c1f1:      	movl	(%rcx), %esi
   8c1f3:      	movq	-0x60(%rbp), %rcx
   8c1f7:      	movl	(%rcx), %edx
   8c1f9:      	movq	0x8(%rcx), %rcx
   8c1fd:      	testl	%esi, %esi
   8c1ff:      	jne	0x8c20b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x42b>
   8c201:      	cmpl	$0x1, %edx
   8c204:      	jne	0x8c20b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x42b>
   8c206:      	cmpq	%rax, %rcx
   8c209:      	je	0x8c220 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x440>
   8c20b:      	movq	-0x38(%rbp), %rax
   8c20f:      	movq	%rax, -0x30(%rbp)
   8c213:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8c218:      	movq	-0x30(%rbp), %rax
   8c21c:      	movq	%rax, -0x38(%rbp)
   8c220:      	cmpl	$0x5, %r14d
   8c224:      	jge	0x8c38a <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5aa>
   8c22a:      	leaq	-0x38(%rbp), %r12
   8c22e:      	movq	%r12, -0x80(%rbp)
   8c232:      	leaq	0x16ba97(%rip), %rax    # 0x1f7cd0 <scoop$1$bs$9ac53e80ac7e59a3c5319bf5bce7949e6233c62f549f27a1956533d23e19898b>
   8c239:      	movq	%rax, -0x78(%rbp)
   8c23d:      	xorps	%xmm0, %xmm0
   8c240:      	movups	%xmm0, -0x118(%rbp)
   8c247:      	movq	$0x0, -0x108(%rbp)
   8c252:      	movl	$0x1, %edx
   8c257:      	leaq	-0x118(%rbp), %r15
   8c25e:      	movq	%r15, %rdi
   8c261:      	leaq	-0x80(%rbp), %rsi
   8c265:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8c26a:      	xorps	%xmm0, %xmm0
   8c26d:      	movups	%xmm0, -0x208(%rbp)
   8c274:      	movups	%xmm0, -0x1f8(%rbp)
   8c27b:      	movups	%xmm0, -0x1e8(%rbp)
   8c282:      	movups	%xmm0, -0x1d8(%rbp)
   8c289:      	leaq	-0x208(%rbp), %r13
   8c290:      	movq	%r13, %rdi
   8c293:      	movq	%r13, %rsi
   8c296:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8c29b:      	callq	0xb3800 <m34_container_begin>
   8c2a0:      	movq	%r13, %rdi
   8c2a3:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8c2a8:      	movq	-0x38(%rbp), %r13
   8c2ac:      	movq	%r15, %rdi
   8c2af:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8c2b4:      	movq	%r13, -0x38(%rbp)
   8c2b8:      	movq	%r13, -0x30(%rbp)
   8c2bc:      	callq	0x9a3e0 <scoop_rt_gc_collect>
   8c2c1:      	movq	-0x30(%rbp), %rax
   8c2c5:      	movq	%rax, -0x38(%rbp)
   8c2c9:      	movq	%r12, -0x90(%rbp)
   8c2d0:      	leaq	0x16ba09(%rip), %rax    # 0x1f7ce0 <scoop$1$bs$eb451bdf32bc9cd18e03f449cf70d4cdd6eed366aebdd7c65a6ec43f7d8ffb1c>
   8c2d7:      	movq	%rax, -0x88(%rbp)
   8c2de:      	xorps	%xmm0, %xmm0
   8c2e1:      	movups	%xmm0, -0x130(%rbp)
   8c2e8:      	movq	$0x0, -0x120(%rbp)
   8c2f3:      	movl	$0x1, %edx
   8c2f8:      	leaq	-0x130(%rbp), %r15
   8c2ff:      	movq	%r15, %rdi
   8c302:      	leaq	-0x90(%rbp), %rsi
   8c309:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8c30e:      	xorps	%xmm0, %xmm0
   8c311:      	movups	%xmm0, -0x248(%rbp)
   8c318:      	movups	%xmm0, -0x238(%rbp)
   8c31f:      	movups	%xmm0, -0x228(%rbp)
   8c326:      	movups	%xmm0, -0x218(%rbp)
   8c32d:      	leaq	-0x248(%rbp), %r12
   8c334:      	movq	%r12, %rdi
   8c337:      	movq	%r12, %rsi
   8c33a:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8c33f:      	movl	$0x4, %edi
   8c344:      	callq	0xb38a0 <m34_container_end>
   8c349:      	movq	%r12, %rdi
   8c34c:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8c351:      	movq	-0x38(%rbp), %r13
   8c355:      	movq	%r15, %rdi
   8c358:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8c35d:      	movq	%r13, -0x38(%rbp)
   8c361:      	cmpq	$0x0, 0x18(%r13)
   8c366:      	jle	0x8c3ae <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x5ce>
   8c368:      	movq	0x10(%r13), %rax
   8c36c:      	cmpq	$0x0, 0x10(%rax)
   8c371:      	jle	0x8c44b <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x66b>
   8c377:      	movq	0x18(%rax), %rax
   8c37b:      	movslq	0x10(%rax), %rax
   8c37f:      	addq	%rax, %rbx
   8c382:      	incl	%r14d
   8c385:      	jmp	0x8c1e0 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x400>
   8c38a:      	xorl	%edi, %edi
   8c38c:      	cmpq	$0x23, %rbx
   8c390:      	sete	%dil
   8c394:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8c399:      	movq	%rbx, %rax
   8c39c:      	addq	$0x268, %rsp            # imm = 0x268
   8c3a3:      	popq	%rbx
   8c3a4:      	popq	%r12
   8c3a6:      	popq	%r13
   8c3a8:      	popq	%r14
   8c3aa:      	popq	%r15
   8c3ac:      	popq	%rbp
   8c3ad:      	retq
   8c3ae:      	movq	0xb05a3(%rip), %rbx     # 0x13c958 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9+0x18>
   8c3b5:      	movq	%rbx, %r15
   8c3b8:      	negq	%r15
   8c3bb:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8c3c5:      	addq	%rbx, %rax
   8c3c8:      	cmpq	$-0x18, %rax
   8c3cc:      	setb	%r12b
   8c3d0:      	leaq	0x17(%rbx), %r14
   8c3d4:      	andq	%r15, %r14
   8c3d7:      	movq	%fs:0x0, %rax
   8c3e0:      	leaq	-0x8(%rax), %rax
   8c3e7:      	movq	(%rax), %rax
   8c3ea:      	movq	(%rax), %rcx
   8c3ed:      	addq	%rcx, %rbx
   8c3f0:      	decq	%rbx
   8c3f3:      	andq	%r15, %rbx
   8c3f6:      	leaq	(%rbx,%r14), %rcx
   8c3fa:      	testq	%rbx, %rbx
   8c3fd:      	setne	%dl
   8c400:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   8c407:      	setb	%sil
   8c40b:      	cmpq	0x8(%rax), %rcx
   8c40f:      	setbe	%dil
   8c413:      	cmpq	$0x0, 0xb05b5(%rip)     # 0x13c9d0 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9+0x90>
   8c41b:      	sete	%r8b
   8c41f:      	andb	%sil, %r8b
   8c422:      	andb	%dl, %r8b
   8c425:      	andb	%dil, %r8b
   8c428:      	testb	%r12b, %r8b
   8c42b:      	je	0x8c4e1 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x701>
   8c431:      	movq	%rcx, (%rax)
   8c434:      	leaq	0xb0505(%rip), %rsi     # 0x13c940 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9>
   8c43b:      	movq	%rbx, %rdi
   8c43e:      	movq	%r14, %rdx
   8c441:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8c446:      	jmp	0x8c4f5 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x715>
   8c44b:      	movq	0xb0506(%rip), %rbx     # 0x13c958 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9+0x18>
   8c452:      	movq	%rbx, %r15
   8c455:      	negq	%r15
   8c458:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8c462:      	addq	%rbx, %rax
   8c465:      	cmpq	$-0x18, %rax
   8c469:      	setb	%r12b
   8c46d:      	leaq	0x17(%rbx), %r14
   8c471:      	andq	%r15, %r14
   8c474:      	movq	%fs:0x0, %rax
   8c47d:      	leaq	-0x8(%rax), %rax
   8c484:      	movq	(%rax), %rax
   8c487:      	movq	(%rax), %rcx
   8c48a:      	addq	%rcx, %rbx
   8c48d:      	decq	%rbx
   8c490:      	andq	%r15, %rbx
   8c493:      	leaq	(%rbx,%r14), %rcx
   8c497:      	testq	%rbx, %rbx
   8c49a:      	setne	%dl
   8c49d:      	cmpq	$0x7f81, %r14           # imm = 0x7F81
   8c4a4:      	setb	%sil
   8c4a8:      	cmpq	0x8(%rax), %rcx
   8c4ac:      	setbe	%dil
   8c4b0:      	cmpq	$0x0, 0xb0518(%rip)     # 0x13c9d0 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9+0x90>
   8c4b8:      	sete	%r8b
   8c4bc:      	andb	%sil, %r8b
   8c4bf:      	andb	%dl, %r8b
   8c4c2:      	andb	%dil, %r8b
   8c4c5:      	testb	%r12b, %r8b
   8c4c8:      	je	0x8c511 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x731>
   8c4ca:      	movq	%rcx, (%rax)
   8c4cd:      	leaq	0xb046c(%rip), %rsi     # 0x13c940 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9>
   8c4d4:      	movq	%rbx, %rdi
   8c4d7:      	movq	%r14, %rdx
   8c4da:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8c4df:      	jmp	0x8c525 <scoop$1$cb$322933305ec04b939607b70b0c448355c5fdb6aac593906766c7a1c9930e7ee2+0x745>
   8c4e1:      	leaq	0xb0458(%rip), %rdi     # 0x13c940 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9>
   8c4e8:      	movl	$0x18, %esi
   8c4ed:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8c4f2:      	movq	%rax, %rbx
   8c4f5:      	movq	%rbx, -0x30(%rbp)
   8c4f9:      	movq	%rbx, %rdi
   8c4fc:      	callq	0x6b950 <scoop$1$cb$d88a9e35cab652ce2c186f206c545e4974be02e613922f4df03e263678dbd5aa>
   8c501:      	movq	-0x30(%rbp), %rdi
   8c505:      	movq	%rdi, -0xb8(%rbp)
   8c50c:      	callq	0xa51d0 <scoop_rt_throw>
   8c511:      	leaq	0xb0428(%rip), %rdi     # 0x13c940 <scoop$1$td$280daf3721d646c4f2b20d11d7129e2aadfd4014f6d24b0bc9fc66152463a0d9>
   8c518:      	movl	$0x18, %esi
   8c51d:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8c522:      	movq	%rax, %rbx
   8c525:      	movq	%rbx, -0x30(%rbp)
   8c529:      	movq	%rbx, %rdi
   8c52c:      	callq	0x6b950 <scoop$1$cb$d88a9e35cab652ce2c186f206c545e4974be02e613922f4df03e263678dbd5aa>
   8c531:      	movq	-0x30(%rbp), %rdi
   8c535:      	movq	%rdi, -0xc0(%rbp)
   8c53c:      	callq	0xa51d0 <scoop_rt_throw>

; buildStrings, MIR fn2, LIR scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000007ec60 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc>:
   7ec60:      	pushq	%rbp
   7ec61:      	movq	%rsp, %rbp
   7ec64:      	pushq	%r15
   7ec66:      	pushq	%r14
   7ec68:      	pushq	%r13
   7ec6a:      	pushq	%r12
   7ec6c:      	pushq	%rbx
   7ec6d:      	subq	$0x1f8, %rsp            # imm = 0x1F8
   7ec74:      	movq	%rdi, %rbx
   7ec77:      	movq	%fs:0x0, %rax
   7ec80:      	leaq	-0x10(%rax), %rax
   7ec87:      	movq	(%rax), %r12
   7ec8a:      	leaq	0x1e397f(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   7ec91:      	movq	(%rax), %rax
   7ec94:      	leaq	0x1e397d(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   7ec9b:      	movl	(%rcx), %esi
   7ec9d:      	movl	(%r12), %edx
   7eca1:      	movq	0x8(%r12), %rcx
   7eca6:      	testl	%esi, %esi
   7eca8:      	jne	0x7ecb4 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x54>
   7ecaa:      	cmpl	$0x1, %edx
   7ecad:      	jne	0x7ecb4 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x54>
   7ecaf:      	cmpq	%rax, %rcx
   7ecb2:      	je	0x7ecb9 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x59>
   7ecb4:      	callq	0x9a3c0 <scoop_rt_safepoint>
   7ecb9:      	movq	0xbc438(%rip), %r14     # 0x13b0f8 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192+0x18>
   7ecc0:      	movq	%r14, %r13
   7ecc3:      	negq	%r13
   7ecc6:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   7ecd0:      	addq	%r14, %rax
   7ecd3:      	cmpq	$-0x18, %rax
   7ecd7:      	setae	-0x39(%rbp)
   7ecdb:      	leaq	0x17(%r14), %r15
   7ecdf:      	andq	%r13, %r15
   7ece2:      	movq	%fs:0x0, %rax
   7eceb:      	leaq	-0x8(%rax), %rax
   7ecf2:      	movq	(%rax), %rax
   7ecf5:      	movq	(%rax), %rcx
   7ecf8:      	addq	%rcx, %r14
   7ecfb:      	decq	%r14
   7ecfe:      	andq	%r13, %r14
   7ed01:      	leaq	(%r14,%r15), %rcx
   7ed05:      	testq	%r14, %r14
   7ed08:      	sete	%dl
   7ed0b:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   7ed12:      	setae	%sil
   7ed16:      	cmpq	0x8(%rax), %rcx
   7ed1a:      	seta	%dil
   7ed1e:      	cmpq	$0x0, 0xbc44a(%rip)     # 0x13b170 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192+0x90>
   7ed26:      	setne	%r8b
   7ed2a:      	orb	%sil, %r8b
   7ed2d:      	orb	%dl, %r8b
   7ed30:      	orb	-0x39(%rbp), %r8b
   7ed34:      	orb	%dil, %r8b
   7ed37:      	jne	0x7ed50 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0xf0>
   7ed39:      	movq	%rcx, (%rax)
   7ed3c:      	leaq	0xbc39d(%rip), %rsi     # 0x13b0e0 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192>
   7ed43:      	movq	%r14, %rdi
   7ed46:      	movq	%r15, %rdx
   7ed49:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   7ed4e:      	jmp	0x7ed64 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x104>
   7ed50:      	leaq	0xbc389(%rip), %rdi     # 0x13b0e0 <scoop$1$td$292374ef2a48ea8668ff1a6b405ebdf9f9ab56a30dce60e6ed71444b401a5192>
   7ed57:      	movl	$0x18, %esi
   7ed5c:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   7ed61:      	movq	%rax, %r14
   7ed64:      	movq	%r14, -0x30(%rbp)
   7ed68:      	movq	%r14, %rdi
   7ed6b:      	callq	0x57280 <scoop$1$cb$7fd0de94188adc1500f086a2b0e914cb1973f941c791324bcba453f6b7bbfd25>
   7ed70:      	movq	-0x30(%rbp), %rax
   7ed74:      	movq	%rax, -0xb8(%rbp)
   7ed7b:      	movq	%rax, -0x38(%rbp)
   7ed7f:      	leaq	-0x38(%rbp), %rax
   7ed83:      	movq	%rax, -0x78(%rbp)
   7ed87:      	leaq	0x156c72(%rip), %rax    # 0x1d5a00 <scoop$1$bs$155609da96eba8ad9ba5a41acdf9f0ce21d17cff97c4b4bd58f560ff46fef257>
   7ed8e:      	movq	%rax, -0x70(%rbp)
   7ed92:      	xorps	%xmm0, %xmm0
   7ed95:      	movups	%xmm0, -0xd0(%rbp)
   7ed9c:      	movq	$0x0, -0xc0(%rbp)
   7eda7:      	leaq	-0xd0(%rbp), %r14
   7edae:      	leaq	-0x78(%rbp), %rsi
   7edb2:      	movl	$0x1, %edx
   7edb7:      	movq	%r14, %rdi
   7edba:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7edbf:      	xorps	%xmm0, %xmm0
   7edc2:      	movups	%xmm0, -0x158(%rbp)
   7edc9:      	movups	%xmm0, -0x148(%rbp)
   7edd0:      	movups	%xmm0, -0x138(%rbp)
   7edd7:      	movups	%xmm0, -0x128(%rbp)
   7edde:      	leaq	-0x158(%rbp), %r15
   7ede5:      	movq	%r15, %rdi
   7ede8:      	movq	%r15, %rsi
   7edeb:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7edf0:      	callq	0xb3800 <m34_container_begin>
   7edf5:      	movq	%r15, %rdi
   7edf8:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7edfd:      	movq	-0x38(%rbp), %r15
   7ee01:      	movq	%r14, %rdi
   7ee04:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7ee09:      	movq	%r15, -0x38(%rbp)
   7ee0d:      	xorl	%r14d, %r14d
   7ee10:      	xorl	%r15d, %r15d
   7ee13:      	leaq	0x1e37f6(%rip), %r13    # 0x262610 <scoop_thread_gc_epoch>
   7ee1a:      	nopw	(%rax,%rax)
   7ee20:      	movq	(%r13), %rax
   7ee24:      	leaq	0x1e37ed(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   7ee2b:      	movl	(%rcx), %esi
   7ee2d:      	movl	(%r12), %edx
   7ee31:      	movq	0x8(%r12), %rcx
   7ee36:      	testl	%esi, %esi
   7ee38:      	jne	0x7ee44 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1e4>
   7ee3a:      	cmpl	$0x1, %edx
   7ee3d:      	jne	0x7ee44 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1e4>
   7ee3f:      	cmpq	%rax, %rcx
   7ee42:      	je	0x7ee59 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1f9>
   7ee44:      	movq	-0x38(%rbp), %rax
   7ee48:      	movq	%rax, -0x30(%rbp)
   7ee4c:      	callq	0x9a3c0 <scoop_rt_safepoint>
   7ee51:      	movq	-0x30(%rbp), %rax
   7ee55:      	movq	%rax, -0x38(%rbp)
   7ee59:      	cmpq	%rbx, %r14
   7ee5c:      	jge	0x7eece <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x26e>
   7ee5e:      	movq	-0x38(%rbp), %rax
   7ee62:      	movq	%rax, -0x30(%rbp)
   7ee66:      	movq	%r14, %rdi
   7ee69:      	callq	0x49100 <scoop$1$cb$ef730a437b587319fa729136c1c496c97d793c073731fe4cbfd67870ec8e2cc0>
   7ee6e:      	movq	-0x30(%rbp), %rdi
   7ee72:      	movq	%rdi, -0x38(%rbp)
   7ee76:      	movq	%rax, -0x58(%rbp)
   7ee7a:      	movq	%rdi, -0x68(%rbp)
   7ee7e:      	movq	%rax, -0x60(%rbp)
   7ee82:      	movq	%rax, -0x50(%rbp)
   7ee86:      	movq	%rax, %rsi
   7ee89:      	callq	0x47c90 <scoop$1$cb$bf2a768a0cb1375f89a63ed498891d7c57dab520218a10fddd14a0197c926150>
   7ee8e:      	movq	-0x50(%rbp), %rdi
   7ee92:      	movq	-0x30(%rbp), %rax
   7ee96:      	movq	%rax, -0x38(%rbp)
   7ee9a:      	movq	%rdi, -0x58(%rbp)
   7ee9e:      	movq	%rax, -0x68(%rbp)
   7eea2:      	movq	%rdi, -0x60(%rbp)
   7eea6:      	movq	-0x38(%rbp), %rax
   7eeaa:      	movq	%rax, -0x30(%rbp)
   7eeae:      	callq	0x57c90 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7eeb3:      	movq	-0x50(%rbp), %rcx
   7eeb7:      	movq	-0x30(%rbp), %rdx
   7eebb:      	movq	%rdx, -0x38(%rbp)
   7eebf:      	movq	%rcx, -0x58(%rbp)
   7eec3:      	addq	%rax, %r15
   7eec6:      	incq	%r14
   7eec9:      	jmp	0x7ee20 <scoop$1$cb$a964f52d9113c3c1cc9138beabd1bdc8171bd1645e6bcca0141726a3005854cc+0x1c0>
   7eece:      	leaq	-0x38(%rbp), %r12
   7eed2:      	movq	%r12, -0x88(%rbp)
   7eed9:      	leaq	0x156b30(%rip), %rax    # 0x1d5a10 <scoop$1$bs$3339f6d95e6e9e5deab0576cb1ad89976cd8650888695dcb4a34160b58f19e84>
   7eee0:      	movq	%rax, -0x80(%rbp)
   7eee4:      	xorps	%xmm0, %xmm0
   7eee7:      	movups	%xmm0, -0xe8(%rbp)
   7eeee:      	movq	$0x0, -0xd8(%rbp)
   7eef9:      	leaq	-0xe8(%rbp), %rbx
   7ef00:      	leaq	-0x88(%rbp), %rsi
   7ef07:      	movl	$0x1, %edx
   7ef0c:      	movq	%rbx, %rdi
   7ef0f:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7ef14:      	xorps	%xmm0, %xmm0
   7ef17:      	movups	%xmm0, -0x198(%rbp)
   7ef1e:      	movups	%xmm0, -0x188(%rbp)
   7ef25:      	movups	%xmm0, -0x178(%rbp)
   7ef2c:      	movups	%xmm0, -0x168(%rbp)
   7ef33:      	leaq	-0x198(%rbp), %r14
   7ef3a:      	movq	%r14, %rdi
   7ef3d:      	movq	%r14, %rsi
   7ef40:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7ef45:      	movl	$0x1, %edi
   7ef4a:      	callq	0xb38a0 <m34_container_end>
   7ef4f:      	movq	%r14, %rdi
   7ef52:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7ef57:      	movq	-0x38(%rbp), %r14
   7ef5b:      	movq	%rbx, %rdi
   7ef5e:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7ef63:      	movq	%r14, -0x38(%rbp)
   7ef67:      	movq	%r12, -0x98(%rbp)
   7ef6e:      	leaq	0x156aab(%rip), %rax    # 0x1d5a20 <scoop$1$bs$6d1b78b7f57c69160f9b8c16c3b0df8401dc0c69fb204db37f962d1d7a3491e5>
   7ef75:      	movq	%rax, -0x90(%rbp)
   7ef7c:      	xorps	%xmm0, %xmm0
   7ef7f:      	movups	%xmm0, -0x100(%rbp)
   7ef86:      	movq	$0x0, -0xf0(%rbp)
   7ef91:      	leaq	-0x100(%rbp), %rbx
   7ef98:      	leaq	-0x98(%rbp), %rsi
   7ef9f:      	movl	$0x1, %edx
   7efa4:      	movq	%rbx, %rdi
   7efa7:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7efac:      	xorps	%xmm0, %xmm0
   7efaf:      	movups	%xmm0, -0x1d8(%rbp)
   7efb6:      	movups	%xmm0, -0x1c8(%rbp)
   7efbd:      	movups	%xmm0, -0x1b8(%rbp)
   7efc4:      	movups	%xmm0, -0x1a8(%rbp)
   7efcb:      	leaq	-0x1d8(%rbp), %r14
   7efd2:      	movq	%r14, %rdi
   7efd5:      	movq	%r14, %rsi
   7efd8:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7efdd:      	callq	0xb3800 <m34_container_begin>
   7efe2:      	movq	%r14, %rdi
   7efe5:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7efea:      	movq	-0x38(%rbp), %r14
   7efee:      	movq	%rbx, %rdi
   7eff1:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7eff6:      	movq	%r14, -0x38(%rbp)
   7effa:      	movq	%r14, -0x30(%rbp)
   7effe:      	movq	%r14, %rdi
   7f001:      	callq	0x41cf0 <scoop$1$cb$22ffc90119bb7e713d02ab7beca1a040fd100be545734e1a344db51e6165d4ce>
   7f006:      	movq	-0x30(%rbp), %rcx
   7f00a:      	movq	%rcx, -0xb0(%rbp)
   7f011:      	movq	%rax, -0x48(%rbp)
   7f015:      	leaq	-0x48(%rbp), %rax
   7f019:      	movq	%rax, -0xa8(%rbp)
   7f020:      	leaq	0x156a09(%rip), %rax    # 0x1d5a30 <scoop$1$bs$9e649712ef74d131942a46acef503ed4d7c9d93cdd1e31bb977b6bbf6739b592>
   7f027:      	movq	%rax, -0xa0(%rbp)
   7f02e:      	xorps	%xmm0, %xmm0
   7f031:      	movups	%xmm0, -0x118(%rbp)
   7f038:      	movq	$0x0, -0x108(%rbp)
   7f043:      	leaq	-0x118(%rbp), %rbx
   7f04a:      	leaq	-0xa8(%rbp), %rsi
   7f051:      	movl	$0x1, %edx
   7f056:      	movq	%rbx, %rdi
   7f059:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7f05e:      	xorps	%xmm0, %xmm0
   7f061:      	movups	%xmm0, -0x218(%rbp)
   7f068:      	movups	%xmm0, -0x208(%rbp)
   7f06f:      	movups	%xmm0, -0x1f8(%rbp)
   7f076:      	movups	%xmm0, -0x1e8(%rbp)
   7f07d:      	leaq	-0x218(%rbp), %r14
   7f084:      	movq	%r14, %rdi
   7f087:      	movq	%r14, %rsi
   7f08a:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7f08f:      	movl	$0x2, %edi
   7f094:      	callq	0xb38a0 <m34_container_end>
   7f099:      	movq	%r14, %rdi
   7f09c:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7f0a1:      	movq	-0x48(%rbp), %r14
   7f0a5:      	movq	%rbx, %rdi
   7f0a8:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7f0ad:      	movq	%r14, -0x48(%rbp)
   7f0b1:      	movq	%r14, -0x30(%rbp)
   7f0b5:      	movq	%r14, %rdi
   7f0b8:      	callq	0x57c90 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7f0bd:      	movq	-0x30(%rbp), %rcx
   7f0c1:      	movq	%rcx, -0x48(%rbp)
   7f0c5:      	xorl	%edi, %edi
   7f0c7:      	cmpq	%r15, %rax
   7f0ca:      	sete	%dil
   7f0ce:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   7f0d3:      	movq	-0x30(%rbp), %rdi
   7f0d7:      	movq	%rdi, -0x48(%rbp)
   7f0db:      	callq	0x57c90 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   7f0e0:      	movq	-0x30(%rbp), %rcx
   7f0e4:      	movq	%rcx, -0x48(%rbp)
   7f0e8:      	addq	$0x1f8, %rsp            # imm = 0x1F8
   7f0ef:      	popq	%rbx
   7f0f0:      	popq	%r12
   7f0f2:      	popq	%r13
   7f0f4:      	popq	%r14
   7f0f6:      	popq	%r15
   7f0f8:      	popq	%rbp
   7f0f9:      	retq

; jsonRoundTrip, MIR fn3, LIR scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

0000000000086760 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364>:
   86760:      	pushq	%rbp
   86761:      	movq	%rsp, %rbp
   86764:      	pushq	%r15
   86766:      	pushq	%r14
   86768:      	pushq	%r13
   8676a:      	pushq	%r12
   8676c:      	pushq	%rbx
   8676d:      	subq	$0x3e8, %rsp            # imm = 0x3E8
   86774:      	movq	%rdi, -0x90(%rbp)
   8677b:      	movq	%fs:0x0, %rax
   86784:      	leaq	-0x10(%rax), %rax
   8678b:      	movq	(%rax), %r12
   8678e:      	leaq	0x1dbe7b(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   86795:      	movq	(%rax), %rax
   86798:      	leaq	0x1dbe79(%rip), %rbx    # 0x262618 <scoop_thread_world_phase>
   8679f:      	movl	(%rbx), %esi
   867a1:      	movl	(%r12), %edx
   867a5:      	movq	0x8(%r12), %rcx
   867aa:      	testl	%esi, %esi
   867ac:      	jne	0x867b8 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x58>
   867ae:      	cmpl	$0x1, %edx
   867b1:      	jne	0x867b8 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x58>
   867b3:      	cmpq	%rax, %rcx
   867b6:      	je	0x867bd <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x5d>
   867b8:      	callq	0x9a3c0 <scoop_rt_safepoint>
   867bd:      	movq	0x183a04(%rip), %r14    # 0x20a1c8 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x18>
   867c4:      	movq	%r14, %r13
   867c7:      	negq	%r13
   867ca:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   867d4:      	addq	%r14, %rax
   867d7:      	cmpq	$-0x20, %rax
   867db:      	setae	-0x49(%rbp)
   867df:      	leaq	0x1f(%r14), %r15
   867e3:      	andq	%r13, %r15
   867e6:      	movq	%fs:0x0, %rax
   867ef:      	leaq	-0x8(%rax), %rax
   867f6:      	movq	(%rax), %rax
   867f9:      	movq	(%rax), %rcx
   867fc:      	addq	%rcx, %r14
   867ff:      	decq	%r14
   86802:      	andq	%r13, %r14
   86805:      	leaq	(%r14,%r15), %rcx
   86809:      	testq	%r14, %r14
   8680c:      	sete	%dl
   8680f:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   86816:      	setae	%sil
   8681a:      	cmpq	0x8(%rax), %rcx
   8681e:      	seta	%dil
   86822:      	cmpq	$0x0, 0x183a16(%rip)    # 0x20a240 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x90>
   8682a:      	setne	%r8b
   8682e:      	orb	%sil, %r8b
   86831:      	orb	%dl, %r8b
   86834:      	orb	-0x49(%rbp), %r8b
   86838:      	orb	%dil, %r8b
   8683b:      	jne	0x86854 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0xf4>
   8683d:      	movq	%rcx, (%rax)
   86840:      	leaq	0x183969(%rip), %rsi    # 0x20a1b0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   86847:      	movq	%r14, %rdi
   8684a:      	movq	%r15, %rdx
   8684d:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   86852:      	jmp	0x86868 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x108>
   86854:      	leaq	0x183955(%rip), %rdi    # 0x20a1b0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   8685b:      	movl	$0x20, %esi
   86860:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   86865:      	movq	%rax, %r14
   86868:      	movq	-0x90(%rbp), %rsi
   8686f:      	movq	%r14, -0x30(%rbp)
   86873:      	movq	%r14, %rdi
   86876:      	callq	0x87070 <scoop$1$cb$fee77490ca77ad5e976dbdde0ee39371498372f80a070cf4802c2070dd379995>
   8687b:      	movq	-0x30(%rbp), %rax
   8687f:      	movq	%rax, -0x1c0(%rbp)
   86886:      	movq	%rax, -0x48(%rbp)
   8688a:      	movq	%rax, -0x88(%rbp)
   86891:      	leaq	-0x48(%rbp), %r14
   86895:      	movq	%r14, -0x160(%rbp)
   8689c:      	leaq	0x16373d(%rip), %rax    # 0x1e9fe0 <scoop$1$bs$49c66c70f85e0f0f1ff93ca5b71d3c1326a359c41b10c8f369725fc037bf4ca1>
   868a3:      	movq	%rax, -0x158(%rbp)
   868aa:      	leaq	-0x88(%rbp), %rax
   868b1:      	movq	%rax, -0x150(%rbp)
   868b8:      	leaq	0x163731(%rip), %rax    # 0x1e9ff0 <scoop$1$bs$b2c272680e1595ce40df5d814864efc59018da7269d6283634e06c08d420667f>
   868bf:      	movq	%rax, -0x148(%rbp)
   868c6:      	xorps	%xmm0, %xmm0
   868c9:      	movups	%xmm0, -0x140(%rbp)
   868d0:      	movq	$0x0, -0x130(%rbp)
   868db:      	leaq	-0x140(%rbp), %rdi
   868e2:      	leaq	-0x160(%rbp), %rsi
   868e9:      	movl	$0x2, %edx
   868ee:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   868f3:      	xorps	%xmm0, %xmm0
   868f6:      	movups	%xmm0, -0x290(%rbp)
   868fd:      	movups	%xmm0, -0x280(%rbp)
   86904:      	movups	%xmm0, -0x270(%rbp)
   8690b:      	movups	%xmm0, -0x260(%rbp)
   86912:      	leaq	-0x290(%rbp), %r15
   86919:      	movq	%r15, %rdi
   8691c:      	movq	%r15, %rsi
   8691f:      	callq	0x9a5e0 <scoop_rt_enter_native_borrowed>
   86924:      	movq	-0x88(%rbp), %rdi
   8692b:      	callq	0xb39d0 <m34_container_layout>
   86930:      	movq	%r15, %rdi
   86933:      	callq	0x9d130 <scoop_rt_leave_native_borrowed>
   86938:      	movq	-0x48(%rbp), %r15
   8693c:      	movq	-0x88(%rbp), %r13
   86943:      	leaq	-0x140(%rbp), %rdi
   8694a:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8694f:      	movq	%r15, -0x48(%rbp)
   86953:      	movq	%r13, -0x88(%rbp)
   8695a:      	movq	-0x90(%rbp), %r13
   86961:      	movq	%r14, -0xe8(%rbp)
   86968:      	leaq	0x163691(%rip), %rax    # 0x1ea000 <scoop$1$bs$9483cebc8b7b69e7dcb8c58ffe984236fc15c13b286041d8339c882587a9912a>
   8696f:      	movq	%rax, -0xe0(%rbp)
   86976:      	xorps	%xmm0, %xmm0
   86979:      	movups	%xmm0, -0x1d8(%rbp)
   86980:      	movq	$0x0, -0x1c8(%rbp)
   8698b:      	leaq	-0x1d8(%rbp), %r14
   86992:      	leaq	-0xe8(%rbp), %rsi
   86999:      	movl	$0x1, %edx
   8699e:      	movq	%r14, %rdi
   869a1:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   869a6:      	xorps	%xmm0, %xmm0
   869a9:      	movups	%xmm0, -0x2d0(%rbp)
   869b0:      	movups	%xmm0, -0x2c0(%rbp)
   869b7:      	movups	%xmm0, -0x2b0(%rbp)
   869be:      	movups	%xmm0, -0x2a0(%rbp)
   869c5:      	leaq	-0x2d0(%rbp), %r15
   869cc:      	movq	%r15, %rdi
   869cf:      	movq	%r15, %rsi
   869d2:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   869d7:      	callq	0xb3800 <m34_container_begin>
   869dc:      	movq	%r15, %rdi
   869df:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   869e4:      	movq	-0x48(%rbp), %r15
   869e8:      	movq	%r14, %rdi
   869eb:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   869f0:      	movq	%r15, -0x48(%rbp)
   869f4:      	xorl	%r14d, %r14d
   869f7:      	leaq	0x1dbc12(%rip), %r15    # 0x262610 <scoop_thread_gc_epoch>
   869fe:      	nop
   86a00:      	movq	(%r15), %rax
   86a03:      	movl	(%rbx), %esi
   86a05:      	movl	(%r12), %edx
   86a09:      	movq	0x8(%r12), %rcx
   86a0e:      	testl	%esi, %esi
   86a10:      	jne	0x86a1c <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2bc>
   86a12:      	cmpl	$0x1, %edx
   86a15:      	jne	0x86a1c <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2bc>
   86a17:      	cmpq	%rax, %rcx
   86a1a:      	je	0x86a31 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2d1>
   86a1c:      	movq	-0x48(%rbp), %rax
   86a20:      	movq	%rax, -0x30(%rbp)
   86a24:      	callq	0x9a3c0 <scoop_rt_safepoint>
   86a29:      	movq	-0x30(%rbp), %rax
   86a2d:      	movq	%rax, -0x48(%rbp)
   86a31:      	cmpq	%r13, %r14
   86a34:      	jge	0x86a61 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x301>
   86a36:      	movq	-0x48(%rbp), %rdi
   86a3a:      	movq	%rdi, -0xb8(%rbp)
   86a41:      	movq	%rdi, -0x30(%rbp)
   86a45:      	movl	%r14d, %esi
   86a48:      	callq	0x94010 <scoop$1$cb$6571e66f09cbc869977ca80836b265c26b4c58d100cec7b4dc74f71fda171ea3>
   86a4d:      	movq	-0x30(%rbp), %rax
   86a51:      	movq	%rax, -0x48(%rbp)
   86a55:      	movq	%rax, -0xb8(%rbp)
   86a5c:      	incq	%r14
   86a5f:      	jmp	0x86a00 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x2a0>
   86a61:      	leaq	-0x48(%rbp), %r12
   86a65:      	movq	%r12, -0xf8(%rbp)
   86a6c:      	leaq	0x16359d(%rip), %rax    # 0x1ea010 <scoop$1$bs$d1068a954b40d440779cb882b70d44c0918c4a0476fa778625b9024fdb53e5be>
   86a73:      	movq	%rax, -0xf0(%rbp)
   86a7a:      	xorps	%xmm0, %xmm0
   86a7d:      	movups	%xmm0, -0x1f0(%rbp)
   86a84:      	movq	$0x0, -0x1e0(%rbp)
   86a8f:      	leaq	-0x1f0(%rbp), %r14
   86a96:      	leaq	-0xf8(%rbp), %rsi
   86a9d:      	movl	$0x1, %edx
   86aa2:      	movq	%r14, %rdi
   86aa5:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   86aaa:      	xorps	%xmm0, %xmm0
   86aad:      	movups	%xmm0, -0x310(%rbp)
   86ab4:      	movups	%xmm0, -0x300(%rbp)
   86abb:      	movups	%xmm0, -0x2f0(%rbp)
   86ac2:      	movups	%xmm0, -0x2e0(%rbp)
   86ac9:      	leaq	-0x310(%rbp), %r15
   86ad0:      	movq	%r15, %rdi
   86ad3:      	movq	%r15, %rsi
   86ad6:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   86adb:      	movl	$0x1, %edi
   86ae0:      	callq	0xb38a0 <m34_container_end>
   86ae5:      	movq	%r15, %rdi
   86ae8:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   86aed:      	movq	-0x48(%rbp), %rbx
   86af1:      	movq	%r14, %rdi
   86af4:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   86af9:      	movq	%rbx, -0x48(%rbp)
   86afd:      	movq	%r12, -0x108(%rbp)
   86b04:      	leaq	0x163515(%rip), %rax    # 0x1ea020 <scoop$1$bs$42931e5c32a0faa35dc1de349255af6bcd5cd0f7bf3b4f8212609241135eb98e>
   86b0b:      	movq	%rax, -0x100(%rbp)
   86b12:      	xorps	%xmm0, %xmm0
   86b15:      	movups	%xmm0, -0x208(%rbp)
   86b1c:      	movq	$0x0, -0x1f8(%rbp)
   86b27:      	leaq	-0x208(%rbp), %r14
   86b2e:      	leaq	-0x108(%rbp), %rsi
   86b35:      	movl	$0x1, %edx
   86b3a:      	movq	%r14, %rdi
   86b3d:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   86b42:      	xorps	%xmm0, %xmm0
   86b45:      	movups	%xmm0, -0x350(%rbp)
   86b4c:      	movups	%xmm0, -0x340(%rbp)
   86b53:      	movups	%xmm0, -0x330(%rbp)
   86b5a:      	movups	%xmm0, -0x320(%rbp)
   86b61:      	leaq	-0x350(%rbp), %r15
   86b68:      	movq	%r15, %rdi
   86b6b:      	movq	%r15, %rsi
   86b6e:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   86b73:      	callq	0xb3800 <m34_container_begin>
   86b78:      	movq	%r15, %rdi
   86b7b:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   86b80:      	movq	-0x48(%rbp), %rbx
   86b84:      	movq	%r14, %rdi
   86b87:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   86b8c:      	movq	%rbx, -0x48(%rbp)
   86b90:      	movq	%rbx, -0x30(%rbp)
   86b94:      	callq	0x7c870 <scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
   86b99:      	movq	-0x30(%rbp), %rax
   86b9d:      	movq	%rax, -0x48(%rbp)
   86ba1:      	movq	0x1db9d8(%rip), %rax    # 0x262580 <scoop$1$ss$026945f606bf54daf4171a1ecb8b4384e089880c55f3eb80fb1b8fa8a5fad2ad>
   86ba8:      	movq	%rax, -0x38(%rbp)
   86bac:      	callq	0x88b50 <scoop$1$cb$1d386762e04a0637ccab94d264901e5abb5d29e334df1df131050f782310aa58>
   86bb1:      	movq	-0x30(%rbp), %rax
   86bb5:      	movq	-0x38(%rbp), %rcx
   86bb9:      	movq	%rcx, -0x78(%rbp)
   86bbd:      	movq	%rax, -0xa0(%rbp)
   86bc4:      	movq	0x1db9c5(%rip), %rax    # 0x262590 <scoop$1$ss$175d083d63b380e686d8a1d190f67523f972e8cb41414cbc333e1e80b4d54d75>
   86bcb:      	movq	%rax, -0x58(%rbp)
   86bcf:      	callq	0x4d1f0 <scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
   86bd4:      	movq	-0x30(%rbp), %rax
   86bd8:      	movq	-0x38(%rbp), %rcx
   86bdc:      	movq	-0x58(%rbp), %rdx
   86be0:      	movq	%rdx, -0xb0(%rbp)
   86be7:      	movq	%rcx, -0x78(%rbp)
   86beb:      	movq	%rax, -0xa0(%rbp)
   86bf2:      	movq	0x1db8bf(%rip), %rax    # 0x2624b8 <scoop$1$ss$d3fe97594dacdfc6152caa8910564318ae43b1abfd37e48f1ba87d4a4aed7e7b>
   86bf9:      	movq	0xb4860(%rip), %rcx     # 0x13b460 <scoop$1$td$125eccdd45921ed82fc373b6db8bbebfe3f4a5fee3aaca18197e2202ff70aa67+0x60>
   86c00:      	movq	0x18(%rcx), %rdx
   86c04:      	movq	%rax, -0xd8(%rbp)
   86c0b:      	movq	-0xd8(%rbp), %rsi
   86c12:      	movq	-0xb0(%rbp), %rdi
   86c19:      	movq	-0x78(%rbp), %rax
   86c1d:      	movq	-0xa0(%rbp), %rcx
   86c24:      	movq	%rdi, -0x30(%rbp)
   86c28:      	movq	%rax, -0x58(%rbp)
   86c2c:      	movq	%rcx, -0x60(%rbp)
   86c30:      	movq	%rsi, -0x38(%rbp)
   86c34:      	callq	0x841f0 <scoop$1$cb$c6fdbb04db5cf532f4362ad4e151e4a1eabd8894c38f88c356bfab2246151c9f>
   86c39:      	movq	%rdx, %rcx
   86c3c:      	movq	-0x60(%rbp), %rsi
   86c40:      	movq	-0x58(%rbp), %rdx
   86c44:      	movq	-0x38(%rbp), %rdi
   86c48:      	movq	-0x30(%rbp), %r8
   86c4c:      	movq	%r8, -0xb0(%rbp)
   86c53:      	movq	%rdi, -0x1b8(%rbp)
   86c5a:      	movq	%rdx, -0x78(%rbp)
   86c5e:      	movq	%rsi, -0xa0(%rbp)
   86c65:      	movq	%rax, -0xd0(%rbp)
   86c6c:      	movq	-0xd0(%rbp), %rdx
   86c73:      	movq	-0x78(%rbp), %rdi
   86c77:      	movq	%rdi, -0x30(%rbp)
   86c7b:      	movq	%rdx, -0x38(%rbp)
   86c7f:      	callq	0x83cd0 <scoop$1$cb$d9dff153dcecd73d8284f8a22832dcee9c1843022762eaa02a94fb367c53d959>
   86c84:      	movq	-0x38(%rbp), %rcx
   86c88:      	movq	-0x60(%rbp), %rdx
   86c8c:      	movq	-0x30(%rbp), %rsi
   86c90:      	movq	%rsi, -0x78(%rbp)
   86c94:      	movq	%rdx, -0x1b0(%rbp)
   86c9b:      	movq	%rcx, -0x1a8(%rbp)
   86ca2:      	movq	%rax, -0x40(%rbp)
   86ca6:      	leaq	-0x40(%rbp), %rbx
   86caa:      	movq	%rbx, -0x118(%rbp)
   86cb1:      	leaq	0x163378(%rip), %rax    # 0x1ea030 <scoop$1$bs$ecc44cb58159f6c0bab76c83012c8adf52edcf172cdf11769cd013d73fc9af25>
   86cb8:      	movq	%rax, -0x110(%rbp)
   86cbf:      	xorps	%xmm0, %xmm0
   86cc2:      	movups	%xmm0, -0x220(%rbp)
   86cc9:      	movq	$0x0, -0x210(%rbp)
   86cd4:      	leaq	-0x220(%rbp), %r14
   86cdb:      	leaq	-0x118(%rbp), %rsi
   86ce2:      	movl	$0x1, %edx
   86ce7:      	movq	%r14, %rdi
   86cea:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   86cef:      	xorps	%xmm0, %xmm0
   86cf2:      	movups	%xmm0, -0x390(%rbp)
   86cf9:      	movups	%xmm0, -0x380(%rbp)
   86d00:      	movups	%xmm0, -0x370(%rbp)
   86d07:      	movups	%xmm0, -0x360(%rbp)
   86d0e:      	leaq	-0x390(%rbp), %r15
   86d15:      	movq	%r15, %rdi
   86d18:      	movq	%r15, %rsi
   86d1b:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   86d20:      	movl	$0x2, %edi
   86d25:      	callq	0xb38a0 <m34_container_end>
   86d2a:      	movq	%r15, %rdi
   86d2d:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   86d32:      	movq	-0x40(%rbp), %r15
   86d36:      	movq	%r14, %rdi
   86d39:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   86d3e:      	movq	%r15, -0x40(%rbp)
   86d42:      	movq	%rbx, -0x128(%rbp)
   86d49:      	leaq	0x1632f0(%rip), %rax    # 0x1ea040 <scoop$1$bs$088eba55011e01cfa69e06024d2e450a87d700547df2cb55d5bac1f31d62eee0>
   86d50:      	movq	%rax, -0x120(%rbp)
   86d57:      	xorps	%xmm0, %xmm0
   86d5a:      	movups	%xmm0, -0x238(%rbp)
   86d61:      	movq	$0x0, -0x228(%rbp)
   86d6c:      	leaq	-0x238(%rbp), %r14
   86d73:      	leaq	-0x128(%rbp), %rsi
   86d7a:      	movl	$0x1, %edx
   86d7f:      	movq	%r14, %rdi
   86d82:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   86d87:      	xorps	%xmm0, %xmm0
   86d8a:      	movups	%xmm0, -0x3d0(%rbp)
   86d91:      	movups	%xmm0, -0x3c0(%rbp)
   86d98:      	movups	%xmm0, -0x3b0(%rbp)
   86d9f:      	movups	%xmm0, -0x3a0(%rbp)
   86da6:      	leaq	-0x3d0(%rbp), %r15
   86dad:      	movq	%r15, %rdi
   86db0:      	movq	%r15, %rsi
   86db3:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   86db8:      	callq	0xb3800 <m34_container_begin>
   86dbd:      	movq	%r15, %rdi
   86dc0:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   86dc5:      	movq	-0x40(%rbp), %r15
   86dc9:      	movq	%r14, %rdi
   86dcc:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   86dd1:      	movq	%r15, -0x40(%rbp)
   86dd5:      	movq	%r15, -0x30(%rbp)
   86dd9:      	callq	0x7c870 <scoop$1$cb$0bded537c1012e7dc164ae08cf779ffaa1fbf40bc9bfe0e762282e5e75f642ac>
   86dde:      	movq	-0x30(%rbp), %rax
   86de2:      	movq	%rax, -0x40(%rbp)
   86de6:      	movq	0x1db793(%rip), %rax    # 0x262580 <scoop$1$ss$026945f606bf54daf4171a1ecb8b4384e089880c55f3eb80fb1b8fa8a5fad2ad>
   86ded:      	movq	%rax, -0x38(%rbp)
   86df1:      	callq	0x88b50 <scoop$1$cb$1d386762e04a0637ccab94d264901e5abb5d29e334df1df131050f782310aa58>
   86df6:      	movq	-0x30(%rbp), %rax
   86dfa:      	movq	-0x38(%rbp), %rcx
   86dfe:      	movq	%rax, -0x40(%rbp)
   86e02:      	movq	%rcx, -0x70(%rbp)
   86e06:      	movq	%rax, -0x98(%rbp)
   86e0d:      	movq	0x1db77c(%rip), %rax    # 0x262590 <scoop$1$ss$175d083d63b380e686d8a1d190f67523f972e8cb41414cbc333e1e80b4d54d75>
   86e14:      	movq	-0x40(%rbp), %rcx
   86e18:      	movq	%rcx, -0x58(%rbp)
   86e1c:      	movq	%rax, -0x60(%rbp)
   86e20:      	callq	0x4d1f0 <scoop$1$cb$5e012350ddc7cb238741da77cd1768afd61bf09fc92c1eb080f58c2c066df7f3>
   86e25:      	movq	-0x30(%rbp), %rax
   86e29:      	movq	-0x38(%rbp), %rcx
   86e2d:      	movq	-0x60(%rbp), %rdx
   86e31:      	movq	-0x58(%rbp), %rsi
   86e35:      	movq	%rsi, -0x40(%rbp)
   86e39:      	movq	%rdx, -0xa8(%rbp)
   86e40:      	movq	%rcx, -0x70(%rbp)
   86e44:      	movq	%rax, -0x98(%rbp)
   86e4b:      	movq	0x1db666(%rip), %rax    # 0x2624b8 <scoop$1$ss$d3fe97594dacdfc6152caa8910564318ae43b1abfd37e48f1ba87d4a4aed7e7b>
   86e52:      	movq	0xb4607(%rip), %rcx     # 0x13b460 <scoop$1$td$125eccdd45921ed82fc373b6db8bbebfe3f4a5fee3aaca18197e2202ff70aa67+0x60>
   86e59:      	movq	0x8(%rcx), %rdx
   86e5d:      	movq	%rax, -0xc8(%rbp)
   86e64:      	movq	-0xc8(%rbp), %rsi
   86e6b:      	movq	-0x40(%rbp), %rax
   86e6f:      	movq	-0xa8(%rbp), %rdi
   86e76:      	movq	-0x70(%rbp), %rcx
   86e7a:      	movq	-0x98(%rbp), %r8
   86e81:      	movq	%rax, -0x30(%rbp)
   86e85:      	movq	%rdi, -0x38(%rbp)
   86e89:      	movq	%rcx, -0x60(%rbp)
   86e8d:      	movq	%r8, -0x80(%rbp)
   86e91:      	movq	%rsi, -0x58(%rbp)
   86e95:      	callq	0x7e150 <scoop$1$cb$c61dc38f54807de9a33aea25bbdb9b2f907d97e85c3bc4af08d02b91907fd188>
   86e9a:      	movq	%rdx, %rcx
   86e9d:      	movq	-0x80(%rbp), %rsi
   86ea1:      	movq	-0x60(%rbp), %rdx
   86ea5:      	movq	-0x58(%rbp), %rdi
   86ea9:      	movq	-0x38(%rbp), %r8
   86ead:      	movq	-0x30(%rbp), %r9
   86eb1:      	movq	%r9, -0x40(%rbp)
   86eb5:      	movq	%r8, -0xa8(%rbp)
   86ebc:      	movq	%rdi, -0x1a0(%rbp)
   86ec3:      	movq	%rdx, -0x70(%rbp)
   86ec7:      	movq	%rsi, -0x98(%rbp)
   86ece:      	movq	%rax, -0xc0(%rbp)
   86ed5:      	movq	-0xc0(%rbp), %rdx
   86edc:      	movq	-0x40(%rbp), %rax
   86ee0:      	movq	-0x70(%rbp), %rdi
   86ee4:      	movq	%rax, -0x30(%rbp)
   86ee8:      	movq	%rdi, -0x38(%rbp)
   86eec:      	movq	%rdx, -0x58(%rbp)
   86ef0:      	callq	0x92d10 <scoop$1$cb$0ba70f6c06fb5b9f1dc70e010618cf6127649aaa072c115f6e2153721bacf5bb>
   86ef5:      	movq	-0x58(%rbp), %rcx
   86ef9:      	movq	-0x80(%rbp), %rdx
   86efd:      	movq	-0x38(%rbp), %rsi
   86f01:      	movq	-0x30(%rbp), %rdi
   86f05:      	movq	%rdi, -0x40(%rbp)
   86f09:      	movq	%rsi, -0x70(%rbp)
   86f0d:      	movq	%rdx, -0x198(%rbp)
   86f14:      	movq	%rcx, -0x190(%rbp)
   86f1b:      	movq	%rax, -0x68(%rbp)
   86f1f:      	movq	%rbx, -0x180(%rbp)
   86f26:      	leaq	0x163123(%rip), %rax    # 0x1ea050 <scoop$1$bs$9fa9f9beae1fedf905007eae116ed2997d3bcbf5d968e2695d9d878dabd7c333>
   86f2d:      	movq	%rax, -0x178(%rbp)
   86f34:      	leaq	-0x68(%rbp), %rax
   86f38:      	movq	%rax, -0x170(%rbp)
   86f3f:      	leaq	0x16311a(%rip), %rax    # 0x1ea060 <scoop$1$bs$823eda99e6d3c5523bf2e8962e0c1167209419cf90614881a9da40f9090ce4a0>
   86f46:      	movq	%rax, -0x168(%rbp)
   86f4d:      	xorps	%xmm0, %xmm0
   86f50:      	movups	%xmm0, -0x250(%rbp)
   86f57:      	movq	$0x0, -0x240(%rbp)
   86f62:      	leaq	-0x250(%rbp), %r14
   86f69:      	leaq	-0x180(%rbp), %rsi
   86f70:      	movl	$0x2, %edx
   86f75:      	movq	%r14, %rdi
   86f78:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   86f7d:      	xorps	%xmm0, %xmm0
   86f80:      	movups	%xmm0, -0x410(%rbp)
   86f87:      	movups	%xmm0, -0x400(%rbp)
   86f8e:      	movups	%xmm0, -0x3f0(%rbp)
   86f95:      	movups	%xmm0, -0x3e0(%rbp)
   86f9c:      	leaq	-0x410(%rbp), %r15
   86fa3:      	movq	%r15, %rdi
   86fa6:      	movq	%r15, %rsi
   86fa9:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   86fae:      	movl	$0x3, %edi
   86fb3:      	callq	0xb38a0 <m34_container_end>
   86fb8:      	movq	%r15, %rdi
   86fbb:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   86fc0:      	movq	-0x40(%rbp), %rbx
   86fc4:      	movq	-0x68(%rbp), %r15
   86fc8:      	movq	%r14, %rdi
   86fcb:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   86fd0:      	movq	%rbx, -0x40(%rbp)
   86fd4:      	movq	%r15, -0x68(%rbp)
   86fd8:      	cmpq	%r13, 0x18(%r15)
   86fdc:      	jne	0x87018 <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x8b8>
   86fde:      	leaq	-0x1(%r13), %rsi
   86fe2:      	movq	-0x40(%rbp), %rax
   86fe6:      	movq	%rax, -0x30(%rbp)
   86fea:      	movq	%r15, -0x38(%rbp)
   86fee:      	movq	%r15, %rdi
   86ff1:      	callq	0x96650 <scoop$1$cb$e678f8a630461fa24384e1c95aa1201c5fb96088bf850cea5d070c63f1f249b6>
   86ff6:      	movq	-0x38(%rbp), %rcx
   86ffa:      	movq	-0x30(%rbp), %rdx
   86ffe:      	movq	%rdx, -0x40(%rbp)
   87002:      	movq	%rcx, -0x68(%rbp)
   87006:      	movq	%rcx, -0x188(%rbp)
   8700d:      	decl	%r13d
   87010:      	cmpl	%r13d, %eax
   87013:      	sete	%al
   87016:      	jmp	0x8701a <scoop$1$cb$e1150f9b7d8fbf3e7aedab7571b885025335db2422f1de12f6ff5267b57e8364+0x8ba>
   87018:      	xorl	%eax, %eax
   8701a:      	movq	-0x40(%rbp), %rcx
   8701e:      	movq	-0x68(%rbp), %rdx
   87022:      	movq	%rcx, -0x30(%rbp)
   87026:      	movq	%rdx, -0x38(%rbp)
   8702a:      	movzbl	%al, %edi
   8702d:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   87032:      	movq	-0x38(%rbp), %rax
   87036:      	movq	-0x30(%rbp), %rdi
   8703a:      	movq	%rdi, -0x40(%rbp)
   8703e:      	movq	%rax, -0x68(%rbp)
   87042:      	callq	0x57c90 <scoop$1$cb$5d615ca666dc5a96865c5ab6514a9d52b1644e41c4b9aa77d720a989de46034d>
   87047:      	movq	-0x38(%rbp), %rcx
   8704b:      	movq	-0x30(%rbp), %rdx
   8704f:      	movq	%rdx, -0x40(%rbp)
   87053:      	movq	%rcx, -0x68(%rbp)
   87057:      	addq	0x18(%rcx), %rax
   8705b:      	addq	$0x3e8, %rsp            # imm = 0x3E8
   87062:      	popq	%rbx
   87063:      	popq	%r12
   87065:      	popq	%r13
   87067:      	popq	%r14
   87069:      	popq	%r15
   8706b:      	popq	%rbp
   8706c:      	retq

; scalarList, MIR fn4, LIR scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000008dc30 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b>:
   8dc30:      	pushq	%rbp
   8dc31:      	movq	%rsp, %rbp
   8dc34:      	pushq	%r15
   8dc36:      	pushq	%r14
   8dc38:      	pushq	%r13
   8dc3a:      	pushq	%r12
   8dc3c:      	pushq	%rbx
   8dc3d:      	subq	$0x438, %rsp            # imm = 0x438
   8dc44:      	movq	%rdi, -0x68(%rbp)
   8dc48:      	movq	%fs:0x0, %rax
   8dc51:      	leaq	-0x10(%rax), %rax
   8dc58:      	movq	(%rax), %r13
   8dc5b:      	leaq	0x1d49ae(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8dc62:      	movq	(%rax), %rax
   8dc65:      	leaq	0x1d49ac(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8dc6c:      	movl	(%rcx), %esi
   8dc6e:      	movl	(%r13), %edx
   8dc72:      	movq	0x8(%r13), %rcx
   8dc76:      	testl	%esi, %esi
   8dc78:      	jne	0x8dc84 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x54>
   8dc7a:      	cmpl	$0x1, %edx
   8dc7d:      	jne	0x8dc84 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x54>
   8dc7f:      	cmpq	%rax, %rcx
   8dc82:      	je	0x8dc89 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x59>
   8dc84:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8dc89:      	xorps	%xmm0, %xmm0
   8dc8c:      	movups	%xmm0, -0x158(%rbp)
   8dc93:      	movq	$0x0, -0x148(%rbp)
   8dc9e:      	leaq	-0x158(%rbp), %r14
   8dca5:      	movq	%r14, %rdi
   8dca8:      	xorl	%esi, %esi
   8dcaa:      	xorl	%edx, %edx
   8dcac:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8dcb1:      	xorps	%xmm0, %xmm0
   8dcb4:      	movups	%xmm0, -0x258(%rbp)
   8dcbb:      	movups	%xmm0, -0x248(%rbp)
   8dcc2:      	movups	%xmm0, -0x238(%rbp)
   8dcc9:      	movups	%xmm0, -0x228(%rbp)
   8dcd0:      	leaq	-0x258(%rbp), %r15
   8dcd7:      	movq	%r15, %rdi
   8dcda:      	movq	%r15, %rsi
   8dcdd:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8dce2:      	callq	0xb3800 <m34_container_begin>
   8dce7:      	movq	%r15, %rdi
   8dcea:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8dcef:      	movq	%r14, %rdi
   8dcf2:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8dcf7:      	movq	0x17c4ca(%rip), %rbx    # 0x20a1c8 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x18>
   8dcfe:      	movq	%rbx, %r12
   8dd01:      	negq	%r12
   8dd04:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8dd0e:      	addq	%rbx, %rax
   8dd11:      	cmpq	$-0x20, %rax
   8dd15:      	setae	-0x39(%rbp)
   8dd19:      	leaq	0x1f(%rbx), %r15
   8dd1d:      	andq	%r12, %r15
   8dd20:      	movq	%fs:0x0, %rax
   8dd29:      	leaq	-0x8(%rax), %rax
   8dd30:      	movq	(%rax), %rax
   8dd33:      	movq	(%rax), %rcx
   8dd36:      	leaq	(%rbx,%rcx), %r14
   8dd3a:      	decq	%r14
   8dd3d:      	andq	%r12, %r14
   8dd40:      	leaq	(%r14,%r15), %rcx
   8dd44:      	testq	%r14, %r14
   8dd47:      	sete	%dl
   8dd4a:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   8dd51:      	setae	%sil
   8dd55:      	cmpq	0x8(%rax), %rcx
   8dd59:      	seta	%dil
   8dd5d:      	cmpq	$0x0, 0x17c4db(%rip)    # 0x20a240 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2+0x90>
   8dd65:      	setne	%r8b
   8dd69:      	orb	%sil, %r8b
   8dd6c:      	orb	%dl, %r8b
   8dd6f:      	orb	-0x39(%rbp), %r8b
   8dd73:      	orb	%dil, %r8b
   8dd76:      	jne	0x8dd8f <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x15f>
   8dd78:      	movq	%rcx, (%rax)
   8dd7b:      	leaq	0x17c42e(%rip), %rsi    # 0x20a1b0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   8dd82:      	movq	%r14, %rdi
   8dd85:      	movq	%r15, %rdx
   8dd88:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8dd8d:      	jmp	0x8dda3 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x173>
   8dd8f:      	leaq	0x17c41a(%rip), %rdi    # 0x20a1b0 <scoop$1$td$334d4429e52e59327c8e6867b5965e5fa5a5f815610b917416dac219097955a2>
   8dd96:      	movl	$0x20, %esi
   8dd9b:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8dda0:      	movq	%rax, %r14
   8dda3:      	movq	%r14, -0x38(%rbp)
   8dda7:      	movq	%r14, %rdi
   8ddaa:      	movq	-0x68(%rbp), %rsi
   8ddae:      	callq	0x87070 <scoop$1$cb$fee77490ca77ad5e976dbdde0ee39371498372f80a070cf4802c2070dd379995>
   8ddb3:      	movq	-0x38(%rbp), %rax
   8ddb7:      	movq	%rax, -0x140(%rbp)
   8ddbe:      	movq	%rax, -0x30(%rbp)
   8ddc2:      	leaq	-0x30(%rbp), %rax
   8ddc6:      	movq	%rax, -0xb0(%rbp)
   8ddcd:      	leaq	0x16f74c(%rip), %rax    # 0x1fd520 <scoop$1$bs$bcb99a0ff252efde9f2461bf5d378d64d441b17151300bffe65e797b4fb3e9c4>
   8ddd4:      	movq	%rax, -0xa8(%rbp)
   8dddb:      	xorps	%xmm0, %xmm0
   8ddde:      	movups	%xmm0, -0x170(%rbp)
   8dde5:      	movq	$0x0, -0x160(%rbp)
   8ddf0:      	leaq	-0x170(%rbp), %r15
   8ddf7:      	leaq	-0xb0(%rbp), %rsi
   8ddfe:      	movl	$0x1, %edx
   8de03:      	movq	%r15, %rdi
   8de06:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8de0b:      	xorps	%xmm0, %xmm0
   8de0e:      	movups	%xmm0, -0x298(%rbp)
   8de15:      	movups	%xmm0, -0x288(%rbp)
   8de1c:      	movups	%xmm0, -0x278(%rbp)
   8de23:      	movups	%xmm0, -0x268(%rbp)
   8de2a:      	leaq	-0x298(%rbp), %r12
   8de31:      	movq	%r12, %rdi
   8de34:      	movq	%r12, %rsi
   8de37:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8de3c:      	xorl	%r14d, %r14d
   8de3f:      	xorl	%edi, %edi
   8de41:      	callq	0xb38a0 <m34_container_end>
   8de46:      	movq	%r12, %rdi
   8de49:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8de4e:      	movq	-0x30(%rbp), %rbx
   8de52:      	movq	%r15, %rdi
   8de55:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8de5a:      	movq	%rbx, -0x30(%rbp)
   8de5e:      	movq	%rbx, -0x60(%rbp)
   8de62:      	leaq	-0x30(%rbp), %rax
   8de66:      	movq	%rax, -0x130(%rbp)
   8de6d:      	leaq	0x16f6bc(%rip), %rax    # 0x1fd530 <scoop$1$bs$aa08dcb8614caef12239188fa5ab82725d322897150ae91401a493d924e26fc4>
   8de74:      	movq	%rax, -0x128(%rbp)
   8de7b:      	leaq	-0x60(%rbp), %rax
   8de7f:      	movq	%rax, -0x120(%rbp)
   8de86:      	leaq	0x16f6b3(%rip), %rax    # 0x1fd540 <scoop$1$bs$7fad4c10f0e5cf7847850fcda78285dcae00dbc4b5afb43515918b1e14e79d87>
   8de8d:      	movq	%rax, -0x118(%rbp)
   8de94:      	xorps	%xmm0, %xmm0
   8de97:      	movups	%xmm0, -0x188(%rbp)
   8de9e:      	movq	$0x0, -0x178(%rbp)
   8dea9:      	leaq	-0x188(%rbp), %r15
   8deb0:      	leaq	-0x130(%rbp), %rsi
   8deb7:      	movl	$0x2, %edx
   8debc:      	movq	%r15, %rdi
   8debf:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8dec4:      	xorps	%xmm0, %xmm0
   8dec7:      	movups	%xmm0, -0x2d8(%rbp)
   8dece:      	movups	%xmm0, -0x2c8(%rbp)
   8ded5:      	movups	%xmm0, -0x2b8(%rbp)
   8dedc:      	movups	%xmm0, -0x2a8(%rbp)
   8dee3:      	leaq	-0x2d8(%rbp), %r12
   8deea:      	movq	%r12, %rdi
   8deed:      	movq	%r12, %rsi
   8def0:      	callq	0x9a5e0 <scoop_rt_enter_native_borrowed>
   8def5:      	movq	-0x60(%rbp), %rdi
   8def9:      	callq	0xb39d0 <m34_container_layout>
   8defe:      	movq	%r12, %rdi
   8df01:      	callq	0x9d130 <scoop_rt_leave_native_borrowed>
   8df06:      	movq	-0x30(%rbp), %rbx
   8df0a:      	movq	-0x60(%rbp), %r12
   8df0e:      	movq	%r15, %rdi
   8df11:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8df16:      	movq	%rbx, -0x30(%rbp)
   8df1a:      	movq	%r12, -0x60(%rbp)
   8df1e:      	leaq	-0x30(%rbp), %rax
   8df22:      	movq	%rax, -0xc0(%rbp)
   8df29:      	leaq	0x16f620(%rip), %rax    # 0x1fd550 <scoop$1$bs$8154c610379b43f30fec6b45882b9a8c1f5296d4cf7d6bc8d37a44c995908446>
   8df30:      	movq	%rax, -0xb8(%rbp)
   8df37:      	xorps	%xmm0, %xmm0
   8df3a:      	movups	%xmm0, -0x1a0(%rbp)
   8df41:      	movq	$0x0, -0x190(%rbp)
   8df4c:      	leaq	-0x1a0(%rbp), %r15
   8df53:      	leaq	-0xc0(%rbp), %rsi
   8df5a:      	movl	$0x1, %edx
   8df5f:      	movq	%r15, %rdi
   8df62:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8df67:      	xorps	%xmm0, %xmm0
   8df6a:      	movups	%xmm0, -0x318(%rbp)
   8df71:      	movups	%xmm0, -0x308(%rbp)
   8df78:      	movups	%xmm0, -0x2f8(%rbp)
   8df7f:      	movups	%xmm0, -0x2e8(%rbp)
   8df86:      	leaq	-0x318(%rbp), %r12
   8df8d:      	movq	%r12, %rdi
   8df90:      	movq	%r12, %rsi
   8df93:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8df98:      	callq	0xb3800 <m34_container_begin>
   8df9d:      	movq	%r12, %rdi
   8dfa0:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8dfa5:      	movq	-0x30(%rbp), %rbx
   8dfa9:      	movq	%r15, %rdi
   8dfac:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8dfb1:      	movq	%rbx, -0x30(%rbp)
   8dfb5:      	movabsq	$0x51d07eae2f8151d1, %rbx # imm = 0x51D07EAE2F8151D1
   8dfbf:      	xorl	%r12d, %r12d
   8dfc2:      	nopw	%cs:(%rax,%rax)
   8dfd0:      	movq	%r14, %rax
   8dfd3:      	mulq	%rbx
   8dfd6:      	movq	%rdx, %r15
   8dfd9:      	leaq	0x1d4630(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8dfe0:      	movq	(%rax), %rax
   8dfe3:      	leaq	0x1d462e(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8dfea:      	movl	(%rcx), %esi
   8dfec:      	movl	(%r13), %edx
   8dff0:      	movq	0x8(%r13), %rcx
   8dff4:      	testl	%esi, %esi
   8dff6:      	jne	0x8e002 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3d2>
   8dff8:      	cmpl	$0x1, %edx
   8dffb:      	jne	0x8e002 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3d2>
   8dffd:      	cmpq	%rax, %rcx
   8e000:      	je	0x8e017 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3e7>
   8e002:      	movq	-0x30(%rbp), %rax
   8e006:      	movq	%rax, -0x38(%rbp)
   8e00a:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8e00f:      	movq	-0x38(%rbp), %rax
   8e013:      	movq	%rax, -0x30(%rbp)
   8e017:      	cmpq	-0x68(%rbp), %r14
   8e01b:      	jge	0x8e08e <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x45e>
   8e01d:      	movq	%r14, %rax
   8e020:      	subq	%r15, %rax
   8e023:      	shrq	%rax
   8e026:      	addq	%r15, %rax
   8e029:      	shrq	$0x6, %rax
   8e02d:      	imull	$0x61, %eax, %eax
   8e030:      	movl	%r14d, %esi
   8e033:      	subl	%eax, %esi
   8e035:      	movq	%r14, %rax
   8e038:      	movabsq	$-0x5717c0a8e83f5717, %rcx # imm = 0xA8E83F5717C0A8E9
   8e042:      	imulq	%rcx
   8e045:      	addq	%r14, %rdx
   8e048:      	movq	%rdx, %rax
   8e04b:      	shrq	$0x3f, %rax
   8e04f:      	sarq	$0x6, %rdx
   8e053:      	addq	%rax, %rdx
   8e056:      	imulq	$0x61, %rdx, %rax
   8e05a:      	movq	%r14, %r15
   8e05d:      	subq	%rax, %r15
   8e060:      	movq	-0x30(%rbp), %rdi
   8e064:      	movq	%rdi, -0xa0(%rbp)
   8e06b:      	movq	%rdi, -0x38(%rbp)
   8e06f:      	callq	0x94010 <scoop$1$cb$6571e66f09cbc869977ca80836b265c26b4c58d100cec7b4dc74f71fda171ea3>
   8e074:      	movq	-0x38(%rbp), %rax
   8e078:      	movq	%rax, -0x30(%rbp)
   8e07c:      	movq	%rax, -0xa0(%rbp)
   8e083:      	addq	%r15, %r12
   8e086:      	incq	%r14
   8e089:      	jmp	0x8dfd0 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x3a0>
   8e08e:      	leaq	-0x30(%rbp), %rax
   8e092:      	movq	%rax, -0xd0(%rbp)
   8e099:      	leaq	0x16f4c0(%rip), %rax    # 0x1fd560 <scoop$1$bs$8293c51dde1c1ee9623a4e4806deb5dfa27735faeabe6a4a42d9d53172184c49>
   8e0a0:      	movq	%rax, -0xc8(%rbp)
   8e0a7:      	xorps	%xmm0, %xmm0
   8e0aa:      	movups	%xmm0, -0x1b8(%rbp)
   8e0b1:      	movq	$0x0, -0x1a8(%rbp)
   8e0bc:      	leaq	-0x1b8(%rbp), %r14
   8e0c3:      	leaq	-0xd0(%rbp), %rsi
   8e0ca:      	movl	$0x1, %edx
   8e0cf:      	movq	%r14, %rdi
   8e0d2:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8e0d7:      	xorps	%xmm0, %xmm0
   8e0da:      	movups	%xmm0, -0x358(%rbp)
   8e0e1:      	movups	%xmm0, -0x348(%rbp)
   8e0e8:      	movups	%xmm0, -0x338(%rbp)
   8e0ef:      	movups	%xmm0, -0x328(%rbp)
   8e0f6:      	leaq	-0x358(%rbp), %r15
   8e0fd:      	movq	%r15, %rdi
   8e100:      	movq	%r15, %rsi
   8e103:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8e108:      	movl	$0x1, %edi
   8e10d:      	callq	0xb38a0 <m34_container_end>
   8e112:      	movq	%r15, %rdi
   8e115:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8e11a:      	movq	-0x30(%rbp), %rbx
   8e11e:      	movq	%r14, %rdi
   8e121:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8e126:      	movq	%rbx, -0x30(%rbp)
   8e12a:      	leaq	-0x30(%rbp), %rax
   8e12e:      	movq	%rax, -0xe0(%rbp)
   8e135:      	leaq	0x16f434(%rip), %rax    # 0x1fd570 <scoop$1$bs$12e7e0bcae28d4be2ecf772279a88b4f8cc520101d08f1e4193f652e07e2ae9c>
   8e13c:      	movq	%rax, -0xd8(%rbp)
   8e143:      	xorps	%xmm0, %xmm0
   8e146:      	movups	%xmm0, -0x1d0(%rbp)
   8e14d:      	movq	$0x0, -0x1c0(%rbp)
   8e158:      	leaq	-0x1d0(%rbp), %r14
   8e15f:      	leaq	-0xe0(%rbp), %rsi
   8e166:      	movl	$0x1, %edx
   8e16b:      	movq	%r14, %rdi
   8e16e:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8e173:      	xorps	%xmm0, %xmm0
   8e176:      	movups	%xmm0, -0x398(%rbp)
   8e17d:      	movups	%xmm0, -0x388(%rbp)
   8e184:      	movups	%xmm0, -0x378(%rbp)
   8e18b:      	movups	%xmm0, -0x368(%rbp)
   8e192:      	leaq	-0x398(%rbp), %r15
   8e199:      	movq	%r15, %rdi
   8e19c:      	movq	%r15, %rsi
   8e19f:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8e1a4:      	callq	0xb3800 <m34_container_begin>
   8e1a9:      	movq	%r15, %rdi
   8e1ac:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8e1b1:      	movq	-0x30(%rbp), %rbx
   8e1b5:      	movq	%r14, %rdi
   8e1b8:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8e1bd:      	movq	%rbx, -0x30(%rbp)
   8e1c1:      	xorl	%r14d, %r14d
   8e1c4:      	xorl	%r15d, %r15d
   8e1c7:      	nopw	(%rax,%rax)
   8e1d0:      	leaq	0x1d4439(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8e1d7:      	movq	(%rax), %rax
   8e1da:      	leaq	0x1d4437(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8e1e1:      	movl	(%rcx), %esi
   8e1e3:      	movl	(%r13), %edx
   8e1e7:      	movq	0x8(%r13), %rcx
   8e1eb:      	testl	%esi, %esi
   8e1ed:      	jne	0x8e1f9 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5c9>
   8e1ef:      	cmpl	$0x1, %edx
   8e1f2:      	jne	0x8e1f9 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5c9>
   8e1f4:      	cmpq	%rax, %rcx
   8e1f7:      	je	0x8e20e <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5de>
   8e1f9:      	movq	-0x30(%rbp), %rax
   8e1fd:      	movq	%rax, -0x38(%rbp)
   8e201:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8e206:      	movq	-0x38(%rbp), %rax
   8e20a:      	movq	%rax, -0x30(%rbp)
   8e20e:      	cmpq	-0x68(%rbp), %r15
   8e212:      	jge	0x8e33a <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x70a>
   8e218:      	movq	-0x30(%rbp), %rdi
   8e21c:      	movq	%rdi, -0x98(%rbp)
   8e223:      	movq	%rdi, -0x78(%rbp)
   8e227:      	movq	%rdi, -0x38(%rbp)
   8e22b:      	movq	%rdi, -0x48(%rbp)
   8e22f:      	movq	%r15, %rsi
   8e232:      	callq	0x89400 <scoop$1$cb$d3cf7d4731a232fce0fc2f6183c77e740ef9ed96baadbcb3890354170b441b67>
   8e237:      	movq	-0x48(%rbp), %rax
   8e23b:      	movq	-0x38(%rbp), %rcx
   8e23f:      	movq	%rcx, -0x30(%rbp)
   8e243:      	movq	%rax, -0x98(%rbp)
   8e24a:      	movq	%rax, -0x78(%rbp)
   8e24e:      	movq	0x10(%rax), %rdi
   8e252:      	movq	%rdi, -0x70(%rbp)
   8e256:      	movq	-0x30(%rbp), %rax
   8e25a:      	movq	%rax, -0x38(%rbp)
   8e25e:      	movq	%rdi, -0x48(%rbp)
   8e262:      	movq	%r15, %rsi
   8e265:      	callq	0x86600 <scoop$1$cb$86896485c946819ef80ecc82fa51e514c7bc37bae0d5f94c2b1cea61212ae96c>
   8e26a:      	movq	-0x48(%rbp), %rcx
   8e26e:      	movq	-0x38(%rbp), %rdx
   8e272:      	movq	%rdx, -0x30(%rbp)
   8e276:      	movq	%rcx, -0x70(%rbp)
   8e27a:      	cltq
   8e27c:      	addq	%rax, %r14
   8e27f:      	movq	-0x30(%rbp), %rdi
   8e283:      	movq	%rdi, -0x58(%rbp)
   8e287:      	movq	%rdi, -0x90(%rbp)
   8e28e:      	movq	%rdi, -0x88(%rbp)
   8e295:      	movq	%rdi, -0x38(%rbp)
   8e299:      	movq	%rdi, -0x48(%rbp)
   8e29d:      	movq	%rdi, -0x50(%rbp)
   8e2a1:      	movq	%r15, %rsi
   8e2a4:      	callq	0x89400 <scoop$1$cb$d3cf7d4731a232fce0fc2f6183c77e740ef9ed96baadbcb3890354170b441b67>
   8e2a9:      	movq	-0x50(%rbp), %rax
   8e2ad:      	movq	-0x48(%rbp), %rcx
   8e2b1:      	movq	-0x38(%rbp), %rdx
   8e2b5:      	movq	%rdx, -0x30(%rbp)
   8e2b9:      	movq	%rcx, -0x58(%rbp)
   8e2bd:      	movq	%rax, -0x90(%rbp)
   8e2c4:      	movq	%rax, -0x88(%rbp)
   8e2cb:      	movq	0x10(%rax), %rdi
   8e2cf:      	movq	%rdi, -0x80(%rbp)
   8e2d3:      	movq	-0x30(%rbp), %rax
   8e2d7:      	movq	-0x58(%rbp), %rcx
   8e2db:      	movq	%rax, -0x38(%rbp)
   8e2df:      	movq	%rcx, -0x48(%rbp)
   8e2e3:      	movq	%rdi, -0x50(%rbp)
   8e2e7:      	movq	%r15, %rsi
   8e2ea:      	callq	0x86600 <scoop$1$cb$86896485c946819ef80ecc82fa51e514c7bc37bae0d5f94c2b1cea61212ae96c>
   8e2ef:      	movq	-0x50(%rbp), %rcx
   8e2f3:      	movq	-0x48(%rbp), %rdx
   8e2f7:      	movq	-0x38(%rbp), %rsi
   8e2fb:      	movq	%rsi, -0x30(%rbp)
   8e2ff:      	movq	%rdx, -0x58(%rbp)
   8e303:      	movq	%rcx, -0x80(%rbp)
   8e307:      	leal	0x1(%rax), %edx
   8e30a:      	movq	-0x30(%rbp), %rax
   8e30e:      	movq	-0x58(%rbp), %rdi
   8e312:      	movq	%rax, -0x38(%rbp)
   8e316:      	movq	%rdi, -0x48(%rbp)
   8e31a:      	movq	%r15, %rsi
   8e31d:      	callq	0x7e420 <scoop$1$cb$ccfa7da2b94cdac76f7ef88f24a6c3c0f26b22e9b6c28b9ca348f29995c47f8c>
   8e322:      	movq	-0x48(%rbp), %rax
   8e326:      	movq	-0x38(%rbp), %rcx
   8e32a:      	movq	%rcx, -0x30(%rbp)
   8e32e:      	movq	%rax, -0x58(%rbp)
   8e332:      	incq	%r15
   8e335:      	jmp	0x8e1d0 <scoop$1$cb$fc7436a4f3c0a0764dbce639c86842b732be5769e0f8297c4f07fb10b2259d3b+0x5a0>
   8e33a:      	leaq	-0x30(%rbp), %r13
   8e33e:      	movq	%r13, -0xf0(%rbp)
   8e345:      	leaq	0x16f234(%rip), %rax    # 0x1fd580 <scoop$1$bs$58ef8f7f24e1c902e9b2e0565447e1f6b2009cc91f904580dc3e7c02e5c9eaef>
   8e34c:      	movq	%rax, -0xe8(%rbp)
   8e353:      	xorps	%xmm0, %xmm0
   8e356:      	movups	%xmm0, -0x1e8(%rbp)
   8e35d:      	movq	$0x0, -0x1d8(%rbp)
   8e368:      	leaq	-0x1e8(%rbp), %rbx
   8e36f:      	leaq	-0xf0(%rbp), %rsi
   8e376:      	movl	$0x1, %edx
   8e37b:      	movq	%rbx, %rdi
   8e37e:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8e383:      	xorps	%xmm0, %xmm0
   8e386:      	movups	%xmm0, -0x3d8(%rbp)
   8e38d:      	movups	%xmm0, -0x3c8(%rbp)
   8e394:      	movups	%xmm0, -0x3b8(%rbp)
   8e39b:      	movups	%xmm0, -0x3a8(%rbp)
   8e3a2:      	leaq	-0x3d8(%rbp), %r15
   8e3a9:      	movq	%r15, %rdi
   8e3ac:      	movq	%r15, %rsi
   8e3af:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8e3b4:      	movl	$0x2, %edi
   8e3b9:      	callq	0xb38a0 <m34_container_end>
   8e3be:      	movq	%r15, %rdi
   8e3c1:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8e3c6:      	movq	-0x30(%rbp), %r15
   8e3ca:      	movq	%rbx, %rdi
   8e3cd:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8e3d2:      	movq	%r15, -0x30(%rbp)
   8e3d6:      	xorl	%edi, %edi
   8e3d8:      	cmpq	%r12, %r14
   8e3db:      	sete	%dil
   8e3df:      	movq	%r15, -0x38(%rbp)
   8e3e3:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8e3e8:      	movq	-0x38(%rbp), %rax
   8e3ec:      	movq	%rax, -0x30(%rbp)
   8e3f0:      	movq	%r13, -0x100(%rbp)
   8e3f7:      	leaq	0x16f192(%rip), %rax    # 0x1fd590 <scoop$1$bs$d3600b0213aa68037cc0823e687015cbf3e60338d644e3efd6f41f0b280c701d>
   8e3fe:      	movq	%rax, -0xf8(%rbp)
   8e405:      	xorps	%xmm0, %xmm0
   8e408:      	movups	%xmm0, -0x200(%rbp)
   8e40f:      	movq	$0x0, -0x1f0(%rbp)
   8e41a:      	leaq	-0x200(%rbp), %rbx
   8e421:      	leaq	-0x100(%rbp), %rsi
   8e428:      	movl	$0x1, %edx
   8e42d:      	movq	%rbx, %rdi
   8e430:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8e435:      	xorps	%xmm0, %xmm0
   8e438:      	movups	%xmm0, -0x418(%rbp)
   8e43f:      	movups	%xmm0, -0x408(%rbp)
   8e446:      	movups	%xmm0, -0x3f8(%rbp)
   8e44d:      	movups	%xmm0, -0x3e8(%rbp)
   8e454:      	leaq	-0x418(%rbp), %r15
   8e45b:      	movq	%r15, %rdi
   8e45e:      	movq	%r15, %rsi
   8e461:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8e466:      	callq	0xb3800 <m34_container_begin>
   8e46b:      	movq	%r15, %rdi
   8e46e:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8e473:      	movq	-0x30(%rbp), %r15
   8e477:      	movq	%rbx, %rdi
   8e47a:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8e47f:      	movq	%r15, -0x30(%rbp)
   8e483:      	movq	%r15, -0x38(%rbp)
   8e487:      	movq	%r15, %rdi
   8e48a:      	callq	0x859c0 <scoop$1$cb$1b62e78994e6929f86ac69ff970c434030a57c0e62b785f6ad4e4c34ef5cc7c1>
   8e48f:      	movq	-0x38(%rbp), %rax
   8e493:      	movq	%rax, -0x30(%rbp)
   8e497:      	movq	%rax, -0x138(%rbp)
   8e49e:      	movq	%r13, -0x110(%rbp)
   8e4a5:      	leaq	0x16f0f4(%rip), %rax    # 0x1fd5a0 <scoop$1$bs$dc7cb9f7620b61f1342ffa4248e1d28aea6b95227e2b2c1fd17df8bc58bc08ea>
   8e4ac:      	movq	%rax, -0x108(%rbp)
   8e4b3:      	xorps	%xmm0, %xmm0
   8e4b6:      	movups	%xmm0, -0x218(%rbp)
   8e4bd:      	movq	$0x0, -0x208(%rbp)
   8e4c8:      	leaq	-0x218(%rbp), %rbx
   8e4cf:      	leaq	-0x110(%rbp), %rsi
   8e4d6:      	movl	$0x1, %edx
   8e4db:      	movq	%rbx, %rdi
   8e4de:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8e4e3:      	xorps	%xmm0, %xmm0
   8e4e6:      	movups	%xmm0, -0x458(%rbp)
   8e4ed:      	movups	%xmm0, -0x448(%rbp)
   8e4f4:      	movups	%xmm0, -0x438(%rbp)
   8e4fb:      	movups	%xmm0, -0x428(%rbp)
   8e502:      	leaq	-0x458(%rbp), %r15
   8e509:      	movq	%r15, %rdi
   8e50c:      	movq	%r15, %rsi
   8e50f:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8e514:      	movl	$0x3, %edi
   8e519:      	callq	0xb38a0 <m34_container_end>
   8e51e:      	movq	%r15, %rdi
   8e521:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8e526:      	movq	-0x30(%rbp), %r15
   8e52a:      	movq	%rbx, %rdi
   8e52d:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8e532:      	movq	%r15, -0x30(%rbp)
   8e536:      	xorl	%edi, %edi
   8e538:      	cmpq	$0x0, 0x18(%r15)
   8e53d:      	sete	%dil
   8e541:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8e546:      	movq	%r14, %rax
   8e549:      	addq	$0x438, %rsp            # imm = 0x438
   8e550:      	popq	%rbx
   8e551:      	popq	%r12
   8e553:      	popq	%r13
   8e555:      	popq	%r14
   8e557:      	popq	%r15
   8e559:      	popq	%rbp
   8e55a:      	retq

; interfaceList, MIR fn5, LIR scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000008ef00 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c>:
   8ef00:      	pushq	%rbp
   8ef01:      	movq	%rsp, %rbp
   8ef04:      	pushq	%r15
   8ef06:      	pushq	%r14
   8ef08:      	pushq	%r13
   8ef0a:      	pushq	%r12
   8ef0c:      	pushq	%rbx
   8ef0d:      	subq	$0x458, %rsp            # imm = 0x458
   8ef14:      	movq	%rdi, -0x50(%rbp)
   8ef18:      	movq	%fs:0x0, %rax
   8ef21:      	leaq	-0x10(%rax), %rax
   8ef28:      	movq	(%rax), %rcx
   8ef2b:      	movq	$0x0, -0x78(%rbp)
   8ef33:      	leaq	0x1d36d6(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8ef3a:      	movq	(%rax), %rax
   8ef3d:      	leaq	0x1d36d4(%rip), %rdx    # 0x262618 <scoop_thread_world_phase>
   8ef44:      	movl	(%rdx), %esi
   8ef46:      	movl	(%rcx), %edx
   8ef48:      	movq	%rcx, -0x60(%rbp)
   8ef4c:      	movq	0x8(%rcx), %rcx
   8ef50:      	testl	%esi, %esi
   8ef52:      	jne	0x8ef5e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x5e>
   8ef54:      	cmpl	$0x1, %edx
   8ef57:      	jne	0x8ef5e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x5e>
   8ef59:      	cmpq	%rax, %rcx
   8ef5c:      	je	0x8ef63 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x63>
   8ef5e:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8ef63:      	movabsq	$0x7fffffffffffffff, %r13 # imm = 0x7FFFFFFFFFFFFFFF
   8ef6d:      	xorps	%xmm0, %xmm0
   8ef70:      	movups	%xmm0, -0x180(%rbp)
   8ef77:      	movq	$0x0, -0x170(%rbp)
   8ef82:      	leaq	-0x180(%rbp), %r14
   8ef89:      	movq	%r14, %rdi
   8ef8c:      	xorl	%esi, %esi
   8ef8e:      	xorl	%edx, %edx
   8ef90:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8ef95:      	xorps	%xmm0, %xmm0
   8ef98:      	movups	%xmm0, -0x280(%rbp)
   8ef9f:      	movups	%xmm0, -0x270(%rbp)
   8efa6:      	movups	%xmm0, -0x260(%rbp)
   8efad:      	movups	%xmm0, -0x250(%rbp)
   8efb4:      	leaq	-0x280(%rbp), %r15
   8efbb:      	movq	%r15, %rdi
   8efbe:      	movq	%r15, %rsi
   8efc1:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8efc6:      	callq	0xb3800 <m34_container_begin>
   8efcb:      	movq	%r15, %rdi
   8efce:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8efd3:      	movq	%r14, %rdi
   8efd6:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8efdb:      	movq	0x156ec6(%rip), %rbx    # 0x1e5ea8 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d+0x18>
   8efe2:      	movq	%rbx, %r12
   8efe5:      	negq	%r12
   8efe8:      	leaq	(%rbx,%r13), %rax
   8efec:      	cmpq	$-0x20, %rax
   8eff0:      	setae	%r13b
   8eff4:      	leaq	0x1f(%rbx), %r15
   8eff8:      	andq	%r12, %r15
   8effb:      	movq	%fs:0x0, %rax
   8f004:      	leaq	-0x8(%rax), %rax
   8f00b:      	movq	(%rax), %rax
   8f00e:      	movq	(%rax), %rcx
   8f011:      	leaq	(%rbx,%rcx), %r14
   8f015:      	decq	%r14
   8f018:      	andq	%r12, %r14
   8f01b:      	leaq	(%r14,%r15), %rcx
   8f01f:      	testq	%r14, %r14
   8f022:      	sete	%dl
   8f025:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   8f02c:      	setae	%sil
   8f030:      	cmpq	0x8(%rax), %rcx
   8f034:      	seta	%dil
   8f038:      	cmpq	$0x0, 0x156ee0(%rip)    # 0x1e5f20 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d+0x90>
   8f040:      	setne	%r8b
   8f044:      	orb	%sil, %r8b
   8f047:      	orb	%dl, %r8b
   8f04a:      	orb	%r13b, %r8b
   8f04d:      	orb	%dil, %r8b
   8f050:      	jne	0x8f069 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x169>
   8f052:      	movq	%rcx, (%rax)
   8f055:      	leaq	0x156e34(%rip), %rsi    # 0x1e5e90 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d>
   8f05c:      	movq	%r14, %rdi
   8f05f:      	movq	%r15, %rdx
   8f062:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8f067:      	jmp	0x8f07d <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x17d>
   8f069:      	leaq	0x156e20(%rip), %rdi    # 0x1e5e90 <scoop$1$td$90d4ce11bee20851bf0c92beaac4183b9505a10eb8b35c4797fe37d740e9f23d>
   8f070:      	movl	$0x20, %esi
   8f075:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8f07a:      	movq	%rax, %r14
   8f07d:      	movq	-0x50(%rbp), %r13
   8f081:      	movq	%r14, -0x38(%rbp)
   8f085:      	movq	%r14, %rdi
   8f088:      	movq	%r13, %rsi
   8f08b:      	callq	0x8d040 <scoop$1$cb$618fe03087f04fe6de447a9d1eef329119340be9a3335c5e390675ee699883ef>
   8f090:      	movq	-0x38(%rbp), %rax
   8f094:      	movq	%rax, -0x168(%rbp)
   8f09b:      	movq	%rax, -0x30(%rbp)
   8f09f:      	leaq	-0x30(%rbp), %r14
   8f0a3:      	movq	%r14, -0xd8(%rbp)
   8f0aa:      	leaq	0x170c4f(%rip), %rax    # 0x1ffd00 <scoop$1$bs$0ccf51103070cd5973bbd27997fe267f45aeaa291fe0b77198fb9dd565d8ce03>
   8f0b1:      	movq	%rax, -0xd0(%rbp)
   8f0b8:      	xorps	%xmm0, %xmm0
   8f0bb:      	movups	%xmm0, -0x198(%rbp)
   8f0c2:      	movq	$0x0, -0x188(%rbp)
   8f0cd:      	leaq	-0x198(%rbp), %r12
   8f0d4:      	leaq	-0xd8(%rbp), %rsi
   8f0db:      	movl	$0x1, %edx
   8f0e0:      	movq	%r12, %rdi
   8f0e3:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f0e8:      	xorps	%xmm0, %xmm0
   8f0eb:      	movups	%xmm0, -0x2c0(%rbp)
   8f0f2:      	movups	%xmm0, -0x2b0(%rbp)
   8f0f9:      	movups	%xmm0, -0x2a0(%rbp)
   8f100:      	movups	%xmm0, -0x290(%rbp)
   8f107:      	leaq	-0x2c0(%rbp), %r15
   8f10e:      	movq	%r15, %rdi
   8f111:      	movq	%r15, %rsi
   8f114:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f119:      	xorl	%edi, %edi
   8f11b:      	callq	0xb38a0 <m34_container_end>
   8f120:      	movq	%r15, %rdi
   8f123:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f128:      	movq	-0x30(%rbp), %rbx
   8f12c:      	movq	%r12, %rdi
   8f12f:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f134:      	movq	%rbx, -0x30(%rbp)
   8f138:      	movq	%rbx, -0x48(%rbp)
   8f13c:      	movq	%r14, -0x158(%rbp)
   8f143:      	leaq	0x170bc6(%rip), %rax    # 0x1ffd10 <scoop$1$bs$dc1e38a95e7aa7813dea4c23668cd78f9bba3a02d4d526a208c168e880ec34c4>
   8f14a:      	movq	%rax, -0x150(%rbp)
   8f151:      	leaq	-0x48(%rbp), %rax
   8f155:      	movq	%rax, -0x148(%rbp)
   8f15c:      	leaq	0x170bbd(%rip), %rax    # 0x1ffd20 <scoop$1$bs$aab25ec34d3c92f7452d4da325ec2f2c57f433d8ea7954916d98cae0718b9a81>
   8f163:      	movq	%rax, -0x140(%rbp)
   8f16a:      	xorps	%xmm0, %xmm0
   8f16d:      	movups	%xmm0, -0x1b0(%rbp)
   8f174:      	movq	$0x0, -0x1a0(%rbp)
   8f17f:      	leaq	-0x1b0(%rbp), %r12
   8f186:      	leaq	-0x158(%rbp), %rsi
   8f18d:      	movl	$0x2, %edx
   8f192:      	movq	%r12, %rdi
   8f195:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f19a:      	xorps	%xmm0, %xmm0
   8f19d:      	movups	%xmm0, -0x300(%rbp)
   8f1a4:      	movups	%xmm0, -0x2f0(%rbp)
   8f1ab:      	movups	%xmm0, -0x2e0(%rbp)
   8f1b2:      	movups	%xmm0, -0x2d0(%rbp)
   8f1b9:      	leaq	-0x300(%rbp), %r15
   8f1c0:      	movq	%r15, %rdi
   8f1c3:      	movq	%r15, %rsi
   8f1c6:      	callq	0x9a5e0 <scoop_rt_enter_native_borrowed>
   8f1cb:      	movq	-0x48(%rbp), %rdi
   8f1cf:      	callq	0xb39d0 <m34_container_layout>
   8f1d4:      	movq	%r15, %rdi
   8f1d7:      	callq	0x9d130 <scoop_rt_leave_native_borrowed>
   8f1dc:      	movq	-0x30(%rbp), %rbx
   8f1e0:      	movq	-0x48(%rbp), %r14
   8f1e4:      	movq	%r12, %rdi
   8f1e7:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f1ec:      	movq	%rbx, -0x30(%rbp)
   8f1f0:      	movq	%r14, -0x48(%rbp)
   8f1f4:      	leaq	-0x30(%rbp), %rax
   8f1f8:      	movq	%rax, -0xe8(%rbp)
   8f1ff:      	leaq	0x170b2a(%rip), %rax    # 0x1ffd30 <scoop$1$bs$4a9eed8bc07d443add3de6fce27dbc079379d7b42a1bdfd844e29ed838d3406c>
   8f206:      	movq	%rax, -0xe0(%rbp)
   8f20d:      	xorps	%xmm0, %xmm0
   8f210:      	movups	%xmm0, -0x1c8(%rbp)
   8f217:      	movq	$0x0, -0x1b8(%rbp)
   8f222:      	leaq	-0x1c8(%rbp), %r12
   8f229:      	leaq	-0xe8(%rbp), %rsi
   8f230:      	movl	$0x1, %edx
   8f235:      	movq	%r12, %rdi
   8f238:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f23d:      	xorps	%xmm0, %xmm0
   8f240:      	movups	%xmm0, -0x340(%rbp)
   8f247:      	movups	%xmm0, -0x330(%rbp)
   8f24e:      	movups	%xmm0, -0x320(%rbp)
   8f255:      	movups	%xmm0, -0x310(%rbp)
   8f25c:      	leaq	-0x340(%rbp), %r15
   8f263:      	movq	%r15, %rdi
   8f266:      	movq	%r15, %rsi
   8f269:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f26e:      	callq	0xb3800 <m34_container_begin>
   8f273:      	movq	%r15, %rdi
   8f276:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f27b:      	movq	-0x30(%rbp), %rbx
   8f27f:      	movq	%r12, %rdi
   8f282:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f287:      	movq	%rbx, -0x30(%rbp)
   8f28b:      	xorl	%ebx, %ebx
   8f28d:      	jmp	0x8f333 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x433>
   8f292:      	nopw	%cs:(%rax,%rax)
   8f2a0:      	movq	-0x30(%rbp), %rax
   8f2a4:      	movq	%rax, -0x38(%rbp)
   8f2a8:      	movq	%rbx, -0x40(%rbp)
   8f2ac:      	movl	$0x18, %esi
   8f2b1:      	leaq	0x182018(%rip), %rdi    # 0x2112d0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8f2b8:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   8f2bd:      	movq	%rax, %r13
   8f2c0:      	movq	-0x40(%rbp), %rax
   8f2c4:      	movq	-0x38(%rbp), %rcx
   8f2c8:      	movq	%rcx, -0x30(%rbp)
   8f2cc:      	movq	%rax, -0x58(%rbp)
   8f2d0:      	movq	-0x80(%rbp), %rbx
   8f2d4:      	movl	%ebx, 0x10(%r13)
   8f2d8:      	movq	0x182051(%rip), %rax    # 0x211330 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x60>
   8f2df:      	movq	0x8(%rax), %rdx
   8f2e3:      	movq	%r13, -0x90(%rbp)
   8f2ea:      	movq	-0x90(%rbp), %rsi
   8f2f1:      	movq	%rsi, -0x88(%rbp)
   8f2f8:      	movq	-0x30(%rbp), %rax
   8f2fc:      	movq	-0x58(%rbp), %rdi
   8f300:      	movq	%rax, -0x38(%rbp)
   8f304:      	movq	%rdi, -0x40(%rbp)
   8f308:      	movq	%rsi, -0x68(%rbp)
   8f30c:      	callq	0x87fc0 <scoop$1$cb$6d53c57dfb720cf365bbd7bd3500d138e2ff85a16ea4802cf62d05958722c237>
   8f311:      	movq	-0x68(%rbp), %rax
   8f315:      	movq	-0x40(%rbp), %rcx
   8f319:      	movq	-0x38(%rbp), %rdx
   8f31d:      	movq	%rdx, -0x30(%rbp)
   8f321:      	movq	%rcx, -0x58(%rbp)
   8f325:      	movq	%rax, -0x88(%rbp)
   8f32c:      	incq	%rbx
   8f32f:      	movq	-0x50(%rbp), %r13
   8f333:      	leaq	0x1d32d6(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8f33a:      	movq	(%rax), %rax
   8f33d:      	leaq	0x1d32d4(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8f344:      	movl	(%rcx), %esi
   8f346:      	movq	-0x60(%rbp), %rcx
   8f34a:      	movl	(%rcx), %edx
   8f34c:      	movq	0x8(%rcx), %rcx
   8f350:      	testl	%esi, %esi
   8f352:      	jne	0x8f35e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x45e>
   8f354:      	cmpl	$0x1, %edx
   8f357:      	jne	0x8f35e <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x45e>
   8f359:      	cmpq	%rax, %rcx
   8f35c:      	je	0x8f373 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x473>
   8f35e:      	movq	-0x30(%rbp), %rax
   8f362:      	movq	%rax, -0x38(%rbp)
   8f366:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8f36b:      	movq	-0x38(%rbp), %rax
   8f36f:      	movq	%rax, -0x30(%rbp)
   8f373:      	cmpq	%r13, %rbx
   8f376:      	jge	0x8f425 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x525>
   8f37c:      	movq	%rbx, -0x80(%rbp)
   8f380:      	movq	-0x30(%rbp), %rbx
   8f384:      	movq	%rbx, -0x58(%rbp)
   8f388:      	movq	0x181f59(%rip), %r13    # 0x2112e8 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x18>
   8f38f:      	movq	%r13, %r15
   8f392:      	negq	%r15
   8f395:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   8f39f:      	addq	%r13, %rax
   8f3a2:      	cmpq	$-0x18, %rax
   8f3a6:      	setb	%r14b
   8f3aa:      	leaq	0x17(%r13), %r12
   8f3ae:      	andq	%r15, %r12
   8f3b1:      	movq	%fs:0x0, %rax
   8f3ba:      	leaq	-0x8(%rax), %rax
   8f3c1:      	movq	(%rax), %rax
   8f3c4:      	movq	(%rax), %rcx
   8f3c7:      	addq	%rcx, %r13
   8f3ca:      	decq	%r13
   8f3cd:      	andq	%r15, %r13
   8f3d0:      	leaq	(%r12,%r13), %rcx
   8f3d4:      	testq	%r13, %r13
   8f3d7:      	setne	%dl
   8f3da:      	cmpq	$0x7f81, %r12           # imm = 0x7F81
   8f3e1:      	setb	%sil
   8f3e5:      	cmpq	0x8(%rax), %rcx
   8f3e9:      	setbe	%dil
   8f3ed:      	cmpq	$0x0, 0x181f6b(%rip)    # 0x211360 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c+0x90>
   8f3f5:      	sete	%r8b
   8f3f9:      	andb	%sil, %r8b
   8f3fc:      	andb	%dl, %r8b
   8f3ff:      	andb	%dil, %r8b
   8f402:      	testb	%r14b, %r8b
   8f405:      	je	0x8f2a0 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x3a0>
   8f40b:      	movq	%rcx, (%rax)
   8f40e:      	movq	%r13, %rdi
   8f411:      	leaq	0x181eb8(%rip), %rsi    # 0x2112d0 <scoop$1$td$f88f8aa2129ef96cc6fe14044e56692ab7083494fa049601ba8a26bd52adc71c>
   8f418:      	movq	%r12, %rdx
   8f41b:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   8f420:      	jmp	0x8f2d0 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x3d0>
   8f425:      	leaq	-0x30(%rbp), %r12
   8f429:      	movq	%r12, -0xf8(%rbp)
   8f430:      	leaq	0x170909(%rip), %rax    # 0x1ffd40 <scoop$1$bs$c9b66f30bb517bd5c355e2a5ec1a5a28a25b388c4f7ea5721bc02694e6ab46df>
   8f437:      	movq	%rax, -0xf0(%rbp)
   8f43e:      	xorps	%xmm0, %xmm0
   8f441:      	movups	%xmm0, -0x1e0(%rbp)
   8f448:      	movq	$0x0, -0x1d0(%rbp)
   8f453:      	leaq	-0x1e0(%rbp), %r14
   8f45a:      	leaq	-0xf8(%rbp), %rsi
   8f461:      	movl	$0x1, %edx
   8f466:      	movq	%r14, %rdi
   8f469:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f46e:      	xorps	%xmm0, %xmm0
   8f471:      	movups	%xmm0, -0x380(%rbp)
   8f478:      	movups	%xmm0, -0x370(%rbp)
   8f47f:      	movups	%xmm0, -0x360(%rbp)
   8f486:      	movups	%xmm0, -0x350(%rbp)
   8f48d:      	leaq	-0x380(%rbp), %r15
   8f494:      	movq	%r15, %rdi
   8f497:      	movq	%r15, %rsi
   8f49a:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f49f:      	movl	$0x1, %edi
   8f4a4:      	callq	0xb38a0 <m34_container_end>
   8f4a9:      	movq	%r15, %rdi
   8f4ac:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f4b1:      	movq	-0x30(%rbp), %rbx
   8f4b5:      	movq	%r14, %rdi
   8f4b8:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f4bd:      	movq	%rbx, -0x30(%rbp)
   8f4c1:      	movq	%r12, -0x108(%rbp)
   8f4c8:      	leaq	0x170881(%rip), %rax    # 0x1ffd50 <scoop$1$bs$bbe2b5508974aa89339bcd35ca3ef528fada9ae570265f72cf96c525c4d95384>
   8f4cf:      	movq	%rax, -0x100(%rbp)
   8f4d6:      	xorps	%xmm0, %xmm0
   8f4d9:      	movups	%xmm0, -0x1f8(%rbp)
   8f4e0:      	movq	$0x0, -0x1e8(%rbp)
   8f4eb:      	leaq	-0x1f8(%rbp), %r14
   8f4f2:      	leaq	-0x108(%rbp), %rsi
   8f4f9:      	movl	$0x1, %edx
   8f4fe:      	movq	%r14, %rdi
   8f501:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f506:      	xorps	%xmm0, %xmm0
   8f509:      	movups	%xmm0, -0x3c0(%rbp)
   8f510:      	movups	%xmm0, -0x3b0(%rbp)
   8f517:      	movups	%xmm0, -0x3a0(%rbp)
   8f51e:      	movups	%xmm0, -0x390(%rbp)
   8f525:      	leaq	-0x3c0(%rbp), %r15
   8f52c:      	movq	%r15, %rdi
   8f52f:      	movq	%r15, %rsi
   8f532:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f537:      	callq	0xb3800 <m34_container_begin>
   8f53c:      	movq	%r15, %rdi
   8f53f:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f544:      	movq	-0x30(%rbp), %rbx
   8f548:      	movq	%r14, %rdi
   8f54b:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f550:      	movq	%rbx, -0x30(%rbp)
   8f554:      	xorl	%r14d, %r14d
   8f557:      	leaq	-0xc8(%rbp), %r15
   8f55e:      	xorl	%r12d, %r12d
   8f561:      	nopw	%cs:(%rax,%rax)
   8f570:      	leaq	0x1d3099(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   8f577:      	movq	(%rax), %rax
   8f57a:      	leaq	0x1d3097(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   8f581:      	movl	(%rcx), %esi
   8f583:      	movq	-0x60(%rbp), %rcx
   8f587:      	movl	(%rcx), %edx
   8f589:      	movq	0x8(%rcx), %rcx
   8f58d:      	testl	%esi, %esi
   8f58f:      	jne	0x8f59b <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x69b>
   8f591:      	cmpl	$0x1, %edx
   8f594:      	jne	0x8f59b <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x69b>
   8f596:      	cmpq	%rax, %rcx
   8f599:      	je	0x8f5b0 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x6b0>
   8f59b:      	movq	-0x30(%rbp), %rax
   8f59f:      	movq	%rax, -0x38(%rbp)
   8f5a3:      	callq	0x9a3c0 <scoop_rt_safepoint>
   8f5a8:      	movq	-0x38(%rbp), %rax
   8f5ac:      	movq	%rax, -0x30(%rbp)
   8f5b0:      	cmpq	%r13, %r12
   8f5b3:      	jge	0x8f692 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x792>
   8f5b9:      	movq	-0x30(%rbp), %rdi
   8f5bd:      	movq	%rdi, -0xb8(%rbp)
   8f5c4:      	movq	%rdi, -0xa8(%rbp)
   8f5cb:      	movq	%rdi, -0x38(%rbp)
   8f5cf:      	movq	%rdi, -0x40(%rbp)
   8f5d3:      	movq	%r12, %rsi
   8f5d6:      	callq	0x80440 <scoop$1$cb$76ad1a6d87307f4848166af22abc76d78c46372f25612e5213bba69396b4209f>
   8f5db:      	movq	-0x40(%rbp), %rax
   8f5df:      	movq	-0x38(%rbp), %rcx
   8f5e3:      	movq	%rcx, -0x30(%rbp)
   8f5e7:      	movq	%rax, -0xb8(%rbp)
   8f5ee:      	movq	%rax, -0xa8(%rbp)
   8f5f5:      	movq	0x10(%rax), %rsi
   8f5f9:      	movq	%rsi, -0xa0(%rbp)
   8f600:      	movq	-0x30(%rbp), %rax
   8f604:      	movq	%rax, -0x38(%rbp)
   8f608:      	movq	%rsi, -0x40(%rbp)
   8f60c:      	movq	%r15, %rdi
   8f60f:      	movq	%r12, %rdx
   8f612:      	callq	0x97470 <scoop$1$cb$fd053d002bf6e336458102ee19116922ca05685cc828608cd9785e53b7f9e79a>
   8f617:      	movq	-0x40(%rbp), %rax
   8f61b:      	movq	-0x38(%rbp), %rcx
   8f61f:      	movq	%rcx, -0x30(%rbp)
   8f623:      	movq	%rax, -0xa0(%rbp)
   8f62a:      	movq	-0xc8(%rbp), %rax
   8f631:      	movq	-0xc0(%rbp), %rcx
   8f638:      	movq	%rax, -0x98(%rbp)
   8f63f:      	movq	-0x98(%rbp), %rax
   8f646:      	movq	%rax, -0xb0(%rbp)
   8f64d:      	movq	-0xb0(%rbp), %rax
   8f654:      	movq	%rax, -0x70(%rbp)
   8f658:      	movq	-0x70(%rbp), %rax
   8f65c:      	movq	-0x70(%rbp), %rdi
   8f660:      	movq	%rdi, -0x78(%rbp)
   8f664:      	movq	-0x30(%rbp), %rax
   8f668:      	movq	(%rcx), %rcx
   8f66b:      	movq	%rdi, -0x40(%rbp)
   8f66f:      	movq	%rax, -0x38(%rbp)
   8f673:      	callq	*%rcx
   8f675:      	movq	-0x40(%rbp), %rcx
   8f679:      	movq	-0x38(%rbp), %rdx
   8f67d:      	movq	%rdx, -0x30(%rbp)
   8f681:      	movq	%rcx, -0x78(%rbp)
   8f685:      	cltq
   8f687:      	addq	%rax, %r14
   8f68a:      	incq	%r12
   8f68d:      	jmp	0x8f570 <scoop$1$cb$10f097a7cf1a327f29f68de17a25c74009e4f70155428a93a94e52bf465ba26c+0x670>
   8f692:      	leaq	-0x30(%rbp), %rax
   8f696:      	movq	%rax, -0x118(%rbp)
   8f69d:      	leaq	0x1706bc(%rip), %rax    # 0x1ffd60 <scoop$1$bs$6ac7501ee1ec08f3b5df91b4d802fe29ce42c93f07f61fa3f910073972d2dde7>
   8f6a4:      	movq	%rax, -0x110(%rbp)
   8f6ab:      	xorps	%xmm0, %xmm0
   8f6ae:      	movups	%xmm0, -0x210(%rbp)
   8f6b5:      	movq	$0x0, -0x200(%rbp)
   8f6c0:      	leaq	-0x210(%rbp), %r15
   8f6c7:      	leaq	-0x118(%rbp), %rsi
   8f6ce:      	movl	$0x1, %edx
   8f6d3:      	movq	%r15, %rdi
   8f6d6:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f6db:      	xorps	%xmm0, %xmm0
   8f6de:      	movups	%xmm0, -0x400(%rbp)
   8f6e5:      	movups	%xmm0, -0x3f0(%rbp)
   8f6ec:      	movups	%xmm0, -0x3e0(%rbp)
   8f6f3:      	movups	%xmm0, -0x3d0(%rbp)
   8f6fa:      	leaq	-0x400(%rbp), %r12
   8f701:      	movq	%r12, %rdi
   8f704:      	movq	%r12, %rsi
   8f707:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f70c:      	movl	$0x2, %edi
   8f711:      	callq	0xb38a0 <m34_container_end>
   8f716:      	movq	%r12, %rdi
   8f719:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f71e:      	movq	-0x30(%rbp), %rbx
   8f722:      	movq	%r15, %rdi
   8f725:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f72a:      	movq	%rbx, -0x30(%rbp)
   8f72e:      	leaq	-0x1(%r13), %rax
   8f732:      	imulq	%r13, %rax
   8f736:      	movq	%rax, %rcx
   8f739:      	shrq	$0x3f, %rcx
   8f73d:      	addq	%rax, %rcx
   8f740:      	sarq	%rcx
   8f743:      	xorl	%edi, %edi
   8f745:      	cmpq	%rcx, %r14
   8f748:      	sete	%dil
   8f74c:      	movq	-0x30(%rbp), %rax
   8f750:      	movq	%rax, -0x38(%rbp)
   8f754:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8f759:      	movq	-0x38(%rbp), %rax
   8f75d:      	movq	%rax, -0x30(%rbp)
   8f761:      	leaq	-0x30(%rbp), %r12
   8f765:      	movq	%r12, -0x128(%rbp)
   8f76c:      	leaq	0x1705fd(%rip), %rax    # 0x1ffd70 <scoop$1$bs$5a427326e8e8bc3c2113ad4359649ef96b2e4a77c8e1bcf61a98dc0c2f9b2467>
   8f773:      	movq	%rax, -0x120(%rbp)
   8f77a:      	xorps	%xmm0, %xmm0
   8f77d:      	movups	%xmm0, -0x228(%rbp)
   8f784:      	movq	$0x0, -0x218(%rbp)
   8f78f:      	leaq	-0x228(%rbp), %rbx
   8f796:      	leaq	-0x128(%rbp), %rsi
   8f79d:      	movl	$0x1, %edx
   8f7a2:      	movq	%rbx, %rdi
   8f7a5:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f7aa:      	xorps	%xmm0, %xmm0
   8f7ad:      	movups	%xmm0, -0x440(%rbp)
   8f7b4:      	movups	%xmm0, -0x430(%rbp)
   8f7bb:      	movups	%xmm0, -0x420(%rbp)
   8f7c2:      	movups	%xmm0, -0x410(%rbp)
   8f7c9:      	leaq	-0x440(%rbp), %r15
   8f7d0:      	movq	%r15, %rdi
   8f7d3:      	movq	%r15, %rsi
   8f7d6:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f7db:      	callq	0xb3800 <m34_container_begin>
   8f7e0:      	movq	%r15, %rdi
   8f7e3:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f7e8:      	movq	-0x30(%rbp), %r15
   8f7ec:      	movq	%rbx, %rdi
   8f7ef:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f7f4:      	movq	%r15, -0x30(%rbp)
   8f7f8:      	movq	%r15, -0x38(%rbp)
   8f7fc:      	movq	%r15, %rdi
   8f7ff:      	callq	0x7f510 <scoop$1$cb$19e03488b99cf6692816ef3ef3377590a052bab440cde4d03fe6a32ee9cd6741>
   8f804:      	movq	-0x38(%rbp), %rax
   8f808:      	movq	%rax, -0x30(%rbp)
   8f80c:      	movq	%rax, -0x160(%rbp)
   8f813:      	movq	%r12, -0x138(%rbp)
   8f81a:      	leaq	0x17055f(%rip), %rax    # 0x1ffd80 <scoop$1$bs$75e66033ce28459377674b8cff0c548e7c8ad30d9cadcc03ec3ed6986dbdb0a0>
   8f821:      	movq	%rax, -0x130(%rbp)
   8f828:      	xorps	%xmm0, %xmm0
   8f82b:      	movups	%xmm0, -0x240(%rbp)
   8f832:      	movq	$0x0, -0x230(%rbp)
   8f83d:      	leaq	-0x240(%rbp), %rbx
   8f844:      	leaq	-0x138(%rbp), %rsi
   8f84b:      	movl	$0x1, %edx
   8f850:      	movq	%rbx, %rdi
   8f853:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   8f858:      	xorps	%xmm0, %xmm0
   8f85b:      	movups	%xmm0, -0x480(%rbp)
   8f862:      	movups	%xmm0, -0x470(%rbp)
   8f869:      	movups	%xmm0, -0x460(%rbp)
   8f870:      	movups	%xmm0, -0x450(%rbp)
   8f877:      	leaq	-0x480(%rbp), %r15
   8f87e:      	movq	%r15, %rdi
   8f881:      	movq	%r15, %rsi
   8f884:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   8f889:      	movl	$0x3, %edi
   8f88e:      	callq	0xb38a0 <m34_container_end>
   8f893:      	movq	%r15, %rdi
   8f896:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   8f89b:      	movq	-0x30(%rbp), %r15
   8f89f:      	movq	%rbx, %rdi
   8f8a2:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   8f8a7:      	movq	%r15, -0x30(%rbp)
   8f8ab:      	xorl	%edi, %edi
   8f8ad:      	cmpq	$0x0, 0x18(%r15)
   8f8b2:      	sete	%dil
   8f8b6:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   8f8bb:      	movq	%r14, %rax
   8f8be:      	addq	$0x458, %rsp            # imm = 0x458
   8f8c5:      	popq	%rbx
   8f8c6:      	popq	%r12
   8f8c8:      	popq	%r13
   8f8ca:      	popq	%r14
   8f8cc:      	popq	%r15
   8f8ce:      	popq	%rbp
   8f8cf:      	retq

; zeroSizeList, MIR fn6, LIR scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0

/home/chenxu/repos/scoop/tmp/m34/containers-on-linux-gnu/containers:	file format elf64-x86-64

Disassembly of section .text:

000000000007fc30 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0>:
   7fc30:      	pushq	%rbp
   7fc31:      	movq	%rsp, %rbp
   7fc34:      	pushq	%r15
   7fc36:      	pushq	%r14
   7fc38:      	pushq	%r13
   7fc3a:      	pushq	%r12
   7fc3c:      	pushq	%rbx
   7fc3d:      	subq	$0x318, %rsp            # imm = 0x318
   7fc44:      	movq	%rdi, %rbx
   7fc47:      	movq	%fs:0x0, %rax
   7fc50:      	leaq	-0x10(%rax), %rax
   7fc57:      	movq	(%rax), %r12
   7fc5a:      	leaq	0x1e29af(%rip), %rax    # 0x262610 <scoop_thread_gc_epoch>
   7fc61:      	movq	(%rax), %rax
   7fc64:      	leaq	0x1e29ad(%rip), %rcx    # 0x262618 <scoop_thread_world_phase>
   7fc6b:      	movl	(%rcx), %esi
   7fc6d:      	movl	(%r12), %edx
   7fc71:      	movq	0x8(%r12), %rcx
   7fc76:      	testl	%esi, %esi
   7fc78:      	jne	0x7fc84 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x54>
   7fc7a:      	cmpl	$0x1, %edx
   7fc7d:      	jne	0x7fc84 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x54>
   7fc7f:      	cmpq	%rax, %rcx
   7fc82:      	je	0x7fc89 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x59>
   7fc84:      	callq	0x9a3c0 <scoop_rt_safepoint>
   7fc89:      	callq	0x8c9a0 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   7fc8e:      	movq	0x1e290b(%rip), %rax    # 0x2625a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   7fc95:      	movq	$0x0, 0x10(%rax)
   7fc9d:      	xorps	%xmm0, %xmm0
   7fca0:      	movups	%xmm0, -0x120(%rbp)
   7fca7:      	movq	$0x0, -0x110(%rbp)
   7fcb2:      	leaq	-0x120(%rbp), %r14
   7fcb9:      	movq	%r14, %rdi
   7fcbc:      	xorl	%esi, %esi
   7fcbe:      	xorl	%edx, %edx
   7fcc0:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7fcc5:      	xorps	%xmm0, %xmm0
   7fcc8:      	movups	%xmm0, -0x1c0(%rbp)
   7fccf:      	movups	%xmm0, -0x1b0(%rbp)
   7fcd6:      	movups	%xmm0, -0x1a0(%rbp)
   7fcdd:      	movups	%xmm0, -0x190(%rbp)
   7fce4:      	leaq	-0x1c0(%rbp), %r15
   7fceb:      	movq	%r15, %rdi
   7fcee:      	movq	%r15, %rsi
   7fcf1:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7fcf6:      	callq	0xb3800 <m34_container_begin>
   7fcfb:      	movq	%r15, %rdi
   7fcfe:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7fd03:      	movq	%r14, %rdi
   7fd06:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7fd0b:      	movq	0x193f16(%rip), %r14    # 0x213c28 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5+0x18>
   7fd12:      	movq	%r14, %r13
   7fd15:      	negq	%r13
   7fd18:      	movabsq	$0x7fffffffffffffff, %rax # imm = 0x7FFFFFFFFFFFFFFF
   7fd22:      	addq	%r14, %rax
   7fd25:      	cmpq	$-0x20, %rax
   7fd29:      	setae	-0x39(%rbp)
   7fd2d:      	leaq	0x1f(%r14), %r15
   7fd31:      	andq	%r13, %r15
   7fd34:      	movq	%fs:0x0, %rax
   7fd3d:      	leaq	-0x8(%rax), %rax
   7fd44:      	movq	(%rax), %rax
   7fd47:      	movq	(%rax), %rcx
   7fd4a:      	addq	%rcx, %r14
   7fd4d:      	decq	%r14
   7fd50:      	andq	%r13, %r14
   7fd53:      	leaq	(%r14,%r15), %rcx
   7fd57:      	testq	%r14, %r14
   7fd5a:      	sete	%dl
   7fd5d:      	cmpq	$0x7f81, %r15           # imm = 0x7F81
   7fd64:      	setae	%sil
   7fd68:      	cmpq	0x8(%rax), %rcx
   7fd6c:      	seta	%dil
   7fd70:      	cmpq	$0x0, 0x193f28(%rip)    # 0x213ca0 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5+0x90>
   7fd78:      	setne	%r8b
   7fd7c:      	orb	%sil, %r8b
   7fd7f:      	orb	%dl, %r8b
   7fd82:      	orb	-0x39(%rbp), %r8b
   7fd86:      	orb	%dil, %r8b
   7fd89:      	jne	0x7fda2 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x172>
   7fd8b:      	movq	%rcx, (%rax)
   7fd8e:      	leaq	0x193e7b(%rip), %rsi    # 0x213c10 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5>
   7fd95:      	movq	%r14, %rdi
   7fd98:      	movq	%r15, %rdx
   7fd9b:      	callq	0x9f2e0 <scoop_runtime_finish_tlab_alloc>
   7fda0:      	jmp	0x7fdb6 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x186>
   7fda2:      	leaq	0x193e67(%rip), %rdi    # 0x213c10 <scoop$1$td$b3eb2a205750996f518b645d55324364de35d0a6c31e114bbbef3d1f91aa4ad5>
   7fda9:      	movl	$0x20, %esi
   7fdae:      	callq	0x9a400 <scoop_runtime_alloc_slow>
   7fdb3:      	movq	%rax, %r14
   7fdb6:      	movq	%r14, -0x38(%rbp)
   7fdba:      	movq	%r14, %rdi
   7fdbd:      	movq	%rbx, %rsi
   7fdc0:      	callq	0x834e0 <scoop$1$cb$bbf2b48d44d7bedf22a7d36826589ee9c8d458b9388e1546c16bcf3e309cc777>
   7fdc5:      	movq	-0x38(%rbp), %rax
   7fdc9:      	movq	%rax, -0x108(%rbp)
   7fdd0:      	movq	%rax, -0x30(%rbp)
   7fdd4:      	leaq	-0x30(%rbp), %r13
   7fdd8:      	movq	%r13, -0x68(%rbp)
   7fddc:      	leaq	0x158c0d(%rip), %rax    # 0x1d89f0 <scoop$1$bs$7d75ca21523a0411697697df983d83243c69e84e9136fc1a69a60a7e81dbd4b0>
   7fde3:      	movq	%rax, -0x60(%rbp)
   7fde7:      	xorps	%xmm0, %xmm0
   7fdea:      	movups	%xmm0, -0xc0(%rbp)
   7fdf1:      	movq	$0x0, -0xb0(%rbp)
   7fdfc:      	leaq	-0xc0(%rbp), %rdi
   7fe03:      	leaq	-0x68(%rbp), %rsi
   7fe07:      	movl	$0x1, %edx
   7fe0c:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7fe11:      	xorps	%xmm0, %xmm0
   7fe14:      	movups	%xmm0, -0x200(%rbp)
   7fe1b:      	movups	%xmm0, -0x1f0(%rbp)
   7fe22:      	movups	%xmm0, -0x1e0(%rbp)
   7fe29:      	movups	%xmm0, -0x1d0(%rbp)
   7fe30:      	leaq	-0x200(%rbp), %r15
   7fe37:      	movq	%r15, %rdi
   7fe3a:      	movq	%r15, %rsi
   7fe3d:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7fe42:      	xorl	%r14d, %r14d
   7fe45:      	xorl	%edi, %edi
   7fe47:      	callq	0xb38a0 <m34_container_end>
   7fe4c:      	movq	%r15, %rdi
   7fe4f:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7fe54:      	movq	-0x30(%rbp), %r15
   7fe58:      	leaq	-0xc0(%rbp), %rdi
   7fe5f:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7fe64:      	movq	%r15, -0x30(%rbp)
   7fe68:      	movq	%r15, -0x50(%rbp)
   7fe6c:      	movq	%r13, -0xf8(%rbp)
   7fe73:      	leaq	0x158b86(%rip), %rax    # 0x1d8a00 <scoop$1$bs$f0296fc550f67d3cf183664287380d188ed2bd1c75965f8c503aa632c18ff3da>
   7fe7a:      	movq	%rax, -0xf0(%rbp)
   7fe81:      	leaq	-0x50(%rbp), %rax
   7fe85:      	movq	%rax, -0xe8(%rbp)
   7fe8c:      	leaq	0x158b7d(%rip), %rax    # 0x1d8a10 <scoop$1$bs$4fa5314fc1a6bd85b6345a7964f95cd9eb9cb0eb910f0a7618c4c86eb13216e3>
   7fe93:      	movq	%rax, -0xe0(%rbp)
   7fe9a:      	xorps	%xmm0, %xmm0
   7fe9d:      	movups	%xmm0, -0xd8(%rbp)
   7fea4:      	movq	$0x0, -0xc8(%rbp)
   7feaf:      	leaq	-0xd8(%rbp), %rdi
   7feb6:      	leaq	-0xf8(%rbp), %rsi
   7febd:      	movl	$0x2, %edx
   7fec2:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7fec7:      	xorps	%xmm0, %xmm0
   7feca:      	movups	%xmm0, -0x240(%rbp)
   7fed1:      	movups	%xmm0, -0x230(%rbp)
   7fed8:      	movups	%xmm0, -0x220(%rbp)
   7fedf:      	movups	%xmm0, -0x210(%rbp)
   7fee6:      	leaq	-0x240(%rbp), %r15
   7feed:      	movq	%r15, %rdi
   7fef0:      	movq	%r15, %rsi
   7fef3:      	callq	0x9a5e0 <scoop_rt_enter_native_borrowed>
   7fef8:      	movq	-0x50(%rbp), %rdi
   7fefc:      	callq	0xb39d0 <m34_container_layout>
   7ff01:      	movq	%r15, %rdi
   7ff04:      	callq	0x9d130 <scoop_rt_leave_native_borrowed>
   7ff09:      	movq	-0x30(%rbp), %r15
   7ff0d:      	movq	-0x50(%rbp), %r13
   7ff11:      	leaq	-0xd8(%rbp), %rdi
   7ff18:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7ff1d:      	movq	%r15, -0x30(%rbp)
   7ff21:      	movq	%r13, -0x50(%rbp)
   7ff25:      	leaq	-0x30(%rbp), %rax
   7ff29:      	movq	%rax, -0x78(%rbp)
   7ff2d:      	leaq	0x158aec(%rip), %rax    # 0x1d8a20 <scoop$1$bs$288a2f95f5369e48a3a4bf0a80dd60640c9f5bb7412b16b7bffa8bed09136e14>
   7ff34:      	movq	%rax, -0x70(%rbp)
   7ff38:      	xorps	%xmm0, %xmm0
   7ff3b:      	movups	%xmm0, -0x138(%rbp)
   7ff42:      	movq	$0x0, -0x128(%rbp)
   7ff4d:      	leaq	-0x138(%rbp), %r13
   7ff54:      	leaq	-0x78(%rbp), %rsi
   7ff58:      	movl	$0x1, %edx
   7ff5d:      	movq	%r13, %rdi
   7ff60:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   7ff65:      	xorps	%xmm0, %xmm0
   7ff68:      	movups	%xmm0, -0x280(%rbp)
   7ff6f:      	movups	%xmm0, -0x270(%rbp)
   7ff76:      	movups	%xmm0, -0x260(%rbp)
   7ff7d:      	movups	%xmm0, -0x250(%rbp)
   7ff84:      	leaq	-0x280(%rbp), %r15
   7ff8b:      	movq	%r15, %rdi
   7ff8e:      	movq	%r15, %rsi
   7ff91:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   7ff96:      	callq	0xb3800 <m34_container_begin>
   7ff9b:      	movq	%r15, %rdi
   7ff9e:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   7ffa3:      	movq	-0x30(%rbp), %r15
   7ffa7:      	movq	%r13, %rdi
   7ffaa:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   7ffaf:      	movq	%r15, -0x30(%rbp)
   7ffb3:      	leaq	0x1e2656(%rip), %r15    # 0x262610 <scoop_thread_gc_epoch>
   7ffba:      	leaq	0x1e2657(%rip), %r13    # 0x262618 <scoop_thread_world_phase>
   7ffc1:      	nopw	%cs:(%rax,%rax)
   7ffd0:      	movq	(%r15), %rax
   7ffd3:      	movl	(%r13), %esi
   7ffd7:      	movl	(%r12), %edx
   7ffdb:      	movq	0x8(%r12), %rcx
   7ffe0:      	testl	%esi, %esi
   7ffe2:      	jne	0x7ffee <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3be>
   7ffe4:      	cmpl	$0x1, %edx
   7ffe7:      	jne	0x7ffee <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3be>
   7ffe9:      	cmpq	%rax, %rcx
   7ffec:      	je	0x80003 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3d3>
   7ffee:      	movq	-0x30(%rbp), %rax
   7fff2:      	movq	%rax, -0x38(%rbp)
   7fff6:      	callq	0x9a3c0 <scoop_rt_safepoint>
   7fffb:      	movq	-0x38(%rbp), %rax
   7ffff:      	movq	%rax, -0x30(%rbp)
   80003:      	cmpq	%rbx, %r14
   80006:      	jge	0x80065 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x435>
   80008:      	movq	-0x30(%rbp), %rax
   8000c:      	movq	%rax, -0x58(%rbp)
   80010:      	movq	%rax, -0x38(%rbp)
   80014:      	movq	%rax, -0x48(%rbp)
   80018:      	callq	0x8c9a0 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   8001d:      	movq	-0x48(%rbp), %rax
   80021:      	movq	-0x38(%rbp), %rcx
   80025:      	movq	%rcx, -0x30(%rbp)
   80029:      	movq	%rax, -0x58(%rbp)
   8002d:      	movq	0x1e256c(%rip), %rax    # 0x2625a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   80034:      	incq	0x10(%rax)
   80038:      	movq	-0x30(%rbp), %rax
   8003c:      	movq	-0x58(%rbp), %rdi
   80040:      	movq	%rax, -0x38(%rbp)
   80044:      	movq	%rdi, -0x48(%rbp)
   80048:      	callq	0x81d10 <scoop$1$cb$8003ec207e511adbb97d22eb90432efef709dc7ae9e8ead9789fb9fb51659730>
   8004d:      	movq	-0x48(%rbp), %rax
   80051:      	movq	-0x38(%rbp), %rcx
   80055:      	movq	%rcx, -0x30(%rbp)
   80059:      	movq	%rax, -0x58(%rbp)
   8005d:      	incq	%r14
   80060:      	jmp	0x7ffd0 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x3a0>
   80065:      	leaq	-0x30(%rbp), %r12
   80069:      	movq	%r12, -0x88(%rbp)
   80070:      	leaq	0x1589b9(%rip), %rax    # 0x1d8a30 <scoop$1$bs$6fc0eb5b3ed64bb375b12533b5b96bdb30dd133bd066b2a1f2682261ae14df2f>
   80077:      	movq	%rax, -0x80(%rbp)
   8007b:      	xorps	%xmm0, %xmm0
   8007e:      	movups	%xmm0, -0x150(%rbp)
   80085:      	movq	$0x0, -0x140(%rbp)
   80090:      	leaq	-0x150(%rbp), %r14
   80097:      	leaq	-0x88(%rbp), %rsi
   8009e:      	movl	$0x1, %edx
   800a3:      	movq	%r14, %rdi
   800a6:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   800ab:      	xorps	%xmm0, %xmm0
   800ae:      	movups	%xmm0, -0x2c0(%rbp)
   800b5:      	movups	%xmm0, -0x2b0(%rbp)
   800bc:      	movups	%xmm0, -0x2a0(%rbp)
   800c3:      	movups	%xmm0, -0x290(%rbp)
   800ca:      	leaq	-0x2c0(%rbp), %r15
   800d1:      	movq	%r15, %rdi
   800d4:      	movq	%r15, %rsi
   800d7:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   800dc:      	movl	$0x1, %edi
   800e1:      	callq	0xb38a0 <m34_container_end>
   800e6:      	movq	%r15, %rdi
   800e9:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   800ee:      	movq	-0x30(%rbp), %r15
   800f2:      	movq	%r14, %rdi
   800f5:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   800fa:      	movq	%r15, -0x30(%rbp)
   800fe:      	cmpq	%rbx, 0x18(%r15)
   80102:      	jne	0x80125 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x4f5>
   80104:      	movq	%r15, -0x38(%rbp)
   80108:      	callq	0x8c9a0 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   8010d:      	movq	-0x38(%rbp), %rax
   80111:      	movq	%rax, -0x30(%rbp)
   80115:      	movq	0x1e2484(%rip), %rax    # 0x2625a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   8011c:      	cmpq	%rbx, 0x10(%rax)
   80120:      	sete	%al
   80123:      	jmp	0x80127 <scoop$1$cb$2b81506d58a76d1dfab247b4dee22766d604d6d0107a05bdc8ae3cefcb5ba7b0+0x4f7>
   80125:      	xorl	%eax, %eax
   80127:      	movq	-0x30(%rbp), %rcx
   8012b:      	movq	%rcx, -0x38(%rbp)
   8012f:      	movzbl	%al, %edi
   80132:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   80137:      	movq	-0x38(%rbp), %rax
   8013b:      	movq	%rax, -0x30(%rbp)
   8013f:      	movq	%r12, -0x98(%rbp)
   80146:      	leaq	0x1588f3(%rip), %rax    # 0x1d8a40 <scoop$1$bs$357f844c2360310b03a9ba68e1ec0f90aa3b34e8fe601e2d1b7840a869f85351>
   8014d:      	movq	%rax, -0x90(%rbp)
   80154:      	xorps	%xmm0, %xmm0
   80157:      	movups	%xmm0, -0x168(%rbp)
   8015e:      	movq	$0x0, -0x158(%rbp)
   80169:      	leaq	-0x168(%rbp), %rbx
   80170:      	leaq	-0x98(%rbp), %rsi
   80177:      	movl	$0x1, %edx
   8017c:      	movq	%rbx, %rdi
   8017f:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   80184:      	xorps	%xmm0, %xmm0
   80187:      	movups	%xmm0, -0x300(%rbp)
   8018e:      	movups	%xmm0, -0x2f0(%rbp)
   80195:      	movups	%xmm0, -0x2e0(%rbp)
   8019c:      	movups	%xmm0, -0x2d0(%rbp)
   801a3:      	leaq	-0x300(%rbp), %r14
   801aa:      	movq	%r14, %rdi
   801ad:      	movq	%r14, %rsi
   801b0:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   801b5:      	callq	0xb3800 <m34_container_begin>
   801ba:      	movq	%r14, %rdi
   801bd:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   801c2:      	movq	-0x30(%rbp), %r14
   801c6:      	movq	%rbx, %rdi
   801c9:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   801ce:      	movq	%r14, -0x30(%rbp)
   801d2:      	movq	%r14, -0x38(%rbp)
   801d6:      	movq	%r14, %rdi
   801d9:      	callq	0x81180 <scoop$1$cb$c1d741454a2cf8c4cf47c4b7548c8fefcac986a4f724129eabf52dcdcb0ccd30>
   801de:      	movq	-0x38(%rbp), %rax
   801e2:      	movq	%rax, -0x30(%rbp)
   801e6:      	movq	%rax, -0x100(%rbp)
   801ed:      	movq	%r12, -0xa8(%rbp)
   801f4:      	leaq	0x158855(%rip), %rax    # 0x1d8a50 <scoop$1$bs$cf9a169a8d86a7c86a7857448ab14c1b66534c2d7e7663b7b3eaba383056c69a>
   801fb:      	movq	%rax, -0xa0(%rbp)
   80202:      	xorps	%xmm0, %xmm0
   80205:      	movups	%xmm0, -0x180(%rbp)
   8020c:      	movq	$0x0, -0x170(%rbp)
   80217:      	leaq	-0x180(%rbp), %rbx
   8021e:      	leaq	-0xa8(%rbp), %rsi
   80225:      	movl	$0x1, %edx
   8022a:      	movq	%rbx, %rdi
   8022d:      	callq	0xa9240 <scoop_rt_push_caller_roots>
   80232:      	xorps	%xmm0, %xmm0
   80235:      	movups	%xmm0, -0x340(%rbp)
   8023c:      	movups	%xmm0, -0x330(%rbp)
   80243:      	movups	%xmm0, -0x320(%rbp)
   8024a:      	movups	%xmm0, -0x310(%rbp)
   80251:      	leaq	-0x340(%rbp), %r14
   80258:      	movq	%r14, %rdi
   8025b:      	movq	%r14, %rsi
   8025e:      	callq	0x9a5c0 <scoop_rt_enter_native_safe>
   80263:      	movl	$0x3, %edi
   80268:      	callq	0xb38a0 <m34_container_end>
   8026d:      	movq	%r14, %rdi
   80270:      	callq	0x9d0f0 <scoop_rt_leave_native_safe>
   80275:      	movq	-0x30(%rbp), %r14
   80279:      	movq	%rbx, %rdi
   8027c:      	callq	0xa92f0 <scoop_rt_pop_caller_roots>
   80281:      	movq	%r14, -0x30(%rbp)
   80285:      	xorl	%edi, %edi
   80287:      	cmpq	$0x0, 0x18(%r14)
   8028c:      	sete	%dil
   80290:      	callq	0x838b0 <scoop$1$cb$2df5aefb30416aa5c6d48771c2f46e14c9fe2343f7a8e1a3230dfd8e3d16b6f8>
   80295:      	callq	0x8c9a0 <scoop$1$cb$b063da8e0c56b9e121e576f8f04bb91e6d1bdacdc6f1e9257e002565af59bbe0>
   8029a:      	movq	0x1e22ff(%rip), %rax    # 0x2625a0 <scoop$1$ss$75d791c8a4cf3099340f92392f51084611c886fc60ee5f70a5d384b272bffa1e>
   802a1:      	movq	0x10(%rax), %rax
   802a5:      	addq	$0x318, %rsp            # imm = 0x318
   802ac:      	popq	%rbx
   802ad:      	popq	%r12
   802af:      	popq	%r13
   802b1:      	popq	%r14
   802b3:      	popq	%r15
   802b5:      	popq	%rbp
   802b6:      	retq
