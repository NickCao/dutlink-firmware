use core::convert::TryInto;
use core::fmt::Write;

use num_enum::TryFromPrimitive;
use usb_device::class_prelude::*;
use usb_device::control::{Recipient, Request, RequestType};
use usb_device::Result;

use crate::config::{ConfigArea, ConfigBlock};
use crate::ctlpins::{CTLPinsTrait, PinState};
use crate::powermeter::PowerMeter;
use crate::storage::StorageSwitchTrait;

const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xff;
const USB_SUBCLASS_JUMPSTARTER: u8 = 0x01;
const USB_PROTOCOL_JUMPSTARTER: u8 = 0x01;

#[repr(u8)]
#[derive(TryFromPrimitive)]
pub enum ControlRequest {
    Nop,
    Power,
    Storage,
    Config,
    Read,
    Set,
}

#[repr(u16)]
#[derive(TryFromPrimitive)]
pub enum PowerAction {
    Nop,
    Off,
    On,
    ForceOff,
    ForceOn,
    Rescue,
}

#[repr(u16)]
#[derive(TryFromPrimitive)]
pub enum StorageAction {
    Nop,
    Off,
    Host,
    DUT,
}

#[repr(u16)]
#[derive(TryFromPrimitive)]
pub enum ConfigKey {
    Name,
    Tags,
    Json,
    UsbConsole,
    PowerOn,
    PowerOff,
    PowerRescue,
}

#[repr(u16)]
#[derive(TryFromPrimitive)]
pub enum ReadKey {
    Version,
    Power,
    Voltage,
    Current,
}

#[repr(u16)]
#[derive(TryFromPrimitive)]
pub enum SetPin {
    Reset,
    A,
    B,
    C,
    D,
}

#[repr(u8)]
#[derive(TryFromPrimitive)]
pub enum SetPinState {
    Low,
    High,
    Floating,
}

pub struct ControlClass {
    iface: InterfaceNumber,
    config: Option<(ConfigKey, heapless::Vec<u8, 256>)>,
    power: Option<PowerAction>,
    storage: Option<StorageAction>,
    pin: Option<(SetPin, SetPinState)>,
    data: Data,
}

pub struct Data {
    power: f32,
    voltage: f32,
    current: f32,
    config: ConfigBlock,
}

impl ControlClass {
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>) -> Self {
        Self {
            iface: alloc.interface(),
            power: None,
            storage: None,
            pin: None,
            config: None,
            data: Data {
                power: 0.0,
                voltage: 0.0,
                current: 0.0,
                config: ConfigBlock::new(),
            },
        }
    }
    pub fn feed(&mut self, config: &ConfigArea, power_meter: &mut dyn PowerMeter) {
        self.data.power = power_meter.get_power();
        self.data.voltage = power_meter.get_voltage();
        self.data.current = power_meter.get_current();
        self.data.config = config.get();
    }
    pub fn handle<C: CTLPinsTrait, S: StorageSwitchTrait>(
        &mut self,
        config: &mut ConfigArea,
        ctlpins: &mut C,
        storage: &mut S,
    ) {
        if let Some((key, value)) = self.config.take() {
            match key {
                ConfigKey::Name => {
                    let cfg = config.get().set_name(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::Tags => {
                    let cfg = config.get().set_tags(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::Json => {
                    let cfg = config.get().set_json(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::UsbConsole => {
                    let cfg = config.get().set_usb_console(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::PowerOn => {
                    let cfg = config.get().set_power_on(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::PowerOff => {
                    let cfg = config.get().set_power_off(&value);
                    config.write_config(&cfg).ok();
                }
                ConfigKey::PowerRescue => {
                    let cfg = config.get().set_power_rescue(&value);
                    config.write_config(&cfg).ok();
                }
            }
        }
        if let Some(action) = self.power.take() {
            match action {
                PowerAction::Nop => (),
                PowerAction::Off => {
                    ctlpins.power_off(&self.data.config.power_off);
                }
                PowerAction::On => {
                    ctlpins.power_on(&self.data.config.power_on);
                }
                PowerAction::ForceOff => {
                    ctlpins.power_off(&[]);
                }
                PowerAction::ForceOn => {
                    ctlpins.power_on(&[]);
                }
                PowerAction::Rescue => {
                    ctlpins.power_on(&self.data.config.power_rescue);
                }
            }
        }
        if let Some(action) = self.storage.take() {
            match action {
                StorageAction::Nop => (),
                StorageAction::Off => {
                    storage.power_off();
                }
                StorageAction::Host => {
                    storage.connect_to_host();
                }
                StorageAction::DUT => {
                    storage.connect_to_dut();
                }
            }
        }
        if let Some((pin, state)) = self.pin.take() {
            let state = match state {
                SetPinState::Low => PinState::Low,
                SetPinState::High => PinState::High,
                SetPinState::Floating => PinState::Floating,
            };
            match pin {
                SetPin::Reset => {
                    ctlpins.set_reset(state);
                }
                SetPin::A => {
                    ctlpins.set_ctl_a(state);
                }
                SetPin::B => {
                    ctlpins.set_ctl_b(state);
                }
                SetPin::C => {
                    ctlpins.set_ctl_c(state);
                }
                SetPin::D => {
                    ctlpins.set_ctl_d(state);
                }
            }
        }
    }
}

impl<B: UsbBus> UsbClass<B> for ControlClass {
    fn get_configuration_descriptors(&self, writer: &mut DescriptorWriter) -> Result<()> {
        writer.iad(
            self.iface,
            1,
            USB_CLASS_VENDOR_SPECIFIC,
            USB_SUBCLASS_JUMPSTARTER,
            USB_PROTOCOL_JUMPSTARTER,
            None,
        )?;

        writer.interface(
            self.iface,
            USB_CLASS_VENDOR_SPECIFIC,
            USB_SUBCLASS_JUMPSTARTER,
            USB_PROTOCOL_JUMPSTARTER,
        )?;

        Ok(())
    }

    fn control_in(&mut self, xfer: ControlIn<B>) {
        let req = xfer.request();

        match req {
            &Request {
                request_type: RequestType::Vendor,
                recipient: Recipient::Interface,
                index,
                ..
            } if index as u8 == self.iface.into() => (),
            _ => return,
        }

        match req.request.try_into() {
            Ok(ControlRequest::Config) => {
                if let Ok(key) = req.value.try_into() {
                    let cfg = &self.data.config;
                    match key {
                        ConfigKey::Name => {
                            xfer.accept_with(&cfg.name).ok();
                        }
                        ConfigKey::Tags => {
                            xfer.accept_with(&cfg.tags).ok();
                        }
                        ConfigKey::Json => {
                            xfer.accept_with(&cfg.json).ok();
                        }
                        ConfigKey::UsbConsole => {
                            xfer.accept_with(&cfg.usb_console).ok();
                        }
                        ConfigKey::PowerOn => {
                            xfer.accept_with(&cfg.power_on).ok();
                        }
                        ConfigKey::PowerOff => {
                            xfer.accept_with(&cfg.power_off).ok();
                        }
                        ConfigKey::PowerRescue => {
                            xfer.accept_with(&cfg.power_rescue).ok();
                        }
                    }
                } else {
                    xfer.reject().unwrap();
                }
            }
            Ok(ControlRequest::Read) => {
                if let Ok(key) = req.value.try_into() {
                    match key {
                        ReadKey::Version => {
                            let mut buf = heapless::Vec::<u8, 128>::new();
                            crate::version::write_version(&mut buf);
                            xfer.accept_with(&buf).ok();
                        }
                        ReadKey::Power => {
                            let mut buf = heapless::Vec::<u8, 128>::new();
                            write!(buf, "{:.2}W", self.data.power).ok();
                            xfer.accept_with(&buf).ok();
                        }
                        ReadKey::Voltage => {
                            let mut buf = heapless::Vec::<u8, 128>::new();
                            write!(buf, "{:.2}V", self.data.voltage).ok();
                            xfer.accept_with(&buf).ok();
                        }
                        ReadKey::Current => {
                            let mut buf = heapless::Vec::<u8, 128>::new();
                            write!(buf, "{:.2}A", self.data.current).ok();
                            xfer.accept_with(&buf).ok();
                        }
                    }
                } else {
                    xfer.reject().unwrap();
                }
            }
            _ => {
                xfer.reject().unwrap();
            }
        }
    }

    fn control_out(&mut self, xfer: ControlOut<B>) {
        let req = xfer.request();

        match req {
            &Request {
                request_type: RequestType::Vendor,
                recipient: Recipient::Interface,
                index,
                ..
            } if index as u8 == self.iface.into() => (),
            _ => return,
        }

        match req.request.try_into() {
            Ok(ControlRequest::Nop) => {
                xfer.accept().unwrap();
            }
            Ok(ControlRequest::Power) => {
                if let Ok(action) = req.value.try_into() {
                    self.power = Some(action);
                    xfer.accept().unwrap();
                } else {
                    xfer.reject().unwrap();
                }
            }
            Ok(ControlRequest::Storage) => {
                if let Ok(action) = req.value.try_into() {
                    self.storage = Some(action);
                    xfer.accept().unwrap();
                } else {
                    xfer.reject().unwrap();
                }
            }
            Ok(ControlRequest::Config) => {
                if let Ok(key) = req.value.try_into() {
                    self.config = Some((key, heapless::Vec::from_slice(xfer.data()).unwrap()));
                    xfer.accept().unwrap();
                } else {
                    xfer.reject().unwrap();
                }
            }
            Ok(ControlRequest::Set) => {
                if let Ok(key) = req.value.try_into() {
                    if let Some(Ok(state)) = xfer
                        .data()
                        .first()
                        .cloned()
                        .map(TryInto::<SetPinState>::try_into)
                    {
                        self.pin = Some((key, state));
                        xfer.accept().unwrap();
                    } else {
                        xfer.reject().unwrap();
                    }
                } else {
                    xfer.reject().unwrap();
                }
            }
            _ => {
                xfer.reject().unwrap();
            }
        }
    }
}
