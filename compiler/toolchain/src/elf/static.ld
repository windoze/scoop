/* Static musl does not apply GNU_RELRO. Relocations are already complete, so
 * Scoop metadata belongs in a read-only load segment. */
SECTIONS
{
  .scoop_metadata (READONLY) : ALIGN(8)
  {
    KEEP(*(.data.rel.ro.scoop .data.rel.ro.scoop.*))
    . = ALIGN(8);
    HIDDEN(__scoop_stackmaps_start = .);
    KEEP(*(.llvm_stackmaps))
    HIDDEN(__scoop_stackmaps_end = .);
  }
}
INSERT AFTER .rodata;
