use std::{collections::HashSet, sync::{LazyLock, Mutex}};

pub struct Driver {
    pub name: &'static str,
    pub num: u16,
    pub base: u32,
    pub inter: u8,
    pub len: u32
}

static DRIVERS: [Driver; 1] = [
    Driver {
        name: "UART 0",
        num: 1,
        base: 0x40034000,
        inter: 20,
        len: 0x1000,
    }
];

static DRIVERS_TAKEN: LazyLock<Mutex<HashSet<u16>>> = LazyLock::new(|| {
    Mutex::new(HashSet::new())
});

pub fn find_driver(num: u16) -> Option<&'static Driver> {
    let mut taken = DRIVERS_TAKEN.lock().unwrap();
    if taken.contains(&num) {
        return None;
    }
    for driver in &DRIVERS {
        if driver.num == num {
            taken.insert(num);
            return Some(driver);
        }
    }
    None
}
