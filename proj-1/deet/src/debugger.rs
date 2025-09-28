use crate::debugger_command::DebuggerCommand;
use crate::inferior::Inferior;
use rustyline::error::ReadlineError;
use rustyline::Editor;
use crate::dwarf_data::{DwarfData, Error as DwarfError};

pub struct Debugger {
    target: String,
    history_path: String,
    readline: Editor<()>,
    inferior: Option<Inferior>,
    debug_data: DwarfData,
    break_points: Vec<Breakpoint>,
}

#[derive(Clone)]
pub struct Breakpoint {
    pub addr: usize,
    pub orig_byte: u8,
}

impl Debugger {
    /// Initializes the debugger.
    pub fn new(target: &str) -> Debugger {
        // Initialize the DwarfData
        let debug_data = match DwarfData::from_file(target) {
            Ok(val) => val,
            Err(DwarfError::ErrorOpeningFile) => {
                println!("Could not open file {}", target);
                std::process::exit(1);
            }
            Err(DwarfError::DwarfFormatError(err)) => {
                println!("Could not debugging symbols from {}: {:?}", target, err);
                std::process::exit(1);
            }
        };
        debug_data.print();

        let history_path = format!("{}/.deet_history", std::env::var("HOME").unwrap());
        let mut readline = Editor::<()>::new();
        // Attempt to load history from ~/.deet_history if it exists
        let _ = readline.load_history(&history_path);

        Debugger {
            target: target.to_string(),
            history_path,
            readline,
            inferior: None,
            debug_data,
            break_points: Vec::new(),
        }
    }

    fn parse_address(addr: &str) -> Option<usize> {
        let addr_without_0x = if addr.to_lowercase().starts_with("0x") {
            &addr[2..]
        } else {
            &addr
        };
        usize::from_str_radix(addr_without_0x, 16).ok()
    }

    pub fn run(&mut self) {
        loop {
            match self.get_next_command() {
                DebuggerCommand::Run(args) => {
                    if let Some(inferior) = &mut self.inferior {
                        inferior.kill();
                    }
                    if let Some(inferior) = Inferior::new(&self.target, &args, &self.break_points) {
                        // Create the inferior
                        self.inferior = Some(inferior);
                        // Make the inferior run
                        // You may use self.inferior.as_mut().unwrap() to get a mutable reference
                        // to the Inferior object
                        self.inferior.as_mut().unwrap().continue_proc(&self.debug_data);
                    } else {
                        println!("Error starting subprocess");
                    }
                }
                DebuggerCommand::Continue => {
                    if let Some(inferior) = &mut self.inferior {
                        inferior.continue_proc(&self.debug_data);
                    } else {
                        println!("Error: no inferior process running. Use 'run' to start a process.");
                    }
                }
                DebuggerCommand::Quit => {
                    if let Some(inferior) = &mut self.inferior {
                        inferior.kill();
                    }
                    return;
                }
                DebuggerCommand::Backtrace => {
                    if let Some(inferior) = &mut self.inferior {
                        let _ = inferior.print_backtrace(&self.debug_data);
                    }
                }
                DebuggerCommand::Break(bp_target) => {
                    let idx;
                    let address;
                    if bp_target.starts_with("*") {
                        address = Debugger::parse_address(&bp_target[1..]);
                    } else if bp_target.parse::<usize>().is_ok() {
                        address = self.debug_data.get_addr_for_line(None, bp_target.parse::<usize>().unwrap());
                    } else {
                        address = self.debug_data.get_addr_for_function(None, &bp_target);
                    }
                    idx = self.break_points.len();
                    self.break_points.push(Breakpoint { 
                        addr: address.unwrap(),
                        orig_byte: 0xcc,
                    });
                    println!("Set breakpoint {} at {:#x}", idx, self.break_points[idx].addr);
                }
                DebuggerCommand::Print => {
                    
                }
            }
        }
    }

    /// This function prompts the user to enter a command, and continues re-prompting until the user
    /// enters a valid command. It uses DebuggerCommand::from_tokens to do the command parsing.
    ///
    /// You don't need to read, understand, or modify this function.
    fn get_next_command(&mut self) -> DebuggerCommand {
        loop {
            // Print prompt and get next line of user input
            match self.readline.readline("(deet) ") {
                Err(ReadlineError::Interrupted) => {
                    // User pressed ctrl+c. We're going to ignore it
                    println!("Type \"quit\" to exit");
                }
                Err(ReadlineError::Eof) => {
                    // User pressed ctrl+d, which is the equivalent of "quit" for our purposes
                    return DebuggerCommand::Quit;
                }
                Err(err) => {
                    panic!("Unexpected I/O error: {:?}", err);
                }
                Ok(line) => {
                    if line.trim().len() == 0 {
                        continue;
                    }
                    self.readline.add_history_entry(line.as_str());
                    if let Err(err) = self.readline.save_history(&self.history_path) {
                        println!(
                            "Warning: failed to save history file at {}: {}",
                            self.history_path, err
                        );
                    }
                    let tokens: Vec<&str> = line.split_whitespace().collect();
                    if let Some(cmd) = DebuggerCommand::from_tokens(&tokens) {
                        return cmd;
                    } else {
                        println!("Unrecognized command.");
                    }
                }
            }
        }
    }
}
