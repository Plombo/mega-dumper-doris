use std::env;
use std::fs;
use std::path::Path;

pub fn write_file<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> std::io::Result<()> {
    fs::write(&path, contents)?;

    // Since this is running as root, give ownership of created files to the actual user, not root.
    #[cfg(unix)]
    let uid = env::var("SUDO_UID").ok().map(|s| u32::from_str_radix(&s, 10).ok()).flatten();
    let gid = env::var("SUDO_GID").ok().map(|s| u32::from_str_radix(&s, 10).ok()).flatten();
    let _ = std::os::unix::fs::chown(&path, uid, gid);

    Ok(())
}
