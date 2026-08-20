/* Input stackmap sections are made writable before this partial script is used,
 * so the loader can relocate their function pointers without DT_TEXTREL. */
SECTIONS
{
  .data.rel.ro.scoop : ALIGN(8)
  {
    KEEP(*(.data.rel.ro.scoop .data.rel.ro.scoop.*))
    . = ALIGN(8);
    HIDDEN(__scoop_stackmaps_start = .);
    KEEP(*(.llvm_stackmaps))
    HIDDEN(__scoop_stackmaps_end = .);
  }
}
INSERT BEFORE .data.rel.ro;
