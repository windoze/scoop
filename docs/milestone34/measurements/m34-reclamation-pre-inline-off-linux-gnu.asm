
/home/chenxu/repos/scoop/tmp/m34/reclamation-off-linux-gnu/region-churn:	file format elf64-x86-64

Disassembly of section .text:

000000000005c930 <scoop$1$cb$a59ba8328a87a3c09df1111305261ccbca23796630e0ce6dea24e3ae106f48ca>:
   5c930:      	pushq	%rbp
   5c931:      	movq	%rsp, %rbp
   5c934:      	pushq	%r15
   5c936:      	pushq	%r14
   5c938:      	pushq	%r13
   5c93a:      	pushq	%r12
   5c93c:      	pushq	%rbx
   5c93d:      	subq	$0xc8, %rsp
   5c944:      	movq	%fs:0x0, %rax
   5c94d:      	leaq	-0x10(%rax), %rax
   5c954:      	movq	(%rax), %rdi
   5c957:      	leaq	, %rax <scoop_thread_gc_epoch>
   5c95e:      	movq	(%rax), %rax
   5c961:      	leaq	, %rcx <scoop_thread_world_phase>
   5c968:      	movl	(%rcx), %esi
   5c96a:      	movl	(%rdi), %edx
   5c96c:      	movq	%rdi, -0x38(%rbp)
   5c970:      	movq	0x8(%rdi), %rcx
   5c974:      	testl	%esi, %esi
   5c976:      	jne	 <L0>
   5c978:      	cmpl	$0x1, %edx
   5c97b:      	jne	 <L0>
   5c97d:      	cmpq	%rax, %rcx
   5c980:      	je	 <L1>
<L0>:
   5c982:      	callq	 <scoop_rt_safepoint>
<L1>:
   5c987:      	xorps	%xmm0, %xmm0
   5c98a:      	movups	%xmm0, -0x50(%rbp)
   5c98e:      	movq	$0x0, -0x40(%rbp)
   5c996:      	xorl	%ebx, %ebx
   5c998:      	leaq	-0x50(%rbp), %r14
   5c99c:      	movq	%r14, %rdi
   5c99f:      	xorl	%esi, %esi
   5c9a1:      	xorl	%edx, %edx
   5c9a3:      	callq	 <scoop_rt_push_caller_roots>
   5c9a8:      	xorps	%xmm0, %xmm0
   5c9ab:      	movups	%xmm0, -0xa8(%rbp)
   5c9b2:      	movups	%xmm0, -0x98(%rbp)
   5c9b9:      	movups	%xmm0, -0x88(%rbp)
   5c9c0:      	movups	%xmm0, -0x78(%rbp)
   5c9c4:      	leaq	-0xa8(%rbp), %r13
   5c9cb:      	movq	%r13, %rdi
   5c9ce:      	movq	%r13, %rsi
   5c9d1:      	callq	 <scoop_rt_enter_native_safe>
   5c9d6:      	callq	 <m34_memory_pins>
   5c9db:      	movl	%eax, %r12d
   5c9de:      	movq	%r13, %rdi
   5c9e1:      	callq	 <scoop_rt_leave_native_safe>
   5c9e6:      	movq	%r14, %rdi
   5c9e9:      	callq	 <scoop_rt_pop_caller_roots>
   5c9ee:      	movl	$0x2, %r14d
   5c9f4:      	movzbl	%r12b, %eax
   5c9f8:      	movl	%eax, -0x2c(%rbp)
   5c9fb:      	leaq	-0xe8(%rbp), %r13
   5ca02:      	xorl	%r12d, %r12d
   5ca05:      	nopw	%cs:(%rax,%rax)
<L2>:
   5ca10:      	leaq	, %rax <scoop_thread_gc_epoch>
   5ca17:      	movq	(%rax), %rax
   5ca1a:      	leaq	, %rcx <scoop_thread_world_phase>
   5ca21:      	movl	(%rcx), %esi
   5ca23:      	movq	-0x38(%rbp), %rcx
   5ca27:      	movl	(%rcx), %edx
   5ca29:      	movq	0x8(%rcx), %rcx
   5ca2d:      	testl	%esi, %esi
   5ca2f:      	jne	 <L3>
   5ca31:      	cmpl	$0x1, %edx
   5ca34:      	jne	 <L3>
   5ca36:      	cmpq	%rax, %rcx
   5ca39:      	je	 <L4>
<L3>:
   5ca3b:      	callq	 <scoop_rt_safepoint>
<L4>:
   5ca40:      	cmpl	$0x2, %ebx
   5ca43:      	jge	 <L5>
   5ca45:      	movl	%ebx, %edi
   5ca47:      	movl	-0x2c(%rbp), %esi
   5ca4a:      	callq	 <scoop$1$cb$9fcafbb211d8f976a0bd7b23d8482b48f61bd5b743ce18a7cd5abcb7f9166e4d>
   5ca4f:      	addq	%rax, %r12
   5ca52:      	callq	 <scoop_rt_gc_collect>
   5ca57:      	xorps	%xmm0, %xmm0
   5ca5a:      	movups	%xmm0, -0x68(%rbp)
   5ca5e:      	movq	$0x0, -0x58(%rbp)
   5ca66:      	leaq	-0x68(%rbp), %r15
   5ca6a:      	movq	%r15, %rdi
   5ca6d:      	xorl	%esi, %esi
   5ca6f:      	xorl	%edx, %edx
   5ca71:      	callq	 <scoop_rt_push_caller_roots>
   5ca76:      	xorps	%xmm0, %xmm0
   5ca79:      	movups	%xmm0, -0xe8(%rbp)
   5ca80:      	movups	%xmm0, -0xd8(%rbp)
   5ca87:      	movups	%xmm0, -0xc8(%rbp)
   5ca8e:      	movups	%xmm0, -0xb8(%rbp)
   5ca95:      	movq	%r13, %rdi
   5ca98:      	movq	%r13, %rsi
   5ca9b:      	callq	 <scoop_rt_enter_native_safe>
   5caa0:      	movl	%r14d, %edi
   5caa3:      	callq	 <m34_memory_sample>
   5caa8:      	movq	%r13, %rdi
   5caab:      	callq	 <scoop_rt_leave_native_safe>
   5cab0:      	movq	%r15, %rdi
   5cab3:      	callq	 <scoop_rt_pop_caller_roots>
   5cab8:      	incl	%ebx
   5caba:      	addl	$0x3, %r14d
   5cabe:      	jmp	 <L2>
<L5>:
   5cac3:      	movq	%r12, %rdi
   5cac6:      	callq	 <scoop$1$cb$8648ec25e6c21462d01c0f426d8f28e404131ecf913864474bfc5c19d1c94a73>
   5cacb:      	addq	$0xc8, %rsp
   5cad2:      	popq	%rbx
   5cad3:      	popq	%r12
   5cad5:      	popq	%r13
   5cad7:      	popq	%r14
   5cad9:      	popq	%r15
   5cadb:      	popq	%rbp
   5cadc:      	retq

0000000000061ff0 <scoop_gc_heap_plan_moving_locked>:
   61ff0:      	endbr64
   61ff4:      	pushq	%rbp
   61ff5:      	movq	%rsp, %rbp
   61ff8:      	pushq	%r15
   61ffa:      	pushq	%r14
   61ffc:      	pushq	%r13
   61ffe:      	pushq	%r12
   62000:      	pushq	%rbx
   62001:      	subq	$0x38, %rsp
   62005:      	leaq	, %r15 <scoop_gc_heap_state>
   6200c:      	movzbl	0x85(%r15), %r14d
   62014:      	testb	%r14b, %r14b
   62017:      	je	 <L45>
   6201d:      	movl	%edi, %ebx
   6201f:      	callq	 <scoop_heap_first_block>
   62024:      	movq	%rax, %r13
   62027:      	testq	%rax, %rax
   6202a:      	je	 <L5>
   6202c:      	xorl	%r12d, %r12d
   6202f:      	nop
<L0>:
   62030:      	movq	%r13, %rdi
   62033:      	callq	 <scoop_heap_active_head>
   62038:      	testb	%al, %al
   6203a:      	je	 <L3>
   6203c:      	cmpq	$0x0, 0x70(%r13)
   62041:      	je	 <L3>
   62043:      	testb	%bl, %bl
   62045:      	je	 <L15>
   6204b:      	movl	0x1c(%r13), %eax
   6204f:      	testl	%eax, %eax
   62051:      	jne	 <L3>
   62053:      	movl	0x18(%r13), %esi
   62057:      	cmpl	$0x2, %esi
   6205a:      	je	 <L44>
   62060:      	movq	0x30(%r13), %rax
   62064:      	leaq	0x200(%rax), %rcx
   6206b:      	jmp	 <L2>
   6206d:      	nopl	(%rax)
<L1>:
   62070:      	addq	$0x8, %rax
   62074:      	cmpq	%rcx, %rax
   62077:      	je	 <L43>
<L2>:
   6207d:      	cmpq	$0x0, (%rax)
   62081:      	je	 <L1>
   62083:      	nop
   62085:      	nopw	%cs:(%rax,%rax)
<L3>:
   62090:      	movq	%r13, %rdi
   62093:      	callq	 <scoop_heap_next_block>
   62098:      	movq	%rax, %r13
   6209b:      	testq	%rax, %rax
   6209e:      	jne	 <L0>
<L4>:
   620a0:      	testb	%bl, %bl
   620a2:      	je	 <L26>
<L5>:
   620a8:      	movq	$0x0, -0x48(%rbp)
   620b0:      	movq	$0x0, -0x40(%rbp)
   620b8:      	movq	$0x0, -0x38(%rbp)
   620c0:      	callq	 <scoop_heap_first_block>
   620c5:      	movq	%rax, %r12
   620c8:      	testq	%rax, %rax
   620cb:      	je	 <L22>
   620d1:      	nopl	(%rax)
   620d5:      	nopw	%cs:(%rax,%rax)
<L6>:
   620e0:      	movl	%r14d, %r13d
   620e3:      	cmpl	$0x3, 0x14(%r12)
   620e9:      	jne	 <L10>
   620ef:      	movl	$0x10, %ebx
   620f4:      	cmpl	$0x2, 0x18(%r12)
   620fa:      	jne	 <L9>
   620fc:      	jmp	 <L21>
   62101:      	nopl	(%rax)
<L7>:
   62108:      	addq	$0x1, %rbx
   6210c:      	movl	%r14d, %r13d
   6210f:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   62116:      	je	 <L10>
<L8>:
   62118:      	testb	%r13b, %r13b
   6211b:      	je	 <L10>
<L9>:
   6211d:      	movq	0x20(%r12), %rdi
   62122:      	movq	%rbx, %rsi
   62125:      	callq	 <scoop_heap_bit_test>
   6212a:      	testb	%al, %al
   6212c:      	je	 <L7>
   6212e:      	movq	0x28(%r12), %rdi
   62133:      	movq	%rbx, %rsi
   62136:      	callq	 <scoop_heap_bit_test>
   6213b:      	testb	%al, %al
   6213d:      	je	 <L7>
   6213f:      	movq	0x30(%r12), %rdi
   62144:      	movq	%rbx, %rsi
   62147:      	callq	 <scoop_heap_bit_test>
   6214c:      	testb	%al, %al
   6214e:      	jne	 <L7>
   62150:      	movq	%rbx, %r8
   62153:      	leaq	-0x38(%rbp), %rdx
   62157:      	leaq	-0x40(%rbp), %rsi
   6215b:      	movq	%r12, %rcx
   6215e:      	leaq	-0x48(%rbp), %rdi
   62162:      	addq	$0x1, %rbx
   62166:      	callq	 <append_move>
   6216b:      	movl	%eax, %r13d
   6216e:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   62175:      	jne	 <L8>
   62177:      	nopw	(%rax,%rax)
<L10>:
   62180:      	movq	%r12, %rdi
   62183:      	callq	 <scoop_heap_next_block>
   62188:      	movq	%rax, %r12
   6218b:      	testq	%rax, %rax
   6218e:      	je	 <L11>
   62190:      	testb	%r13b, %r13b
   62193:      	jne	 <L6>
<L11>:
   62199:      	movq	-0x48(%rbp), %r12
   6219d:      	testb	%r13b, %r13b
   621a0:      	je	 <L30>
   621a6:      	movq	-0x40(%rbp), %rax
   621aa:      	movq	%rax, -0x58(%rbp)
   621ae:      	testq	%rax, %rax
   621b1:      	je	 <L22>
   621b7:      	movq	%r12, %r13
   621ba:      	xorl	%ebx, %ebx
   621bc:      	jmp	 <L14>
   621be:      	nop
<L12>:
   621c0:      	movq	0x8(%r13), %rsi
   621c4:      	movl	$0x1, %ecx
   621c9:      	callq	 <scoop_heap_record_small_object>
   621ce:      	movq	-0x60(%rbp), %rax
   621d2:      	movq	0x8(%r13), %rcx
   621d6:      	movq	0x18(%r13), %rdx
   621da:      	movq	0x58(%rax), %rax
   621de:      	movq	%rcx, (%rax,%rdx,8)
<L13>:
   621e2:      	addq	$0x1, 0x190(%r15)
   621ea:      	addq	$0x1, %rbx
   621ee:      	addq	$0x30, %r13
   621f2:      	cmpq	-0x58(%rbp), %rbx
   621f6:      	je	 <L22>
<L14>:
   621fc:      	movq	0x8(%r13), %rdi
   62200:      	movq	0x10(%r13), %rdx
   62204:      	movq	(%r13), %rsi
   62208:      	callq	 <memcpy@plt>
   6220d:      	movq	0x10(%r13), %rax
   62211:      	addq	%rax, 0x110(%r15)
   62218:      	movq	0x20(%r13), %rax
   6221c:      	movq	0x10(%r13), %rdx
   62220:      	movq	0x28(%r13), %rdi
   62224:      	movq	%rax, -0x60(%rbp)
   62228:      	cmpq	$0x7f80, %rdx           # imm = 0x7F80
   6222f:      	jbe	 <L12>
   62231:      	movl	$0x1, %esi
   62236:      	callq	 <scoop_heap_publish_large_object>
   6223b:      	movq	0x8(%r13), %rdx
   6223f:      	movq	-0x60(%rbp), %rax
   62243:      	movq	%rdx, 0x78(%rax)
   62247:      	jmp	 <L13>
   62249:      	nopl	(%rax)
<L15>:
   62250:      	cmpb	$0x0, 0x81(%r15)
   62258:      	jne	 <L24>
   6225e:      	testq	%r12, %r12
   62261:      	je	 <L20>
   62267:      	movq	0x68(%r13), %rax
   6226b:      	testq	%rax, %rax
   6226e:      	js	 <L28>
   62274:      	pxor	%xmm4, %xmm4
   62278:      	movq	%r13, %rdi
   6227b:      	cvtsi2sd	%rax, %xmm4
   62280:      	movsd	%xmm4, -0x58(%rbp)
   62285:      	callq	 <scoop_heap_block_bytes>
   6228a:      	testq	%rax, %rax
   6228d:      	js	 <L29>
<L16>:
   62293:      	pxor	%xmm0, %xmm0
   62297:      	cvtsi2sd	%rax, %xmm0
<L17>:
   6229c:      	movsd	-0x58(%rbp), %xmm2
   622a1:      	movq	0x68(%r12), %rax
   622a6:      	divsd	%xmm0, %xmm2
   622aa:      	movsd	%xmm2, -0x58(%rbp)
   622af:      	testq	%rax, %rax
   622b2:      	js	 <L42>
   622b8:      	pxor	%xmm5, %xmm5
   622bc:      	cvtsi2sd	%rax, %xmm5
   622c1:      	movsd	%xmm5, -0x60(%rbp)
<L18>:
   622c6:      	movq	%r12, %rdi
   622c9:      	callq	 <scoop_heap_block_bytes>
   622ce:      	testq	%rax, %rax
   622d1:      	js	 <L41>
   622d7:      	pxor	%xmm0, %xmm0
   622db:      	cvtsi2sd	%rax, %xmm0
<L19>:
   622e0:      	movsd	-0x60(%rbp), %xmm1
   622e5:      	divsd	%xmm0, %xmm1
   622e9:      	comisd	-0x58(%rbp), %xmm1
   622ee:      	jbe	 <L3>
<L20>:
   622f4:      	movq	%r13, %rdi
   622f7:      	movq	%r13, %r12
   622fa:      	callq	 <scoop_heap_next_block>
   622ff:      	movq	%rax, %r13
   62302:      	testq	%rax, %rax
   62305:      	jne	 <L0>
   6230b:      	jmp	 <L4>
<L21>:
   62310:      	leaq	-0x38(%rbp), %rdx
   62314:      	leaq	-0x40(%rbp), %rsi
   62318:      	movl	$0x10, %r8d
   6231e:      	movq	%r12, %rcx
   62321:      	leaq	-0x48(%rbp), %rdi
   62325:      	callq	 <append_move>
   6232a:      	movl	%eax, %r13d
   6232d:      	jmp	 <L10>
   62332:      	nopw	(%rax,%rax)
<L22>:
   62338:      	movq	%r12, %rdi
   6233b:      	callq	 <free@plt>
<L23>:
   62340:      	addq	$0x38, %rsp
   62344:      	movl	%r14d, %eax
   62347:      	popq	%rbx
   62348:      	popq	%r12
   6234a:      	popq	%r13
   6234c:      	popq	%r14
   6234e:      	popq	%r15
   62350:      	popq	%rbp
   62351:      	retq
   62352:      	nopw	(%rax,%rax)
<L24>:
   62358:      	movl	$0x3, 0x14(%r13)
   62360:      	cmpl	$0x1, 0x18(%r13)
   62365:      	jne	 <L3>
<L25>:
   6236b:      	movl	$0x8, %esi
   62370:      	movl	$0x1000, %edi           # imm = 0x1000
   62375:      	callq	 <calloc@plt>
   6237a:      	movq	%rax, 0x58(%r13)
   6237e:      	testq	%rax, %rax
   62381:      	je	 <L27>
   62383:      	movq	%r13, %rdi
   62386:      	callq	 <scoop_heap_next_block>
   6238b:      	movq	%rax, %r13
   6238e:      	testq	%rax, %rax
   62391:      	jne	 <L0>
   62397:      	jmp	 <L4>
   6239c:      	nopl	(%rax)
<L26>:
   623a0:      	cmpb	$0x1, 0x81(%r15)
   623a8:      	je	 <L5>
   623ae:      	testq	%r12, %r12
   623b1:      	je	 <L5>
   623b7:      	movl	$0x3, 0x14(%r12)
   623c0:      	cmpl	$0x1, 0x18(%r12)
   623c6:      	jne	 <L5>
   623cc:      	movl	$0x8, %esi
   623d1:      	movl	$0x1000, %edi           # imm = 0x1000
   623d6:      	callq	 <calloc@plt>
   623db:      	movq	%rax, 0x58(%r12)
   623e0:      	testq	%rax, %rax
   623e3:      	jne	 <L5>
<L27>:
   623e9:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x3af4>
   623f0:      	callq	 <scoop_heap_fatal>
   623f5:      	nopl	(%rax)
<L28>:
   623f8:      	movq	%rax, %rcx
   623fb:      	andl	$0x1, %eax
   623fe:      	pxor	%xmm0, %xmm0
   62402:      	movq	%r13, %rdi
   62405:      	shrq	%rcx
   62408:      	orq	%rax, %rcx
   6240b:      	cvtsi2sd	%rcx, %xmm0
   62410:      	addsd	%xmm0, %xmm0
   62414:      	movsd	%xmm0, -0x58(%rbp)
   62419:      	callq	 <scoop_heap_block_bytes>
   6241e:      	testq	%rax, %rax
   62421:      	jns	 <L16>
<L29>:
   62427:      	movq	%rax, %rcx
   6242a:      	andl	$0x1, %eax
   6242d:      	pxor	%xmm0, %xmm0
   62431:      	shrq	%rcx
   62434:      	orq	%rax, %rcx
   62437:      	cvtsi2sd	%rcx, %xmm0
   6243c:      	addsd	%xmm0, %xmm0
   62440:      	jmp	 <L17>
   62445:      	nopl	(%rax)
<L30>:
   62448:      	movq	%r12, %rdi
   6244b:      	callq	 <free@plt>
   62450:      	callq	 <scoop_heap_first_block>
   62455:      	movq	%rax, %rbx
   62458:      	testq	%rax, %rax
   6245b:      	jne	 <L32>
   6245d:      	jmp	 <L37>
   6245f:      	nop
<L31>:
   62460:      	movq	%rbx, %rdi
   62463:      	callq	 <scoop_heap_next_block>
   62468:      	movq	%rax, %rbx
   6246b:      	testq	%rax, %rax
   6246e:      	je	 <L37>
<L32>:
   62470:      	movl	0x14(%rbx), %eax
   62473:      	cmpl	$0x4, %eax
   62476:      	je	 <L38>
   6247c:      	cmpl	$0x3, %eax
   6247f:      	jne	 <L31>
   62481:      	cmpl	$0x2, 0x18(%rbx)
   62485:      	je	 <L39>
   6248b:      	movq	0x30(%rbx), %rdx
   6248f:      	leaq	0x200(%rdx), %rax
   62496:      	jmp	 <L34>
   62498:      	nopl	(%rax,%rax)
<L33>:
   624a0:      	addq	$0x8, %rdx
   624a4:      	cmpq	%rdx, %rax
   624a7:      	je	 <L40>
<L34>:
   624a9:      	cmpq	$0x0, (%rdx)
   624ad:      	je	 <L33>
<L35>:
   624af:      	movl	$0x5, %eax
<L36>:
   624b4:      	movl	%eax, 0x14(%rbx)
   624b7:      	movq	0x58(%rbx), %rdi
   624bb:      	callq	 <free@plt>
   624c0:      	movq	$0x0, 0x58(%rbx)
   624c8:      	movq	%rbx, %rdi
   624cb:      	callq	 <scoop_heap_next_block>
   624d0:      	movq	%rax, %rbx
   624d3:      	testq	%rax, %rax
   624d6:      	jne	 <L32>
<L37>:
   624d8:      	movzbl	0x81(%r15), %r14d
   624e0:      	pxor	%xmm0, %xmm0
   624e4:      	movq	$0x0, 0x188(%r15)
   624ef:      	movups	%xmm0, 0x178(%r15)
   624f7:      	testb	%r14b, %r14b
   624fa:      	je	 <L23>
   62500:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x3b24>
   62507:      	callq	 <scoop_heap_fatal>
   6250c:      	nopl	(%rax)
<L38>:
   62510:      	movq	%rbx, %rdi
   62513:      	callq	 <scoop_heap_release_block>
   62518:      	jmp	 <L31>
<L39>:
   6251d:      	cmpb	$0x0, 0x82(%rbx)
   62524:      	jne	 <L35>
<L40>:
   62526:      	movl	$0x2, %eax
   6252b:      	jmp	 <L36>
<L41>:
   6252d:      	movq	%rax, %rcx
   62530:      	andl	$0x1, %eax
   62533:      	pxor	%xmm0, %xmm0
   62537:      	shrq	%rcx
   6253a:      	orq	%rax, %rcx
   6253d:      	cvtsi2sd	%rcx, %xmm0
   62542:      	addsd	%xmm0, %xmm0
   62546:      	jmp	 <L19>
<L42>:
   6254b:      	movq	%rax, %rcx
   6254e:      	andl	$0x1, %eax
   62551:      	pxor	%xmm0, %xmm0
   62555:      	shrq	%rcx
   62558:      	orq	%rax, %rcx
   6255b:      	cvtsi2sd	%rcx, %xmm0
   62560:      	addsd	%xmm0, %xmm0
   62564:      	movsd	%xmm0, -0x60(%rbp)
   62569:      	jmp	 <L18>
<L43>:
   6256e:      	movl	$0x3, 0x14(%r13)
   62576:      	cmpl	$0x1, %esi
   62579:      	je	 <L25>
   6257f:      	movq	%r13, %rdi
   62582:      	callq	 <scoop_heap_next_block>
   62587:      	movq	%rax, %r13
   6258a:      	testq	%rax, %rax
   6258d:      	jne	 <L0>
   62593:      	jmp	 <L4>
<L44>:
   62598:      	cmpb	$0x0, 0x82(%r13)
   625a0:      	jne	 <L3>
   625a6:      	movl	$0x3, 0x14(%r13)
   625ae:      	jmp	 <L3>
<L45>:
   625b3:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x3acc>
   625ba:      	callq	 <scoop_heap_fatal>
   625bf:      	nop

000000000006d3f0 <scoop_heap_region_destroy>:
   6d3f0:      	endbr64
   6d3f4:      	pushq	%rbp
   6d3f5:      	movq	%rsp, %rbp
   6d3f8:      	pushq	%r14
   6d3fa:      	movl	$0x88, %r14d
   6d400:      	pushq	%r13
   6d402:      	movq	%rdi, %r13
   6d405:      	pushq	%r12
   6d407:      	pushq	%rbx
   6d408:      	callq	 <scoop_heap_page_map_remove>
   6d40d:      	movq	0x8(%r13), %rsi
   6d411:      	movq	(%r13), %rdi
   6d415:      	callq	 <release_mapping>
   6d41a:      	cmpb	$0x1, 0x2a(%r13)
   6d41f:      	sbbq	%rax, %rax
   6d422:      	testl	$0x1ff, %eax            # imm = 0x1FF
   6d427:      	movl	$0x11000, %eax          # imm = 0x11000
   6d42c:      	cmovneq	%rax, %r14
   6d430:      	xorl	%r12d, %r12d
   6d433:      	nop
   6d435:      	nopw	%cs:(%rax,%rax)
<L0>:
   6d440:      	movq	0x20(%r13), %rbx
   6d444:      	addq	%r12, %rbx
   6d447:      	addq	$0x88, %r12
   6d44e:      	movq	0x20(%rbx), %rdi
   6d452:      	callq	 <free@plt>
   6d457:      	movq	0x28(%rbx), %rdi
   6d45b:      	callq	 <free@plt>
   6d460:      	movq	0x30(%rbx), %rdi
   6d464:      	callq	 <free@plt>
   6d469:      	movq	0x38(%rbx), %rdi
   6d46d:      	callq	 <free@plt>
   6d472:      	movq	0x40(%rbx), %rdi
   6d476:      	callq	 <free@plt>
   6d47b:      	movq	0x48(%rbx), %rdi
   6d47f:      	callq	 <free@plt>
   6d484:      	movq	0x50(%rbx), %rdi
   6d488:      	callq	 <free@plt>
   6d48d:      	movq	0x58(%rbx), %rdi
   6d491:      	callq	 <free@plt>
   6d496:      	cmpq	%r12, %r14
   6d499:      	jne	 <L0>
   6d49b:      	movq	0x20(%r13), %rdi
   6d49f:      	callq	 <free@plt>
   6d4a4:      	movq	0x10(%r13), %rdi
   6d4a8:      	callq	 <free@plt>
   6d4ad:      	movq	%r13, %rdi
   6d4b0:      	callq	 <free@plt>
   6d4b5:      	popq	%rbx
   6d4b6:      	popq	%r12
   6d4b8:      	popq	%r13
   6d4ba:      	popq	%r14
   6d4bc:      	popq	%rbp
   6d4bd:      	retq
   6d4be:      	nop

0000000000073990 <scoop_gc_heap_finish_collection_locked>:
   73990:      	endbr64
   73994:      	pushq	%rbp
   73995:      	movq	%rsp, %rbp
   73998:      	pushq	%r15
   7399a:      	pushq	%r14
   7399c:      	pushq	%r13
   7399e:      	pushq	%r12
   739a0:      	pushq	%rbx
   739a1:      	subq	$0x68, %rsp
   739a5:      	leaq	, %rax <scoop_gc_heap_state>
   739ac:      	movl	%esi, -0x68(%rbp)
   739af:      	cmpb	$0x0, 0x85(%rax)
   739b6:      	je	 <L51>
   739bc:      	movq	%rdi, %r12
   739bf:      	leaq	-0x50(%rbp), %rdi
   739c3:      	callq	 <scoop_thread_allocation_totals_locked>
   739c8:      	movq	-0x50(%rbp), %r14
   739cc:      	movq	-0x48(%rbp), %rbx
   739d0:      	cmpb	$0x0, -0x68(%rbp)
   739d4:      	je	 <L47>
   739da:      	leaq	, %rcx <scoop_gc_heap_state>
   739e1:      	movq	0x70(%rcx), %rax
   739e5:      	addq	%r14, %rax
   739e8:      	addq	0x60(%rcx), %rax
   739ec:      	subq	0x68(%rcx), %rax
   739f0:      	subq	%rbx, %rax
   739f3:      	addq	%rax, %r12
<L0>:
   739f6:      	callq	 <scoop_heap_first_block>
   739fb:      	testq	%rax, %rax
   739fe:      	je	 <L10>
   73a04:      	movq	%r14, -0x80(%rbp)
   73a08:      	movq	%rax, %r15
   73a0b:      	movq	%rbx, %r14
   73a0e:      	movq	%r12, -0x88(%rbp)
   73a15:      	jmp	 <L4>
   73a17:      	nopw	(%rax,%rax)
<L1>:
   73a20:      	testb	%dl, %dl
   73a22:      	je	 <L45>
   73a28:      	cmpb	$0x0, 0x82(%r15)
   73a30:      	movl	$0x5, %edx
   73a35:      	cmovnel	%edx, %eax
<L2>:
   73a38:      	movl	%eax, 0x14(%r15)
   73a3c:      	pxor	%xmm2, %xmm2
   73a40:      	movq	$0x0, 0x78(%r15)
   73a48:      	movb	$0x0, 0x81(%r15)
   73a50:      	movb	$0x0, 0x83(%r15)
   73a58:      	movups	%xmm2, 0x68(%r15)
   73a5d:      	nopl	(%rax)
<L3>:
   73a60:      	movq	%r15, %rdi
   73a63:      	callq	 <scoop_heap_next_block>
   73a68:      	movq	%rax, %r15
   73a6b:      	testq	%rax, %rax
   73a6e:      	je	 <L9>
<L4>:
   73a74:      	movq	%r15, %rdi
   73a77:      	callq	 <scoop_heap_active_head>
   73a7c:      	testb	%al, %al
   73a7e:      	je	 <L3>
   73a80:      	movl	0x1c(%r15), %eax
   73a84:      	cmpb	$0x0, -0x68(%rbp)
   73a88:      	je	 <L5>
   73a8a:      	cmpl	$0x1, %eax
   73a8d:      	je	 <L43>
<L5>:
   73a93:      	testl	%eax, %eax
   73a95:      	jne	 <L6>
   73a97:      	leaq	, %rcx <scoop_gc_heap_state>
   73a9e:      	movq	0x68(%r15), %rax
   73aa2:      	addq	%rax, 0xb8(%rcx)
<L6>:
   73aa9:      	movl	0x18(%r15), %eax
   73aad:      	movl	$0x1, 0x1c(%r15)
   73ab5:      	cmpl	$0x1, %eax
   73ab8:      	je	 <L16>
   73abe:      	cmpl	$0x2, %eax
   73ac1:      	jne	 <L53>
   73ac7:      	leaq	, %rcx <scoop_gc_heap_state>
   73ace:      	movzbl	0x81(%r15), %edx
   73ad6:      	movzbl	0x81(%rcx), %ebx
   73add:      	cmpl	$0x3, 0x14(%r15)
   73ae2:      	jne	 <L1>
   73ae8:      	cmpb	$0x0, 0x82(%r15)
   73af0:      	jne	 <L44>
   73af6:      	testb	%dl, %dl
   73af8:      	je	 <L45>
   73afe:      	cmpq	$0x0, 0x78(%r15)
   73b03:      	je	 <L52>
<L7>:
   73b09:      	testb	%bl, %bl
   73b0b:      	je	 <L14>
   73b11:      	movq	0x60(%r15), %rbx
   73b15:      	movq	%r15, %rdi
   73b18:      	callq	 <scoop_heap_block_base>
   73b1d:      	movl	$0xa5, %esi
   73b22:      	leaq	0x80(%rax), %rdi
   73b29:      	movq	%rbx, %rdx
   73b2c:      	callq	 <memset@plt>
   73b31:      	movq	%r15, %rdi
<L8>:
   73b34:      	callq	 <scoop_heap_quarantine_block>
   73b39:      	movq	%r15, %rdi
   73b3c:      	callq	 <scoop_heap_next_block>
   73b41:      	movq	%rax, %r15
   73b44:      	testq	%rax, %rax
   73b47:      	jne	 <L4>
<L9>:
   73b4d:      	movq	%r14, %rbx
   73b50:      	movq	-0x88(%rbp), %r12
   73b57:      	movq	-0x80(%rbp), %r14
<L10>:
   73b5b:      	leaq	, %rax <scoop_gc_heap_state>
   73b62:      	movq	%r12, %xmm4
   73b67:      	movq	%r14, %xmm5
   73b6c:      	pxor	%xmm0, %xmm0
   73b70:      	punpcklqdq	%xmm5, %xmm4    # xmm4 = xmm4[0],xmm5[0]
   73b74:      	movq	$0x0, 0x188(%rax)
   73b7f:      	movups	%xmm0, 0x178(%rax)
   73b86:      	movaps	%xmm4, -0x60(%rbp)
   73b8a:      	callq	 <scoop_heap_release_empty_large_regions>
   73b8f:      	leaq	, %rax <scoop_gc_heap_state>
   73b96:      	movq	0x28(%rax), %r12
   73b9a:      	testq	%r12, %r12
   73b9d:      	je	 <L12>
   73b9f:      	nop
<L11>:
   73ba0:      	movq	0x8(%r12), %rdx
   73ba5:      	movq	0x10(%r12), %rdi
   73baa:      	xorl	%esi, %esi
   73bac:      	shrq	$0x9, %rdx
   73bb0:      	callq	 <memset@plt>
   73bb5:      	movq	0x18(%r12), %r12
   73bba:      	testq	%r12, %r12
   73bbd:      	jne	 <L11>
<L12>:
   73bbf:      	leaq	, %rax <scoop_gc_heap_state>
   73bc6:      	movdqa	-0x60(%rbp), %xmm6
   73bcb:      	movq	$0x0, 0x88(%rax)
   73bd6:      	movq	%rbx, 0x70(%rax)
   73bda:      	movups	%xmm6, 0x60(%rax)
   73bde:      	leaq	, %rax <scoop_gc_heap_state>
   73be5:      	leaq	, %rcx <scoop_gc_heap_state>
   73bec:      	movq	0x190(%rax), %rax
   73bf3:      	movq	%rax, 0x78(%rcx)
   73bf7:      	cmpb	$0x0, -0x68(%rbp)
   73bfb:      	jne	 <L13>
   73bfd:      	leaq	, %rax <scoop_gc_heap_state>
   73c04:      	movq	0x48(%rax), %rcx
   73c08:      	movq	$-0x1, %rax
   73c0f:      	leaq	(%rcx,%rcx), %rdx
   73c13:      	testq	%rcx, %rcx
   73c16:      	cmovnsq	%rdx, %rax
   73c1a:      	movl	$0x1000000, %edx        # imm = 0x1000000
   73c1f:      	cmpq	%rdx, %rax
   73c22:      	cmovaeq	%rax, %rdx
   73c26:      	testq	%rcx, %rcx
   73c29:      	leaq	, %rcx <scoop_gc_heap_state>
   73c30:      	cmovnsq	%rdx, %rax
   73c34:      	movq	%rax, 0x50(%rcx)
<L13>:
   73c38:      	leaq	, %rax <scoop_gc_heap_state>
   73c3f:      	movb	$0x0, 0x85(%rax)
   73c46:      	addq	$0x68, %rsp
   73c4a:      	popq	%rbx
   73c4b:      	popq	%r12
   73c4d:      	popq	%r13
   73c4f:      	popq	%r14
   73c51:      	popq	%r15
   73c53:      	popq	%rbp
   73c54:      	retq
<L14>:
   73c55:      	movq	%r15, %rdi
<L15>:
   73c58:      	callq	 <scoop_heap_release_block>
   73c5d:      	jmp	 <L3>
<L16>:
   73c62:      	leaq	, %rax <scoop_gc_heap_state>
   73c69:      	pxor	%xmm0, %xmm0
   73c6d:      	movb	$0x0, -0x70(%rbp)
   73c71:      	movl	$0x80, %r12d
   73c77:      	movb	$0x0, -0x62(%rbp)
   73c7b:      	movl	$0x10, %ebx
   73c80:      	movzbl	0x81(%rax), %eax
   73c87:      	movq	%r14, -0x78(%rbp)
   73c8b:      	movb	%al, -0x61(%rbp)
   73c8e:      	movq	0x40(%r15), %rax
   73c92:      	movups	%xmm0, (%rax)
   73c95:      	movups	%xmm0, 0x10(%rax)
   73c99:      	jmp	 <L22>
   73c9b:      	nopl	(%rax,%rax)
<L17>:
   73ca0:      	testq	%r13, %r13
   73ca3:      	je	 <L28>
<L18>:
   73ca9:      	movq	%r15, %rdi
   73cac:      	callq	 <scoop_heap_block_base>
   73cb1:      	leaq	(%rax,%r12), %rdi
   73cb5:      	callq	 <release_dead_object>
<L19>:
   73cba:      	cmpb	$0x0, -0x61(%rbp)
   73cbe:      	jne	 <L26>
<L20>:
   73cc4:      	movq	0x20(%r15), %rdi
   73cc8:      	movq	%rbx, %rsi
   73ccb:      	callq	 <scoop_heap_bit_clear>
   73cd0:      	movq	0x30(%r15), %rdi
   73cd4:      	movq	%rbx, %rsi
   73cd7:      	callq	 <scoop_heap_bit_clear>
   73cdc:      	movq	0x50(%r15), %rax
   73ce0:      	xorl	%edx, %edx
   73ce2:      	movw	%dx, (%rax,%r14)
<L21>:
   73ce7:      	addq	$0x1, %rbx
   73ceb:      	addq	$0x8, %r12
   73cef:      	cmpq	$0x1000, %rbx           # imm = 0x1000
   73cf6:      	je	 <L30>
<L22>:
   73cfc:      	movq	0x20(%r15), %rdi
   73d00:      	movq	%rbx, %rsi
   73d03:      	callq	 <scoop_heap_bit_test>
   73d08:      	testb	%al, %al
   73d0a:      	je	 <L21>
   73d0c:      	movq	0x30(%r15), %rdi
   73d10:      	movq	%rbx, %rsi
   73d13:      	callq	 <scoop_heap_bit_test>
   73d18:      	movq	0x28(%r15), %rdi
   73d1c:      	movq	%rbx, %rsi
   73d1f:      	movb	%al, -0x60(%rbp)
   73d22:      	movl	%eax, %r14d
   73d25:      	callq	 <scoop_heap_bit_test>
   73d2a:      	cmpl	$0x3, 0x14(%r15)
   73d2f:      	jne	 <L23>
   73d31:      	testb	%r14b, %r14b
   73d34:      	je	 <L27>
<L23>:
   73d3a:      	movq	0x50(%r15), %rdx
   73d3e:      	leaq	(%rbx,%rbx), %r14
   73d42:      	movzwl	(%rdx,%rbx,2), %edx
   73d46:      	leaq	(,%rdx,8), %r13
   73d4e:      	testb	%al, %al
   73d50:      	je	 <L17>
   73d56:      	testq	%r13, %r13
   73d59:      	je	 <L48>
   73d5f:      	leaq	-0x1(%r12,%r13), %rdx
   73d64:      	movq	%r12, %r14
   73d67:      	shrq	$0x7, %rdx
   73d6b:      	shrq	$0x7, %r14
   73d6f:      	leaq	0x1(%rdx), %r13
   73d73:      	cmpq	%r14, %rdx
   73d76:      	jb	 <L25>
   73d78:      	movq	%rbx, -0x70(%rbp)
   73d7c:      	movq	%r15, %rbx
   73d7f:      	movl	%eax, %r15d
   73d82:      	nopl	(%rax)
   73d85:      	nopw	%cs:(%rax,%rax)
<L24>:
   73d90:      	movq	0x40(%rbx), %rdi
   73d94:      	movq	%r14, %rsi
   73d97:      	addq	$0x1, %r14
   73d9b:      	callq	 <scoop_heap_bit_set>
   73da0:      	cmpq	%r13, %r14
   73da3:      	jne	 <L24>
   73da5:      	movl	%r15d, %eax
   73da8:      	movq	%rbx, %r15
   73dab:      	movq	-0x70(%rbp), %rbx
<L25>:
   73daf:      	movzbl	-0x60(%rbp), %edx
   73db3:      	movb	%al, -0x70(%rbp)
   73db6:      	orb	%dl, -0x62(%rbp)
   73db9:      	jmp	 <L21>
   73dbe:      	nop
<L26>:
   73dc0:      	movq	%r15, %rdi
   73dc3:      	callq	 <scoop_heap_block_base>
   73dc8:      	movq	%r13, %rdx
   73dcb:      	movl	$0xa5, %esi
   73dd0:      	leaq	(%rax,%r12), %rdi
   73dd4:      	callq	 <memset@plt>
   73dd9:      	jmp	 <L20>
   73dde:      	nop
<L27>:
   73de0:      	testb	%al, %al
   73de2:      	je	 <L29>
   73de4:      	movq	0x58(%r15), %rax
   73de8:      	cmpq	$0x0, (%rax,%r12)
   73ded:      	je	 <L50>
   73df3:      	movq	0x50(%r15), %rax
   73df7:      	leaq	(%rbx,%rbx), %r14
   73dfb:      	movzwl	(%rax,%rbx,2), %edx
   73dff:      	shlq	$0x3, %rdx
   73e03:      	movq	%rdx, %r13
   73e06:      	jne	 <L19>
<L28>:
   73e0c:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6dcc>
   73e13:      	callq	 <scoop_heap_fatal>
   73e18:      	nopl	(%rax,%rax)
<L29>:
   73e20:      	movq	0x50(%r15), %rax
   73e24:      	leaq	(%rbx,%rbx), %r14
   73e28:      	movzwl	(%rax,%rbx,2), %edx
   73e2c:      	shlq	$0x3, %rdx
   73e30:      	movq	%rdx, %r13
   73e33:      	jne	 <L18>
   73e39:      	jmp	 <L28>
   73e3b:      	nopl	(%rax,%rax)
<L30>:
   73e40:      	movq	0x28(%r15), %rdx
   73e44:      	movq	-0x78(%rbp), %r14
   73e48:      	xorl	%eax, %eax
   73e4a:      	pxor	%xmm0, %xmm0
   73e4e:      	leaq	0x8(%rdx), %rdi
   73e52:      	movq	$0x0, (%rdx)
   73e59:      	movq	$0x0, 0x1f8(%rdx)
   73e64:      	andq	$-0x8, %rdi
   73e68:      	subq	%rdi, %rdx
   73e6b:      	leal	0x200(%rdx), %ecx
   73e71:      	shrl	$0x3, %ecx
   73e74:      	rep		stosq	%rax, %es:(%rdi)
   73e77:      	movq	0x38(%r15), %rdx
   73e7b:      	leaq	0x8(%rdx), %rdi
   73e7f:      	movq	$0x0, (%rdx)
   73e86:      	movq	$0x0, 0x1f8(%rdx)
   73e91:      	andq	$-0x8, %rdi
   73e95:      	subq	%rdi, %rdx
   73e98:      	leal	0x200(%rdx), %ecx
   73e9e:      	shrl	$0x3, %ecx
   73ea1:      	rep		stosq	%rax, %es:(%rdi)
   73ea4:      	movq	0x48(%r15), %rax
   73ea8:      	movups	%xmm0, (%rax)
   73eab:      	movups	%xmm0, 0x10(%rax)
   73eaf:      	movq	0x58(%r15), %rdi
   73eb3:      	callq	 <free@plt>
   73eb8:      	pxor	%xmm3, %xmm3
   73ebc:      	movq	$0x0, 0x58(%r15)
   73ec4:      	movups	%xmm3, 0x68(%r15)
   73ec9:      	cmpb	$0x0, -0x70(%rbp)
   73ecd:      	je	 <L46>
   73ed3:      	cmpb	$0x1, -0x62(%rbp)
   73ed7:      	sbbl	%eax, %eax
   73ed9:      	andl	$-0x3, %eax
   73edc:      	addl	$0x5, %eax
   73edf:      	movl	%eax, 0x14(%r15)
   73ee3:      	cmpb	$0x0, -0x61(%rbp)
   73ee7:      	jne	 <L3>
   73eed:      	movl	$0x1, %ebx
   73ef2:      	xorl	%r13d, %r13d
   73ef5:      	jmp	 <L33>
   73ef7:      	nopw	(%rax,%rax)
<L31>:
   73f00:      	testq	%r13, %r13
   73f03:      	jne	 <L40>
   73f09:      	leaq	0x1(%rbx), %r12
   73f0d:      	cmpq	$0xff, %rbx
   73f14:      	je	 <L36>
<L32>:
   73f16:      	movq	%rbx, %r13
   73f19:      	movq	%r12, %rbx
<L33>:
   73f1c:      	movq	0x40(%r15), %rdi
   73f20:      	movq	%rbx, %rsi
   73f23:      	callq	 <scoop_heap_bit_test>
   73f28:      	testb	%al, %al
   73f2a:      	je	 <L31>
<L34>:
   73f2c:      	testq	%r13, %r13
   73f2f:      	jne	 <L38>
   73f35:      	leaq	0x1(%rbx), %r12
<L35>:
   73f39:      	cmpq	$0x100, %r12            # imm = 0x100
   73f40:      	je	 <L3>
   73f46:      	xorl	%ebx, %ebx
   73f48:      	jmp	 <L32>
   73f4a:      	nopw	(%rax,%rax)
<L36>:
   73f50:      	movl	$0xff, %r13d
   73f56:      	movl	$0x1, %ebx
<L37>:
   73f5b:      	movl	$0x18, %edi
   73f60:      	callq	 <malloc@plt>
   73f65:      	testq	%rax, %rax
   73f68:      	je	 <L49>
   73f6e:      	leaq	, %rcx <scoop_gc_heap_state>
   73f75:      	movq	%r15, %xmm1
   73f7a:      	addq	$0x1, %r12
   73f7e:      	movq	0x40(%rcx), %rcx
   73f82:      	movq	$0x0, 0x10(%rax)
   73f8a:      	movw	%r13w, 0x10(%rax)
   73f8f:      	movq	%rcx, %xmm0
   73f94:      	movq	%rcx, -0x60(%rbp)
   73f98:      	leaq	, %rcx <scoop_gc_heap_state>
   73f9f:      	punpcklqdq	%xmm1, %xmm0    # xmm0 = xmm0[0],xmm1[0]
   73fa3:      	movw	%bx, 0x12(%rax)
   73fa7:      	movups	%xmm0, (%rax)
   73faa:      	movq	%rax, 0x40(%rcx)
   73fae:      	cmpq	$0x101, %r12            # imm = 0x101
   73fb5:      	jne	 <L35>
   73fbb:      	jmp	 <L3>
<L38>:
   73fc0:      	movq	%rbx, %r12
<L39>:
   73fc3:      	movq	%r12, %rbx
   73fc6:      	subq	%r13, %rbx
   73fc9:      	jne	 <L37>
   73fcb:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0xcab>
   73fd2:      	callq	 <scoop_heap_fatal>
   73fd7:      	nopw	(%rax,%rax)
<L40>:
   73fe0:      	leaq	0x1(%rbx), %r12
<L41>:
   73fe4:      	cmpq	$0x100, %r12            # imm = 0x100
   73feb:      	je	 <L39>
   73fed:      	movq	0x40(%r15), %rdi
   73ff1:      	movq	%r12, %rsi
   73ff4:      	callq	 <scoop_heap_bit_test>
   73ff9:      	testb	%al, %al
   73ffb:      	je	 <L42>
   73ffd:      	movq	%r12, %rbx
   74000:      	jmp	 <L34>
   74005:      	nopl	(%rax)
<L42>:
   74008:      	addq	$0x1, %r12
   7400c:      	jmp	 <L41>
<L43>:
   7400e:      	cmpl	$0x4, 0x14(%r15)
   74013:      	je	 <L6>
   74019:      	jmp	 <L3>
<L44>:
   7401e:      	movl	$0x5, %eax
   74023:      	testb	%dl, %dl
   74025:      	jne	 <L2>
   7402b:      	nopl	(%rax,%rax)
<L45>:
   74030:      	movq	%r15, %rdi
   74033:      	callq	 <scoop_heap_block_base>
   74038:      	leaq	0x80(%rax), %rdi
   7403f:      	callq	 <release_dead_object>
   74044:      	jmp	 <L7>
<L46>:
   74049:      	movq	%r15, %rdi
   7404c:      	cmpb	$0x0, -0x61(%rbp)
   74050:      	je	 <L15>
   74056:      	jmp	 <L8>
<L47>:
   7405b:      	callq	 <scoop_heap_free_run_nodes>
   74060:      	jmp	 <L0>
<L48>:
   74065:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6df4>
   7406c:      	callq	 <scoop_heap_fatal>
<L49>:
   74071:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6e14>
   74078:      	callq	 <scoop_heap_fatal>
<L50>:
   7407d:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6d9c>
   74084:      	callq	 <scoop_heap_fatal>
<L51>:
   74089:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6d74>
   74090:      	callq	 <scoop_heap_fatal>
<L52>:
   74095:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6e3c>
   7409c:      	callq	 <scoop_heap_fatal>
   740a1:      	nopl	(%rax)
<L53>:
   740a8:      	leaq	, %rdi <scoop$1$be$a8aeda2f669439e4bf95d48ea8d9f706812a3b1692426c3736551144fcbdbdbf+0x6e74>
   740af:      	callq	 <scoop_heap_fatal>
   740b4:      	nopw	%cs:(%rax,%rax)
   740be:      	nop
