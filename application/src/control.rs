use core::convert::TryInto;

use num_enum::TryFromPrimitive;
use usb_device::class_prelude::*;
use usb_device::control::{Recipient, Request, RequestType};
use usb_device::Result;

use crate::config::ConfigArea;
use crate::ctlpins::CTLPinsTrait;

const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xff;
const USB_SUBCLASS_JUMPSTARTER: u8 = 0x01;
const USB_PROTOCOL_JUMPSTARTER: u8 = 0x01;

#[repr(u8)]
#[derive(TryFromPrimitive)]
pub enum ControlRequest {
    Nop = 0,
    Power = 1,
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

pub struct ControlClass {
    iface: InterfaceNumber,
    power: Option<PowerAction>,
}

impl ControlClass {
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>) -> Self {
        Self {
            iface: alloc.interface(),
            power: None,
        }
    }
    pub fn handle<C: CTLPinsTrait>(&mut self, ctlpins: &mut C, config: &ConfigArea) {
        if let Some(action) = self.power.take() {
            match action {
                PowerAction::Off => {
                    ctlpins.power_off(&config.get().power_off);
                }
                PowerAction::On => {
                    ctlpins.power_on(&config.get().power_on);
                }
                PowerAction::ForceOff => {
                    ctlpins.power_off(&[]);
                }
                PowerAction::ForceOn => {
                    ctlpins.power_on(&[]);
                }
                PowerAction::Rescue => {
                    ctlpins.power_on(&config.get().power_rescue);
                }
                _ => (),
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

        match req.request {
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
            _ => {
                xfer.reject().unwrap();
            }
        }
    }
}
