use alloc::vec::Vec;
use core::convert::TryInto;

const LEAPOCH: i64 = 946684800i64 + 86400 * (31 + 29);

const DAYS_PER_400Y: i64 = 365 * 400 + 97;
const DAYS_PER_100Y: i64 = 365 * 100 + 24;
const DAYS_PER_4Y: i64 = 365 * 4 + 1;

const DAYS_IN_MONTH: [u8; 12] = [31, 30, 31, 30, 31, 31, 30, 31, 30, 31, 31, 29];

trait SliceExt: Sized {
    fn read_u32_be(&mut self) -> Option<u32>;
    fn read(&mut self, n: usize) -> Option<Self>;
}

impl SliceExt for &[u8] {
    fn read(&mut self, n: usize) -> Option<Self> {
        if self.len() < n {
            None
        } else {
            let (head, tail) = self.split_at(n);
            *self = tail;
            Some(head)
        }
    }

    fn read_u32_be(&mut self) -> Option<u32> {
        if self.len() < 4 {
            return None;
        }
        let (head, tail) = self.split_at(4);
        *self = tail;
        Some(u32::from_be_bytes(head.try_into().unwrap()))
    }
}

fn parse_header(buffer: &[u8]) -> Option<Header> {
    // We only support version 2, so validate the magic bytes and version all at once.
    if buffer.get(..5)? != b"TZif2" {
        return None;
    }
    let mut header = buffer.get(0x14..=0x2b)?;
    let tzh_ttisgmtcnt = header.read_u32_be()?;
    let tzh_ttisstdcnt = header.read_u32_be()?;
    let tzh_leapcnt = header.read_u32_be()?;
    let tzh_timecnt = header.read_u32_be()?;
    let tzh_typecnt = header.read_u32_be()?;
    let tzh_charcnt = header.read_u32_be()?;
    // V2 format data start
    let s = (tzh_timecnt * 5
        + tzh_typecnt * 6
        + tzh_leapcnt * 8
        + tzh_charcnt
        + tzh_ttisstdcnt
        + tzh_ttisgmtcnt
        + 44) as usize;
    let mut header = buffer.get(s + 0x14..=s + 0x2b)?;
    let _ignored_fields = header.read(12)?;
    Some(Header {
        tzh_timecnt: header.read_u32_be()?,
        tzh_typecnt: header.read_u32_be()?,
        v2_header_start: s,
    })
}

fn parse_data(buffer: &[u8], header: Header) -> Option<Tzinfo> {
    let mut buffer = buffer.get(HEADER_LEN + header.v2_header_start..)?;

    // Extracting data fields
    let tzh_timecnt_data: Vec<i64> = buffer
        .read(header.tzh_timecnt as usize * 8)?
        .chunks_exact(8)
        .map(read_i64)
        .collect();

    let tzh_timecnt_indices = buffer.read(header.tzh_timecnt as usize)?.to_vec();

    let gmt_offsets: Vec<_> = buffer
        .read(header.tzh_typecnt as usize * 6)?
        .chunks_exact(6)
        .map(|tti| read_i32(&tti[..4]))
        .collect();

    Some(Tzinfo {
        tzh_timecnt_data,
        tzh_timecnt_indices,
        gmt_offsets,
    })
}

fn read_i32(bytes: &[u8]) -> i32 {
    i32::from_be_bytes(bytes[..4].try_into().unwrap())
}

fn read_i64(bytes: &[u8]) -> i64 {
    i64::from_be_bytes(bytes[..8].try_into().unwrap())
}

const HEADER_LEN: usize = 0x2C;

struct Header {
    //tzh_ttisgmtcnt: u32,
    //tzh_ttisstdcnt: u32,
    //tzh_leapcnt: u32,
    tzh_timecnt: u32,
    tzh_typecnt: u32,
    //tzh_charcnt: u32,
    v2_header_start: usize,
}

pub struct Tzinfo {
    /// transition times timestamps table
    tzh_timecnt_data: Vec<i64>,
    /// indices for the next field
    tzh_timecnt_indices: Vec<u8>,
    gmt_offsets: Vec<i32>,
}

pub struct LocalTime {
    pub year: i32,
    pub month: i32,
    pub day_of_month: i32,
    pub hour: i32,
    pub minute: i32,
}

impl Tzinfo {
    #[inline(never)]
    pub fn new(zi: &[u8]) -> Self {
        let header = parse_header(zi).unwrap();
        parse_data(zi, header).unwrap()
    }

    fn gmt_offset(&self, time: i64) -> i64 {
        let best_idx = match self.tzh_timecnt_data.binary_search(&time) {
            Ok(i) => i,
            Err(i) => i + 1,
        };
        let idx = *self
            .tzh_timecnt_indices
            .get(best_idx)
            .or_else(|| self.tzh_timecnt_indices.last())
            .unwrap_or(&0) as usize;
        *self.gmt_offsets.get(idx).unwrap_or(&0) as i64
    }

    // Ported from musl's localtime_r impl, src/time/__secs_to_tm.c
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
            c_cycles -= 1;
        }
        remdays -= c_cycles * DAYS_PER_100Y;

        let mut q_cycles = remdays / DAYS_PER_4Y;
        if q_cycles == 25 {
            q_cycles -= 1;
        }
        remdays -= q_cycles * DAYS_PER_4Y;

        let mut remyears = remdays / 365;
        if remyears == 4 {
            remyears -= 1;
        }
        remdays -= remyears * 365;

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
