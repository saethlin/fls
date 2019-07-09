#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Reset,
    Red,
    Green,
    Yellow,
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
    WhiteBold,
}

impl Style {
    pub fn to_bytes(self) -> &'static [u8] {
        use Style::*;
        match self {
            Reset => b"\x1B[m",
            Red => b"\x1B[0;31m",
            Green => b"\x1B[0;32m",
            Yellow => b"\x1B[0;33m",
            Blue => b"\x1B[0;34m",
            Magenta => b"\x1B[0;35m",
            Cyan => b"\x1B[0;36m",
            White => b"\x1B[0;37m",
            Gray => b"\x1B[0;38;5;244m",
            RedBold => b"\x1B[1;31m",
            GreenBold => b"\x1B[1;32m",
            YellowBold => b"\x1B[1;33m",
            BlueBold => b"\x1B[1;34m",
            MagentaBold => b"\x1B[1;35m",
            CyanBold => b"\x1B[1;36m",
            WhiteBold => b"\x1B[1;37m",
        }
    }
}
