/*!
 * Probes a GIF to detect if it _actually_ has transparent pixels, and accumulates misc data while we're at it.
 *
 * The final algorithm for this is lightweight and simple, and only requires reading the first frame in full.
 *
 * For the common GIF, there are only two ways to obtain real transparency. It can either have transparent
 * pixels in the first frame, or clears parts of the image using the `Background` dispose method after a frame.
 * Technically, the `Background` dispose method is supposed to fill in the frame with the background color,
 * but everyone ignores that.
 *
 * Therefore, it is not necessary to actually accumulate and dispose pixels values.
 *
 * Note: This binary intentionally has bad error handling. It either succeeds or it doesn't.
 * Any file that fails to process should be considered invalid.
 *
 * Usage:
 * ```
 * gif_probe
 *     [-l max_duration_in_ms]
 *     [-d max_pixels]
 *     [-m max_memory_in_bytes]
 *      -i path/file.gif
 * ```
 *
 * Or pass `-i -` to read from stdin, through this currently does not work in Windows terminals.
 *
 * Example output:
 * ```json
 * {
 *   "alpha": false,
 *   "max_colors": 256,
 *   "duration": 267,
 *   "frames": 40,
 *   "width": 480,
 *   "height": 270
 * }
 * ```
 */

use std::{fs::File, io::BufReader, num::NonZeroU64, path::PathBuf};

use gif::{ColorOutput, DecodeOptions, DisposalMethod, MemoryLimit};

pub struct GifProbe {
    pub alpha: bool,
    pub max_colors: u16,
    pub duration: u64,
    pub frames: u64,
    pub width: u16,
    pub height: u16,
}

///
#[derive(argh::FromArgs)]
pub struct Arguments {
    /// stop processing after this duration is reached
    #[argh(option, short = 'j')]
    pub max_duration: Option<u64>,

    /// panic if the given number of pixels is more than this
    #[argh(option, short = 'd')]
    pub max_pixels: Option<u64>,

    /// don't decode if the decoder would allocate more than this (in bytes)
    #[argh(option, short = 'm')]
    pub max_memory: Option<NonZeroU64>,

    /// path to the GIF file, or `-` to read from stdin
    #[argh(option, short = 'i')]
    pub input: PathBuf,
}

fn main() {
    let args: Arguments = argh::from_env();

    let f = BufReader::new(match args.input.as_path() {
        path if path.as_os_str() == "-" => {
            // try to unbuffer the buffered stream here for windows and unix
            #[cfg(windows)]
            let file = unsafe {
                use std::os::windows::io::{AsRawHandle, FromRawHandle};
                File::from_raw_handle(std::io::stdin().as_raw_handle())
            };

            #[cfg(unix)]
            let file = unsafe {
                use std::os::fd::{AsRawFd, FromRawFd};
                File::from_raw_fd(std::io::stdin().as_raw_fd())
            };

            // can't unbuffer, will be double-buffered, oh well
            #[cfg(not(any(windows, unix)))]
            let file = Box::new(std::io::stdin().lock()) as Box<dyn std::io::Read>;

            file
        }
        #[cfg(any(windows, unix))]
        path => File::open(path).expect("To open the file"),
        #[cfg(not(any(windows, unix)))]
        path => Box::new(File::open(path).expect("To open the file")) as Box<dyn std::io::Read>,
    });

    let mut opts = DecodeOptions::new();

    opts.set_color_output(ColorOutput::Indexed);
    opts.check_frame_consistency(true);
    opts.allow_unknown_blocks(false);
    opts.check_lzw_end_code(false);
    opts.set_memory_limit(MemoryLimit::Bytes(
        // user-specified or 20 MiB
        args.max_memory
            // SAFETY: Obviously non-zero
            .unwrap_or(unsafe { NonZeroU64::new_unchecked(1024 * 1024 * 20) }),
    ));

    let mut decoder = opts.read_info(f).expect("To read the GIF");

    let mut probe = GifProbe {
        width: decoder.width(),
        height: decoder.height(),
        alpha: false,
        max_colors: 0,
        duration: 0,
        frames: 0,
    };

    if matches!(args.max_pixels, Some(m) if m < (probe.width as u64 * probe.height as u64)) {
        panic!("Image too large!");
    }

    if let Some(p) = decoder.global_palette() {
        probe.max_colors = u16::try_from(p.len() / 3).expect("colors to u16");
    }

    if let Some(frame) = decoder.read_next_frame().expect("to read the first frame") {
        probe.alpha |= matches!(frame.transparent, Some(tr) if frame.buffer.contains(&tr));
        probe.frames += 1;
        probe.duration += frame.delay as u64;

        if let Some(ref p) = frame.palette {
            probe.max_colors = probe.max_colors.max(u16::try_from(p.len() / 3).expect("colors to u16"));
        }
    }

    let max_duration = args.max_duration.unwrap_or(u64::MAX);

    while let Some(frame) = decoder.next_frame_info().expect("to read the frame") {
        probe.alpha |= frame.dispose == DisposalMethod::Background && frame.width > 0 && frame.height > 0;
        probe.frames += 1;
        probe.duration += frame.delay as u64;

        if let Some(ref p) = frame.palette {
            probe.max_colors = probe.max_colors.max(u16::try_from(p.len() / 3).expect("colors to u16"));
        }

        if probe.duration >= max_duration {
            break;
        }
    }

    println!(
        r#"{{"alpha":{},"max_colors":{},"duration":{},"frames":{},"width":{},"height":{}}}"#,
        probe.alpha, probe.max_colors, probe.duration, probe.frames, probe.width, probe.height
    );
}
