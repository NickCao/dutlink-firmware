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
const N_COMMANDS: usize = 1;
const COMMANDS: [&str; N_COMMANDS] = ["set"];
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
)
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
            return;
        }

        if val != 'l' && val != 'h' && val != 'z' {
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
    }
}
