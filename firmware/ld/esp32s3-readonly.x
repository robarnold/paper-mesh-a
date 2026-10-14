/* SPDX-License-Identifier: MIT
 * Derived from esp-hal 1.2.2 (see LICENSE-MIT):
 * https://github.com/esp-rs/esp-hal/blob/915465df59aee91b627fe538f0b442e56c7f4fc9/esp-hal/ld/esp32s3/esp32s3.x
 *
 * The only functional change makes the flash-mapping padding read-only.
 * Keep its NOBITS type, size, and alignment.
 * When esp-hal changes, compare this script with upstream.
 * When upstream corrects the section, remove this override.
 */
INCLUDE exception.x

SECTIONS {
  .rotext_dummy (READONLY) :
  {
    /* This dummy section represents the .rodata section within ROTEXT.
    * Since the same physical memory is mapped to both DROM and IROM,
    * we need to make sure the .rodata and .text sections don't overlap.
    * We skip the amount of memory taken by .rodata* in .text
    */

    /* Start at the same alignment constraint than .flash.text */

    . = ALIGN(ALIGNOF(.rodata));
    . = ALIGN(ALIGNOF(.rodata.wifi));

    /* Create an empty gap as big as .text section */

    . = . + SIZEOF(.flash.appdesc);
    . = . + SIZEOF(.rodata);
    . = . + SIZEOF(.rodata.wifi);

    /* Prepare the alignment of the section above. Few bytes (0x20) must be
     * added for the mapping header.
     */

    . = ALIGN(0x10000) + 0x20;
    _rotext_reserved_start = .;
  } > ROTEXT
}
INSERT BEFORE .text;

/* Similar to .rotext_dummy this represents .rwtext but in .data */
SECTIONS {
  .rwdata_dummy (NOLOAD) : ALIGN(4)
  {
    . = . + SIZEOF(.rwtext) + SIZEOF(.rwtext.wifi) + SIZEOF(.vectors);
  } > RWDATA
}
INSERT BEFORE .data;

/* Shared sections - ordering matters */
SECTIONS {
  INCLUDE "rwtext.x"
  INCLUDE "rwdata.x"
}
INCLUDE "rodata.x"
INCLUDE "text.x"
INCLUDE "rtc_fast.x"
INCLUDE "rtc_slow.x"
INCLUDE "stack.x"
INCLUDE "dram2.x"
INCLUDE "dcache_reclaimed.x"
INCLUDE "metadata.x"
INCLUDE "eh_frame.x"
/* End of Shared sections */
