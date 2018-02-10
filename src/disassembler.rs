use std::io::{self, BufReader, BufRead, Cursor, Error, Lines};
use std::ffi::OsStr;
use std::process::Command;
use std::str;

use byteorder::{BigEndian, ReadBytesExt};

use dol::Segment;

const DOL_CODE_START: usize = 0x100;

pub struct Disassembler<'a> {
    objdump_path: &'a OsStr,
}

impl<'a> Disassembler<'a> {
    pub fn objdump_path<P>(objdump_path: &'a P) -> io::Result<Disassembler<'a>>
        where P: AsRef<OsStr>
    {
        Disassembler::check_objdump_version(objdump_path.as_ref())?;
        Ok(Disassembler {
            objdump_path: objdump_path.as_ref(),
        })
    }

    pub fn disasm<P>(&self, file_path: P, segment: &Segment) -> io::Result<DisasmIter>
        where P: AsRef<OsStr>
    {
        let output = Command::new(self.objdump_path)
            .args(&["-mpowerpc", "-D", "-b", "binary", "-EB", "-M", "750cl",
                "--start-address", &segment.start.to_string(),
                "--stop-address", &(segment.start + segment.size).to_string(),
                ])
            .arg(file_path.as_ref())
            .output()?;

        if output.status.success() {
            let mut d = DisasmIter {
                lines: BufReader::new(Cursor::new(output.stdout)).lines()
            };
            d.advance_to_start()?;
            Ok(d)
        } else {
            Err(match output.status.code() {
                Some(c) => Error::new(io::ErrorKind::InvalidInput, &format!("The program `objdump` failed with a status of {}.", c)[..]),
                None => Error::new(io::ErrorKind::Interrupted, "The program `objdump` was interupted."),
            })
        }

    }

    pub fn new<P>() -> io::Result<Disassembler<'a>>
        where P: AsRef<OsStr>
    {
        Disassembler::objdump_path(&"objdump")
    }

    fn check_objdump_version<P>(objdump_path: P) -> io::Result<()>
        where P: AsRef<OsStr>
    {
        let expected_str = "GNU objdump";
        let yep = Command::new(objdump_path.as_ref())
            .arg("--version")
            .output().ok().map(|o| {
                let count = expected_str.len();
                o.stdout.len() >= count &&
                    str::from_utf8(&o.stdout[..count]) == Ok(expected_str)
            }) == Some(true);
        if yep {
            Ok(())
        } else {
            Err(Error::new(io::ErrorKind::InvalidInput, "GNU objdump required."))
        }
    }
    
    // pub fn write_with_sections<W>(&mut self, output: &mut W) -> io::Result<()>
        // where W: Write
    // {
        // for (pc, instr) in self.enumerate() {
            // // TODO: add labels for sections and branches
            // output.write(instr.text.as_bytes())?;
        // }
        // Ok(())
    // }
}

#[derive(Debug)]
pub struct Instruction {
    pub text: String,
    pub opcode: u32,
    pub location: Option<u64>,
}

impl Instruction {
    pub fn from_objdump(text: &str) -> Option<Instruction> {
        let mut parts = text.split_whitespace();
        let location = match parts.nth(0) {
            Some("...") => return Some(Instruction { text: "...".to_owned(), opcode: 0, location: None }),
            Some(s) if s.chars().last() == Some(':') =>
                u64::from_str_radix(&s[..(s.len() - 1)], 16).ok(),
            _ => return None,
        };

        // This assumes every opcode is 4 bytes, is that right?
        // Aren't there exceptions?
        let mut bytes = Cursor::new((&mut parts).take(4).map(|s| 
            u8::from_str_radix(s, 16).unwrap()
        ).collect::<Vec<_>>());

        bytes.read_u32::<BigEndian>().ok().map(|opcode| {
            // this filter isn't needed anymore, right?
            let text = parts.collect::<Vec<_>>().join(" ");
            Instruction { text, opcode, location }
        })
    }
}

// TODO: use a generic
pub struct DisasmIter {
    lines: Lines<BufReader<Cursor<Vec<u8>>>>,
}

impl DisasmIter {
    fn advance_to_start(&mut self) -> io::Result<()> {
        loop {
            match self.lines.next() {
                // Some(Ok(ref s)) if s == "00000000 <.data>:" => return Ok(()),
                Some(Ok(ref s)) if s.contains("<.data") => return Ok(()),
                Some(_) => (),
                None => return Err(Error::new(io::ErrorKind::InvalidInput, "Invalid output from `objdump`.")),
            }
        }
    }
}

impl Iterator for DisasmIter {
    type Item = Instruction;

    fn next(&mut self) -> Option<Instruction> {
        self.lines.next().and_then(|line| {
            line.ok().and_then(|line| {
                Instruction::from_objdump(&line)
            })
        })
    }
}

