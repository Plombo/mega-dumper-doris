use std::cmp::Ordering;
use std::io::Read;
use xml::EventReader;
use xml::reader::XmlEvent;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rom {
    pub name: String,
    pub size: usize,
    pub serial: Option<String>,
    pub crc32: u32,
}

impl Rom {
    // If the ROM checksum doesn't match a No-Intro entry but the serial does, the dumped ROM will
    // be named like "Title (unknown variant).gen". If there are multiple titles for one serial,
    // this will determine which one gets used.
    fn priority(&self) -> i32 {
        // Various betas, prototypes, and bootlegs reuse the serial numbers from better-known games.
        // No one is dumping those with this. If you actually have rare prototype cartridges, please
        // use a dumper that you didn't get for $20 from AliExpress.
        if self.name.contains("(Pirate") {
            11
        } else if self.name.contains("(Proto") {
            10
        } else if self.name.contains("(Beta") {
            9
        } else if self.name.contains("(Aftermarket") {
            8
        } else if self.name.contains("(Unl") {
            7
        } else if self.name.contains("(Demo") {
            6
        } else if self.name.contains("(Sample") {
            5
        // If there are still multiple titles for the same serial number, prioritize the titles
        // most likely to be in English.
        } else if self.name.contains("(USA") || self.name.contains(", USA") {
            -3
        } else if self.name.contains("(Europe") || self.name.contains(", Europe") {
            -2
        // Korea gets lower priority than Japan.
        } else if self.name.contains("(Korea)") {
            1
        } else {
            0
        }
    }
}

impl PartialOrd for Rom {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.priority().partial_cmp(&other.priority())
    }
}

impl Ord for Rom {
    fn cmp(&self, other: &Self) -> Ordering {
        println!("Comparing '{}' to '{}'", self.name, other.name);
        self.priority().cmp(&other.priority())
    }
}

fn read_dat<R>(reader: R) -> std::io::Result<Vec<Rom>>
    where R: Read
{
    let mut roms = Vec::new();
    let parser = EventReader::new(reader);

    for e in parser {
        match e {
            Ok(XmlEvent::StartElement { name, attributes, .. }) => {
                if name.local_name == "rom" {
                    let mut name = None;
                    let mut size = None;
                    let mut serial = None;
                    let mut crc32 = None;
                    for attr in &attributes {
                        match attr.name.local_name.as_str() {
                            "name" => { name = Some(attr.value.strip_suffix(".md").unwrap_or(&attr.value).to_string()) },
                            "size" => { size = usize::from_str_radix(&attr.value, 10).ok(); },
                            "serial" => { serial = Some(attr.value.clone()); },
                            "crc" => { crc32 = u32::from_str_radix(&attr.value, 16).ok(); },
                            _ => {},
                        }
                    }

                    if name.is_some() && size.is_some() && crc32.is_some() {
                        roms.push(Rom {
                            name: name.unwrap(),
                            size: size.unwrap(),
                            serial,
                            crc32: crc32.unwrap(),
                        })
                    }
                }
            }
            Err(e) => {
                eprintln!("Error: {e}");
                break;
            }
            _ => {}
        }
    }

    Ok(roms)
}

pub fn read_no_intro() -> std::io::Result<Vec<Rom>> {
    let mut roms = read_dat(&include_bytes!("../dat/Sega - Mega Drive - Genesis (20260221-141231).dat")[..])?;
    roms.extend_from_slice(&read_dat(&include_bytes!("../dat/Sega - Mega Drive - Genesis (Private) (20260221-141231).dat")[..])?);

    // Add entries for the fake ROM header that Tanglewood uses when locked onto Sonic & Knuckles.
    // Probably no one will ever use the digital-only variants, since you'd have to flash them onto
    // a cartridge and then dump that cartridge while locked on to Sonic & Knuckles. But hey, maybe
    // someone out there wants to do that. If you're that person, congratulations. It's your lucky day.
    roms.push(Rom {
        name: "Tanglewood (World) (Aftermarket) (Unl)".to_string(),
        size: 2097152,
        serial: Some("GM 00001009-00".to_string()),
        crc32: 0x234254c7,
    });
    roms.push(Rom {
        name: "Tanglewood (World) (GOG, Itch.io) (Aftermarket) (Unl)".to_string(),
        size: 2097152,
        serial: Some("GM 00001009-00".to_string()),
        crc32: 0xd4127487,
    });
    roms.push(Rom {
        name: "Tanglewood (World) (GOG) (Windows) (Aftermarket) (Unl)".to_string(),
        size: 2097152,
        serial: Some("GM 00001009-00".to_string()),
        crc32: 0x6d1079dc,
    });

    // Add entries for Sonic Classics/Sonic Compilation, which does the same thing. Though in this
    // case, it's not a fake ROM header per se. They just put an entire Sonic 1 ROM at the 2 MB
    // position in the ROM.
    roms.push(Rom {
        name: "Sonic Compilation (Europe)".to_string(),
        size: 1048576,
        serial: Some("GM 00001009-00".to_string()),
        crc32: 0x83bfb8bb,
    });
    roms.push(Rom {
        name: "Sonic Compilation (USA, Europe, Korea) (En) (Rev A)".to_string(),
        size: 1048576,
        serial: Some("GM 00001009-00".to_string()),
        crc32: 0xa85ddcae,
    });


    Ok(roms)
}
