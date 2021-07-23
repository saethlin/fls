static IMAGE: &[&str] = &[
    "png", "jpeg", "jpg", "gif", "bmp", "tiff", "tif", "ppm", "pgm", "pbm", "pnm", "webp", "raw",
    "arw", "svg", "stl", "eps", "dvi", "ps", "cbr", "jpf", "cbz", "xpm", "ico", "cr2", "orf",
    "nef", "heif",
];

static VIDEO: &[&str] = &[
    "avi", "flv", "m2v", "m4v", "mkv", "mov", "mp4", "mpeg", "mpg", "ogm", "ogv", "vo", "wmv",
    "webm", "m2ts", "heic",
];

static MUSIC: &[&str] = &["aac", "m4a", "mp3", "ogg", "wma", "mka", "opus"];

static LOSSLESS: &[&str] = &["alac", "ape", "flac", "wav"];

static CRYPTO: &[&str] = &["asc", "enc", "gpg", "pgp", "sig", "signature", "pfx", "p12"];

static DOCUMENT: &[&str] = &[
    "djvu", "doc", "docx", "dvi", "eml", "eps", "fotd", "key", "keynote", "numbers", "odp", "odt",
    "pages", "pdf", "ppt", "pptx", "rtf", "xls", "xlsx",
];

static COMPRESSED: &[&str] = &[
    "zip", "tar", "Z", "z", "gz", "bz2", "a", "ar", "7z", "iso", "dmg", "tc", "rar", "par", "tgz",
    "xz", "txz", "lz", "tlz", "lzma", "de", "rpm", "zst",
];

static TEMP: &[&str] = &["tmp", "swp", "swo", "swn", "bak", "bk"];

static STYLES: &[(&[&str], &str)] = &[
    (TEMP, "Style::Fixed(244)"),
    (IMAGE, "Style::Fixed(133)"),
    (VIDEO, "Style::Fixed(135)"),
    (MUSIC, "Style::Fixed(92)"),
    (LOSSLESS, "Style::Fixed(93)"),
    (CRYPTO, "Style::Fixed(109)"),
    (DOCUMENT, "Style::Fixed(105)"),
    (COMPRESSED, "Style::Red"),
];

use std::io::Write;

fn main() {
    #[cfg(feature = "no-libc")]
    println!("cargo:rustc-link-arg=-nostartfiles");
    #[cfg(feature = "no-libc")]
    println!("cargo:rustc-link-arg=-nodefaultlibs");

    #[cfg(not(feature = "no-libc"))]
    println!("cargo:rustc-link-lib=c");

    let mut all_styles = Vec::new();
    for (extensions, style) in STYLES {
        for ext in *extensions {
            all_styles.push((ext, style));
        }
    }
    all_styles.sort_by(|a, b| a.0.cmp(b.0));

    let path = std::path::Path::new(&std::env::var("OUT_DIR").unwrap()).join("codegen.rs");
    let mut file = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
    writeln!(file, "static EXTENSION_STYLES: &[(&[u8], Style)] = &[").unwrap();

    for (ext, sty) in &all_styles {
        writeln!(file, "(b\"{}\", {}),", ext, sty).unwrap();
    }
    writeln!(file, "];").unwrap();
}
