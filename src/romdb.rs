use std::io::Read;
use xml::EventReader;
use xml::reader::XmlEvent;

#[derive(Clone, Debug)]
pub struct Rom {
    pub name: String,
    pub size: usize,
    pub serial: Option<String>,
    pub crc32: u32,
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


    Ok(roms)
}
