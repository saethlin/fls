use crate::directory::{Directory, Style};
use crate::Error;
use alloc::vec;
use alloc::vec::Vec;

pub fn write_grid(
    root: &[u8],
    out: &mut BufferedStdout,
    terminal_width: usize,
) -> Result<(), Error> {
    let mut entries = Vec::new();
    let dir = match Directory::open(root) {
        Ok(d) => d,
        Err(2) => return out.write_all(b"path doesn't exist (ENOENT)\n"),
        Err(13) => return out.write_all(b"access denied (EACCES)\n"),
        Err(20) => return out.write_all(b"path isn't a directory (ENOTDIR)\n"),
        Err(e) => return Err(e)?,
    };
    for e in dir.iter()?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    let mut columns = 1;
    let mut widths = match entries.iter().map(|e| e.name().len()).max() {
        Some(max_len) => vec![max_len],
        None => return Ok(()), // No entries, nothing to do here
    };
    for n_columns in 1..=entries.len() {
        let mut tmp_widths = vec![0; n_columns];
        let entries_per_column =
            entries.len() / n_columns + ((entries.len() % n_columns != 0) as usize);
        for (c, column) in entries.chunks(entries_per_column).enumerate() {
            tmp_widths[c] = column
                .iter()
                .map(|entry| entry.name().len())
                .max()
                .unwrap_or(1)
                + 2;
        }
        if tmp_widths.iter().sum::<usize>() > terminal_width as usize {
            break;
        } else {
            columns = n_columns;
            widths = tmp_widths;
        }
    }

    let rows = entries.len() / columns + ((entries.len() % columns != 0) as usize);

    let mut previous_style = Style::Regular;
    out.write_all(previous_style.as_ref())?;

    for r in 0..rows {
        for (c, width) in widths.iter().enumerate() {
            let e = match entries.get(c * rows + r) {
                Some(e) => e,
                None => break,
            };

            let style = e.style()?;
            if style != previous_style {
                out.write_all(style.as_ref())?;
                previous_style = style;
            }
            out.write_all(e.name())?;

            for _ in 0..width - e.name().len() {
                out.write_all(b" ")?;
            }
        }
        out.write_all(b"\n")?;
    }
    out.write_all(Style::Regular.as_ref())?;

    Ok(())
}

pub fn write_single_column(root: &[u8], out: &mut BufferedStdout) -> Result<(), Error> {
    let mut entries = Vec::new();
    let dir = Directory::open(root)?;
    for e in dir.iter()?.filter(|e| e.name().get(0) != Some(&b'.')) {
        entries.push(e)
    }

    entries.sort_by(|a, b| {
        a.name()
            .iter()
            .map(u8::to_ascii_lowercase)
            .cmp(b.name().iter().map(u8::to_ascii_lowercase))
    });

    for e in entries {
        out.write_all(e.name())?;
        out.write_all(b"\n")?;
    }
    Ok(())
}

pub struct BufferedStdout {
    buf: arrayvec::ArrayVec<[u8; 4096]>,
}

impl BufferedStdout {
    pub fn new() -> Self {
        Self {
            buf: arrayvec::ArrayVec::new(),
        }
    }

    pub fn write_all(&mut self, bytes: &[u8]) -> Result<(), Error> {
        for b in bytes {
            if let Err(_) = self.buf.try_push(*b) {
                write_to_stdout(&self.buf)?;
                self.buf.clear();
                self.buf.push(*b);
            }
        }
        Ok(())
    }
}

impl Drop for BufferedStdout {
    fn drop(&mut self) {
        let _ = write_to_stdout(&self.buf);
    }
}

fn write_to_stdout(bytes: &[u8]) -> Result<(), i32> {
    unsafe {
        let mut bytes_written = 0;
        while bytes_written < bytes.len() {
            let ret = syscall::syscall!(
                WRITE,
                1,
                bytes.as_ptr().offset(bytes_written as isize),
                bytes.len() - bytes_written
            ) as i32;
            if ret < 0 {
                return Err(-ret);
            } else {
                bytes_written += ret as usize;
            }
        }
    }
    Ok(())
}
