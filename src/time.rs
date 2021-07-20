#![allow(unused)] // File is very WIP
use crate::CStr;
use alloc::vec::Vec;
use core::{
    convert::TryInto,
    sync::atomic::{AtomicI64, Ordering::SeqCst},
};

const LEAPOCH: i64 = 946684800i64 + 86400 * (31 + 29);

const DAYS_PER_400Y: i64 = 365 * 400 + 97;
const DAYS_PER_100Y: i64 = 365 * 100 + 24;
const DAYS_PER_4Y: i64 = 365 * 4 + 1;

const DAYS_IN_MONTH: [u8; 12] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];

// Hacked up components from tzfile: https://github.com/nicolasbauw/rs-tzfile

fn parse_header(buffer: &[u8]) -> Header {
    let magic = read_u32(&buffer[0x00..=0x03]);
    if magic != MAGIC {
        panic!("Not a TZ file");
    }
    if buffer[4] != 50 {
        panic!("Unsupported TZ format");
    }
    let tzh_ttisgmtcnt = read_i32(&buffer[0x14..=0x17]) as usize;
    let tzh_ttisstdcnt = read_i32(&buffer[0x18..=0x1B]) as usize;
    let tzh_leapcnt = read_i32(&buffer[0x1C..=0x1F]) as usize;
    let tzh_timecnt = read_i32(&buffer[0x20..=0x23]) as usize;
    let tzh_typecnt = read_i32(&buffer[0x24..=0x27]) as usize;
    let tzh_charcnt = read_i32(&buffer[0x28..=0x2b]) as usize;
    // V2 format data start
    let s: usize = tzh_timecnt * 5
        + tzh_typecnt * 6
        + tzh_leapcnt * 8
        + tzh_charcnt
        + tzh_ttisstdcnt
        + tzh_ttisgmtcnt
        + 44;
    Header {
        tzh_ttisgmtcnt: read_i32(&buffer[s + 0x14..=s + 0x17]) as usize,
        tzh_ttisstdcnt: read_i32(&buffer[s + 0x18..=s + 0x1B]) as usize,
        tzh_leapcnt: read_i32(&buffer[s + 0x1C..=s + 0x1F]) as usize,
        tzh_timecnt: read_i32(&buffer[s + 0x20..=s + 0x23]) as usize,
        tzh_typecnt: read_i32(&buffer[s + 0x24..=s + 0x27]) as usize,
        tzh_charcnt: read_i32(&buffer[s + 0x28..=s + 0x2b]) as usize,
        v2_header_start: s,
    }
}

fn parse_data(buffer: &[u8], header: Header) -> Tzinfo {
    // Calculates fields lengths and indexes (Version 2 format)
    let tzh_timecnt_len: usize = header.tzh_timecnt * 9;
    let tzh_typecnt_len: usize = header.tzh_typecnt * 6;
    let tzh_leapcnt_len: usize = header.tzh_leapcnt * 12;
    let tzh_charcnt_len: usize = header.tzh_charcnt;
    let tzh_timecnt_end: usize = HEADER_LEN + header.v2_header_start + tzh_timecnt_len;
    let tzh_typecnt_end: usize = tzh_timecnt_end + tzh_typecnt_len;
    let tzh_leapcnt_end: usize = tzh_typecnt_end + tzh_leapcnt_len;
    let tzh_charcnt_end: usize = tzh_leapcnt_end + tzh_charcnt_len;

    // Extracting data fields
    let tzh_timecnt_data: Vec<i64> = buffer[HEADER_LEN + header.v2_header_start
        ..HEADER_LEN + header.v2_header_start + header.tzh_timecnt * 8]
        .chunks_exact(8)
        .map(|tt| read_i64(tt))
        .collect();

    let tzh_timecnt_indices: &[u8] =
        &buffer[HEADER_LEN + header.v2_header_start + header.tzh_timecnt * 8..tzh_timecnt_end];

    let abbrs = &buffer[tzh_leapcnt_end..tzh_charcnt_end];

    let tzh_typecnt: Vec<Ttinfo> = buffer[tzh_timecnt_end..tzh_typecnt_end]
        .chunks_exact(6)
        .map(|tti| {
            let offset = tti[5];
            let index = abbrs
                .iter()
                .take(offset as usize)
                .filter(|x| **x == b'\0')
                .count();
            Ttinfo {
                tt_gmtoff: read_i32(&tti[0..4]) as isize,
                tt_isdst: tti[4],
                tt_abbrind: index as u8,
            }
        })
        .collect();

    Tzinfo {
        tzh_timecnt_data,
        tzh_timecnt_indices: tzh_timecnt_indices.to_vec(),
        tzh_typecnt,
    }
}

fn read_i32(bytes: &[u8]) -> i32 {
    i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn read_u32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn read_i64(bytes: &[u8]) -> i64 {
    i64::from_be_bytes(bytes[..8].try_into().unwrap())
}

const MAGIC: u32 = 0x545A6966;
const HEADER_LEN: usize = 0x2C;

struct Header {
    tzh_ttisgmtcnt: usize,
    tzh_ttisstdcnt: usize,
    tzh_leapcnt: usize,
    tzh_timecnt: usize,
    tzh_typecnt: usize,
    tzh_charcnt: usize,
    v2_header_start: usize,
}

pub struct Tzinfo {
    /// transition times timestamps table
    tzh_timecnt_data: Vec<i64>,
    /// indices for the next field
    tzh_timecnt_indices: Vec<u8>,
    /// a struct containing UTC offset, daylight saving time, abbreviation index
    tzh_typecnt: Vec<Ttinfo>,
}

pub struct LocalTime {
    pub year: i32,
    pub month: i32,
    pub day_of_month: i32,
    pub hour: i32,
    pub minute: i32,
}

impl Tzinfo {
    pub fn new() -> Self {
        let zi = crate::utils::fs_read(CStr::from_bytes(&b"/etc/localtime\0"[..])).unwrap();
        let header = parse_header(&zi);
        parse_data(&zi, header)
    }

    fn gmt_offset(&self, time: i64) -> i64 {
        let best_idx = match self.tzh_timecnt_data.binary_search(&time) {
            Ok(i) => i - 1,
            Err(i) => i,
        };
        let idx = self.tzh_timecnt_indices[best_idx] as usize;
        self.tzh_typecnt[idx].tt_gmtoff as i64
    }

    // Ported from musl's localtime_r impl
    #[inline(never)]
    pub fn convert_to_localtime(&self, t: i64) -> LocalTime {
        let t = t + self.gmt_offset(t);

        let secs = t - LEAPOCH;
        let mut days = secs / 86400;
        let mut remsecs = secs % 86400;
        if remsecs < 0 {
            remsecs += 86400;
            days -= 1;
        }

        let mut qc_cycles = days / DAYS_PER_400Y;
        let mut remdays = days % DAYS_PER_400Y;
        if remdays < 0 {
            remdays += DAYS_PER_400Y;
            qc_cycles -= 1;
        }

        let mut c_cycles = remdays / DAYS_PER_100Y;
        if c_cycles == 4 {
            c_cycles -= 1
        }
        remdays -= c_cycles * DAYS_PER_100Y;

        let mut q_cycles = remdays / DAYS_PER_4Y;
        if q_cycles == 25 {
            q_cycles -= 1
        }
        remdays -= q_cycles * DAYS_PER_4Y;

        let mut remyears = remdays / 365;
        if remyears == 4 {
            remyears -= 1
        }
        remdays -= remyears * 365;

        let leap = remyears != 0 && ((q_cycles == 0) || c_cycles != 0);

        let mut years = remyears + 4 * q_cycles + 100 * c_cycles + 400 * qc_cycles;

        let mut months: i64 = 0;
        while DAYS_IN_MONTH[months as usize] as i64 <= remdays {
            remdays -= DAYS_IN_MONTH[months as usize] as i64;
            months += 1;
        }

        if months >= 10 {
            months -= 12;
            years += 1;
        }

        LocalTime {
            year: (years + 100).try_into().unwrap(),
            month: (months + 2).try_into().unwrap(),
            day_of_month: (remdays + 1).try_into().unwrap(),
            hour: (remsecs / 3600).try_into().unwrap(),
            minute: (remsecs / 60 % 60).try_into().unwrap(),
        }
    }
}

struct Ttinfo {
    tt_gmtoff: isize,
    tt_isdst: u8,
    tt_abbrind: u8,
}
