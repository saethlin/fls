use crate::output::OutputBuffer;
use alloc::vec::Vec;
use veneer::{env, syscalls::*, CStr, Error};

macro_rules! error {
    ($($item:expr),+) => {
        {
            use crate::output::Writable;
            let mut buf = crate::output::OutputBuffer::to_fd(2);
            $($item.write(&mut buf);)*
        }
    };
}

pub struct App {
    pub print_inode: bool,
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
    pub out: OutputBuffer,
    pub convert_id_to_name: bool,
    pub print_owner: bool,
    pub print_group: bool,
    pub color: Color,

    pub args: Args,

    etc_passwd: &'static [u8],
    uid_names: Vec<(u32, (usize, usize))>,
    etc_group: &'static [u8],
    gid_names: Vec<(u32, (usize, usize))>,

    tzinfo: Option<crate::time::Tzinfo>,

    pub needs_details: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Always,
    Auto,
    Never,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TimeField {
    Accessed,
    Modified,
    StatusChanged,
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

impl App {
    pub const DEFAULT: Self = Self {
        print_inode: false,
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
        convert_id_to_name: true,
        print_owner: true,
        print_group: true,
        color: Color::Auto,
        out: OutputBuffer::to_fd(1),
        args: Args::None,
        uid_names: Vec::new(),
        gid_names: Vec::new(),
        etc_passwd: &[],
        etc_group: &[],
        needs_details: false,
        tzinfo: None,
    };

    #[inline(never)]
    pub fn init(&mut self) -> Result<(), Error> {
        let app = self;

        app.args = Args::new();

        #[allow(non_snake_case)]
        let mut switches_contains_H_or_L = false;
        for arg in env::args().skip(1) {
            if arg.get(0) == Some(b'-') {
                for switch in arg.as_bytes().iter().copied().skip(1) {
                    if switch == b'H' || switch == b'L' {
                        switches_contains_H_or_L = true;
                        break;
                    }
                }
            }
        }

        let mut print_version = false;
        let mut args_valid = true;

        for arg in env::args().skip(1) {
            if arg.as_bytes() == b"--" {
                break;
            } else if arg.as_bytes().starts_with(b"--") {
                // Things like --color=always
                match arg.as_bytes() {
                    b"--color=never" => app.color = Color::Never,
                    b"--color=auto" => app.color = Color::Auto,
                    b"--color=always" => app.color = Color::Always,
                    b"--version" => print_version = true,
                    _ => {
                        error!("unrecognized option \'", arg, "\'\n");
                        args_valid = false;
                    }
                }
            } else if arg.get(0) == Some(b'-') {
                // Things like -R
                for switch in arg.as_bytes().iter().copied().skip(1) {
                    if app.handle_switch(switches_contains_H_or_L, switch).is_err() {
                        args_valid = false;
                    }
                }
            }
        }

        if !args_valid {
            exit(1);
        }

        if print_version {
            app.out
                .write(concat!("fls ", env!("CARGO_PKG_VERSION"), "\n").as_bytes())
                .flush();
            exit(0);
        }

        let terminal_width = winsize().ok().map(|d| d.ws_col as usize);

        match (terminal_width, app.display_mode) {
            (Some(width), DisplayMode::Grid(_)) => app.display_mode = DisplayMode::Grid(width),
            (None, DisplayMode::Grid(_)) => app.display_mode = DisplayMode::SingleColumn,
            _ => {}
        }

        if terminal_width.is_none() && app.color == Color::Auto {
            app.color = Color::Never;
        }
        if app.color == Color::Never {
            app.out.color = false;
        }

        if app.display_mode == DisplayMode::Long {
            Self::init_id_map(
                &b"/etc/passwd\0"[..],
                &mut app.etc_passwd,
                &mut app.uid_names,
            )?;
            Self::init_id_map(&b"/etc/group\0"[..], &mut app.etc_group, &mut app.gid_names)?;
            let zi = crate::utils::fs_read(CStr::from_bytes(&b"/etc/localtime\0"[..])).unwrap();
            app.tzinfo = Some(crate::time::Tzinfo::new(&zi));
        }

        app.needs_details = app.display_mode == DisplayMode::Long
            || app.sort_field == Some(SortField::Time)
            || app.sort_field == Some(SortField::Size)
            || app.display_size_in_blocks;

        Ok(())
    }

    #[allow(non_snake_case)]
    fn handle_switch(&mut self, switches_contains_H_or_L: bool, switch: u8) -> Result<(), ()> {
        let app = self;
        match switch {
            b'A' => {
                app.show_all = ShowAll::Almost;
            }
            b'C' => {
                app.display_mode = DisplayMode::Grid(0);
                app.grid_sort_direction = SortDirection::Vertical;
            }
            b'F' => {
                if !switches_contains_H_or_L {
                    app.follow_symlinks = FollowSymlinks::Never;
                }
                app.suffixes = Suffixes::All;
            }
            b'H' => {
                app.follow_symlinks = FollowSymlinks::WhenExplicit;
            }
            b'L' => {
                app.follow_symlinks = FollowSymlinks::Always;
            }
            b'R' => {
                app.recurse = true;
            }
            b'S' => {
                app.sort_field = Some(SortField::Size);
            }
            b'a' => {
                app.show_all = ShowAll::Yes;
            }
            b'c' => {
                app.time_field = TimeField::StatusChanged;
                app.sort_field = Some(SortField::Time);
            }
            b'd' => {
                if !switches_contains_H_or_L {
                    app.follow_symlinks = FollowSymlinks::Never;
                }
                app.list_directory_contents = false;
            }
            b'f' => {
                app.sort_field = None;
                app.show_all = ShowAll::Yes;
            }
            b'g' => {
                app.display_mode = DisplayMode::Long;
                app.print_owner = false;
            }
            b'i' => {
                app.print_inode = true;
            }
            b'k' => {
                app.block_size_is_kilobytes = true;
            }
            b'l' => {
                app.display_mode = DisplayMode::Long;
            }
            b'm' => {
                app.display_mode = DisplayMode::Stream;
            }
            b'n' => {
                app.display_mode = DisplayMode::Long;
                app.convert_id_to_name = false;
            }
            b'o' => {
                app.display_mode = DisplayMode::Long;
                app.print_group = false;
            }
            b'p' => {
                app.suffixes = Suffixes::Directories;
            }
            b'q' => {
                app.replace_unprintable_bytes = true;
                unimplemented!("-q is not yet implemented");
            }
            b'r' => {
                app.reverse_sorting = true;
            }
            b's' => {
                app.display_size_in_blocks = true;
            }
            b't' => {
                app.time_field = TimeField::Modified;
                app.sort_field = Some(SortField::Time);
            }
            b'u' => {
                app.time_field = TimeField::Accessed;
                app.sort_field = Some(SortField::Time);
            }
            b'x' => {
                app.grid_sort_direction = SortDirection::Horizontal;
                unimplemented!("-x is not yet implemented");
            }
            b'1' => match app.display_mode {
                DisplayMode::Long => {}
                _ => app.display_mode = DisplayMode::SingleColumn,
            },
            s => {
                error!("invalid option \'", s, "\'\n");
                return Err(());
            }
        }
        Ok(())
    }

    pub fn convert_to_localtime(&self, time: i64) -> crate::time::LocalTime {
        self.tzinfo.as_ref().unwrap().convert_to_localtime(time)
    }

    #[inline(never)]
    fn init_id_map(
        path: &'static [u8],
        slab: &mut &'static [u8],
        map: &mut Vec<(u32, (usize, usize))>,
    ) -> Result<(), Error> {
        use crate::utils::atoi;
        let contents = crate::utils::fs_read(CStr::from_bytes(path))?;
        *slab = alloc::boxed::Box::leak(contents.into_boxed_slice());

        let mut offset = 0;
        for line in slab.split(|b| *b == b'\n') {
            if line.is_empty() {
                offset += line.len() + 1;
                continue;
            }
            let mut it = line.split(|b| *b == b':');
            let name = it.next().unwrap();
            let _passwd = it.next().unwrap();
            let uid = atoi(it.next().unwrap()) as u32;

            map.push((uid, (offset, offset + name.len())));

            offset += line.len() + 1;
        }
        Ok(())
    }

    pub fn getpwuid(&self, uid: u32) -> &'static [u8] {
        self.uid_names
            .iter()
            .find(|(id, _)| *id == uid)
            .map(|(_id, (start, end))| &self.etc_passwd[*start..*end])
            .unwrap_or_default()
    }

    pub fn getgrgid(&self, gid: u32) -> &'static [u8] {
        self.gid_names
            .iter()
            .find(|(id, _)| *id == gid)
            .map(|(_id, (start, end))| &self.etc_group[*start..*end])
            .unwrap_or_default()
    }

    pub fn convert_status(&self, status: libc::stat64) -> crate::Status {
        use TimeField::*;
        crate::Status {
            device: status.st_dev,
            links: status.st_nlink,
            mode: status.st_mode,
            size: status.st_size,
            blocks: status.st_blocks,
            block_size: status.st_blksize,
            uid: status.st_uid,
            gid: status.st_gid,
            inode: status.st_ino,
            time: match self.time_field {
                Accessed => status.st_atime,
                Modified => status.st_mtime,
                StatusChanged => status.st_ctime,
            },
        }
    }
}

#[derive(Clone, Copy)]
pub enum Args {
    None,
    One,
    Multiple,
}

struct ArgsIter<I> {
    iter: I,
    hit_only_arg_marker: bool,
    any_args: bool,
}

impl Args {
    fn new() -> Self {
        let mut args = 0usize;
        let mut hit_only_arg_marker = false;
        for arg in veneer::env::args().skip(1) {
            if arg.as_bytes() == b"--" {
                hit_only_arg_marker = true;
                continue;
            }
            if hit_only_arg_marker || arg.as_bytes().first() != Some(&b'-') {
                args += 1;
            }
            if args > 1 {
                return Args::Multiple;
            }
        }
        // We should break out above to handle the more-than-one case faster
        match args {
            0 => Args::None,
            1 => Args::One,
            _ => unreachable!(),
        }
    }

    pub fn iter(self) -> impl Iterator<Item = CStr<'static>> {
        ArgsIter {
            iter: veneer::env::args().skip(1),
            hit_only_arg_marker: false,
            any_args: false,
        }
    }
}

impl<I> Iterator for ArgsIter<I>
where
    I: Iterator<Item = CStr<'static>>,
{
    type Item = CStr<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        for arg in self.iter.by_ref() {
            if self.hit_only_arg_marker || arg.as_bytes().first() != Some(&b'-') {
                self.any_args = true;
                return Some(arg);
            }
            if arg.as_bytes() == b"--" {
                self.hit_only_arg_marker = true;
                continue;
            }
        }
        if !self.any_args {
            self.any_args = true;
            Some(CStr::from_bytes(b".\0"))
        } else {
            None
        }
    }
}
