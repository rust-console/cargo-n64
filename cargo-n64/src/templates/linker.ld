ENTRY(_start)

SECTIONS {
    . = 0x80000400;
    __boot_start = .;

    .boot : {
        *(.boot)
    }

    .text : {
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        . = ALIGN(4);
        __bss_start = .;
        *(.bss .bss.*)
        __bss_end = .;
    }

    . = ALIGN(2);
    __rom_end = . - __boot_start + 0xB0001000;

    /DISCARD/ : {
        *(.MIPS.*)
        *(.comment)
        *(.mdebug.*)
        *(.pdr)
        *(.reginfo)

        /*
         * We may need the global offset table some day.
         * Our target is currently set with a static relocation-model, so this
         * might not be needed after all.
         */
        *(.got)
    }
}
