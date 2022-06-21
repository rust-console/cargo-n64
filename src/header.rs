use crate::ipl3::IPL3;

pub(crate) const HEADER_SIZE: usize = 0x40;

#[derive(Debug, Clone, Copy)]
pub(crate) struct N64Header {
    // 0x00
    device_latency: u8,             // PI_BSD_DOM1_LAT_REG
    device_rw_pulse_width: u8,      // PI_BSD_DOM1_PWD_REG
    device_page_size: u8,           // PI_BSD_DOM1_PGS_REG
    device_rw_release_duration: u8, // PI_BSD_DOM1_RLS_REG
    clock_rate: u32,                // Unused by IPL and OS
    entry_point: u32,               // Executable start address/entry point
    release: u32,                   // Unused by IPL and OS

    // 0x10
    crc1: u32,
    crc2: u32,
    _reserved_1: [u8; 8],

    // 0x20
    name: [u8; 20],
    _reserved_2: [u8; 7],
    manufacturer: u8,
    cart_id: [u8; 2],
    region_code: u8,
    _reserved_3: u8,
}

impl N64Header {
    pub(crate) fn new(
        entry_point: u32,
        name_str: &str,
        program: &[u8],
        fs: &[u8],
        ipl3: &IPL3,
    ) -> N64Header {
        let (crc1, crc2) = ipl3.compute_crcs(program, fs);
        let entry_point = ipl3.offset(entry_point);

        let name_str = format!("{:20}", name_str);
        let mut name = [0; 20];
        name.copy_from_slice(name_str.as_bytes());
        let name = name;

        let cart_id_str = b"KW"; // KodeWerx!
        let mut cart_id = [0; 2];
        cart_id.copy_from_slice(cart_id_str);
        let cart_id = cart_id;

        N64Header {
            // 0x00
            device_latency: 128,
            device_rw_pulse_width: 55,
            device_page_size: 18,
            device_rw_release_duration: 64,
            clock_rate: 15,
            entry_point,
            release: 0,

            // 0x10
            crc1,
            crc2,
            _reserved_1: [0; 8],

            // 0x20
            name,
            _reserved_2: [0; 7],
            manufacturer: b'N', // Nintendo
            cart_id,
            region_code: b'E', // USA/English
            _reserved_3: 0,
        }
    }

    pub(crate) fn to_vec(self) -> Vec<u8> {
        // 0x00
        let mut buffer = vec![
            self.device_latency,
            self.device_rw_pulse_width,
            self.device_page_size,
            self.device_rw_release_duration,
        ];
        buffer.extend_from_slice(&self.clock_rate.to_be_bytes());
        buffer.extend_from_slice(&self.entry_point.to_be_bytes());
        buffer.extend_from_slice(&self.release.to_be_bytes());

        // 0x10
        buffer.extend_from_slice(&self.crc1.to_be_bytes());
        buffer.extend_from_slice(&self.crc2.to_be_bytes());
        buffer.extend_from_slice(&self._reserved_1);

        // 0x20
        buffer.extend_from_slice(&self.name);
        buffer.extend_from_slice(&self._reserved_2);
        buffer.push(self.manufacturer);
        buffer.extend_from_slice(&self.cart_id);
        buffer.push(self.region_code);
        buffer.push(self._reserved_3);

        buffer
    }
}
