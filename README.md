## Mega Dumper Doris - Genesis/Mega Drive ROM Dumper

Mega Dumper Doris aims to turn a $20 Genesis ROM flashing device, the "Epicjoy MD Rewriter", into a competent ROM dumper, allowing you to back up your collection of Genesis cartridges as ROMs that can be used with emulators or flash carts. Epicjoy's [official software](https://github.com/Epicjoy/md_rewriter) purports to support ROM dumping, but this feature is completely broken in their software. Mega Dumper Doris goes much further than simply making the MD Rewriter work for dumping ROMs, though. It seeks to make it an *actually good* dumper.

### Features
- Compatible with almost all licensed and aftermarket Genesis and Mega Drive cartridges.
- Automatically dumps SRAM (save data) from cartridges that have it.
- Compares CRC32 and serial numbers against the [No-Intro checksum database](https://datomatic.no-intro.org/) to identify the game being dumped, verify good dumps, and use the game's title for the filename of the ROM and save data.
- Supports Sonic & Knuckles lock-on combination with all supported cartridges except Sonic 2. (See the "incompatible games" section below.)

### Instructions
**Windows**

Double-click on mega-dumper-doris.exe. That's it.

**Linux**
```
chmod +x mega-dumper-doris
 sudo ./mega-dumper-doris
```

Mega Dumper Doris needs to be run as root on Linux, but the files it creates are owned by the user that invoked sudo, not by root. (If you don't know what this means, you don't need to worry about it.)

### Incompatible games
These can't be supported without changes to the firmware running on the MD Rewriter. Although its firmware is open source, I don't know how to flash new firmware onto it. If you figure out a way, please let me know.

- Sonic 2 locked on to Sonic & Knuckles will produce an incorrect ROM. (The two games work fine separately, though, and lock-on works with all other games.)
- I don't have a cartridge of Super Street Fighter II: The New Challengers, but it uses a custom mapper to store an extra 1 MB in the ROM, so there's no way it works.
- I don't have or especially want a copy of Pier Solar, but it has built-in copy protection among other things, so it definitely doesn't work.
- A handful of cartridges, mostly late-era sports games, use EEPROM rather than SRAM for save data. I don't own any of these, but their saves can't be dumped. Hopefully the ROMs can still be dumped from these games, though. If you have one, please let me know whether this is the case.
- Master System games through the Power Base Converter. I don't have the hardware to test this, so I don't know whether this can be made to work without changes to the firmware.

Please file an issue in this repository if you encounter a game not listed here that can't be dumped, but works on a real Genesis or Mega Drive console.

### Notice
I have no affiliation with Epicjoy, the makers of the MD Rewriter, so I cannot guarantee the quality of their product if you buy one to use it with this.

This also means you shouldn't ask Epicjoy for help with Mega Dumper Doris. If you encounter a problem with it, file an issue here instead.

### License
Copyright (C) 2026 Bryan Cain (Plombo)

Mega Dumper Doris is free software, available under the terms of the GNU General Public License, version 3.
