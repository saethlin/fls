use crate::output::{OutputBuffer, Writable};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Reset,
    Red,
    Blue,
    Magenta,
    Cyan,
    White,
    Gray,
    RedBold,
    GreenBold,
    YellowBold,
    BlueBold,
    MagentaBold,
    CyanBold,
    Fixed(u8),
}

impl Style {
    #[inline(never)]
    pub fn write_to(self, out: &mut OutputBuffer) {
        use Style::*;
        let bytes = match self {
            Reset => &b"\x1B[m"[..],
            Red => &b"\x1B[0;31m"[..],
            Blue => &b"\x1B[0;34m"[..],
            Magenta => &b"\x1B[0;35m"[..],
            Cyan => &b"\x1B[0;36m"[..],
            White => &b"\x1B[0;37m"[..],
            Gray => &b"\x1B[0;38;5;244m"[..],
            RedBold => &b"\x1B[1;31m"[..],
            GreenBold => &b"\x1B[1;32m"[..],
            YellowBold => &b"\x1B[1;33m"[..],
            BlueBold => &b"\x1B[1;34m"[..],
            MagentaBold => &b"\x1B[1;35m"[..],
            CyanBold => &b"\x1B[1;36m"[..],
            Fixed(c) => {
                out.write(&b"\x1B[0;38;5;"[..]);
                u64::from(c).write(out);
                out.push(b'm');
                return;
            }
        };
        out.write(bytes);
    }
}
