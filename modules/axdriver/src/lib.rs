//! [ArceOS](https://github.com/rcore-os/arceos) device drivers.
//!
//! # Usage
//!
//! All detected devices are composed into a large struct [`AllDevices`]
//! and returned by the [`init_drivers`] function. The upperlayer subsystems
//! (e.g., the network stack) may unpack the struct to get the specified device
//! driver they want.
//!
//! For each device category (i.e., net, block, display, etc.), an unified type
//! is used to represent all devices in that category. Currently, there are 3
//! categories: [`AxNetDevice`], [`AxBlockDevice`], and [`AxDisplayDevice`].
//!
//! # Concepts
//!
//! This crate supports two device models depending on the `dyn` feature:
//!

//! 静态：所有设备的类型都是静态的，它在编译时由相应的 cargo 特性确定。例如，如果启用了 virtio-net 特性，则 [AxNetDevice] 将成为 [VirtioNetDev] 的别名。这个模型提供了最佳的性能，因为它避免了动态分发。但有一个限制，每个设备类别只支持一个设备实例。
//! 动态：所有设备实例都使用[特征对象（trait objects）]，并包装在 Box<dyn Trait> 中。例如，[AxNetDevice] 将成为 [Box<dyn NetDriverOps>]。当调用设备提供的方法时，它使用[动态分发][dyn]，这可能会引入一些开销。但另一方面，它更加灵活，支持每个设备类别的多个实例。
//! - **Static**: The type of all devices is static, it is determined at compile
//!  time by corresponding cargo features. For example, [`AxNetDevice`] will be
//! an alias of [`VirtioNetDev`] if the `virtio-net` feature is enabled. This
//! model provides the best performance as it avoids dynamic dispatch. But on
//! limitation, only one device instance is supported for each device category.
//! - **Dynamic**: All device instance is using [trait objects] and wrapped in a
//! `Box<dyn Trait>`. For example, [`AxNetDevice`] will be [`Box<dyn NetDriverOps>`].
//! When call a method provided by the device, it uses [dynamic dispatch][dyn]
//! that may introduce a little overhead. But on the other hand, it is more
//! flexible, multiple instances of each device category are supported.
//!
//! # Supported Devices
//!
//! | Device Category | Cargo Feature | Description |
//! |-|-|-|
//! | Block | `ramdisk` | A RAM disk that stores data in a vector |
//! | Block | `virtio-blk` | VirtIO block device |
//! | Network | `virtio-net` | VirtIO network device |
//! | Display | `virtio-gpu` | VirtIO graphics device |
//!
//! # Other Cargo Features
//!
//! - `dyn`: use the dynamic device model (see above).
//! - `bus-mmio`: use device tree to probe all MMIO devices. This feature is
//!    enabeld by default.
//! - `bus-pci`: use PCI bus to probe all PCI devices.
//! - `virtio`: use VirtIO devices. This is enabled if any of `virtio-blk`,
//!   `virtio-net` or `virtio-gpu` is enabled.
//! - `net`: use network devices. This is enabled if any feature of network
//!    devices is selected. If this feature is enabled without any network device
//!    features, a dummy struct is used for [`AxNetDevice`].
//! - `block`: use block storage devices. Similar to the `net` feature.
//! - `display`: use graphics display devices. Similar to the `net` feature.
//!
//! [`VirtioNetDev`]: driver_virtio::VirtIoNetDev
//! [`Box<dyn NetDriverOps>`]: driver_net::NetDriverOps
//! [trait objects]: https://doc.rust-lang.org/book/ch17-02-trait-objects.html
//! [dyn]: https://doc.rust-lang.org/std/keyword.dyn.html

#![no_std]
#![feature(doc_auto_cfg)]
#![feature(associated_type_defaults)]

#[macro_use]
extern crate log;

#[cfg(feature = "dyn")]
extern crate alloc;

#[macro_use]
mod macros;

mod bus;
mod drivers;
mod dummy;
mod structs;

#[cfg(feature = "virtio")]
mod virtio;

#[cfg(feature = "ixgbe")]
mod ixgbe;

pub mod prelude;

#[allow(unused_imports)]
use self::prelude::*;
pub use self::structs::{AxDeviceContainer, AxDeviceEnum};

#[cfg(feature = "block")]
pub use self::structs::AxBlockDevice;
#[cfg(feature = "display")]
pub use self::structs::AxDisplayDevice;
#[cfg(feature = "net")]
pub use self::structs::AxNetDevice;

/// A structure that contains all device drivers, organized by their category.
#[derive(Default)]
pub struct AllDevices {
    /// All network device drivers.
    #[cfg(feature = "net")]
    pub net: AxDeviceContainer<AxNetDevice>,
    /// All block device drivers.
    #[cfg(feature = "block")]
    pub block: AxDeviceContainer<AxBlockDevice>,
    /// All graphics device drivers.
    #[cfg(feature = "display")]
    pub display: AxDeviceContainer<AxDisplayDevice>,
}

impl AllDevices {
    /// Returns the device model used, either `dyn` or `static`.
    ///
    /// See the [crate-level documentation](crate) for more details.
    pub const fn device_model() -> &'static str {
        if cfg!(feature = "dyn") {
            "dyn"
        } else {
            "static"
        }
    }

    /// Probes all supported devices. 在系统启动时自动识别并注册可用的设备，以便系统能够与这些设备进行交互。这种动态的设备管理允许系统适应各种不同配置和硬件环境
    fn probe(&mut self) {
        for_each_drivers!(type Driver, {
            if let Some(dev) = Driver::probe_global() {
                info!(
                    "registered a new {:?} device: {:?}",
                    dev.device_type(),
                    dev.device_name(),
                );
                self.add_device(dev);
            }
        });

        self.probe_bus_devices();
    }

    /// Adds one device into the corresponding container, according to its device category.
    #[allow(dead_code)]
    fn add_device(&mut self, dev: AxDeviceEnum) {
        match dev {
            #[cfg(feature = "net")]
            AxDeviceEnum::Net(dev) => self.net.push(dev),
            #[cfg(feature = "block")]
            AxDeviceEnum::Block(dev) => self.block.push(dev),
            #[cfg(feature = "display")]
            AxDeviceEnum::Display(dev) => self.display.push(dev),
        }
    }
}

/// Probes and initializes all device drivers, returns the [`AllDevices`] struct.
pub fn init_drivers() -> AllDevices {
    info!("Initialize device drivers...");
    info!("  device model: {}", AllDevices::device_model());

    let mut all_devs = AllDevices::default();
    all_devs.probe();

    #[cfg(feature = "net")]
    {
        debug!("number of NICs: {}", all_devs.net.len());
        for (i, dev) in all_devs.net.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Net);
            debug!("  NIC {}: {:?}", i, dev.device_name());
        }
    }
    #[cfg(feature = "block")]
    {
        debug!("number of block devices: {}", all_devs.block.len());
        for (i, dev) in all_devs.block.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Block);
            debug!("  block device {}: {:?}", i, dev.device_name());
        }
    }
    #[cfg(feature = "display")]
    {
        debug!("number of graphics devices: {}", all_devs.display.len());
        for (i, dev) in all_devs.display.iter().enumerate() {
            assert_eq!(dev.device_type(), DeviceType::Display);
            debug!("  graphics device {}: {:?}", i, dev.device_name());
        }
    }

    all_devs
}
