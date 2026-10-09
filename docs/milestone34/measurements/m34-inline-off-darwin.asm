
tmp/m34/inline-off-darwin/mir-inlining:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010001d9d8 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d>:
10001d9d8:     	stp	x22, x21, [sp, #-0x30]!
10001d9dc:     	stp	x20, x19, [sp, #0x10]
10001d9e0:     	stp	x29, x30, [sp, #0x20]
10001d9e4:     	add	x29, sp, #0x20
10001d9e8:     	mov	w19, w0
10001d9ec:     	mov	w0, #0x5678             ; =22136
10001d9f0:     	mov	w21, wzr
10001d9f4:     	mov	w20, wzr
10001d9f8:     	movk	w0, #0x1234, lsl #16
10001d9fc:     	cmp	w21, w19
10001da00:     	b.ge	0x10001da20 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x48>
10001da04:     	mov	w1, wzr
10001da08:     	bl	0x10001d8d0 <_scoop$1$cb$282a33668916ebe7a926c8f62abf2ad7d3253d12099213678cdee754a7cefd97>
10001da0c:     	bl	0x10001d8b4 <_scoop$1$cb$c56cd1861f9adfdb1366c5782dbbdf9045f283ad72cd878590a00b5e3a8a082c>
10001da10:     	add	w21, w21, #0x1
10001da14:     	eor	w20, w20, w0
10001da18:     	cmp	w21, w19
10001da1c:     	b.lt	0x10001da04 <_scoop$1$cb$39b1e762e7942b0a97360b4cfbe4a9726f9c7720c8e75fa2e96be05178cbea5d+0x2c>
10001da20:     	mov	w0, w20
10001da24:     	ldp	x29, x30, [sp, #0x20]
10001da28:     	ldp	x20, x19, [sp, #0x10]
10001da2c:     	ldp	x22, x21, [sp], #0x30
10001da30:     	ret
