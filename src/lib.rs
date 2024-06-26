#![no_std]
#![deny(unsafe_code)]
#![deny(missing_docs)]
#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/imxrt-rs/imxrt-uart-panic/issues")]
#![cfg_attr(docsrs, feature(doc_cfg))]

#[doc(hidden)]
pub mod _deps {
    pub use cortex_m;
    pub use embedded_io;
    pub use imxrt_hal;
    pub use imxrt_ral;
}

/// Registers the UART panic handler.
///
/// # Arguments
///
/// - `uart`: A peripheral defined in [`imxrt_ral::lpuart`].
/// - `tx_pin`: The UART tx pin. Usually defined in the bsp, such as [`teensy4_bsp::pins::common`](https://docs.rs/teensy4-pins/0.3.2/teensy4_pins/common).
/// - `rx_pin`: The UART rx pin. Usually defined in the bsp, such as [`teensy4_bsp::pins::common`](https://docs.rs/teensy4-pins/0.3.2/teensy4_pins/common).
/// - `baud`: The UART baud rate. Most common ones are `9600` and `115200`.
/// - `idle_func`: Optional. Specifies what function to enter in the end. Default is [`cortex_m::asm::udf`], but this could
///   for example be used to enter [`teensy4_panic::sos`](https://docs.rs/teensy4-panic/0.2.3/teensy4_panic/fn.sos.html).
#[macro_export]
macro_rules! register {
    ($uart: ident, $tx_pin: ident, $rx_pin: ident, $baud: expr, $idle_func: expr) => {
        #[panic_handler]
        fn panic(info: &::core::panic::PanicInfo) -> ! {
            use ::core::fmt::Write as _;

            use $crate::_deps::embedded_io::Write as _;
            use $crate::_deps::imxrt_hal as hal;
            use $crate::_deps::imxrt_ral as ral;

            use hal::ccm;
            use hal::lpuart::{Baud, Direction, Lpuart, Pins, Watermark};

            // Initialize clocks
            const UART_DIVIDER: u32 = 3;
            pub const UART_FREQUENCY: u32 = hal::ccm::XTAL_OSCILLATOR_HZ / UART_DIVIDER;
            let mut ccm = unsafe { ral::ccm::CCM::instance() };
            ccm::clock_gate::UART_CLOCK_GATES
                .iter()
                .for_each(|locator| locator.set(&mut ccm, ccm::clock_gate::OFF));
            ccm::uart_clk::set_selection(&mut ccm, ccm::uart_clk::Selection::Oscillator);
            ccm::uart_clk::set_divider(&mut ccm, UART_DIVIDER);
            ccm::clock_gate::UART_CLOCK_GATES
                .iter()
                .for_each(|locator| locator.set(&mut ccm, ccm::clock_gate::ON));

            // Initialize UART
            let registers = unsafe { ral::lpuart::$uart::instance() };
            let pins = Pins {
                tx: unsafe { $tx_pin::new() },
                rx: unsafe { $rx_pin::new() },
            };
            let mut uart = Lpuart::new(registers, pins);

            // Configure UART
            const BAUD: Baud = Baud::compute(UART_FREQUENCY, $baud);
            uart.disable(|uart| {
                uart.set_baud(&BAUD);
                uart.enable_fifo(Watermark::tx(4));
                uart.disable_fifo(Direction::Rx);
            });

            struct UartWriter<P, const N: u8> {
                uart: hal::lpuart::Lpuart<P, N>,
            }
            impl<P, const N: u8> ::core::fmt::Write for UartWriter<P, N> {
                fn write_str(&mut self, s: &str) -> ::core::fmt::Result {
                    for &b in s.as_bytes() {
                        if b == b'\n' {
                            let _ = self.uart.write_all(b"\r");
                        }
                        let _ = self.uart.write_all(core::slice::from_ref(&b));
                    }
                    Ok(())
                }
            }

            let mut uart = UartWriter { uart };

            ::core::writeln!(uart).ok();
            ::core::writeln!(uart, "{}", info).ok();
            ::core::writeln!(uart).ok();

            let _ = uart.uart.flush();

            $idle_func();
        }
    };
    ($uart: ident, $tx_pin: ident, $rx_pin: ident, $baud: expr) => {
        $crate::register!(
            $uart,
            $tx_pin,
            $rx_pin,
            $baud,
            $crate::_deps::cortex_m::asm::udf
        );
    };
}
