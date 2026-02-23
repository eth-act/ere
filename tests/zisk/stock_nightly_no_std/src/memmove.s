# Copied from https://github.com/0xPolygonHermez/zisk/blob/v0.16.0/ziskos/entrypoint/src/dma/memmove.s

        .section ".note.GNU-stack","",@progbits
        .text
        .attribute      4, 16
        .attribute      5, "rv64im"
        .globl  memmove
        .p2align        4
        .type   memmove,@function
memmove:
        csrs    0x813, a2                  # Marker: Write count (a2) to CSR 0x813
        add	x0,a0,a1
        ret        
        .size memmove, .-memmove
        .section .text.hot,"ax",@progbits