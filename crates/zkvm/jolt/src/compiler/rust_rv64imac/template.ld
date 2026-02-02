/* Copied from https://github.com/a16z/jolt/blob/6dcd401/jolt-core/src/host/mod.rs#L30 */

MEMORY {
  program (rwx) : ORIGIN = 0x80000000, LENGTH = {EMULATOR_MEMORY}
}

SECTIONS {
  .text.boot : {
    *(.text.boot)
  } > program

  .text : {
    *(.text)
  } > program

  .data : {
    *(.data)
  } > program

  .bss (NOLOAD) : {
    __bss_start = .;
    *(.bss .bss.*)
    *(COMMON)
    . = ALIGN(8);
    __bss_end = .;
  } > program

  /DISCARD/ : {
    *(.eh_frame*)
    *(.eh_frame_hdr*)
  }

  . = ALIGN(8);
  _STACK_END = .;
  . = . + {STACK_CANARY};
  . = . + {STACK_SIZE};
  . = ALIGN(8);
  _STACK_PTR = .;

  . = ALIGN(8);
  _HEAP_PTR = .;
}
