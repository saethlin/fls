use crate::atoi;
use crate::output::OutputBuffer;
use crate::veneer;
use crate::veneer::CStr;
use alloc::vec::Vec;

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

    pub args: Vec<CStr<'static>>,

    etc_passwd: &'static [u8],
    uid_names: Vec<(u32, (usize, usize))>,
    etc_group: &'static [u8],
    gid_names: Vec<(u32, (usize, usize))>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Always,
    Auto,
    Never,
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

impl App {
    pub fn from_arguments(raw_args: Vec<CStr<'static>>) -> Result<Self, crate::Error> {
        let mut args = Vec::with_capacity(raw_args.len());
        let mut switches = Vec::with_capacity(16);
        let mut named_arguments = Vec::new();
        let mut args_valid = true;

        let mut hit_only_arg_marker = false;

        for arg in raw_args.into_iter().skip(1) {
            if arg.as_bytes() == b"--" {
                hit_only_arg_marker = true;
            } else if hit_only_arg_marker {
                args.push(arg);
            // Things like --color=always
            } else if arg.as_bytes().starts_with(b"--") {
                named_arguments.push(arg);
            // Things like -R
            } else if arg.get(0) == Some(b'-') {
                switches.extend(arg.as_bytes().iter().copied().skip(1));
            } else {
                args.push(arg);
            }
        }
        if args.is_empty() {
            args.push(CStr::from_bytes(b".\0"));
        }

        let mut app = App {
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
            color: Color::Always,
            out: OutputBuffer::terminal(),
            args,
            uid_names: Vec::new(),
            gid_names: Vec::new(),
            etc_passwd: &[],
            etc_group: &[],
        };

        for arg in named_arguments.iter().map(CStr::as_bytes) {
            if let Some(p) = arg.iter().position(|b| *b == b'=') {
                let (name, value) = arg.split_at(p);
                let name = &name[2..]; // Trim off the --
                let value = &value[1..]; // Trim off the =
                if name == b"color" {
                    match value {
                        b"always" => app.color = Color::Always,
                        b"auto" => app.color = Color::Auto,
                        b"never" => app.color = Color::Never,
                        _ => {
                            error!("invalid argument \'", value, "\' for \'", name, "'\n");
                        }
                    }
                } else {
                    error!("unrecognized option \'", arg, "\'\n");
                }
            } else {
                error!("unrecognized option \'", arg, "\'\n");
            }
        }

        for switch in switches.iter().copied() {
            match switch {
                b'A' => {
                    app.show_all = ShowAll::Almost;
                }

                b'C' => {
                    app.display_mode = DisplayMode::Grid(0);
                    app.grid_sort_direction = SortDirection::Horizontal;
                }
                b'F' => {
                    if !switches.contains(&b'H') && !switches.contains(&b'L') {
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
                    app.time_field = TimeField::StatusModified;
                    app.sort_field = Some(SortField::Time);
                }
                b'd' => {
                    if !switches.contains(&b'H') && !switches.contains(&b'L') {
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
                }
                b'1' => match app.display_mode {
                    DisplayMode::Long => {}
                    _ => app.display_mode = DisplayMode::SingleColumn,
                },
                s => {
                    error!("invalid option \'", s, "\'\n");
                    args_valid = false;
                }
            }
        }

        let terminal_width = veneer::syscalls::winsize().ok().map(|d| d.ws_col as usize);

        match (terminal_width, app.display_mode) {
            (Some(width), DisplayMode::Grid(_)) => app.display_mode = DisplayMode::Grid(width),
            (None, DisplayMode::Grid(_)) => app.display_mode = DisplayMode::SingleColumn,
            _ => {}
        }

        app.out = if terminal_width.is_some() {
            OutputBuffer::terminal()
        } else {
            OutputBuffer::pipe()
        };

        if !args_valid {
            return Err(veneer::Error(-1));
        }

        use crate::veneer::syscalls::*;
        // Initialize name and group id lookup
        let fd = open(
            CStr::from_bytes(&b"/etc/passwd\0"[..]),
            OpenFlags::RDONLY,
            OpenMode::empty(),
        )?;
        let len = fstat(fd)?.st_size as usize;
        let mut contents = alloc::vec![0; len];
        read(fd, &mut contents)?;
        close(fd)?;
        app.etc_passwd = alloc::boxed::Box::leak(contents.into_boxed_slice());

        let mut offset = 0;
        for line in app.etc_passwd.split(|b| *b == b'\n') {
            if line.is_empty() {
                offset += line.len() + 1;
                continue;
            }
            let mut it = line.split(|b| *b == b':');
            let name = it.next().unwrap();
            let _passwd = it.next().unwrap();
            let uid = atoi(it.next().unwrap()) as u32;

            app.uid_names.push((uid, (offset, offset + name.len())));

            offset += line.len() + 1;
        }

        // Initialize name and group id lookup
        let fd = open(
            CStr::from_bytes(&b"/etc/group\0"[..]),
            OpenFlags::RDONLY,
            OpenMode::empty(),
        )?;
        let len = fstat(fd)?.st_size as usize;
        let mut contents = alloc::vec![0; len];
        read(fd, &mut contents)?;
        close(fd)?;

        app.etc_group = alloc::boxed::Box::leak(contents.into_boxed_slice());

        let mut offset = 0;
        for line in app.etc_group.split(|b| *b == b'\n') {
            if line.is_empty() {
                offset += line.len() + 1;
                continue;
            }
            let mut it = line.split(|b| *b == b':');
            let name = it.next().unwrap().to_vec();
            let _passwd = it.next().unwrap();
            let gid = atoi(it.next().unwrap()) as u32;

            app.gid_names.push((gid, (offset, offset + name.len())));
            offset += line.len() + 1;
        }

        Ok(app)
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
            links: status.st_nlink as u64,
            mode: status.st_mode,
            size: status.st_size,
            blocks: status.st_blocks,
            block_size: status.st_blksize as i64,
            uid: status.st_uid,
            gid: status.st_gid,
            inode: status.st_ino,
            time: match self.time_field {
                Accessed => status.st_atime,
                Created => status.st_ctime,
                Modified => status.st_mtime,
                StatusModified => status.st_mtime,
            },
        }
    }
}
