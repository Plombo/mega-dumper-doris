use std::{collections::HashMap, error::Error};
use std::fs;
use std::fs::File;
use std::io::Write as _;
use std::time::Instant;
use encoding_rs::SHIFT_JIS;
use serialport::{SerialPort, SerialPortType, UsbPortInfo};

mod romdb;

trait SerialPortExt {
    fn connect(&mut self) -> Result<(), Box<dyn Error>>;
    fn read_all(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    fn expect_string(&mut self, expected: &str) -> Result<(), Box<dyn Error>>;
    fn dump_header(&mut self) -> Result<RomHeader, Box<dyn Error>>;
    fn dump_rom(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    fn dump_sram(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
    fn read_length(&mut self, length: usize) -> Result<Vec<u8>, Box<dyn Error>>;
}

impl SerialPortExt for dyn SerialPort {
    fn connect(&mut self) -> Result<(), Box<dyn Error>> {
        while self.bytes_to_read()? > 0 {
            println!("{:?}", self.bytes_to_read());
            println!("{:?}", String::from_utf8_lossy(&self.read_all()?));
        }
        self.write(&[0x0c, 0xaa, 0x55, 0xaa, 0xbb])?;
        self.expect_string("FlashMaster MD Dumper is connected\r\n")?;
        println!("Connected to dumper.");
        Ok(())
    }

    // Even though the header is contained within the first 512 bytes of the ROM, the smallest
    // dump size supported by the firmware is 512KB, so do that.
    fn dump_header(&mut self) -> Result<RomHeader, Box<dyn Error>> {
        const BUF_SIZE: usize = 1024;
        const TOTAL_SIZE: usize = 1024 * 512;
        self.write(&[0x0a, 0xaa, 0x55, 0xaa, 0xbb, 0x01])?;
        self.expect_string("512K ROM DUMP START!!!\r\n")?;
        let mut response = vec![];
        while response.len() < TOTAL_SIZE {
            let response_buf = self.read_length(BUF_SIZE)?;
            response.extend_from_slice(&response_buf);
        }
        self.expect_string("DUMPER ROM FINISH!!!\r\nPUSH SAVE GAME BUTTON!!!\r\n")?;

        Ok(RomHeader::from_bytes(&response[0x100..0x200]))
    }

    // Always dump the full 4 MB; some ROMs are larger than their header says.
    fn dump_rom(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        const BUF_SIZE: usize = 1024;
        const TOTAL_SIZE: usize = 1024 * 1024 * 4;
        self.write(&[0x0a, 0xaa, 0x55, 0xaa, 0xbb, 0x04])?;
        self.expect_string("4M ROM DUMP START!!!\r\n")?;
        let mut response = vec![];
        while response.len() < TOTAL_SIZE {
            let response_buf = self.read_length(BUF_SIZE)?;
            response.extend_from_slice(&response_buf);
        }
        self.expect_string("DUMPER ROM FINISH!!!\r\nPUSH SAVE GAME BUTTON!!!\r\n")?;

        Ok(response)
    }

    fn dump_sram(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        const BUF_SIZE: usize = 1024;
        const TOTAL_SIZE: usize = 1024 * 32;
        self.write(&[0x1a, 0xaa, 0x55, 0xaa, 0xbb, 0x01])?;
        self.expect_string("32K RAM DUMP START!!!\r\n")?;
        let mut response = vec![];
        while response.len() < TOTAL_SIZE {
            let response_buf = self.read_length(BUF_SIZE)?;
            response.extend_from_slice(&response_buf);
        }
        self.expect_string("DUMPER RAM FINISH!!!\r\n")?;

        Ok(response)
    }

    fn read_all(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
        let mut response = vec![0; self.bytes_to_read()?.try_into().unwrap()];
        self.read_exact(&mut response)?;
        Ok(response)
    }

    fn read_length(&mut self, length: usize) -> Result<Vec<u8>, Box<dyn Error>> {
        let start_time = Instant::now();
        while (self.bytes_to_read()? as usize) < length {
            // time out after 15 seconds
            if start_time.elapsed().as_secs() >= 15 {
                return Err("operation timed out".to_string().into());
            }
        }
        let mut response = vec![0; length];
        self.read_exact(&mut response)?;
        Ok(response)
    }

    fn expect_string(&mut self, expected: &str) -> Result<(), Box<dyn Error>> {
        if expected.as_bytes() != &self.read_length(expected.len())? {
            Err("did not get the expected string".to_string().into())
        } else {
            Ok(())
        }
    }
}

struct RomHeader {
    system_type: String,
    copyright: String,
    domestic_title: String,
    overseas_title: String,
    serial: String,
    checksum: u16,
    device_support: String,
    rom_address_start: u32,
    rom_size: usize,
    sram: Option<Sram>,
    regions: String,
}

impl RomHeader {
    fn from_bytes(header: &[u8]) -> RomHeader {
        assert!(header.len() == 256);
        RomHeader {
            system_type: String::from_utf8_lossy(&header[0..0x10]).into(),
            copyright: String::from_utf8_lossy(&header[0x10..0x20]).into(),
            domestic_title: SHIFT_JIS.decode(&header[0x20..0x50]).0.into(),
            overseas_title: SHIFT_JIS.decode(&header[0x50..0x80]).0.into(),
            serial: String::from_utf8_lossy(&header[0x80..0x8e]).into(),
            checksum: u16::from_be_bytes([header[0x8e], header[0x8f]]),
            device_support: String::from_utf8_lossy(&header[0x90..0xa0]).into(),
            rom_address_start: u32::from_be_bytes(header[0xa0..0xa4].try_into().unwrap()),
            rom_size: (u32::from_be_bytes(header[0xa4..0xa8].try_into().unwrap()) as usize) + 1,
            sram: Sram::from_bytes(&header[0xb0..0xbc]),
            regions: String::from_utf8_lossy(&header[0xf0..0xf3]).into(),
        }
    }

    fn valid(&self) -> bool {
        self.system_type.starts_with("SEGA") &&
        self.system_type.is_ascii() &&
        self.copyright.is_ascii() &&
        self.serial.is_ascii() &&
        self.device_support.is_ascii() &&
        self.rom_address_start == 0 &&
        self.rom_size <= 4*1024*1024
    }

    fn print(&self) {
        println!("System type: {}", &self.system_type);
        println!("Copyright: {}", &self.copyright);
        println!("Domestic title: {}", &self.domestic_title);
        println!("Overseas title: {}", &self.overseas_title);
        println!("Serial number: {}", &self.serial);
        println!("Checksum: {:04X}", self.checksum);
        println!("Device support: {}", &self.device_support);
        println!("ROM size: {} bytes", self.rom_size);
        println!("SRAM: {:?}", self.sram);
        println!("Supported regions: {}", &self.regions);
        println!();
    }
}

#[derive(Debug)]
struct Sram {
    start_address: u32,
    end_address: u32,
}

impl Sram {
    fn from_bytes(data: &[u8]) -> Option<Sram> {
        if &data[0..2] == &[b'R', b'A'] {
            println!("SRAM type: {:02X}", data[2]);
            Some(Sram {
                start_address: u32::from_be_bytes(data[4..8].try_into().unwrap()),
                end_address: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            })
        } else {
            None
        }
    }
}

fn checksum(rom: &[u8]) -> u16 {
    // If the rom size is odd for some reason, leave out the last byte to prevent this function from panicking.
    // The checksum is unlikely to match in such a ROM anyway.
    let end_point = rom.len() & !1;
    rom[0x200..end_point].chunks(2).map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]) as usize).sum::<usize>() as u16
}

// TODO move this into romdb.rs?
// returns rom name and size
fn find_no_intro_match(rom_data: &[u8], mut rom_size: usize, romdb: &[romdb::Rom]) -> (String, usize) {
    let header = RomHeader::from_bytes(&rom_data[0x100..0x200]);
    let mut name = "Unknown Game".to_string();
    let mut no_intro_match_found = false;
    let crc32 = crc32fast::hash(&rom_data[0..rom_size]);
    println!("CRC32: {:08x}", crc32);
    if let Some(dbmatch) = romdb.iter().find(|e| e.crc32 == crc32) {
        println!("Found No-Intro match: {}", dbmatch.name);
        name = dbmatch.name.clone();
        no_intro_match_found = true;
    } else {
        let mut crcs: HashMap<usize, u32> = HashMap::new();
        for dbmatch in romdb {
            // Some ROMs, like "Zero Wing (Europe)", are listed in No-Intro with a different
            // size from the header.
            if dbmatch.size != rom_size {
                if dbmatch.size > rom_data.len() { continue; }
                let crc_for_size = if crcs.contains_key(&dbmatch.size) {
                    *crcs.get(&dbmatch.size).unwrap()
                } else {
                    let crc = crc32fast::hash(&rom_data[0..dbmatch.size]);
                    crcs.insert(dbmatch.size, crc);
                    crc
                };
                if dbmatch.crc32 == crc_for_size {
                    no_intro_match_found = true;
                    // Don't say this for Sonic & Knuckles; it just means a cartridge is locked on.
                    // This won't happen when nothing is locked on, since the size in the S&K ROM
                    // header (2MB) matches its size in No-Intro.
                    if dbmatch.name != "Sonic & Knuckles (World)" {
                        println!("The ROM size in the No-Intro database ({}) differs from the size in the ROM header ({}). This is normal and not a problem.", dbmatch.size, rom_size);
                    }
                    println!("Found No-Intro match: {}", dbmatch.name);
                    name = dbmatch.name.clone();
                    rom_size = dbmatch.size;
                }
            }
        }

        if !no_intro_match_found {
            let serial_matches: Vec<_> = romdb.iter().filter(|e| {
                if let Some(db_serial) = e.serial.as_ref() {
                    header.serial.replace(" ", "").replace("-", "").contains(&db_serial.replace(" ", "").replace("-", ""))
                } else { false }
            }).collect();

            if header.serial == "GM 00001009-00" {
                if header.domestic_title.trim() == "" {
                    // Tanglewood has a fake ROM header at the 2 MB mark to fool Sonic & Knuckles
                    // into thinking Sonic 1 is locked on.
                    name = "Tanglewood (unknown variant)".to_string();
                } else {
                    // There's a random European prototype dumped by Hidden Palace that has the Sonic 1
                    // serial number in its header. No one is dumping that. This is Sonic. If you have
                    // rare prototype cartridges, please use a dumper that you didn't get for $20 from
                    // AliExpress.
                    name = "Sonic The Hedgehog (unknown variant)".to_string();
                }
            } else if !serial_matches.is_empty() && !header.serial.contains("00000000-00") {
                println!("No exact match found in the No-Intro database, but shares a serial number with these entries:");
                for dbmatch in &serial_matches {
                    println!("\t{}", dbmatch.name);
                }
                name = format!("{} (unknown variant)", serial_matches[0].name.split_once(" (").map_or(serial_matches[0].name.as_str(), |m| m.0));
            } else {
                println!("No match found in the No-Intro database.");
                // use default name
            }
        }
    }

    let header_checksum = u16::from_be_bytes([rom_data[0x18e], rom_data[0x18f]]);
    // At least one game (the aforementioned Zero Wing) has a header checksum calculated with the
    // actual ROM size, not the ROM size in the header.
    let calculated_checksum1 = if header.rom_size <= rom_data.len() { checksum(&rom_data[0..header.rom_size]) } else { 0 };
    let calculated_checksum2 = if rom_size <= rom_data.len() { checksum(&rom_data[0..rom_size]) } else { 0 };
    if header_checksum == calculated_checksum1 || header_checksum == calculated_checksum2 {
        println!("Calculated checksum matches ROM header!");
        if !no_intro_match_found {
            println!();
            println!("Since the checksum is correct, this might be a good dump of a cartridge that's not in the No-Intro database. Try running \"{name}\" in your favorite emulator and see if it works.");
        }
    } else if no_intro_match_found {
        println!("Calculated checksum {calculated_checksum1:04X} does not match ROM header.");
        println!();
        println!("Since this matches an entry in the No-Intro database, it is still a good dump. The mismatched checksum can be safely ignored.");
    } else {
        println!("Warning: calculated checksum {calculated_checksum1:04X} does not match ROM header!");
        println!();
        println!("Since the checksum is mismatched and no match was found in No-Intro, this might be a bad dump. It is recommended to disconnect the dumper, remove and reinsert the cartridge, redo the dump, and see if the ROM dumped is the same.");
    }

    (name, rom_size)
}

fn dump(force: bool) -> Result<(), Box<dyn Error>> {
    let mut device_path = None;
    for p in serialport::available_ports()? {
        if let SerialPortType::UsbPort(UsbPortInfo {vid: 0x0483, pid: 0x5740, .. }) = p.port_type {
            device_path = Some(p.port_name);
        }
    }

    let device_path = device_path.ok_or("Dumper device not found".to_string())?;
    println!("Opening device at {}", device_path);

    let mut conn = serialport::new(device_path, 1000000).open()?;
    conn.connect()?;

    let mut header = conn.dump_header()?;
    let mut rom_size = header.rom_size;

    println!("\nROM header:");
    header.print();

    if header.valid() {
        println!("Header seems to be valid.");
    } else {
        println!("Header seems to be invalid.");
        if force {
            println!("Dumping the ROM anyway since --force was specified.");
            rom_size = 4 * 1024 * 1024;
        } else {
            println!("Use --force to dump the ROM anyway.");
            println!("The --force option might be necessary for dumping Sonic 3.");
            return Err("Invalid ROM header.".to_string().into());
        }
    }

    println!("Dumping ROM...");
    let mut rom_data = conn.dump_rom()?;
    println!("Finished dumping ROM.");

    // For some reason, my Sonic 3 cartridge shows up in the bottom 2MB of the dump instead of the
    // top 2MB. This means its header is detected as invalid and "--force" must be used to dump it.
    // So, if "--force" has been used, check whether this has happened.
    if !header.valid() {
        let second_header = RomHeader::from_bytes(&rom_data[0x200100..0x200200]);
        if second_header.valid() {
            println!("Found a valid header 2 MB into the ROM. Swapping the top and bottom halves.");

            header = second_header;
            let mut swapped_rom_data = Vec::new();
            swapped_rom_data.extend_from_slice(&rom_data[0x200000..]);
            swapped_rom_data.extend_from_slice(&rom_data[..0x200000]);
            rom_data = swapped_rom_data;
            rom_size = header.rom_size;

            println!("\nROM header:");
            header.print();
        }
    }

    if let Some(sram) = &header.sram {
        println!("Dumping SRAM...");
        let sram_data = conn.dump_sram()?;
        println!("Finished dumping SRAM.");

        let sram_size = (sram.end_address - sram.start_address + 2) as usize;
        println!("SRAM size: {} KiB", sram_size / 1024 / 2);

        // TODO move this
        fs::write("save.sav", &sram_data[..sram_size])?;
        println!("\nWrote SRAM to \"save.sav\"");
    }

    process_dump(rom_data, header, rom_size)
}

// This is only useful for debugging.
fn process_from_file(path: &str) -> Result<(), Box<dyn Error>> {
    let mut rom_data = fs::read(path)?;
    rom_data.resize(4 * 1024 * 1024, 0xff);
    let header = RomHeader::from_bytes(&rom_data[0x100..0x200]);
    let rom_size = if header.valid() { header.rom_size } else { 4 * 1024 * 1024 };
    process_dump(rom_data, header, rom_size)
}

fn process_dump(rom_data: Vec<u8>, header: RomHeader, mut rom_size: usize) -> Result<(), Box<dyn Error>> {
    let romdb = romdb::read_no_intro()?;

    let locked_on;
    if header.overseas_title.trim() == "SONIC & KNUCKLES" {
        println!("\nSonic & Knuckles detected. Seeing if anything is locked on...");
        let second_header = RomHeader::from_bytes(&rom_data[0x200100..0x200200]);
        if second_header.valid() {
            locked_on = true;
            rom_size += second_header.rom_size;
            println!("\nLocked-on cartridge header:");
            second_header.print();
            match second_header.domestic_title.trim() {
                "SONIC THE             HEDGEHOG 3" => {
                    println!("Sonic 3 is locked on.");
                }
                "SONIC THE             HEDGEHOG 2" => {
                    println!("Sonic 2 is locked on. This isn't supported and won't work properly.");
                }
                _ => {}
            }
        } else if rom_data[0x200000..] == [0xff; 0x200000] {
            locked_on = false;
            println!("Nothing is locked on.");
        } else {
            locked_on = true;
            println!("An unsupported cartridge (>2MB) is locked on, or the lock-on connection is bad.");
            println!("\nLocked-on cartridge header:");
            second_header.print();
        }
        println!();
    } else {
        locked_on = false;
    }

    let (mut name, mut rom_size) = find_no_intro_match(&rom_data, rom_size, &romdb);
    if name == "Sonic & Knuckles (World)" && locked_on {
        println!("\nTrying to identify the locked-on cartridge...");
        let second_rom = &rom_data[0x200000..];
        let second_header = RomHeader::from_bytes(&second_rom[0x100..0x200]);
        let second_size = if second_header.valid() { second_header.rom_size } else { 2*1024*1024 };
        let (mut second_name, second_size) = find_no_intro_match(second_rom, second_size, &romdb);
        if second_name == "Unknown Game" {
            second_name = format!("Unsupported Cartridge ({:08x})", crc32fast::hash(&second_rom[..second_size]));
        }
        name = format!("Sonic & Knuckles + {}", second_name);
        rom_size += second_size;
    }

    let filename = format!("{name}.gen");
    let mut file = File::create(&filename)?;
    file.write_all(&rom_data[0..rom_size])?;
    println!("\nWrote ROM to \"{}\"", &filename);

    Ok(())
}

fn main() {
    // very haphazard argument processing because who cares
    let args: Vec<_> = std::env::args().collect();
    let mut force = false;
    if args.iter().any(|a| a == "--force") {
        force = true;
    } else if args.len() > 1 {
        if let Err(err) = process_from_file(&args[1]) {
            eprintln!("Error: {:?}", err);
            std::process::exit(1);
        }
        std::process::exit(0);
    }

    if let Err(err) = dump(force) {
        eprintln!("Error: {:?}", err);
        std::process::exit(1);
    }
}
