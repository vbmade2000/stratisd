// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

extern crate loopdev;

use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use devicemapper::{Bytes, IEC, Sectors};

use self::loopdev::{LoopControl, LoopDevice};

use super::logger::init_logger;
use super::tempdir::TempDir;
use super::util::clean_up;

use super::super::device::wipe_sectors;


/// Ways of specifying range of numbers of devices to use for tests.
/// Unlike real tests, there is no AtLeast constructor, as, at least in theory
/// there is no upper bound to the number of loop devices that can be made.
pub enum DeviceLimits {
    Exactly(usize),
    Range(usize, usize), // inclusive
}

pub struct LoopTestDev {
    ld: LoopDevice,
}

impl LoopTestDev {
    /// Create a new loopbacked device.
    /// Create its backing store of 1 GiB wiping the first 1 MiB.
    pub fn new(lc: &LoopControl, path: &Path) -> LoopTestDev {
        clean_up();
        let mut f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&path)
            .unwrap();

        // the proper way to do this is fallocate, but nix doesn't implement yet.
        // TODO: see https://github.com/nix-rust/nix/issues/596
        f.seek(SeekFrom::Start(IEC::Gi)).unwrap();
        f.write(&[0]).unwrap();
        f.flush().unwrap();

        let ld = lc.next_free().unwrap();
        ld.attach(path, 0).unwrap();
        // Wipe 1 MiB at the beginning, as data sits around on the files.
        wipe_sectors(&ld.get_path().unwrap(),
                     Sectors(0),
                     Bytes(IEC::Mi).sectors())
                .unwrap();

        LoopTestDev { ld: ld }
    }
}

impl Drop for LoopTestDev {
    fn drop(&mut self) {
        clean_up();
        self.ld.detach().unwrap()
    }
}

/// Get a list of counts of devices to use for tests.
fn get_device_counts(limits: DeviceLimits) -> Vec<usize> {
    match limits {
        DeviceLimits::Exactly(num) => vec![num],
        DeviceLimits::Range(lower, upper) => {
            assert!(lower < upper);
            vec![lower, upper]
        }
    }
}

/// Setup count loop backed devices in dir.
fn get_devices(count: usize, dir: &TempDir) -> Vec<LoopTestDev> {
    let lc = LoopControl::open().unwrap();
    let mut loop_devices = Vec::new();

    for index in 0..count {
        let path = dir.path().join(format!("store{}", &index));
        loop_devices.push(LoopTestDev::new(&lc, &path));
    }
    loop_devices
}


/// Run the designated tests according to the specification.
pub fn test_with_spec<F>(limits: DeviceLimits, test: F) -> ()
    where F: Fn(&[&Path]) -> ()
{
    let counts = get_device_counts(limits);

    init_logger();

    for count in counts {
        let tmpdir = TempDir::new("stratis").unwrap();
        let loop_devices: Vec<LoopTestDev> = get_devices(count, &tmpdir);
        let device_paths: Vec<PathBuf> = loop_devices
            .iter()
            .map(|x| x.ld.get_path().unwrap())
            .collect();
        let device_paths: Vec<&Path> = device_paths.iter().map(|x| x.as_path()).collect();
        test(&device_paths);
    }
}
