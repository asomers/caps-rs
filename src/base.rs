use super::{Capability, CapSet};
use errors::*;
use nr;

use libc;

const CAPS_V3: u32 = 0x20080522;

fn capget(hdr: &mut CapUserHeader, data: &mut CapUserData) -> Result<()> {
    let r = unsafe { libc::syscall(nr::CAPGET, hdr, data) };
    return match r {
        0 => Ok(()),
        _ => bail!("capget error {:?}", r),
    };
}

fn capset(hdr: &mut CapUserHeader, data: &CapUserData) -> Result<()> {
    let r = unsafe { libc::syscall(nr::CAPSET, hdr, data) };
    return match r {
        0 => Ok(()),
        _ => bail!("capset error {:?}", r),
    };
}

pub fn has_cap(tid: i32, cset: CapSet, cap: Capability) -> Result<bool> {
    let mut hdr = CapUserHeader {
        version: CAPS_V3,
        pid: tid,
    };
    let mut data: CapUserData = Default::default();
    try!(capget(&mut hdr, &mut data));
    let caps: u64 = match cset {
        CapSet::Effective => ((data.effective_s1 as u64) << 32) + data.effective_s0 as u64,
        CapSet::Inheritable => ((data.inheritable_s1 as u64) << 32) + data.inheritable_s0 as u64,
        CapSet::Permitted => ((data.permitted_s1 as u64) << 32) + data.permitted_s0 as u64,
        CapSet::Bounding | CapSet::Ambient => bail!("not a base set"),
    };
    let has_cap = (caps & cap.bitmask()) != 0;
    return Ok(has_cap);
}

pub fn clear(tid: i32, cset: CapSet) -> Result<()> {
    let mut hdr = CapUserHeader {
        version: CAPS_V3,
        pid: tid,
    };
    let mut data: CapUserData = Default::default();
    try!(capget(&mut hdr, &mut data));
    match cset {
        CapSet::Effective => {
            data.effective_s0 = 0;
            data.effective_s1 = 0;
        }
        CapSet::Inheritable => {
            data.inheritable_s0 = 0;
            data.inheritable_s1 = 0;
        }
        CapSet::Permitted => {
            data.effective_s0 = 0;
            data.effective_s1 = 0;
            data.permitted_s0 = 0;
            data.permitted_s1 = 0;
        }
        CapSet::Bounding | CapSet::Ambient => bail!("not a base set"),
    }
    return capset(&mut hdr, &mut data);
}

pub fn read(tid: i32, cset: CapSet) -> Result<super::CapsHashSet> {
    let mut hdr = CapUserHeader {
        version: CAPS_V3,
        pid: tid,
    };
    let mut data: CapUserData = Default::default();
    try!(capget(&mut hdr, &mut data));
    let caps: u64 = match cset {
        CapSet::Effective => ((data.effective_s1 as u64) << 32) + data.effective_s0 as u64,
        CapSet::Inheritable => ((data.inheritable_s1 as u64) << 32) + data.inheritable_s0 as u64,
        CapSet::Permitted => ((data.permitted_s1 as u64) << 32) + data.permitted_s0 as u64,
        CapSet::Bounding | CapSet::Ambient => bail!("not a base set"),
    };
    let mut res = super::CapsHashSet::new();
    for c in super::Capability::iter_variants() {
        if (caps & c.bitmask()) != 0 {
            res.insert(c);
        }
    }
    return Ok(res);
}

pub fn set(tid: i32, cset: CapSet, value: super::CapsHashSet) -> Result<()> {
    let mut hdr = CapUserHeader {
        version: CAPS_V3,
        pid: tid,
    };
    let mut data: CapUserData = Default::default();
    try!(capget(&mut hdr, &mut data));
    {
        let (s1, s0) = match cset {
            CapSet::Effective => (&mut data.effective_s1, &mut data.effective_s0),
            CapSet::Inheritable => (&mut data.inheritable_s1, &mut data.inheritable_s0),
            CapSet::Permitted => (&mut data.permitted_s1, &mut data.permitted_s0),
            CapSet::Bounding | CapSet::Ambient => bail!("not a base set"),
        };
        *s1 = 0;
        *s0 = 0;
        for c in value {
            match c.index() {
                0...31 => {
                    *s0 |= c.bitmask() as u32;
                }
                32...63 => {
                    *s1 |= (c.bitmask() >> 32) as u32;
                }
                _ => bail!("overlarge cap index {}", c.index()),
            }
        }
    }
    try!(capset(&mut hdr, &data));
    return Ok(());
}

pub fn drop(tid: i32, cset: CapSet, cap: Capability) -> Result<()> {
    let mut caps = try!(read(tid, cset));
    if caps.remove(&cap) {
        try!(set(tid, cset, caps));
    };
    return Ok(());
}

pub fn raise(tid: i32, cset: CapSet, cap: Capability) -> Result<()> {
    let mut caps = try!(read(tid, cset));
    if caps.insert(cap) {
        try!(set(tid, cset, caps));
    };
    return Ok(());
}

#[derive(Debug)]
#[repr(C)]
struct CapUserHeader {
    // Linux capabilities version (runtime kernel support)
    version: u32,
    // Process ID (thread)
    pid: i32,
}

#[derive(Debug, Default, Clone)]
#[repr(C)]
struct CapUserData {
    effective_s0: u32,
    permitted_s0: u32,
    inheritable_s0: u32,
    effective_s1: u32,
    permitted_s1: u32,
    inheritable_s1: u32,
}