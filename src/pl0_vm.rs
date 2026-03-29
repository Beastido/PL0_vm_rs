use crate::opcodes::OpCode;
use crate::pl0_vm::Data::{B16, B32, B64};
use std::fmt::Debug;
use std::io::{stderr, stdin, BufRead, Write};
use rust_i18n::t;

fn error(msg: &str) {
    stderr().write(msg.as_bytes()).expect("Could not write to stderr");
    stderr().write("\n".as_bytes()).expect("Could not write to stderr");
}

const ARG_SIZE: usize = 2;
const HEX_ARG_SIZE: usize = ARG_SIZE * 2;

#[derive(Debug)]
struct Procedure {
    // byte position of procedure in program
    start_pos: usize,
    // starts with space for variables
    frame_ptr: usize,
}

// wrapper for differently sized integers
#[derive(Debug, Clone)]
enum Data {
    B16(i16),
    B32(i32),
    B64(i64),
}
impl Data {
    fn i64(&self) -> i64 {
        self.clone().into()
    }
    fn to_bytes(&self) -> Vec<u8> {
        match self {
            B16(x) => x.to_le_bytes().to_vec(),
            B32(x) => x.to_le_bytes().to_vec(),
            B64(x) => x.to_le_bytes().to_vec(),
        }
    }
}
impl Into<i64> for Data {
    fn into(self) -> i64 {
        match self {
            B16(num) => num as i64,
            B32(num) => num as i64,
            B64(num) => num,
        }
    }
}

pub struct PL0VM {
    program: Vec<u8>,
    bits: Data,
    debug: bool,
}

impl PL0VM {
    pub fn new(debug: bool) -> PL0VM {
        PL0VM {
            program: vec![],
            bits: B16(0),
            debug,
        }
    }
    fn data_size(&self) -> usize { match self.bits { B16(_) => 2, B32(_) => 4, B64(_) => 8 } }

    fn data_true(&self) -> Data { match self.bits { B16(_) => B16(1), B32(_) => B32(1), B64(_) => B64(1) } }
    fn data_false(&self) -> Data { match self.bits { B16(_) => B16(0), B32(_) => B32(0), B64(_) => B64(0) } }
    fn data_bool(&self, val: bool) -> Data { match val { true => self.data_true(), false => self.data_false() } }

    pub fn from_file(debug: bool, filename: &str) -> Result<PL0VM, std::io::Error> {
        let mut pl0vm = PL0VM::new(debug);
        match pl0vm.load_from_file(filename) {
            Ok(_) => Ok(pl0vm),
            Err(e) => Err(e),
        }
    }

    pub fn load_from_file(&mut self, filename: &str) -> Result<bool, std::io::Error> {
        match std::fs::read(filename) {
            Ok(bytes) => {
                self.program = bytes;
                self.bits = match self.read_arg(ARG_SIZE) {
                    Some(val) => match val {
                        2 => B16(0),
                        4 => B32(0),
                        8 => B64(0),
                        _ => return Ok(false),
                    },
                    None => return Ok(false),
                };
                Ok(true)
            },
            Err(err) => { Err(err) },
        }
    }

    fn read_arg(&self, offset: usize) -> Option<i16> {
        match self.program.get(offset..(offset + ARG_SIZE)) {
            Some(val) => Some(i16::from_le_bytes(val.try_into().expect("Invalid byte count?!"))),
            None => None
        }
    }
    fn bytes_to_data(&self, bytes: &Option<&[u8]>) -> Option<Data> {
        match bytes {
            Some(bytes) => Some(match self.bits {
                B16(_) => B16(i16::from_le_bytes(bytes[0..2].try_into().expect("Invalid byte count?!"))),
                B32(_) => B32(i32::from_le_bytes(bytes[0..4].try_into().expect("Invalid byte count?!"))),
                B64(_) => B64(i64::from_le_bytes(bytes[0..8].try_into().expect("Invalid byte count?!"))),
            }),
            None => None,
        }
    }
    fn read_data(&self, offset: usize) -> Option<Data> {
        self.bytes_to_data(&self.program.get(offset..))
    }

    pub fn print_analysis(&self) {
        if self.program.len() <= 4 || self.program[3] > 0 {
            error(&t!("pl0.invalid_file"));
            return;
        }

        let mut pc = 4;
        let mut procedure_count = match self.read_arg(0) {
            Some(val) => val,
            None => return error("unreachable code"),
        };
        print!("0000: {}: {:04X} = {}, ", t!("pl0.procedure_count"), procedure_count, procedure_count);
        let arch = match self.read_arg(ARG_SIZE) {
            Some(val) => val,
            None => return error("unreachable code"),
        };
        print!("{}: {:04X} = ", t!("pl0.arch"), arch);
        match arch {
            2 => println!("16 bit"),
            4 => println!("32 bit"),
            8 => println!("64 bit"),
            _ => println!("{}", t!("pl0.invalid")),
        }
        if arch != 2 && arch != 4 && arch != 8 {
            error(&t!("pl0.arch_invalid", arch = arch:{:04X}));
            return;
        }

        let print_arg = |pc: &mut usize, last: bool| {
            let val = match self.read_arg(*pc) {
                Some(val) => val,
                None => return false,
            };
            print!("{:0HEX_ARG_SIZE$X}{}", val, if last { "" } else { ", " });
            *pc += ARG_SIZE;
            return true;
        };

        let mut rem_bytes = 0;
        loop {
            let byte = match self.program.get(pc) {
                Some(val) => val,
                None => return error(&t!("pl0.error.invalid_pc", pc = pc:{:04X})),
            };
            let opc = pc;
            let op = match OpCode::try_from(*byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&t!("pl0.unknown_opcode", op = byte:{:02X}));
                    break;
                },
            };
            print!("{:04X}: {:02X} {:<21} ", pc, byte, op);
            pc += 1;
            match op {
                OpCode::PushValueLocalVar | OpCode::PushValueMainVar
                    | OpCode::PushAddressLocalVar | OpCode::PushAddressMainVar
                    | OpCode::CallProc | OpCode::PushConstant => {
                    print_arg(&mut pc, true);
                },
                OpCode::Jump | OpCode::JumpIfFalse => {
                    let arg = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let target = match (pc + ARG_SIZE).checked_add_signed(arg as isize) {
                        Some(target) => target,
                        None => {
                            error(&t!("pl0.invalid_jump", pc = pc, arg = arg));
                            break;
                        },
                    };
                    print!("{}{:0HEX_ARG_SIZE$X} => {:0HEX_ARG_SIZE$X}", if arg < 0 { "-" } else { "" }, arg.abs(), target);
                    pc += ARG_SIZE;
                },
                OpCode::PushValueGlobalVar | OpCode::PushAddressGlobalVar => {
                    print_arg(&mut pc, false);
                    print_arg(&mut pc, true);
                },
                OpCode::EntryProc => {
                    rem_bytes = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    print!("{:0HEX_ARG_SIZE$X}, ", rem_bytes);
                    pc += ARG_SIZE;
                    let pid = match self.read_arg(pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    print!("{:0HEX_ARG_SIZE$X}, ", pid);
                    pc += ARG_SIZE;
                    print_arg(&mut pc, true);
                    print!(" <<< {}", if pid == 0 { t!("pl0.proc_start_main") } else { t!("pl0.proc_start") });
                    procedure_count -= 1;
                }
                OpCode::PutString => {
                    let strb: Vec<_> = self.program.iter().skip(pc).take_while(|&&b| b != 0).map(|b| *b).collect();
                    pc += strb.len() + 1;
                    let str = match String::from_utf8(strb) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&t!("pl0.invalid_str", err = err));
                            break;
                        }
                    };
                    print!("\"{str}\"");
                }
                _ => {},
            }
            rem_bytes -= (pc - opc) as i16;

            println!();

            if rem_bytes <= 0 && procedure_count == 0 { break; }
        }
        (0..((self.program.len() - pc) / self.data_size())).map(|i| self.read_data(pc + self.data_size() * i)).enumerate().for_each(|(i, constant)| {
            let ds2 = self.data_size() * 2;
            let c = match constant {
                Some(val) => val,
                None => return error(&t!("pl0.error.invalid_constant_read", i = i)),
            }.i64();
            let cstr = format!("{:0ds2$X}", c);
            println!("{} {:04}: 0x{} = {}", t!("pl0.constant"), i, &cstr[cstr.len() - ds2..], c);
        });
    }

    fn load_data(&self) -> Option<(Vec<Procedure>, Vec<Data>)> {
        let mut procedure_count = self.read_arg(0).expect("failed to read procedure count - should be unreachable");
        let mut procedures = Vec::with_capacity(procedure_count as usize);
        procedures.resize_with(procedures.capacity(), || None);
        let mut pc = 4;

        let mut rem_bytes = 0;
        loop {
            let byte = match self.program.get(pc) {
                Some(val) => *val,
                None => { error(&t!("pl0.error.preload_error")); return None },
            };
            let opc = pc;
            pc += 1;
            if rem_bytes == 0 && byte == <OpCode as Into<u8>>::into(OpCode::EntryProc) {
                rem_bytes = match self.read_arg(pc) {
                    Some(val) => val,
                    None => { error(&t!("pl0.error.preload_error")); return None },
                };
                pc += ARG_SIZE;
                let proc_id = match self.read_arg(pc) {
                    Some(val) => val,
                    None => { error(&t!("pl0.error.preload_error")); return None },
                } as usize;
                pc += ARG_SIZE * 2;
                if proc_id >= procedures.len() {
                    error(&t!("pl0.error.invalid_preload_procedure"));
                    return None;
                }
                procedures[proc_id] = Some(Procedure {
                    start_pos: pc - 1 - ARG_SIZE * 3,
                    frame_ptr: 0,
                });
                procedure_count -= 1;
            }
            rem_bytes -= (pc - opc) as i16;

            if rem_bytes <= 0 && procedure_count == 0 { break; }
        }
        Some((
            procedures.into_iter().map(|procedure| procedure.unwrap()).collect(),
            (0..((self.program.len() - pc) / self.data_size())).map(|i| self.read_data(pc + self.data_size() * i).expect(&t!("pl0.error.invalid_constant_read", i = i))).collect(),
        ))
    }

    //noinspection RsConstantConditionIf
    pub fn execute(&self) {
        if self.program.len() <= 4 || self.program[3] > 0 {
            error(&t!("pl0.invalid_file"));
            return;
        }

        // --- architecture check ---
        let arch_bytes = match self.read_arg(ARG_SIZE) {
            Some(val) => val,
            None => return error(&t!("pl0.error.failed_arch_read")),
        };
        if self.debug {
            let invalid = t!("pl0.invalid");
            println!("\t@0000: {:<21}{arch_bytes:04X} = {}", t!("pl0.set_arch"), match arch_bytes {
                2 => "16 bit",
                4 => "32 bit",
                8 => "64 bit",
                _ => &invalid,
            });
        }
        if arch_bytes != 2 && arch_bytes != 4 && arch_bytes != 8 {
            error(&t!("pl0.arch_invalid", arch = arch_bytes:{:04X}));
            return;
        }

        let (mut procedures, constants) = match self.load_data() {
            Some(val) => val,
            None => return,
        };

        // --- execution state ---
        // program counter = index of currently executed byte
        let mut pc = procedures[0].start_pos;
        // stack = contains all dynamic runtime data
        let mut stack: Vec<u8> = vec![];
        // frame pointer = index of start of current stack frame in vector stack
        let mut fp = 0usize;
        // current procedure index = index of current procedure in vector procedures
        let mut cur_proc_i = 0usize;

        // --- collection of functions used for execution ---
        // pop one Data from the stack
        let pop_data = |stack: &mut Vec<u8>| -> Option<Data> {
            let size = self.data_size();
            let len = stack.len();

            if len < size {
                return None;
            }

            let start = len - size;
            
            let data_bytes = &stack[start..];
            let data = self.bytes_to_data(&Some(data_bytes));

            stack.truncate(start);

            data
        };
        // push a Data onto the stack
        let push_data = |stack: &mut Vec<u8>, data: Data| {
            stack.append(&mut data.to_bytes());
        };
        // pop one argument from the bytecode, by increasing the program counter by ARG_SIZE
        let pop_argument = |pc: &mut usize| -> Option<i16> {
            *pc += ARG_SIZE;
            self.read_arg(*pc - ARG_SIZE)
        };
        // set the bytes at the specified position (fp) in the stack to the value in data
        let set_addr = |stack: &mut Vec<u8>, fp: &usize, data: &Data| {
            if stack.len() < (fp + self.data_size()) { stack.resize(fp + self.data_size(), 0); }
            let bytes = match data {
                B16(v) => v.to_le_bytes().to_vec(), B32(v) => v.to_le_bytes().to_vec(), B64(v) => v.to_le_bytes().to_vec(),
            };
            stack.splice(fp..&(fp + self.data_size()), bytes);
        };
        // calculate the address start + offset, with respect to types
        let offsetted = |start: &usize, offset: isize| start.checked_add_signed(offset).expect("invalid variable offset");

        // --- main execution loop ---
        loop {
            let byte = self.program[pc];

            // try to get op code from current byte
            let op = match OpCode::try_from(byte) {
                Ok(op) => op,
                Err(_) => {
                    error(&t!("pl0.unknown_opcode", op = byte:{:02X}));
                    break;
                },
            };
            if self.debug { print!("\t@{pc:04X}: {:<21}", op); }
            // increase program counter already, so that next pop_argument call returns valid data
            pc += 1;
            match op {
                OpCode::EntryProc => {
                    pc += ARG_SIZE;
                    let proc_i = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if proc_i < 0 {
                        error(&t!("pl0.enter_invalid_proc", id = proc_i));
                        return;
                    }
                    let varlen = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    fp = procedures[proc_i as usize].frame_ptr;
                    stack.resize(fp + varlen, 0);
                    if self.debug { print!("{}", t!("pl0.reserved_varspace", bytes = varlen)); }
                }
                OpCode::ReturnProc => {
                    if cur_proc_i == 0 {
                        if self.debug { println!("{}", t!("pl0.exiting")); }
                        break;
                    } else {
                        stack.truncate(procedures[cur_proc_i].frame_ptr);
                        let new_proc_i = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        let new_fp = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        let new_pc = u64::from_le_bytes(stack.drain(stack.len() - 8..).collect::<Vec<u8>>().try_into().expect("jumping back failed - stack invalid"));
                        if self.debug { print!("pc: {pc} => {new_pc}, fp: {fp} => {new_fp}, cpi: {cur_proc_i} => {new_proc_i}"); }
                        pc = new_pc as usize;
                        fp = new_fp as usize;
                        cur_proc_i = new_proc_i as usize;
                    }
                }
                OpCode::CallProc => {
                    let proc_id = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if proc_id < 0 {
                        error(&t!("pl0.call_invalid_proc", id = proc_id));
                        return;
                    }
                    stack.extend((pc as u64).to_le_bytes());
                    stack.extend((fp as u64).to_le_bytes());
                    stack.extend((cur_proc_i as u64).to_le_bytes());
                    let proc = &mut procedures[proc_id as usize];
                    if self.debug { print!("pc: {pc} => {}, fp: {fp} => {}, cpi: {cur_proc_i} => {}", proc.start_pos, stack.len(), proc_id); }
                    cur_proc_i = proc_id as usize;
                    pc = proc.start_pos;
                    proc.frame_ptr = stack.len();
                }

                OpCode::PushValueLocalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_local_var_val", addr = addr));
                        return;
                    }
                    let data = match self.bytes_to_data(&stack.get(offsetted(&fp, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&fp, addr as isize))); }
                    push_data(&mut stack, data);
                }
                OpCode::PushValueMainVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_main_var_val", addr = addr));
                        return;
                    }
                    let data = match self.bytes_to_data(&stack.get(offsetted(&procedures[0].frame_ptr, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&procedures[0].frame_ptr, addr as isize))); }
                    push_data(&mut stack, data);
                }
                OpCode::PushValueGlobalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let proc_index = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    if addr < 0 {
                        error(&t!("pl0.invalid_global_var_val", addr = addr, proc_index = proc_index));
                        return;
                    }
                    let data = match self.bytes_to_data(&stack.get(offsetted(&procedures[proc_index].frame_ptr, addr as isize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { print!("{}", t!("pl0.took_from_addr", val = data.i64(), addr = offsetted(&procedures[proc_index].frame_ptr, addr as isize))); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressLocalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_local_var_addr", addr = addr));
                        return;
                    }
                    let data = self.bytes_to_data(&Some(&offsetted(&fp, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    if self.debug { print!("{}", t!("pl0.pushed_addr", addr = offsetted(&fp, addr as isize))); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressMainVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if addr < 0 {
                        error(&t!("pl0.invalid_main_var_addr", addr = addr));
                        return;
                    }
                    let data = self.bytes_to_data(&Some(&offsetted(&procedures[0].frame_ptr, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    if self.debug { print!("{}", t!("pl0.pushed_addr", addr = offsetted(&procedures[0].frame_ptr, addr as isize))); }
                    push_data(&mut stack, data);
                }
                OpCode::PushAddressGlobalVar => {
                    let addr = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    let proc_index = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    } as usize;
                    if addr < 0 {
                        error(&t!("pl0.invalid_global_var_addr", addr = addr, proc_index = proc_index));
                        return;
                    }
                    if self.debug {
                        print!("{}", t!("pl0.pushed_global_addr", proc_index = proc_index, addr = addr, push_addr = offsetted(&procedures[proc_index].frame_ptr, addr as isize)));
                    }
                    let data = self.bytes_to_data(&Some(&offsetted(&procedures[proc_index].frame_ptr, addr as isize).to_le_bytes())).expect("failed to convert offset to Data");
                    push_data(&mut stack, data);
                }
                OpCode::PushConstant => {
                    let c = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if c < 0 {
                        error(&t!("pl0.invalid_constant", c = c));
                        return;
                    }
                    let cd = constants[c as usize].clone();
                    if self.debug { print!("{}", t!("pl0.pushed_constant", c = c, val = cd.i64())); }
                    push_data(&mut stack, cd);
                }
                OpCode::StoreValue => {
                    let data = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let addr = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    if self.debug { print!("{}", t!("pl0.stored_value", val = data.i64(), addr = addr)) }
                    set_addr(&mut stack, &(addr as usize), &data);
                }

                OpCode::OutputValue => {
                    let data = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug {
                        print!("{}\n{}", data.i64(), data.i64());
                    } else {
                        println!("{}", data.i64());
                    }
                }
                OpCode::InputToAddr => {
                    let addr = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { println!("{}", t!("pl0.to_address", addr = addr.i64())); }
                    // wait for user to input a valid number
                    'input_loop: loop {
                        let mut line = String::new();
                        stdin().lock().read_line(&mut line).expect("Input failed");
                        let input: Result<i64, _> = line.trim().parse();
                        match input {
                            Ok(num) => {
                                set_addr(&mut stack, &offsetted(&fp, addr.i64() as isize), &self.bytes_to_data(&Some(&num.to_le_bytes())).expect("failed to convert number to Data - unreachable error"));
                                break 'input_loop;
                            },
                            Err(_) => {
                                error(&t!("pl0.invalid_number_input"));
                            }
                        }
                    }
                }

                OpCode::Minusify => {
                    let int = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let data = match int {
                        B16(x) => B16(-x), B32(x) => B32(-x), B64(x) => B64(-x),
                    };
                    if self.debug { print!("{} => {}", int.i64(), data.i64()); }
                    push_data(&mut stack, data);
                }
                OpCode::IsOdd => {
                    let int = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = int % 2 == 1;
                    if self.debug { print!("{} => {}", int, val); }
                    push_data(&mut stack, self.data_bool(val));
                }

                OpCode::OpAdd => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left + right;
                    if self.debug { print!("{left} + {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpSubtract => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left - right;
                    if self.debug { print!("{left} - {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpMultiply => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left * right;
                    if self.debug { print!("{left} * {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }
                OpCode::OpDivide => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left / right;
                    if self.debug { print!("{left} / {right} = {val}") }
                    push_data(&mut stack, match self.bits {
                        B16(_) => B16(val as i16), B32(_) => B32(val as i32), B64(_) => B64(val),
                    });
                }

                OpCode::CompareEq => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left == right;
                    if self.debug { print!("{left} == {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareNotEq => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left != right;
                    if self.debug { print!("{left} != {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareLT => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left < right;
                    if self.debug { print!("{left} < {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareGT => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left > right;
                    if self.debug { print!("{left} > {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareLTEq => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left <= right;
                    if self.debug { print!("{left} <= {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }
                OpCode::CompareGTEq => {
                    let right = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let left = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let val = left >= right;
                    if self.debug { print!("{left} >= {right} = {val}") }
                    push_data(&mut stack, self.data_bool(val));
                }

                OpCode::Jump => {
                    let offset = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    pc = offsetted(&pc, offset as isize);
                    if self.debug { print!("{}", t!("pl0.jumping_to", pc = pc:{:04X})); }
                }
                OpCode::JumpIfFalse => {
                    let dat = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let offset = match pop_argument(&mut pc) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_arg_read", addr = pc:{:04X})),
                    };
                    if self.debug { print!("{}", t!("pl0.jumping_if_bool", bool = dat == 0)); }
                    if dat == 0 {
                        pc = offsetted(&pc, offset as isize);
                        if self.debug { print!("{}", t!("pl0.jumping_if_where", pc = pc:{:04X})); }
                    }
                }

                OpCode::PutString => {
                    let bytes: Vec<u8> = self.program[pc..].iter().take_while(|&&b| b != 0).map(|&b| b).collect();
                    pc += bytes.len() + 1;
                    let str = match String::from_utf8(bytes) {
                        Ok(str) => str,
                        Err(err) => {
                            error(&format!("\n{}", t!("pl0.invalid_str", err = err)));
                            break;
                        }
                    };
                    if self.debug {
                        print!("\"{str}\"\n{str}");
                    } else {
                        println!("{str}");
                    }
                }

                OpCode::Pop => {
                    if self.debug {
                        println!("{}", t!("pl0.popped", data = match pop_data(&mut stack) {
                            Some(val) => val,
                            None => return error(&t!("pl0.error.invalid_stack_read")),
                        }.i64()));
                    } else {
                        pop_data(&mut stack);
                    }
                }
                OpCode::Swap => {
                    let offset = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let data = match self.bytes_to_data(&stack.get((offset as usize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { print!("{}", t!("pl0.swapped", addr = offset as usize, val = data.i64())) }
                    push_data(&mut stack, data);
                }

                OpCode::EndOfCode => {
                    if self.debug { println!(); }
                    break;
                }

                /*
                ---- Store variable value at dynamic calculated address ----
                Stack before (bottom -> top): ... | AddressOffset | Value
                Stack after: ... Stack[abs_addr] = Value
            
                ---- Sequence of operations: ----
                1. Pop Value from stack
                2. Pop AddressOffset from stack
                3. Calculate AbsoluteAddress = fp + AddressOffset
                4. Write Value to stack at AbsoluteAddress
                */
                OpCode::Put => {
                    let data = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    let addr = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let abs_addr = offsetted(&fp, addr as isize);
                    if self.debug { println!("Put: addr (rel) = {addr}, abs_addr = {abs_addr}, value = {:?}", data); }
                    set_addr(&mut stack, &abs_addr, &data);
                }

                /*
                ---- Get variable value from dynamic calculated address ----
                Stack before (bottom -> top): ... | addressOffset
                Stack after: ... | Value
            
                ---- Sequence of operations: ----
                0. ADD-Instruction:  addressOffset = baseAddress + indexOffset
                1. Pop addressOffset from stack
                2. Read Value at stack[addressOffset]
                3. push value onto the stack
                */
                OpCode::Get => {
                    let addr = match pop_data(&mut stack) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    }.i64();
                    let data = match self.bytes_to_data(&stack.get((addr as usize)..)) {
                        Some(val) => val,
                        None => return error(&t!("pl0.error.invalid_stack_read")),
                    };
                    if self.debug { println!("Get: addr = {addr}, value = {:?}", data); }
                    push_data(&mut stack, data);
                }

                OpCode::OpAddAddr => { todo!() }
            }

            match op {
                OpCode::InputToAddr => (),
                _ => if self.debug { println!(); }
            };
        }
    }
}
