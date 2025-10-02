use nix::sys::ptrace;
use nix::sys::signal;
use nix::sys::wait::{waitpid, WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::collections::HashMap;
use std::mem::size_of;
use std::os::unix::process::CommandExt;
use std::process::Child;
use std::process::Command;

use std::fs::File;
use std::io::{self, BufRead};

use crate::debugger::Breakpoint;
use crate::dwarf_data::DwarfData;

pub enum Status {
    /// Indicates inferior stopped. Contains the signal that stopped the process, as well as the
    /// current instruction pointer that it is stopped at.
    Stopped(signal::Signal, usize),

    /// Indicates inferior exited normally. Contains the exit status code.
    Exited(i32),

    /// Indicates the inferior exited due to a signal. Contains the signal that killed the
    /// process.
    Signaled(signal::Signal),
}

/// This function calls ptrace with PTRACE_TRACEME to enable debugging on a process. You should use
/// pre_exec with Command to call this in the child process.
fn child_traceme() -> Result<(), std::io::Error> {
    ptrace::traceme().or(Err(std::io::Error::new(
        std::io::ErrorKind::Other,
        "ptrace TRACEME failed",
    )))
}

pub struct Inferior {
    child: Child,
    break_points: HashMap<usize, Breakpoint>,
}

fn align_addr_to_word(addr: usize) -> usize {
    addr & (-(size_of::<usize>() as isize) as usize)
}

impl Inferior {
    /// Attempts to start a new inferior process. Returns Some(Inferior) if successful, or None if
    /// an error is encountered.
    pub fn new(
        target: &str,
        args: &Vec<String>,
        break_points: &Vec<Breakpoint>,
    ) -> Option<Inferior> {
        // implement me!
        let mut cmd = Command::new(target);
        cmd.args(args);
        unsafe {
            cmd.pre_exec(child_traceme);
        }
        let child = cmd.spawn().ok()?;
        let mut bp_map = HashMap::new();
        for bp in break_points {
            bp_map.insert(bp.addr, bp.clone());
        }
        let infer = Inferior {
            child,
            break_points: bp_map,
        };
        match infer.wait(None) {
            Ok(Status::Stopped(signal::SIGTRAP, _)) => Some(infer),
            _ => None,
        }
    }

    /// Returns the pid of this inferior.
    pub fn pid(&self) -> Pid {
        nix::unistd::Pid::from_raw(self.child.id() as i32)
    }

    /// Calls waitpid on this inferior and returns a Status to indicate the state of the process
    /// after the waitpid call.
    pub fn wait(&self, options: Option<WaitPidFlag>) -> Result<Status, nix::Error> {
        Ok(match waitpid(self.pid(), options)? {
            WaitStatus::Exited(_pid, exit_code) => Status::Exited(exit_code),
            WaitStatus::Signaled(_pid, signal, _core_dumped) => Status::Signaled(signal),
            WaitStatus::Stopped(_pid, signal) => {
                let regs = ptrace::getregs(self.pid())?;
                Status::Stopped(signal, regs.rip as usize)
            }
            other => panic!("waitpid returned unexpected status: {:?}", other),
        })
    }

    fn set_break_points(&mut self) {
        let addrs: Vec<usize> = self.break_points.keys().copied().collect();
        let mut orig_bytes = Vec::new();
        for addr in &addrs {
            let orig_byte = self.write_byte(*addr, 0xcc).unwrap();
            orig_bytes.push(orig_byte);
        }
        for (addr, &byte) in addrs.iter().zip(orig_bytes.iter()) {
            if let Some(bp) = self.break_points.get_mut(addr) {
                bp.orig_byte = byte;
            }
        }
    }

    fn check_stop_at_b(&mut self) {
        let regs = ptrace::getregs(self.pid()).unwrap();
        let bp_addr = regs.rip as usize;
        if self.break_points.contains_key(&bp_addr) {
            let _ = ptrace::step(self.pid(), None);
            let wait_result = self.wait(None);
            match wait_result {
                Ok(Status::Exited(_)) | Ok(Status::Signaled(_)) => {
                    return;
                }
                Ok(Status::Stopped(signal, rip)) => {
                    let _ = self.write_byte(bp_addr, 0xcc);
                }
                Err(error) => {
                    println!("Error waiting for child: {}", error);
                    return;
                }
            }
        }
    }

    fn set_back_rip(&mut self) {
        let mut regs = ptrace::getregs(self.pid()).unwrap();
        let bp_addr = (regs.rip - 1) as usize;
        if self.break_points.contains_key(&bp_addr) {
            let _ = self.write_byte(bp_addr, self.break_points[&bp_addr].orig_byte);
            regs.rip = regs.rip.wrapping_sub(1);         // %rip = %rip - 1
            let _ = ptrace::setregs(self.pid(), regs); 
        }
    }

    pub fn continue_proc(&mut self, debug_data: &DwarfData) {
        self.set_break_points();
        self.check_stop_at_b();

        let _ = ptrace::cont(self.pid(), None);
        let wait_result = self.wait(None);
        match wait_result {
            Ok(Status::Exited(exit_code)) => {
                println!("Child exited (status {})", exit_code);
                return;
            }
            Ok(Status::Signaled(signal)) => {
                println!("Child terminated (signal {:?})", signal);
            }
            Ok(Status::Stopped(signal, rip)) => {
                println!("Child stopped (signal {:?})", signal);
                let line = debug_data.get_line_from_addr(rip).unwrap();
                println!("Stopped at {}", line);
                if let Ok(file) = File::open(line.file) {
                    let lines: Vec<_> = io::BufReader::new(file).lines().collect();
                    if line.number > 0 && line.number <= lines.len() {
                        if let Ok(src) = &lines[line.number - 1] {
                            println!("Source: {}", src);
                        }
                    }
                }
            }
            Err(error) => {
                println!("Error waiting for child: {}", error);
                return;
            }
        }

        self.set_back_rip();
    }

    pub fn kill(&mut self) {
        println!("Killing running inferior (pid {})", self.pid());
        let _ = Child::kill(&mut self.child);
        let _ = self.child.wait();
    }

    pub fn print_backtrace(&self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        let regs = ptrace::getregs(self.pid()).unwrap();
        let mut rip = regs.rip as usize;
        let mut rbp = regs.rbp as usize;

        while true {
            if let Some(function) = debug_data.get_function_from_addr(rip) {
                print!("{} ", function);
                if function == "main" {
                    if let Some(line) = debug_data.get_line_from_addr(rip) {
                        println!("({})", line);
                    }
                    break;
                }
            }
            if let Some(line) = debug_data.get_line_from_addr(rip) {
                println!("({})", line);
            }
            rip = ptrace::read(self.pid(), (rbp + 8) as ptrace::AddressType)? as usize;
            rbp = ptrace::read(self.pid(), rbp as ptrace::AddressType)? as usize;
        }

        Ok(())
    }

    pub fn step_to_next_line(&mut self, debug_data: &DwarfData) -> Result<(), nix::Error> {
        let regs = ptrace::getregs(self.pid())?;
        let current_rip = regs.rip as usize;
        
        if let Some(current_line) = debug_data.get_line_from_addr(current_rip) {
            let next_line_num = current_line.number + 1;
            if let Some(next_addr) = debug_data.get_addr_for_line(None, next_line_num) {
                let orig_byte = self.write_byte(next_addr, 0xcc)?;
                self.continue_proc(debug_data);
                let _ = self.write_byte(next_addr, orig_byte);
            }
        }

        Ok(())
    }

    fn write_byte(&mut self, addr: usize, val: u8) -> Result<u8, nix::Error> {
        let aligned_addr = align_addr_to_word(addr);
        let byte_offset = addr - aligned_addr;
        let word = ptrace::read(self.pid(), aligned_addr as ptrace::AddressType)? as u64;
        let orig_byte = (word >> 8 * byte_offset) & 0xff;
        let masked_word = word & !(0xff << 8 * byte_offset);
        let updated_word = masked_word | ((val as u64) << 8 * byte_offset);
        ptrace::write(
            self.pid(),
            aligned_addr as ptrace::AddressType,
            updated_word as *mut std::ffi::c_void,
        )?;
        Ok(orig_byte as u8)
    }
}
