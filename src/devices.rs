/* 
 * Copyright 2026 Fraser Griffin
 *
 * This file is part of Pkg.
 *
 * Pkg is free software: you can redistribute it and/or modify it under 
 * the terms of the GNU General Public License as published by the Free Software Foundation, 
 * either version 3 of the License, or (at your option) any later version.
 *
 * Pkg is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; 
 * without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. 
 * See the GNU General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public License along with Pkg. 
 * If not, see <https://www.gnu.org/licenses/>. 
 * 
 */

use std::{collections::HashSet, process::exit, sync::{LazyLock, Mutex}};

use crate::driver_args::{DriverArgs, PAD_ANALOG, PAD_NORMAL, PAD_PULL_UP};

/// A device
pub struct Device {
    /// Device name
    pub name: &'static str,
    /// Device number
    pub num: u16,
    /// Device base address
    pub base: u32,
    /// Device interrupts (0xff for no interrupt)
    pub inter: [u8; 4],
    /// Available GPIO for the device
    pub available_gpio: HashSet<u8>,
    /// Device's function select
    pub func_sel: Option<u8>,
    /// Size of device memory mapped IO
    pub len: u32
}

/// List of all supported devices
static DEVICES: LazyLock<[Device; 32]> = LazyLock::new(|| [
    // ADC device
    Device {
        name: "ADC",
        num: 1,
        base: 0x4004c000,
        inter: [0x16, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([26, 27, 28, 29]),
        func_sel: Some(5),
        len: 0x1000
    },
    // Bus Control device
    Device {
        name: "Bus Control",
        num: 2,
        base: 0x40030000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // DMA device
    Device {
        name: "DMA",
        num: 3,
        base: 0x50000000,
        inter: [0xb, 0xc, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // I2C 0 device
    Device {
        name: "I2C0",
        num: 4,
        base: 0x40044000,
        inter: [0x17, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 4, 5, 8, 9, 12, 13, 16, 17, 20, 21, 24, 25, 28, 29]),
        func_sel: Some(3),
        len: 0x1000
    },
    // I2C 1 device
    Device {
        name: "I2C1",
        num: 5,
        base: 0x40048000,
        inter: [0x18, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([2, 3, 6, 7, 10, 11, 14, 15, 18, 19, 22, 23, 26, 27]),
        func_sel: Some(3),
        len: 0x1000
    },
    // IO Bank 0 device
    Device {
        name: "IO Bank0",
        num: 6,
        base: 0x40014000,
        inter: [0xd, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // IO QSPI device
    Device {
        name: "IO QSPI",
        num: 7,
        base: 0x40018000,
        inter: [0xe, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // IO Bank 0 Pads device
    Device {
        name: "IO Bank0 Pads",
        num: 8,
        base: 0x4001c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // IO QSPI Pads device
    Device {
        name: "IO QSPI Pads",
        num: 9,
        base: 0x40020000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Programmable IO 0 device
    Device {
        name: "PIO0",
        num: 10,
        base: 0x50200000,
        inter: [0x7, 0x8, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(6),
        len: 0x1000
    },
    // Programmable IO 1 device
    Device {
        name: "PIO1",
        num: 11,
        base: 0x50300000,
        inter: [0x9, 0xa, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(7),
        len: 0x1000
    },
    // System PLL device
    Device {
        name: "PLL_SYS",
        num: 12,
        base: 0x40028000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // USB PLL device
    Device {
        name: "PLL_USB",
        num: 13,
        base: 0x4002c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Pulse Width Module device
    Device {
        name: "PWM",
        num: 14,
        base: 0x40050000,
        inter: [0x4, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(4),
        len: 0x1000
    },
    // Real Time Clock device
    Device {
        name: "RTC",
        num: 15,
        base: 0x4005c000,
        inter: [0x19, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // SPI 0 device
    Device {
        name: "SPI0",
        num: 16,
        base: 0x4003c000,
        inter: [0x12, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]),
        func_sel: Some(1),
        len: 0x1000
    },
    // SPI 1 device
    Device {
        name: "SPI1",
        num: 17,
        base: 0x40040000,
        inter: [0x13, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(1),
        len: 0x1000
    },
    // System config device
    Device {
        name: "Syscfg",
        num: 18,
        base: 0x40004000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // System info device
    Device {
        name: "Sysinfo",
        num: 19,
        base: 0x40000000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Timer device
    Device {
        name: "Timer",
        num: 20,
        base: 0x40054000,
        inter: [0x0, 0x1, 0x2, 0x3],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // UART 0 device
    Device {
        name: "UART0",
        num: 21,
        base: 0x40034000,
        inter: [0x14, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 12, 13, 14, 15, 16, 17, 18, 19, 28, 29]),
        func_sel: Some(2),
        len: 0x1000,
    },
    // USB device
    Device {
        name: "USB",
        num: 22,
        base: 0x50110000,
        inter: [0x5, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(9),
        len: 0x1000
    },
    // Power Startup Machine device
    Device {
        name: "PSM",
        num: 23,
        base: 0x40010000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // Ring Oscillator device
    Device {
        name: "ROSC",
        num: 24,
        base: 0x40060000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Crystal Oscillator device
    Device {
        name: "XOSC",
        num: 25,
        base: 0x40024000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Clocks device
    Device {
        name: "Clocks",
        num: 26,
        base: 0x40008000,
        inter: [0x11, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    // Subsystem Reset device
    Device {
        name: "Subsystem Reset",
        num: 27,
        base: 0x4000c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // Execute In Place device
    Device {
        name: "XIP",
        num: 28,
        base: 0x14000000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // SSI device
    Device {
        name: "SSI",
        num: 29,
        base: 0x18000000,
        inter: [0x6, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: Some(0),
        len: 0x1000
    },
    // Chip Level Reset device
    Device {
        name: "Chip Reset",
        num: 30,
        base: 0x40064000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // Also one for proc 1 at same address although we only use a single core
    // Single Cycle IO Processor 0 device
    Device {
        name: "SIO Proc 0",
        num: 31,
        base: 0xd0000000,
        inter: [0xf, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(5),
        len: 0x1000
    },
    // Watchdog device
    Device {
        name: "Watchdog",
        num: 32,
        base: 0x40058000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
]);

/// List of all allocted devices
static DEVICES_TAKEN: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| {
    Mutex::new(HashSet::new())
});

/// List of all GPIOs allocated
static PINS_TAKEN: LazyLock<Mutex<HashSet<u8>>> = LazyLock::new(|| {
    Mutex::new(HashSet::from([4]))
});

/// Device allocation error
#[derive(Debug)]
pub enum DeviceError {
    /// Device taken
    Taken,
    /// Invalid device specified
    Invalid
}

/// Finds and allocates a device  
/// `name` is the name of the device to allocate  
/// Returns a reference to the device on success or a `DeviceError` on failure
pub fn find_device(name: &str) -> Result<&'static Device, DeviceError> {
    let mut taken = DEVICES_TAKEN.lock().unwrap();
    for device in &*DEVICES {
        if device.name == name {
            if taken.contains(&device.num) {
                return Err(DeviceError::Taken);
            }
            taken.insert(device.num);
            return Ok(device);
        }
    }
    Err(DeviceError::Invalid)
}

/// Looks up a device name from its number  
/// `num` is the device number
pub fn lookup_device(num: u16) -> Option<&'static str> {
    for device in &*DEVICES {
        if device.num == num {
            return Some(device.name);
        }
    }
    None
}

/// GPIO pin error
pub enum PinError {
    /// GPIO pin already taken
    Taken(Vec<u8>),
    /// Invalid GPIO pin specified
    Invalid(Vec<u8>)
}

/// Alloctes GPIO pins and updates the kernel driver arguments  
/// `driver_args` are the current kernel driver arguments that will be updated  
/// `pins` is the list of pins being allocated  
/// `device` is the device the pins are being allocated for
pub fn take_pins(driver_args: &mut DriverArgs, pins: &[u8], device: &Device) -> Result<(), PinError> {
    let mut taken = Vec::new();
    let mut invalid = Vec::new();
    let mut taken_pins = PINS_TAKEN.lock().unwrap();
    if device.num >= 1 && device.num <= 22 {
        let extra_shift = if device.num > 21 {
            // skip UART1, TBMAN and JTAG
            3
        } else if device.num > 19 {
            // skip TBMAN and JTAG
            2
        } else if device.num > 7 {
            // skip JTAG
            1
        } else {
            0
        };
        driver_args.resets |= 1 << (device.num + extra_shift - 1);
    }
    for pin in pins {
        if !device.available_gpio.contains(pin) {
            invalid.push(*pin)
        } else if taken_pins.contains(pin) {
            taken.push(*pin);
        } else {
            taken_pins.insert(*pin);
            let pin_index = *pin as usize / 8;
            let pin_shift = (*pin as usize % 8) * 4;
            let pin_mask = 0xf << pin_shift;
            driver_args.pin_func[pin_index] &= !pin_mask;
            driver_args.pin_func[pin_index] |= (device.func_sel.unwrap() as u32) << pin_shift;
            let pin_index = *pin as usize / 16;
            let pin_shift = (*pin as usize % 16) * 2;
            if device.name == "ADC" {
                driver_args.pads[pin_index] |= PAD_ANALOG << pin_shift;
            } else if device.name == "I2C0" || device.name == "I2C1" {
                driver_args.pads[pin_index] |= PAD_PULL_UP << pin_shift;
            } else {
                driver_args.pads[pin_index] |= PAD_NORMAL << pin_shift;
            }
        }
    }
    if invalid.len() > 0 {
        Err(PinError::Invalid(invalid))
    } else if taken.len() > 0 {
        Err(PinError::Taken(taken))
    } else {
        Ok(())
    }
}

mod test {
    use super::*;

    #[test]
    fn test_lookup_device() {
        assert_eq!(lookup_device(1).unwrap(), "ADC");
        assert_eq!(lookup_device(2).unwrap(), "Bus Control");
        assert_eq!(lookup_device(3).unwrap(), "DMA");
        assert_eq!(lookup_device(4).unwrap(), "I2C0");
        assert_eq!(lookup_device(5).unwrap(), "I2C1");
        assert_eq!(lookup_device(6).unwrap(), "IO Bank0");
        assert_eq!(lookup_device(7).unwrap(), "IO QSPI");
        assert_eq!(lookup_device(8).unwrap(), "IO Bank0 Pads");
        assert_eq!(lookup_device(9).unwrap(), "IO QSPI Pads");
        assert_eq!(lookup_device(10).unwrap(), "PIO0");
        assert_eq!(lookup_device(11).unwrap(), "PIO1");
        assert_eq!(lookup_device(12).unwrap(), "PLL_SYS");
        assert_eq!(lookup_device(13).unwrap(), "PLL_USB");
        assert_eq!(lookup_device(14).unwrap(), "PWM");
        assert_eq!(lookup_device(15).unwrap(), "RTC");
        assert_eq!(lookup_device(16).unwrap(), "SPI0");
        assert_eq!(lookup_device(17).unwrap(), "SPI1");
        assert_eq!(lookup_device(18).unwrap(), "Syscfg");
        assert_eq!(lookup_device(19).unwrap(), "Sysinfo");
        assert_eq!(lookup_device(20).unwrap(), "Timer");
        assert_eq!(lookup_device(21).unwrap(), "UART0");
        assert_eq!(lookup_device(22).unwrap(), "USB");
        assert_eq!(lookup_device(23).unwrap(), "PSM");
        assert_eq!(lookup_device(24).unwrap(), "ROSC");
        assert_eq!(lookup_device(25).unwrap(), "XOSC");
        assert_eq!(lookup_device(26).unwrap(), "Clocks");
        assert_eq!(lookup_device(27).unwrap(), "Subsystem Reset");
        assert_eq!(lookup_device(28).unwrap(), "XIP");
        assert_eq!(lookup_device(29).unwrap(), "SSI");
        assert_eq!(lookup_device(30).unwrap(), "Chip Reset");
        assert_eq!(lookup_device(31).unwrap(), "SIO Proc 0");
        assert_eq!(lookup_device(32).unwrap(), "Watchdog");
        assert_eq!(lookup_device(33), None);
    }

    #[test]
    fn test_find_device() {
        assert_eq!(find_device("ADC").unwrap().num, 1);
        assert_eq!(find_device("Bus Control").unwrap().num, 2);
        assert_eq!(find_device("DMA").unwrap().num, 3);
        assert_eq!(find_device("I2C0").unwrap().num, 4);
        assert_eq!(find_device("I2C1").unwrap().num, 5);
        assert_eq!(find_device("IO Bank0").unwrap().num, 6);
        assert_eq!(find_device("IO QSPI").unwrap().num, 7);
        assert_eq!(find_device("IO Bank0 Pads").unwrap().num, 8);
        assert_eq!(find_device("IO QSPI Pads").unwrap().num, 9);
        assert_eq!(find_device("PIO0").unwrap().num, 10);
        assert_eq!(find_device("PIO1").unwrap().num, 11);
        assert_eq!(find_device("PLL_SYS").unwrap().num, 12);
        assert_eq!(find_device("PLL_USB").unwrap().num, 13);
        assert_eq!(find_device("PWM").unwrap().num, 14);
        assert_eq!(find_device("RTC").unwrap().num, 15);
        assert_eq!(find_device("SPI0").unwrap().num, 16);
        assert_eq!(find_device("SPI1").unwrap().num, 17);
        assert_eq!(find_device("Syscfg").unwrap().num, 18);
        assert_eq!(find_device("Sysinfo").unwrap().num, 19);
        assert_eq!(find_device("Timer").unwrap().num, 20);
        assert_eq!(find_device("UART0").unwrap().num, 21);
        assert_eq!(find_device("USB").unwrap().num, 22);
        assert_eq!(find_device("PSM").unwrap().num, 23);
        assert_eq!(find_device("ROSC").unwrap().num, 24);
        assert_eq!(find_device("XOSC").unwrap().num, 25);
        assert_eq!(find_device("Clocks").unwrap().num, 26);
        assert_eq!(find_device("Subsystem Reset").unwrap().num, 27);
        assert_eq!(find_device("XIP").unwrap().num, 28);
        assert_eq!(find_device("SSI").unwrap().num, 29);
        assert_eq!(find_device("Chip Reset").unwrap().num, 30);
        assert_eq!(find_device("SIO Proc 0").unwrap().num, 31);
        assert_eq!(find_device("Watchdog").unwrap().num, 32);
        assert!(matches!(find_device("ADC"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Bus Control"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("DMA"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("I2C0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("I2C1"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("IO Bank0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("IO QSPI"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("IO Bank0 Pads"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("IO QSPI Pads"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PIO0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PIO1"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PLL_SYS"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PLL_USB"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PWM"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("RTC"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("SPI0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("SPI1"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Syscfg"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Sysinfo"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Timer"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("UART0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("USB"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("PSM"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("ROSC"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("XOSC"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Clocks"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Subsystem Reset"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("XIP"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("SSI"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Chip Reset"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("SIO Proc 0"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Watchdog"), Err(DeviceError::Taken)));
        assert!(matches!(find_device("Device"), Err(DeviceError::Invalid)));
    }
}
