/*
 * zkvm.ld — Linker script for SP1 guest ELFs (RV64IM, LP64).
 *
 * Memory layout:
 *
 *   0x0000_0000 ────────────────────────────────────────
 *               │ stack (grows down from STACK_TOP)    │
 *   0x7800_0000 ── STACK_TOP (sp init value) ──────────
 *               │ .text / .rodata / .data / .bss       │
 *   _end        │ SP1's own heap (256 MiB)             │
 *   _heap_start │ the guest's heap                     │
 *   _heap_end   │ reserved input region                 │
 *   MAX_MEMORY  ── 0x20_0000_0000 (1 << 37) ───────────
 *
 * Note: the const `STACK_TOP` in `sp1_primitives::consts` is the *base*
 * of the stack (stack grows DOWN from there toward 0), and
 * simultaneously the *start* of the loaded code/data image. SP1's
 * executor errors out if any ELF segment is below `STACK_TOP`
 * (`crates/core/executor/src/disassembler/elf.rs`), which matches
 * sp1-build's `--image-base=STACK_TOP` linker arg.
 */

OUTPUT_FORMAT("elf64-littleriscv")
OUTPUT_ARCH(riscv)
ENTRY(_start)

/* SP1 memory parameters. Override with --defsym at link time if needed. */
PROVIDE(__sp1_stack_top    = 0x78000000);
PROVIDE(__sp1_max_memory   = 0x2000000000); /* 1 << 37 = 128 GiB */
PROVIDE(__sp1_input_region = 0x0400000000); /* 1 << 34 = 16  GiB */
/* The size of SP1's own heap, apart from the guest's. */
PROVIDE(__sp1_vendor_heap  = 0x0010000000); /* 256 MiB */

MEMORY
{
    /* Code/data live above STACK_TOP. Length runs up to (but not into)
     * the reserved input region near the top of memory. */
    /* lld accepts r/w/x/!. The GNU `a` (allocate) and `i` (init) attrs
     * are implicit for any non-discarded section. */
    RAM (rwx) : ORIGIN = 0x78000000, LENGTH = 0x1B88000000  /* MAX_MEMORY - INPUT_REGION - STACK_TOP */
}

/*
 * Instruction-only, read-only and read-write segments, as the zkvm-standards "Static Library and
 * Linker Script" standard requires; SP1 rejects a segment both writable and executable. Declaring
 * them also keeps the ELF and program headers out of every loadable segment: SP1 rejects any below
 * STACK_TOP.
 */
PHDRS
{
    text PT_LOAD FLAGS(5);   /* R X */
    rodata PT_LOAD FLAGS(4); /* R */
    data PT_LOAD FLAGS(6);   /* R W */
}

SECTIONS
{
    . = ORIGIN(RAM);

    .text : ALIGN(16)
    {
        KEEP(*(.text._start))
        *(.text .text.*)
        *(.init) *(.fini)
    } > RAM :text

    .rodata : ALIGN(16)
    {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
        *(.data.rel.ro .data.rel.ro.*)
        *(.eh_frame .eh_frame.*)
        *(.gcc_except_table .gcc_except_table.*)
    } > RAM :rodata

    .init_array : ALIGN(8)
    {
        PROVIDE(__init_array_start = .);
        KEEP(*(SORT_BY_INIT_PRIORITY(.init_array.*)))
        KEEP(*(.init_array))
        PROVIDE(__init_array_end = .);
    } > RAM :rodata

    .fini_array : ALIGN(8)
    {
        PROVIDE(__fini_array_start = .);
        KEEP(*(SORT_BY_INIT_PRIORITY(.fini_array.*)))
        KEEP(*(.fini_array))
        PROVIDE(__fini_array_end = .);
    } > RAM :rodata

    .data : ALIGN(16)
    {
        __data_start = .;
        *(.data .data.*)
        *(.sdata .sdata.*)
        __data_end = .;
    } > RAM :data
    __data_load = LOADADDR(.data);

    .bss (NOLOAD) : ALIGN(16)
    {
        __bss_start = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
        *(COMMON)
        . = ALIGN(8);
        __bss_end = .;
    } > RAM :data

    /*
     * `_end` marks the end of the static data region. `sp1-zkvm`'s
     * embedded allocator (`crates/zkvm/entrypoint/src/allocators/embedded.rs`)
     * starts its heap there; in the static library that allocator is `embedded-alloc/`,
     * whose region is `_end` to `_heap_start`.
     */
    . = ALIGN(16);
    _end = .;

    /*
     * Stack grows DOWN from `__stack_top` (== STACK_TOP) toward 0;
     * `sp1-zkvm`'s `_start` initializes `sp` to this value.
     */
    __stack_top = __sp1_stack_top;

    /* riscv-relax friendliness: define the conventional gp anchor. */
    PROVIDE(__global_pointer$ = __data_start + 0x800);

    /*
     * Reserved input region — SP1's embedded allocator places hint inputs
     * here so the guest can read them without a heap allocation. Mirrors
     * EMBEDDED_RESERVED_INPUT_START in crates/zkvm/entrypoint/src/lib.rs.
     */
    __input_start = __sp1_max_memory - __sp1_input_region;
    __input_end   = __sp1_max_memory;

    /*
     * The application heap of the zkvm-standards "Static Library and
     * Linker Script" standard: the rest of memory, up to the input region.
     * Before it, from `_end`, SP1's own allocations have a private region.
     */
    _heap_start = _end + __sp1_vendor_heap;
    _heap_end   = __input_start;
    ASSERT(_heap_start <= _heap_end, "the program image and the vendor heap leave no guest heap")

    /DISCARD/ :
    {
        *(.comment)
        *(.note .note.*)
        *(.riscv.attributes)
    }
}
