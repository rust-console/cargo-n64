use goblin::elf::section_header::SectionHeader;
use goblin::elf::Elf;
use goblin::error::Error as GoblinError;
use std::fs;
use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ElfError {
    #[error("I/O error")]
    Io(#[from] io::Error),

    #[error("ELF parsing error")]
    Goblin(#[from] GoblinError),

    #[error("Dump error: {0}")]
    Dump(String),
}

pub(crate) struct SectionInfo<'a> {
    header: &'a SectionHeader,
    binary: &'a [u8],
}

pub(crate) fn dump(filename: &str) -> Result<(u32, Vec<u8>), ElfError> {
    use self::ElfError::Dump;
    use goblin::elf::section_header;

    // Read the file
    let data = fs::read(filename)?;

    // Parse it
    let elf = Elf::parse(&data)?;

    // Do some basic validation
    validate(&elf)?;

    // Dump .boot section
    let section = dump_section(&elf, &data, ".boot")?;

    // Validate the .boot section
    if (section.header.sh_flags & u64::from(section_header::SHF_EXECINSTR)) == 0 {
        return Err(Dump(format!(
            "Non-executable .boot section: {}",
            section.header.sh_flags
        )));
    }
    if section.header.sh_addr != elf.header.e_entry {
        return Err(Dump(
            "First byte of .boot section must be program entry point".into(),
        ));
    }

    let mut binary = section.binary.to_vec();
    let mut offset = section.header.sh_addr + section.header.sh_size;

    // Copy data sections
    for name in [".text", ".rodata", ".data", ".got"].iter() {
        let section = dump_section(&elf, &data, name);
        if section.is_err() {
            continue;
        }
        let section = section.unwrap();

        // Align the buffer to this section
        let section_offset = section.header.sh_addr;
        if offset < section_offset {
            let length = binary.len() + (section_offset - offset) as usize;
            binary.resize(length, 0);
            offset = section_offset;
        }

        // Append this section to the buffer
        binary.extend_from_slice(section.binary);

        offset += section.header.sh_size;
    }

    Ok((elf.header.e_entry as u32, binary))
}

fn validate(elf: &Elf<'_>) -> Result<(), ElfError> {
    use self::ElfError::Dump;
    use goblin::elf::header;

    if elf.header.e_type != header::ET_EXEC {
        let e = format!("Unexpected ELF type: {}", elf.header.e_type);
        return Err(Dump(e));
    }
    if elf.header.e_machine != header::EM_MIPS {
        let e = format!("Unexpected ELF machine: {}", elf.header.e_machine);
        return Err(Dump(e));
    }
    if elf.header.e_entry > u64::from(u32::max_value()) {
        let e = format!("Entry point out if range: {}", elf.header.e_entry);
        return Err(Dump(e));
    }
    if elf.little_endian {
        return Err(Dump(format!(
            "Unexpected ELF endianness: {}",
            elf.little_endian
        )));
    }
    if elf.section_headers.is_empty() {
        return Err(Dump("Missing ELF section headers".into()));
    }

    Ok(())
}

fn dump_section<'a>(
    elf: &'a Elf<'_>,
    data: &'a [u8],
    name: &str,
) -> Result<SectionInfo<'a>, ElfError> {
    use self::ElfError::Dump;

    // Find the section by name
    let header = elf
        .section_headers
        .iter()
        .find(|&h| elf.shdr_strtab.get_at(h.sh_name).unwrap_or("") == name)
        .ok_or_else(|| Dump(format!("Could not find {} section", name)))?;

    // Get section data
    let start = header.sh_offset as usize;
    let end = start + header.sh_size as usize;
    let binary = data
        .get(start..end)
        .ok_or_else(|| Dump("Index out of range".into()))?;

    Ok(SectionInfo { header, binary })
}
