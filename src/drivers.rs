use std::{collections::HashSet, sync::{LazyLock, Mutex}};

pub struct Driver {
    pub name: &'static str,
    pub num: u16,
    pub base: u32,
    pub inter: [u8; 4],
    pub len: u32
}

static DRIVERS: [Driver; 30] = [
    // Also one for proc 1 at same address although we only use a single core
    Driver {
        name: "SIO Proc 0",
        num: 1,
        base: 0xd0000000,
        inter: [0xf, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "DMA",
        num: 2,
        base: 0x50000000,
        inter: [0xb, 0xc, 0xff, 0xff],
        len: 0x1000,
    },
    Driver {
        name: "Chip Reset",
        num: 3,
        base: 0x40064000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000,
    },
    Driver {
        name: "PSM",
        num: 4,
        base: 0x40010000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000,
    },
    Driver {
        name: "Subsystem Reset",
        num: 5,
        base: 0x4000c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000,
    },
    Driver {
        name: "Clocks",
        num: 6,
        base: 0x40008000,
        inter: [0x11, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "XOSC",
        num: 7,
        base: 0x40024000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "ROSC",
        num: 8,
        base: 0x40060000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "PLL_SYS",
        num: 9,
        base: 0x40028000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "PLL_USB",
        num: 10,
        base: 0x4002c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "IO Bank0",
        num: 11,
        base: 0x40014000,
        inter: [0xd, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "IO QSPI",
        num: 12,
        base: 0x40018000,
        inter: [0xe, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "IO Bank0 Pad",
        num: 13,
        base: 0x4001c000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "IO QSPI Pad",
        num: 14,
        base: 0x40020000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "Sysinfo",
        num: 15,
        base: 0x40000000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "Syscfg",
        num: 16,
        base: 0x40004000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "PIO0",
        num: 17,
        base: 0x50200000,
        inter: [0x7, 0x8, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "PIO1",
        num: 18,
        base: 0x50300000,
        inter: [0x9, 0xa, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "USB",
        num: 19,
        base: 0x50110000,
        inter: [0x5, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "UART0",
        num: 20,
        base: 0x40034000,
        inter: [0x14, 0xff, 0xff, 0xff],
        len: 0x1000,
    },
    Driver {
        name: "I2C0",
        num: 21,
        base: 0x40044000,
        inter: [0x17, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "I2C1",
        num: 22,
        base: 0x40048000,
        inter: [0x18, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "SPI0",
        num: 23,
        base: 0x4003c000,
        inter: [0x12, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "SPI1",
        num: 24,
        base: 0x40040000,
        inter: [0x13, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "PWM",
        num: 25,
        base: 0x40050000,
        inter: [0x4, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "Timer",
        num: 26,
        base: 0x40054000,
        inter: [0x0, 0x1, 0x2, 0x3],
        len: 0x1000
    },
    Driver {
        name: "Watchdog",
        num: 27,
        base: 0x40058000,
        inter: [0xff, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "RTC",
        num: 28,
        base: 0x4005c000,
        inter: [0x19, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "ADC",
        num: 29,
        base: 0x4004c000,
        inter: [0x16, 0xff, 0xff, 0xff],
        len: 0x1000
    },
    Driver {
        name: "SSI",
        num: 30,
        base: 0x18000000,
        inter: [0x6, 0xff, 0xff, 0xff],
        len: 0x1000
    }
];

static DRIVERS_TAKEN: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| {
    Mutex::new(HashSet::new())
});

pub fn find_driver(name: &str) -> Option<&'static Driver> {
    let mut taken = DRIVERS_TAKEN.lock().unwrap();
    for driver in &DRIVERS {
        if driver.name == name {
            if taken.contains(&driver.num) {
                return None;
            }
            taken.insert(driver.num);
            return Some(driver);
        }
    }
    None
}

pub fn lookup_driver(num: u16) -> Option<&'static str> {
    for driver in &DRIVERS {
        if driver.num == num {
            return Some(driver.name);
        }
    }
    None
}
