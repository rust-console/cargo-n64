use byteorder::{BigEndian, ByteOrder};
use itertools::Itertools;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::num::Wrapping;
use std::path::Path;

use crc32fast::Hasher;
use failure::Fail;

crate const IPL_SIZE: usize = 0x0fc0;
crate const PROGRAM_SIZE: usize = 1024 * 1024;

#[derive(Debug, Fail)]
pub enum IPL3Error {
    #[fail(display = "IO Error")]
    IOError(#[cause] io::Error),

    #[fail(display = "Unable to read IPL3: {}", _0)]
    IPL3ReadError(String),
}

impl From<io::Error> for IPL3Error {
    fn from(e: io::Error) -> Self {
        IPL3Error::IOError(e)
    }
}

/// IPL3 definitions.
crate enum IPL3 {
    IPL3_6101([u8; IPL_SIZE]),
    IPL3_6102([u8; IPL_SIZE]),
    IPL3_6103([u8; IPL_SIZE]),
    IPL3_6105([u8; IPL_SIZE]),
    IPL3_6106([u8; IPL_SIZE]),
    IPL3_7102([u8; IPL_SIZE]),
    UNKNOWN([u8; IPL_SIZE]),
}

impl fmt::Display for IPL3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            IPL3::IPL3_6101(_) => "NUS-CIC-6101",
            IPL3::IPL3_6102(_) => "NUS-CIC-6102",
            IPL3::IPL3_6103(_) => "NUS-CIC-6103",
            IPL3::IPL3_6105(_) => "NUS-CIC-6105",
            IPL3::IPL3_6106(_) => "NUS-CIC-6106",
            IPL3::IPL3_7102(_) => "NUS-CIC-7102",
            IPL3::UNKNOWN(_) => "Unknown",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Debug for IPL3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        <IPL3 as fmt::Display>::fmt(self, f)
    }
}

impl IPL3 {
    crate fn read(path: impl AsRef<Path>) -> Result<IPL3, IPL3Error> {
        // TODO
        let mut f = File::open(path)?;

        // Check the file size
        let metadata = f.metadata()?;
        let len = metadata.len();
        if len as usize != IPL_SIZE {
            Err(IPL3Error::IPL3ReadError(format!("Expected file size {}, found {}", IPL_SIZE, len)))?;
        }

        // Read file contents
        let mut ipl = [0; IPL_SIZE];
        f.read_exact(&mut ipl)?;

        // Check for known IPLs
        let mut hasher = Hasher::new();
        hasher.update(&ipl);
        let ipl3 = match hasher.finalize() {
            0x6170a4a1 => IPL3::IPL3_6101(ipl),
            0x90bb6cb5 => IPL3::IPL3_6102(ipl),
            0x0b050ee0 => IPL3::IPL3_6103(ipl),
            0x98bc2c86 => IPL3::IPL3_6105(ipl),
            0xacc8580a => IPL3::IPL3_6106(ipl),
            0x009e9ea3 => IPL3::IPL3_7102(ipl),
            _ => IPL3::UNKNOWN(ipl),
        };

        Ok(ipl3)
    }

    crate fn get_ipl(&self) -> &[u8; IPL_SIZE] {
        match self {
            IPL3::IPL3_6101(bin) => bin,
            IPL3::IPL3_6102(bin) => bin,
            IPL3::IPL3_6103(bin) => bin,
            IPL3::IPL3_6105(bin) => bin,
            IPL3::IPL3_6106(bin) => bin,
            IPL3::IPL3_7102(bin) => bin,
            IPL3::UNKNOWN(bin) => bin,
        }
    }

    crate fn compute_crcs(&self, program: &[u8], fs: &[u8]) -> (u32, u32) {
        let padding_length = (2 - (program.len() & 1)) & 1;
        let padding = [0; 1];
        let program = program
            .iter()
            .chain(&padding[0..padding_length])
            .chain(fs.iter())
            .chain(std::iter::repeat(&0))
            .take(PROGRAM_SIZE)
            .cloned()
            .chunks(4);

        // Initial checksum value
        let checksum = match self {
            IPL3::IPL3_6103(_) => 0xa3886759,
            IPL3::IPL3_6105(_) => 0xdf26f436,
            IPL3::IPL3_6106(_) => 0x1fea617a,
            _ => 0xf8ca4ddc,
        };

        // NUS-IPL3-6105 has a special 64-word table hidden in the IPL
        let mut ipl = self.get_ipl().chunks(4).skip(452).take(64).cycle();

        // Six accumulators
        let mut acc1 = Wrapping(checksum);
        let mut acc2 = Wrapping(checksum);
        let mut acc3 = Wrapping(checksum);
        let mut acc4 = Wrapping(checksum);
        let mut acc5 = Wrapping(checksum);
        let mut acc6 = Wrapping(checksum);

        // Some temporary state
        let mut current;
        let mut rotated;

        // Iterate 1-word at a time
        for chunk in &program {
            // Fetch the current word and rotate it by itself
            current = Wrapping(BigEndian::read_u32(&chunk.collect::<Vec<_>>()));
            rotated = current.rotate_left((current & Wrapping(0x1f)).0);

            // Advance accumulator 1
            acc1 += current;

            // Advance accumulator 2
            if acc1 < current {
                acc2 += Wrapping(1);
            }

            // Advance accumulator 3
            acc3 ^= current;

            // Advance accumulator 4
            acc4 += rotated;

            // Advance accumulator 5
            if acc5 > current {
                acc5 ^= rotated;
            } else {
                acc5 ^= acc1 ^ current;
            }

            // Advance accumulator 6
            match self {
                IPL3::IPL3_6105(_) => {
                    let current_ipl = ipl.next().unwrap();
                    let current_ipl = Wrapping(BigEndian::read_u32(&current_ipl));
                    acc6 += current ^ current_ipl;
                }
                _ => {
                    acc6 += current ^ acc4;
                }
            }
        }

        let (crc1, crc2) = match self {
            IPL3::IPL3_6103(_) => ((acc1 ^ acc2) + acc3, (acc4 ^ acc5) + acc6),
            IPL3::IPL3_6106(_) => (acc1 * acc2 + acc3, acc4 * acc5 + acc6),
            _ => (acc1 ^ acc2 ^ acc3, acc4 ^ acc5 ^ acc6),
        };

        (crc1.0, crc2.0)
    }

    /// Offset the entry point for the current IPL3
    crate fn offset(&self, entry_point: u32) -> u32 {
        entry_point + match self {
            IPL3::IPL3_6103(_) => 0x00100000,
            IPL3::IPL3_6106(_) => 0x00200000,
            _ => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_ipl3_6101() {
        let ipl3 = IPL3::IPL3_6101([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0xfac847da);
        assert_eq!(crc2, 0xb2dea121);
    }

    #[test]
    fn crc_ipl3_6102() {
        let ipl3 = IPL3::IPL3_6102([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0xfac847da);
        assert_eq!(crc2, 0xb2dea121);
    }

    #[test]
    fn crc_ipl3_6103() {
        let ipl3 = IPL3::IPL3_6103([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0xa98e6d67);
        assert_eq!(crc2, 0x3beec487);
    }

    #[test]
    fn crc_ipl3_6105() {
        let ipl3 = IPL3::IPL3_6105([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0xe124ee34);
        assert_eq!(crc2, 0xb59efe63);
    }

    #[test]
    fn crc_ipl3_6106() {
        let ipl3 = IPL3::IPL3_6106([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0x66c670aa);
        assert_eq!(crc2, 0x38749798);
    }

    #[test]
    fn crc_ipl3_7102() {
        let ipl3 = IPL3::IPL3_7102([0; IPL_SIZE]);
        let program: Vec<u8> = (0..PROGRAM_SIZE).map(|i| i as u8).collect();

        let (crc1, crc2) = ipl3.compute_crcs(&program, &[]);

        assert_eq!(crc1, 0xfac847da);
        assert_eq!(crc2, 0xb2dea121);
    }

    #[test]
    fn offset_ipl3_6101() {
        let ipl3 = IPL3::IPL3_6101([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80000400);
    }

    #[test]
    fn offset_ipl3_6102() {
        let ipl3 = IPL3::IPL3_6102([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80000400);
    }

    #[test]
    fn offset_ipl3_6103() {
        let ipl3 = IPL3::IPL3_6103([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80100400);
    }

    #[test]
    fn offset_ipl3_6105() {
        let ipl3 = IPL3::IPL3_6105([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80000400);
    }

    #[test]
    fn offset_ipl3_6106() {
        let ipl3 = IPL3::IPL3_6106([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80200400);
    }

    #[test]
    fn offset_ipl3_7102() {
        let ipl3 = IPL3::IPL3_7102([0; IPL_SIZE]);
        assert_eq!(ipl3.offset(0x80000400), 0x80000400);
    }
}
