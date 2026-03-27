use std::fmt::{Display, Formatter};
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
    fn dump_512k(&mut self) -> Result<Vec<u8>, Box<dyn Error>>;
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
    fn dump_512k(&mut self) -> Result<Vec<u8>, Box<dyn Error>> {
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

        Ok(response)
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
    sram: Option<SramInfo>,
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
            sram: SramInfo::from_bytes(&header[0xb0..0xbc]),
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
        self.rom_size > 512 &&
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
        if let Some(sram_info) = &self.sram {
            println!("SRAM: {}", sram_info);
        } else {
            println!("SRAM: none");
        }
        println!("Supported regions: {}", &self.regions);
        println!();
    }
}

#[derive(Debug)]
struct SramInfo {
    ram_type: u8,
    start_address: u32,
    end_address: u32,
}

impl SramInfo {
    fn from_bytes(data: &[u8]) -> Option<SramInfo> {
        if &data[0..2] == &[b'R', b'A'] {
            Some(SramInfo {
                ram_type: data[2],
                start_address: u32::from_be_bytes(data[4..8].try_into().unwrap()),
                end_address: u32::from_be_bytes(data[8..12].try_into().unwrap()),
            })
        } else {
            None
        }
    }
}

impl Display for SramInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), std::fmt::Error> {
        let description;
        if self.ram_type & 0x10 == 0 {
            description = "16-bit";
        } else if self.ram_type & 0x08 == 0 {
            description = "8-bit with even addresses";
        } else {
            description = "8-bit with odd addresses";
        }
        write!(f, "type={:02X} ({}), start={:x}, end={:x}", self.ram_type, description, self.start_address, self.end_address)?;
        Ok(())
    }
}

fn checksum(rom: &[u8], size: usize) -> Option<u16> {
    if size < 0x200 || size > rom.len() {
        return None;
    }
    // If the rom size is odd for some reason, leave out the last byte to prevent this function from panicking.
    // The checksum is unlikely to match in such a ROM anyway.
    let end_point = size & !1;
    Some(rom[0x200..end_point].chunks(2).map(|chunk| u16::from_be_bytes([chunk[0], chunk[1]]) as usize).sum::<usize>() as u16)
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
                        println!("The ROM size in the No-Intro database ({}) differs from the \
                                 size in the ROM header ({}). This is normal and not a problem.",
                                 dbmatch.size, rom_size);
                    }
                    println!("Found No-Intro match: {}", dbmatch.name);
                    name = dbmatch.name.clone();
                    rom_size = dbmatch.size;
                }
            }
        }

        if !no_intro_match_found {
            let mut serial_matches: Vec<_> = romdb.iter().filter(|e| {
                if let Some(db_serial) = e.serial.as_ref() {
                    header.serial.replace(" ", "").replace("-", "").contains(&db_serial.replace(" ", "").replace("-", ""))
                } else { false }
            }).collect();
            serial_matches.sort();

            if header.serial == "GM 00001009-00" && header.domestic_title.trim() == "" {
                // Tanglewood has a fake ROM header at the 2 MB mark to fool Sonic & Knuckles
                // into thinking Sonic 1 is locked on.
                name = "Tanglewood (unknown variant)".to_string();
            } else if !serial_matches.is_empty() && !header.serial.contains("00000000-00") {
                println!("No exact match found in the No-Intro database, but shares a serial number with these entries:");
                for dbmatch in &serial_matches {
                    println!("\t{}", dbmatch.name);
                }
                name = format!("{} (unknown variant)", serial_matches[0].name.split_once(" (").map_or(serial_matches[0].name.as_str(), |m| m.0));
            } else {
                println!("No match found in the No-Intro database.");
                // use default name ("Unknown Game")
            }
        }
    }

    let header_checksum = u16::from_be_bytes([rom_data[0x18e], rom_data[0x18f]]);
    // At least one game (the aforementioned Zero Wing) has a header checksum calculated with the
    // actual ROM size, not the ROM size in the header.
    let calculated_checksum1 = checksum(&rom_data, header.rom_size);
    let calculated_checksum2 = checksum(&rom_data, rom_size);
    if calculated_checksum1 == Some(header_checksum) || calculated_checksum2 == Some(header_checksum) {
        println!("Calculated checksum matches ROM header!");
        if !no_intro_match_found {
            println!();
            println!("Since the checksum is correct, this might be a good dump of a cartridge that's not in the No-Intro database. Try running \"{name}\" in your favorite emulator and see if it works.");
        }
    } else if let Some(calculated_checksum) = calculated_checksum1 && no_intro_match_found {
        println!("Calculated checksum {calculated_checksum:04X} does not match ROM header.");
        println!();
        println!("Since this matches an entry in the No-Intro database, it is still a good dump. The mismatched checksum can be safely ignored.");
    } else if let Some(calculated_checksum) = calculated_checksum1 {
        println!("Warning: calculated checksum {calculated_checksum:04X} does not match ROM header!");
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

    // Sonic 3 maps its entire ROM into the bottom 2 MB of address space, rather than the top, unless
    // we read SRAM first. Strictly Limited Games' release of Panorama Cotton, which doesn't even
    // have SRAM, won't map the ROM at all unless we try to read from SRAM. Conversely, 16-Bit Rhythm
    // Land from Columbus Circle won't map the ROM at all if we *do* try to read SRAM at any point.
    // So the only universally compatible approach is this:
    //     1) Read the first 512K bytes of the ROM.
    //     2) If every byte of that 512K dump is 0xFF, read SRAM, then reread the first 512K bytes
    //        of the ROM.
    let mut first_512k = conn.dump_512k()?;
    if first_512k == [0xff; 512*1024] {
        println!("The ROM header isn't showing up. Some games can have their ROMs \"unlocked\" by \
                  reading from SRAM. Trying that now.");
        let _ = conn.dump_sram()?;
        first_512k = conn.dump_512k()?;
    }

    let header = RomHeader::from_bytes(&first_512k[0x100..0x200]);
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
            return Err("Invalid ROM header.".to_string().into());
        }
    }

    println!("Dumping ROM...");
    let rom_data = conn.dump_rom()?;
    println!("Finished dumping ROM.");

    // TODO try this with something that uses a different SRAM type than F8
    let sram_data = if let Some(_) = &header.sram {
        println!("Dumping SRAM...");
        let sram_data = conn.dump_sram()?;
        println!("Finished dumping SRAM.");
        Some(sram_data)
    } else {
        None
    };

    process_dump(rom_data, header, rom_size, sram_data)
}

// This is only useful for debugging.
fn process_from_file(path: &str) -> Result<(), Box<dyn Error>> {
    let mut rom_data = fs::read(path)?;
    rom_data.resize(4 * 1024 * 1024, 0xff);
    let header = RomHeader::from_bytes(&rom_data[0x100..0x200]);
    let rom_size = if header.valid() { header.rom_size } else { 4 * 1024 * 1024 };

    println!("\nROM header:");
    header.print();

    let sram_data = if let Some(_) = &header.sram {
        Some(vec![0; 32*1024])
    } else {
        None
    };

    process_dump(rom_data, header, rom_size, sram_data)
}

fn process_dump(rom_data: Vec<u8>, header: RomHeader, mut rom_size: usize, sram: Option<Vec<u8>>) -> Result<(), Box<dyn Error>> {
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
    fs::write(&filename, &rom_data[0..rom_size])?;
    println!("\nWrote ROM to \"{}\"", &filename);

    // TODO handle errors: end_address < start_address or sram_size > 32K
    if let Some(sram) = sram && let Some(sram_info) = header.sram {
        // It's oddly tricky to deduce the SRAM size from the header. I referenced the
        // read_ram_header() function in BlastEm to figure this out.
        let start_address = sram_info.start_address & 0xfffffe;
        let end_address = sram_info.end_address | 1;
        let mut sram_size = (end_address - start_address + 1) as usize;
        if sram_info.ram_type & 0x10 != 0 { // this SRAM has 8-bit accesses
            sram_size /= 2;
        }
        sram_size &= !1; // Psy-O-Blade shows up as 32769 bytes without this
        println!("\nSRAM size: {} bytes", sram_size);

        let filename = format!("{name}.srm");
        fs::write(&filename, &sram[..sram_size])?;
        println!("Wrote SRAM to \"{}\"", &filename);
    }

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

    // TODO remove this temporary stuff
    let romdb = romdb::read_no_intro().unwrap();
    let mut serials: HashMap<String, Vec<_>> = HashMap::new();
    for rom in &romdb {
        if rom.serial == None { continue; }
        let serial = rom.serial.as_ref().unwrap();
        if let Some(vec) = serials.get_mut(serial) {
            vec.push(rom);
        } else {
            serials.insert(serial.clone(), vec![rom]);
        }
    }
    for (serial, vec) in serials.iter() {
        if vec.len() > 1 {
            let mut titles = std::collections::HashSet::new();
            for rom in vec {
                let name = &rom.name;
                if !name.contains("(Pirate)") && !name.contains("(Beta") && !name.contains("(Proto") {
                    titles.insert(name.split_once(" (").map_or(name.as_str(), |m| m.0));
                }
            }
            if titles.len() > 1 {
                println!("{serial}:");
                let mut sorted = vec.clone();
                sorted.sort();
                for rom in sorted {
                    println!("\t{}", &rom.name);
                }
                println!();
            }
        }
    }
}
