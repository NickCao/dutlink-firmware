use usb_device::class_prelude::*;
use usb_device::control::{Recipient, Request, RequestType};
use usb_device::Result;

use crate::config::ConfigArea;
use crate::ctlpins::CTLPinsTrait;

const USB_CLASS_VENDOR_SPECIFIC: u8 = 0xff;
const USB_SUBCLASS_JUMPSTARTER: u8 = 0x01;
const USB_PROTOCOL_JUMPSTARTER: u8 = 0x01;

#[repr(u8)]
#[non_exhaustive]
pub enum ControlRequest {
    Nop = 0,
    Power = 1,
}

pub struct ControlClass {
    iface: InterfaceNumber,
    power: Option<bool>,
}

impl ControlClass {
    pub fn new<B: UsbBus>(alloc: &UsbBusAllocator<B>) -> Self {
        Self {
            iface: alloc.interface(),
            power: None,
        }
    }
    pub fn handle<C: CTLPinsTrait>(&mut self, ctlpins: &mut C, config: &ConfigArea) {
        if let Some(power) = self.power.take() {
            if power {
                ctlpins.power_on(&config.get().power_on);
            } else {
                ctlpins.power_off(&config.get().power_off);
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

        match req.request {
            r if r == ControlRequest::Nop as u8 => {
                xfer.accept().unwrap();
            }
            r if r == ControlRequest::Power as u8 => match req.value {
                0 => {
                    self.power = Some(false);
                    xfer.accept().unwrap();
                }
                1 => {
                    self.power = Some(true);
                    xfer.accept().unwrap();
                }
                _ => {
                    xfer.reject().unwrap();
                }
            },
            _ => {
                xfer.reject().unwrap();
            }
        }
    }
}
