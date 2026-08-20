
tmp/m34/counters-off-darwin/single:	file format mach-o arm64

Disassembly of section __TEXT,__text:

000000010003ccc8 <_scoop_runtime_finish_tlab_alloc>:
10003ccc8:     	stp	x22, x21, [sp, #-0x30]!
10003cccc:     	stp	x20, x19, [sp, #0x10]
10003ccd0:     	stp	x29, x30, [sp, #0x20]
10003ccd4:     	add	x29, sp, #0x20
10003ccd8:     	mov	x21, x2
10003ccdc:     	mov	x19, x1
10003cce0:     	mov	x20, x0
10003cce4:     	bl	0x10002f138 <_scoop_gc_stress_move_enabled>
10003cce8:     	tbnz	w0, #0x0, 0x10003cd20 <_scoop_runtime_finish_tlab_alloc+0x58>
10003ccec:     	ldr	x8, [x19, #0x90]
10003ccf0:     	cbnz	x8, 0x10003cd20 <_scoop_runtime_finish_tlab_alloc+0x58>
10003ccf4:     	mov	x0, x19
10003ccf8:     	mov	x1, x21
10003ccfc:     	bl	0x10003f3a0 <_scoop_shape_normalize_allocation>
10003cd00:     	mov	x2, x0
10003cd04:     	mov	x0, x20
10003cd08:     	mov	x1, x19
10003cd0c:     	bl	0x10003cd2c <_finish_small_allocation>
10003cd10:     	ldp	x29, x30, [sp, #0x20]
10003cd14:     	ldp	x20, x19, [sp, #0x10]
10003cd18:     	ldp	x22, x21, [sp], #0x30
10003cd1c:     	ret
10003cd20:     	adrp	x0, 0x100060000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x2450>
10003cd24:     	add	x0, x0, #0xeef
10003cd28:     	bl	0x1000435fc <_scoop_heap_fatal>

000000010003cd2c <_finish_small_allocation>:
10003cd2c:     	sub	sp, sp, #0x40
10003cd30:     	stp	x22, x21, [sp, #0x10]
10003cd34:     	stp	x20, x19, [sp, #0x20]
10003cd38:     	stp	x29, x30, [sp, #0x30]
10003cd3c:     	add	x29, sp, #0x30
10003cd40:     	cbz	x0, 0x10003cdfc <_finish_small_allocation+0xd0>
10003cd44:     	mov	x21, x1
10003cd48:     	cbz	x1, 0x10003cdfc <_finish_small_allocation+0xd0>
10003cd4c:     	mov	x19, x2
10003cd50:     	mov	w8, #0x7f80             ; =32640
10003cd54:     	cmp	x2, x8
10003cd58:     	b.hi	0x10003cdfc <_finish_small_allocation+0xd0>
10003cd5c:     	mov	x20, x0
10003cd60:     	ldr	x8, [x21, #0x18]
10003cd64:     	udiv	x9, x0, x8
10003cd68:     	msub	x8, x9, x8, x0
10003cd6c:     	cbnz	x8, 0x10003cdfc <_finish_small_allocation+0xd0>
10003cd70:     	add	x1, sp, #0xc
10003cd74:     	mov	x0, x20
10003cd78:     	bl	0x10002f15c <_scoop_heap_pointer_block_index>
10003cd7c:     	tbz	w0, #0x0, 0x10003cdfc <_finish_small_allocation+0xd0>
10003cd80:     	mov	x0, x20
10003cd84:     	mov	x1, x19
10003cd88:     	bl	0x100044d18 <dyld_stub_binder+0x100044d18>
10003cd8c:     	stp	x21, xzr, [x20]
10003cd90:     	ldr	w0, [sp, #0xc]
10003cd94:     	mov	x1, x20
10003cd98:     	mov	x2, x19
10003cd9c:     	mov	w3, #0x0                ; =0
10003cda0:     	bl	0x100042848 <_scoop_heap_record_small_object>
10003cda4:     	adrp	x8, 0x10014c000 <dyld_stub_binder+0x10014c000>
10003cda8:     	add	x8, x8, #0x2b8
10003cdac:     	add	x9, x8, #0x90
10003cdb0:     	mov	w10, #0x1               ; =1
10003cdb4:     	ldadd	x10, x9, [x9]
10003cdb8:     	add	x9, x8, #0xb8
10003cdbc:     	ldadd	x19, x9, [x9]
10003cdc0:     	ldr	x9, [x8, #0x58]
10003cdc4:     	ldr	w10, [sp, #0xc]
10003cdc8:     	add	x9, x9, x10, lsl #7
10003cdcc:     	ldr	w9, [x9, #0x8]
10003cdd0:     	cbnz	w9, 0x10003cde8 <_finish_small_allocation+0xbc>
10003cdd4:     	add	x9, x8, #0xb0
10003cdd8:     	mov	w10, #0x1               ; =1
10003cddc:     	ldadd	x10, x9, [x9]
10003cde0:     	add	x8, x8, #0xc0
10003cde4:     	ldadd	x19, x8, [x8]
10003cde8:     	ldp	x29, x30, [sp, #0x30]
10003cdec:     	ldp	x20, x19, [sp, #0x20]
10003cdf0:     	ldp	x22, x21, [sp, #0x10]
10003cdf4:     	add	sp, sp, #0x40
10003cdf8:     	ret
10003cdfc:     	adrp	x0, 0x100060000 <_scoop$1$bs$a6f1194dd136655efc78022800ab702ac81569688b2b1a68408accecda600c93+0x2450>
10003ce00:     	add	x0, x0, #0xf70
10003ce04:     	bl	0x1000435fc <_scoop_heap_fatal>
