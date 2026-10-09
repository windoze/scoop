
tmp/m34/inline-on-darwin/mir-inlining:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001e7f0 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d>:
10001e7f0:     	stp	x29, x30, [sp, #-0x10]!
10001e7f4:     	mov	x29, sp
10001e7f8:     	mov	w10, #0x5678            ; =22136
10001e7fc:     	mov	w9, wzr
10001e800:     	mov	w8, wzr
10001e804:     	movk	w10, #0x1234, lsl #16
10001e808:     	cmp	w9, w0
10001e80c:     	b.ge	0x10001e830 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x40>
10001e810:     	add	w10, w10, #0x1
10001e814:     	add	w9, w9, #0x1
10001e818:     	eor	w10, w10, w10, lsl #13
10001e81c:     	eor	w10, w10, w10, lsr #17
10001e820:     	eor	w10, w10, w10, lsl #5
10001e824:     	eor	w8, w8, w10
10001e828:     	cmp	w9, w0
10001e82c:     	b.lt	0x10001e810 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x20>
10001e830:     	mov	w0, w8
10001e834:     	ldp	x29, x30, [sp], #0x10
10001e838:     	ret
