use core::str;
use embedded_hal::digital::OutputPin;
use usb_device::endpoint::EndpointDirection;
use core::fmt::Write;

use arrayvec::ArrayString;

use crate::config::ConfigArea;
use crate::ctlpins::{PinState, CTLPinsTrait};
use crate::powermeter::PowerMeter;
use crate::{usbserial::*, ctlpins::CTLPins};
use crate::storage::StorageSwitchTrait;

use ushell::{
    autocomplete::StaticAutocomplete, history::LRUHistory, Input as ushell_input,
    ShellError as ushell_error, UShell,
};
const N_COMMANDS: usize = 4;
const COMMANDS: [&str; N_COMMANDS] = ["get-config", "set", "set-config", "monitor"];
pub type ShellType<D> = UShell<USBSerialType<D>, StaticAutocomplete<N_COMMANDS>, LRUHistory<512, 10>, 512>;

pub const SHELL_PROMPT: &str = "#> ";
pub const CR: &str = "\r\n";

pub const HELP: &str = "\r\n\
        set r|a|b|c|d l|h|z : set RESET, CTL_A,B,C or D to low, high or high impedance\r\n\
        set-config name|tags|json|usb_console|poweron|poweroff value : set the config value in flash\r\n\
        get-config          : print all the config parameters\r\n\
        ";

pub fn new<E: EndpointDirection>(serial:USBSerialType<E>) -> ShellType<E> {
    let autocomplete = StaticAutocomplete(COMMANDS);
    let history = LRUHistory::default();
    let shell: ShellType<E> = UShell::new(serial, autocomplete, history);
    shell
}

pub fn handle_shell_commands<L, S, P, E>(shell: &mut ShellType<E>,
                                      led_cmd: &mut L,
                                      storage: &mut S,
                                      ctl_pins:&mut CTLPins<P>,
                                      power_meter: &mut dyn PowerMeter,
                                      config: &mut ConfigArea)
where
    L: OutputPin,
    S: StorageSwitchTrait,
    P: OutputPin,
    E: EndpointDirection
{
    loop {
        let mut response = ArrayString::<512>::new();
        write!(response, "{0:}", CR).ok();

        let result = shell.poll();

        match result {
            Ok(Some(ushell_input::Command((cmd, args)))) => {
                led_cmd.set_low().ok();
                match cmd {
                        "set" =>        { handle_set_cmd(&mut response, args, ctl_pins); }
                        "set-config" => { handle_set_config_cmd(&mut response, args, config); }
                        "get-config" => { handle_get_config_cmd(&mut response, args, config); }
                        _ =>            { write!(shell, "{0:}unsupported command{0:}", CR).ok(); }
                }
                // If response was added complete with an additional CR
                if response.len() > 2 {
                    write!(response, "{0:}", CR).ok();
                }
                write!(response, "{}", SHELL_PROMPT).ok();
                shell.write_str(&response).ok();

            }
            Err(ushell_error::WouldBlock) => break,
            _ => {}
        }
    }
}

fn handle_set_cmd<B, C>(response:&mut B, args: &str, ctl_pins:&mut C)
where
    B: Write,
    C: CTLPinsTrait

 {

    if args.len() == 3 && args.as_bytes()[1] == ' ' as u8{
        let mut chars = args.chars();
        let ctl     = chars.next().unwrap();
        let _space  = chars.next().unwrap();
        let val     = chars.next().unwrap();

        if ctl != 'r' && ctl != 'a' && ctl != 'b' && ctl != 'c' && ctl != 'd' {
            write_set_usage(response);
            return;
        }

        if val != 'l' && val != 'h' && val != 'z' {
            write_set_usage(response);
            return;
        }

        let ctl_str = match ctl {
            'r' => "/RESET",
            'a' => "CTL_A",
            'b' => "CTL_B",
            'c' => "CTL_C",
            'd' => "CTL_D",
            _ => "",
        };

        let val_str = match val {
            'l' => "LOW",
            'h' => "HIGH",
            'z' => "HIGH IMPEDANCE",
            _ => "",
        };

        let ps = match val {
            'l' => PinState::Low,
            'h' => PinState::High,
            'z' => PinState::Floating,
            _ => PinState::Floating,
        };

        match ctl {
            'r' => ctl_pins.set_reset(ps),
            'a' => ctl_pins.set_ctl_a(ps),
            'b' => ctl_pins.set_ctl_b(ps),
            'c' => ctl_pins.set_ctl_c(ps),
            'd' => ctl_pins.set_ctl_d(ps),
            _ => {},
        };

        write!(response, "Set {} to {}", ctl_str, val_str).ok();
    } else {
        write_set_usage(response)
    }
}

fn handle_set_config_cmd<B>(response:&mut B, args: &str, config: &mut ConfigArea)
where
    B: Write
 {
    let mut split_args = args.split_ascii_whitespace();
    let key = split_args.next();
    let mut val = split_args.next();
    let mut usage = false;

    // empty argument = clear
    if val == None {
        val = Some("");
    }

    if let (Some(k), Some(v)) = (key, val) {
        let cfg = config.get();
        if k == "name" {
            let cfg = cfg.set_name(v.as_bytes());
            config.write_config(&cfg).ok();
            write!(response, "Set name to {}", v).ok();

        } else if k == "tags" {
            let cfg = cfg.set_tags(v.as_bytes());
            write!(response, "Set tags to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "json" {
            let cfg = cfg.set_json(v.as_bytes());
            write!(response, "Set json to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "usb_console" {
            let cfg = cfg.set_usb_console(v.as_bytes());
            write!(response, "Set usb_console to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "power_on" {
            let cfg = cfg.set_power_on(v.as_bytes());
            write!(response, "Set power_on to {}", v).ok();
            config.write_config(&cfg).ok();

        } else if k == "power_off" {
            let cfg = cfg.set_power_off(v.as_bytes());
            write!(response, "Set power_off to {}", v).ok();
            config.write_config(&cfg).ok();
        } else if k == "power_rescue" {
            let cfg = cfg.set_power_rescue(v.as_bytes());
            write!(response, "Set power_rescue to {}", v).ok();
            config.write_config(&cfg).ok();
        } else {
            usage = true;
        }
    } else {
        usage = true;
    }

    if usage {
        write!(response, "usage: set-config name|tags|storage|usb_storage value").ok();
    }
}

fn handle_get_config_cmd<B>(response:&mut B, args: &str, config: &mut ConfigArea)
where
    B: Write
 {
    let cfg = config.get();

    if args == "name" {
        write_u8(response, &cfg.name);
    } else if args == "tags" {
        write_u8(response, &cfg.tags);
    } else if args == "json" {
        write_u8(response, &cfg.json);
    } else if args == "usb_console" {
        write_u8(response, &cfg.usb_console);
    } else if args == "power_on" {
        write_u8(response, &cfg.power_on);
    } else if args == "power_off" {
        write_u8(response, &cfg.power_off);
    } else if args == "power_rescue" {
        write_u8(response, &cfg.power_rescue);
    } else if args == "" {
        write!(response, "name: ").ok();
        write_u8(response, &cfg.name);
        write!(response, "\r\ntags: ").ok();
        write_u8(response, &cfg.tags);
        write!(response, "\r\njson: ").ok();
        write_u8(response, &cfg.json);
        write!(response, "\r\nusb_console: ").ok();
        write_u8(response, &cfg.usb_console);
        write!(response, "\r\npower_on: ").ok();
        write_u8(response, &cfg.power_on);
        write!(response, "\r\npower_off: ").ok();
        write_u8(response, &cfg.power_off);
        write!(response, "\r\npower_rescue: ").ok();
        write_u8(response, &cfg.power_rescue);
    } else {
        write!(response, "usage: get-config [name|tags|json|usb_console|power_on|power_off|power_rescue]").ok();
    }
}

fn write_u8<B>(response:&mut B, val:&[u8])
where
    B: Write
{
    for c in val.iter() {
        if *c == 0 {
            break;
        }
        response.write_char(*c as char).ok();
    }
}

fn write_set_usage<B>(response:&mut B)
where
    B: Write
 {
    write!(response, "usage: set r|a|b|c|d l|h|z").ok();
}
