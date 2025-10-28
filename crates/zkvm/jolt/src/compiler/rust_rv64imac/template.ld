/* Copied from https://github.com/a16z/jolt/blob/v0.3.0-alpha/jolt-core/src/host/mod.rs#L28 */

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
    *(.bss)
  } > program

  . = ALIGN(8);
  _STACK_END = .;
  . = . + {STACK_CANARY};
  . = . + {STACK_SIZE};
  _STACK_PTR = .;

  . = ALIGN(8);
  _HEAP_PTR = .;
}
