//! Changes that a check whose clock runs ahead of the files can see, whatever the file system's
//! timestamp resolution.

use std::fs;
use std::path::Path;
use std::time::Duration;

/// Runs `change`, which changes the file or directory `path`, then sets the mtime of `path` a
/// second past the one it had before. A request whose clock is 2 s or more ahead of `path`'s last
/// change settles its stat, so without this a change in the timestamp tick of the stat that
/// request saw would leave that stat as it was where timestamps are coarse, as with Linux 6.8 on
/// GitHub's `ubuntu-22.04` images (1 ms). A request's real check finds such a change by the 2 s
/// rule (DES-INPUTS-AND-KEYS).
pub fn visibly(path: &Path, change: impl FnOnce()) {
    let before = fs::symlink_metadata(path).unwrap().modified().unwrap();
    change();
    let after = before + Duration::from_secs(1);
    fs::File::open(path).unwrap().set_modified(after).unwrap();
}
