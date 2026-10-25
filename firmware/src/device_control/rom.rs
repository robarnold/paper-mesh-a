//! Enter the ESP32-S3 ROM downloader after the board reaches a safe state.

use esp_hal::{
    delay::Delay,
    efuse,
    peripherals::{LPWR, SYSTEM, USB_DEVICE, USB_FS, USB_WRAP},
};

/// Check the read-only settings required by this downloader and its host helper.
pub fn supported() -> bool {
    [
        efuse::DIS_FORCE_DOWNLOAD,
        efuse::DIS_DOWNLOAD_MODE,
        efuse::DIS_USB_SERIAL_JTAG,
        efuse::DIS_USB_SERIAL_JTAG_DOWNLOAD_MODE,
        efuse::ENABLE_SECURITY_DOWNLOAD,
        efuse::USB_PHY_SEL,
    ]
    .into_iter()
    .all(|field| !efuse::read_bit(field))
}

/// Disconnect application USB, restore the ROM USB path, and reset both cores.
///
/// The caller must park core 1, stop application work, and confirm display shutdown.
/// The recovery watchdog must be idle. Send the command response before this call.
/// The board must retain its default USB PHY selection and permit software download.
pub fn enter() -> ! {
    // The caller has exclusive hardware access. No interrupt may resume USB work.
    xtensa_lx::interrupt::disable();

    // Keep the OTG clock available until the disconnect request reaches the PHY.
    USB_FS::regs().pcgcctl().modify(|_, w| {
        w.stoppclk().clear_bit();
        w.gatehclk().clear_bit()
    });
    USB_FS::regs().dctl().modify(|_, w| w.sftdiscon().set_bit());
    USB_WRAP::regs()
        .otg_conf()
        .modify(|_, w| w.usb_pad_enable().clear_bit());

    SYSTEM::regs()
        .perip_clk_en1()
        .modify(|_, w| w.usb_device_clk_en().set_bit());
    USB_DEVICE::regs()
        .conf0()
        .modify(|_, w| w.usb_pad_enable().clear_bit());
    // This interval gives the host a separate disconnect before ROM enumeration.
    Delay::new().delay_millis(250);

    // Arduino-ESP32 3.3.2, esp32-hal-tinyusb.c, usb_switch_to_cdc_jtag():
    // https://github.com/espressif/arduino-esp32/blob/3.3.2/cores/esp32/esp32-hal-tinyusb.c#L468-L525
    // Stop OTG before removing its software PHY selection.
    SYSTEM::regs()
        .perip_rst_en0()
        .modify(|_, w| w.usb_rst().set_bit());
    SYSTEM::regs()
        .perip_clk_en0()
        .modify(|_, w| w.usb_clk_en().clear_bit());

    // Set the retained boot request before the hardware USB controller reconnects.
    LPWR::regs()
        .option1()
        .modify(|_, w| w.force_download_boot().set_bit());
    LPWR::regs().usb_conf().modify(|_, w| {
        w.sw_hw_usb_phy_sel().clear_bit();
        w.sw_usb_phy_sel().clear_bit();
        w.usb_pad_enable_override().clear_bit();
        w.usb_pad_enable().clear_bit();
        w.usb_pad_pull_override().clear_bit();
        w.usb_tx_en_override().clear_bit();
        w.usb_reset_disable().clear_bit();
        w.io_mux_reset_disable().clear_bit()
    });

    // ESP-IDF 5.5.1, usb_serial_jtag_ll.h: reset the controller registers.
    // https://github.com/espressif/esp-idf/blob/v5.5.1/components/hal/esp32s3/include/hal/usb_serial_jtag_ll.h#L308-L314
    // CONF0 resets to 0x4200: internal PHY, D+ pull-up, and USB pads enabled.
    SYSTEM::regs()
        .perip_rst_en1()
        .modify(|_, w| w.usb_device_rst().set_bit());
    SYSTEM::regs()
        .perip_rst_en1()
        .modify(|_, w| w.usb_device_rst().clear_bit());

    // Reset the digital system. A CPU-only reset would retain peripheral state.
    // The host must clear FORCE_DOWNLOAD_BOOT before the later normal restart.
    esp_hal::system::software_reset()
}
