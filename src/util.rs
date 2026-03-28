// Copyright (C) 2026 Bryan Cain
//
// This program is free software; you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation; either version 3, or (at your option)
// any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

use std::fs;
use std::path::Path;

pub fn exit(code: i32) {
    // On Windows, the user has probably run Doris by just double-clicking the EXE. Add a "Press
    // Enter to continue" prompt so that the terminal window doesn't disappear immediately when
    // the dumping process finishes.
    #[cfg(target_os = "windows")] {
        use std::io::{Read, Write};
        print!("\nPress Enter to continue...");
        std::io::stdout().flush();
        let _ = std::io::stdin().read(&mut [0u8]).unwrap();
    }

    std::process::exit(code);
}

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
    fs::write(&path, contents)?;

    // Since this is running as root, give ownership of created files to the actual user, not root.
    #[cfg(unix)] {
        let uid = std::env::var("SUDO_UID").ok().map(|s| u32::from_str_radix(&s, 10).ok()).flatten();
        let gid = std::env::var("SUDO_GID").ok().map(|s| u32::from_str_radix(&s, 10).ok()).flatten();
        let _ = std::os::unix::fs::chown(&path, uid, gid);
    }

    Ok(())
}
