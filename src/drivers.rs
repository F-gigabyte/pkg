use std::{collections::HashSet, sync::{LazyLock, Mutex}};

use crate::driver_args::{DriverArgs, PAD_ANALOG, PAD_NORMAL, PAD_PULL_UP};

pub struct Driver {
    pub name: &'static str,
    pub num: u16,
    pub base: u32,
    pub inter: [u8; 4],
    pub available_gpio: HashSet<u8>,
    pub func_sel: Option<u8>,
    pub len: u32
}

static DRIVERS: LazyLock<[Driver; 32]> = LazyLock::new(|| [
    Driver {
        name: "ADC",
        num: 1,
        base: 0x4004c000,
        inter: [0x16, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([26, 27, 28, 29]),
        func_sel: Some(5),
        len: 0x1000
    },
    Driver {
        name: "Bus Control",
        num: 2,
        base: 0x40030000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "DMA",
        num: 3,
        base: 0x50000000,
        inter: [0xb, 0xc, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    Driver {
        name: "I2C0",
        num: 4,
        base: 0x40044000,
        inter: [0x17, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 4, 5, 8, 9, 12, 13, 16, 17, 20, 21, 24, 25, 28, 29]),
        func_sel: Some(3),
        len: 0x1000
    },
    Driver {
        name: "I2C1",
        num: 5,
        base: 0x40048000,
        inter: [0x18, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([2, 3, 6, 7, 10, 11, 14, 15, 18, 19, 22, 23, 26, 27]),
        func_sel: Some(3),
        len: 0x1000
    },
    Driver {
        name: "IO Bank0",
        num: 6,
        base: 0x40014000,
        inter: [0xd, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "IO QSPI",
        num: 7,
        base: 0x40018000,
        inter: [0xe, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "IO Bank0 Pads",
        num: 8,
        base: 0x4001c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "IO QSPI Pads",
        num: 9,
        base: 0x40020000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "PIO0",
        num: 10,
        base: 0x50200000,
        inter: [0x7, 0x8, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(6),
        len: 0x1000
    },
    Driver {
        name: "PIO1",
        num: 11,
        base: 0x50300000,
        inter: [0x9, 0xa, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(7),
        len: 0x1000
    },
    Driver {
        name: "PLL_SYS",
        num: 12,
        base: 0x40028000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "PLL_USB",
        num: 13,
        base: 0x4002c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "PWM",
        num: 14,
        base: 0x40050000,
        inter: [0x4, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(4),
        len: 0x1000
    },
    Driver {
        name: "RTC",
        num: 15,
        base: 0x4005c000,
        inter: [0x19, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "SPI0",
        num: 16,
        base: 0x4003c000,
        inter: [0x12, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23]),
        func_sel: Some(1),
        len: 0x1000
    },
    Driver {
        name: "SPI1",
        num: 17,
        base: 0x40040000,
        inter: [0x13, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([8, 9, 10, 11, 12, 13, 14, 15, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(1),
        len: 0x1000
    },
    Driver {
        name: "Syscfg",
        num: 18,
        base: 0x40004000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "Sysinfo",
        num: 19,
        base: 0x40000000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "Timer",
        num: 20,
        base: 0x40054000,
        inter: [0x0, 0x1, 0x2, 0x3],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "UART0",
        num: 21,
        base: 0x40034000,
        inter: [0x14, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 12, 13, 14, 15, 16, 17, 18, 19, 28, 29]),
        func_sel: Some(2),
        len: 0x1000,
    },
    Driver {
        name: "USB",
        num: 22,
        base: 0x50110000,
        inter: [0x5, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(9),
        len: 0x1000
    },
    Driver {
        name: "PSM",
        num: 23,
        base: 0x40010000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    Driver {
        name: "ROSC",
        num: 24,
        base: 0x40060000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "XOSC",
        num: 25,
        base: 0x40024000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "Clocks",
        num: 26,
        base: 0x40008000,
        inter: [0x11, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
    Driver {
        name: "Subsystem Reset",
        num: 27,
        base: 0x4000c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    Driver {
        name: "XIP",
        num: 28,
        base: 0x14000000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    Driver {
        name: "SSI",
        num: 29,
        base: 0x18000000,
        inter: [0x6, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: Some(0),
        len: 0x1000
    },
    Driver {
        name: "Chip Reset",
        num: 30,
        base: 0x40064000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000,
    },
    // Also one for proc 1 at same address although we only use a single core
    Driver {
        name: "SIO Proc 0",
        num: 31,
        base: 0xd0000000,
        inter: [0xf, 0xff, 0xff, 0xff],
        available_gpio: HashSet::from([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29]),
        func_sel: Some(5),
        len: 0x1000
    },
    Driver {
        name: "Watchdog",
        num: 32,
        base: 0x40058000,
        inter: [0xff, 0xff, 0xff, 0xff],
        available_gpio: HashSet::new(),
        func_sel: None,
        len: 0x1000
    },
]);

static DRIVERS_TAKEN: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| {
    Mutex::new(HashSet::new())
});

static PINS_TAKEN: LazyLock<Mutex<HashSet<u8>>> = LazyLock::new(|| {
    Mutex::new(HashSet::from([4]))
});

pub enum DriverError {
    Taken,
    Invalid
}

pub fn find_driver(name: &str) -> Result<&'static Driver, DriverError> {
    let mut taken = DRIVERS_TAKEN.lock().unwrap();
    for driver in &*DRIVERS {
        if driver.name == name {
            if taken.contains(&driver.num) {
                return Err(DriverError::Taken);
            }
            taken.insert(driver.num);
            return Ok(driver);
        }
    }
    Err(DriverError::Invalid)
}

pub fn lookup_driver(num: u16) -> Option<&'static str> {
    for driver in &*DRIVERS {
        if driver.num == num {
            return Some(driver.name);
        }
    }
    None
}

pub enum PinError {
    Taken(Vec<u8>),
    Invalid(Vec<u8>)
}

pub fn take_pins(driver_args: &mut DriverArgs, pins: &[u8], driver: &Driver) -> Result<(), PinError> {
    let mut taken = Vec::new();
    let mut invalid = Vec::new();
    let mut taken_pins = PINS_TAKEN.lock().unwrap();
    if driver.num >= 1 && driver.num <= 22 {
        let extra_shift = if driver.num > 21 {
            // skip UART1, TBMAN and JTAG
            3
        } else if driver.num > 19 {
            // skip TBMAN and JTAG
            2
        } else if driver.num > 7 {
            // skip JTAG
            1
        } else {
            0
        };
        println!("Have reset for driver {} of 0x{:x}", driver.num, 1 << (driver.num + extra_shift - 1));
        driver_args.resets |= 1 << (driver.num + extra_shift - 1);
    }
    for pin in pins {
        if !driver.available_gpio.contains(pin) {
            invalid.push(*pin)
        } else if taken_pins.contains(pin) {
            taken.push(*pin);
        } else {
            taken_pins.insert(*pin);
            let pin_index = *pin as usize / 8;
            let pin_shift = (*pin as usize % 8) * 4;
            let pin_mask = 0xf << pin_shift;
            driver_args.pin_func[pin_index] &= !pin_mask;
            driver_args.pin_func[pin_index] |= (driver.func_sel.unwrap() as u32) << pin_shift;
            let pin_index = *pin as usize / 16;
            let pin_shift = (*pin as usize % 16) * 2;
            if driver.name == "ADC" {
                driver_args.pads[pin_index] |= PAD_ANALOG << pin_shift;
            } else if driver.name == "I2C0" || driver.name == "I2C1" {
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
