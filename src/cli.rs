use alloc::vec::Vec;
use veneer::CStr;

pub struct Options {
    pub inode: bool,
    pub block_size_is_kilobytes: bool,
    pub replace_unprintable_bytes: bool,
    pub reverse_sorting: bool,
    pub grid_sort_direction: SortDirection,
    pub display_size_in_blocks: bool,
    pub display_mode: DisplayMode,
    pub show_all: ShowAll,
    pub suffixes: Suffixes,
    pub follow_symlinks: FollowSymlinks,
    pub recurse: bool,
    pub sort_field: Option<SortField>,
    pub time_field: TimeField,
    pub list_directory_contents: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            inode: false,
            block_size_is_kilobytes: false,
            replace_unprintable_bytes: false,
            reverse_sorting: false,
            grid_sort_direction: SortDirection::Horizontal,
            display_size_in_blocks: false,
            display_mode: DisplayMode::Grid(0),
            show_all: ShowAll::No,
            suffixes: Suffixes::None,
            follow_symlinks: FollowSymlinks::Never,
            recurse: false,
            sort_field: Some(SortField::Name),
            time_field: TimeField::Modified,
            list_directory_contents: true,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeField {
    Modified,
    StatusModified,
    Created,
    Accessed,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortField {
    Name,
    Size,
    Time,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FollowSymlinks {
    Never,
    WhenExplicit,
    Always,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Suffixes {
    None,
    Directories,
    All,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ShowAll {
    Yes,
    No,
    Almost,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Horizontal,
    Vertical,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DisplayMode {
    Grid(usize),
    Long,
    SingleColumn,
    Stream,
}

macro_rules! error {
    ($($item:expr),+) => {
        use crate::output::Writable;
        let mut output = Vec::new();
        output.extend(b"fls: ");
        $(output.extend_from_slice($item.bytes_repr());)*
        let _ = veneer::syscalls::write(2, output.as_slice());
    };
}

pub fn parse_arguments(
    raw_args: &[CStr<'static>],
) -> Result<(Options, Vec<CStr<'static>>), crate::Error> {
    let mut args = Vec::with_capacity(raw_args.len());
    let mut switches = Vec::with_capacity(16);
    let mut args_valid = true;

    let mut hit_only_arg_marker = false;

    for arg in raw_args.iter().cloned().skip(1) {
        if arg.as_bytes() == b"--" {
            hit_only_arg_marker = true;
            continue;
        }
        if hit_only_arg_marker || arg.get(0) != Some(b'-') {
            args.push(arg);
        } else {
            // Somebody passed (or tried to pass) a GNU option. Ignore it.
            if arg.get(0) == Some(b'-') && arg.get(1) == Some(b'-') {
                args_valid = false;
                error!("unrecognized option \'", arg, "\'\n");
                continue;
            }
            if arg.get(0) == Some(b'-') {
                switches.extend(arg.as_bytes().iter().cloned().skip(1));
            }
        }
    }
    if args.is_empty() {
        args.push(CStr::from_bytes(b".\0"));
    }

    let mut opt = Options::default();
    for switch in switches.iter().cloned() {
        match switch {
            b'A' => {
                opt.show_all = ShowAll::Almost;
            }

            b'C' => {
                opt.display_mode = DisplayMode::Grid(0);
                opt.grid_sort_direction = SortDirection::Horizontal;
            }
            b'F' => {
                if !switches.contains(&b'H') && !switches.contains(&b'L') {
                    opt.follow_symlinks = FollowSymlinks::Never;
                }
                opt.suffixes = Suffixes::All;
            }
            b'H' => {
                opt.follow_symlinks = FollowSymlinks::WhenExplicit;
            }
            b'L' => {
                opt.follow_symlinks = FollowSymlinks::Always;
            }
            b'R' => {
                opt.recurse = true;
            }
            b'S' => {
                opt.sort_field = Some(SortField::Size);
            }
            b'a' => {
                opt.show_all = ShowAll::Yes;
            }
            b'c' => {
                opt.time_field = TimeField::StatusModified;
            }
            b'd' => {
                if !switches.contains(&b'H') && !switches.contains(&b'L') {
                    opt.follow_symlinks = FollowSymlinks::Never;
                }
                opt.list_directory_contents = false;
            }
            b'f' => {
                opt.sort_field = None;
                opt.show_all = ShowAll::Yes;
            }
            b'g' => {
                opt.display_mode = DisplayMode::Long; // TODO: Disable owner column
            }
            b'i' => {
                opt.inode = true;
            }
            b'k' => {
                opt.block_size_is_kilobytes = true;
            }
            b'l' => {
                opt.display_mode = DisplayMode::Long;
            }
            b'm' => {
                opt.display_mode = DisplayMode::Stream;
            }
            b'n' => {
                opt.display_mode = DisplayMode::Long; // TODO: Display UID/GID instead of name
            }
            b'p' => {
                opt.suffixes = Suffixes::Directories;
            }
            b'q' => {
                opt.replace_unprintable_bytes = true;
            }
            b'r' => {
                opt.reverse_sorting = true;
            }
            b's' => {
                opt.display_size_in_blocks = true;
            }
            b't' => {
                opt.sort_field = Some(SortField::Time);
            }
            b'u' => {
                opt.time_field = TimeField::Accessed;
            }
            b'x' => {
                opt.grid_sort_direction = SortDirection::Horizontal;
            }
            b'1' => match opt.display_mode {
                DisplayMode::Long => {}
                _ => opt.display_mode = DisplayMode::SingleColumn,
            },
            s => {
                error!(b"invalid option \'", &[s], b"\'\n");
                args_valid = false;
            }
        }
    }

    if !args_valid {
        Err(veneer::Error(-1))
    } else {
        Ok((opt, args))
    }
}
