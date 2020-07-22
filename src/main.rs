extern crate getopts;
extern crate inotify;
extern crate regex;
extern crate time;

use std::env;
use getopts::Options;
use std::fs::{read_dir, File};
use std::io;
use std::path::Path;
use std::collections::HashMap;
use std::ffi::OsString;
use inotify::{event_mask, watch_mask, Inotify, EventMask, WatchDescriptor};
use regex::bytes::Regex;

const DEFAULT_DIR: &'static str = "/tmp";
const DEFAULT_REGEXP: &'static str = r"(?P<ELF>\x7FELF)|(?P<PHP><\?)";
const RE_STREAM_BUFFER: usize = 4096; // will read file by these chunks, and test regex on read buffer
const RE_STREAM_OVERLAP: usize = 64; // this number of bytes will be kept from previous chunk, before filling in the buffer with next data, so will detect files even if marker was partially read at the end of previous chunk

fn main()
{	let (re, dirs) = match get_options()
	{	Ok((re, dirs)) => (re, dirs),
		Err(err) =>
		{	eprintln!("{}", err);
			return;
		}
	};
	inotify_r
	(	dirs,
		|event_mask, path|
		{	if !event_mask.contains(event_mask::ISDIR)
			{	if let Some(detected) = detect(&re, path)
				{	let detected = if detected.is_empty() {"(default)"} else {&detected};
					match File::create(path)
					{	Ok(_) =>
						{	println!("{}: {} file truncated: {}", time::strftime("%c", &time::now()).unwrap(), detected, path.display());
						},
						Err(e) =>
						{	println!("{}: {} file NOT truncated: {}: {}", time::strftime("%c", &time::now()).unwrap(), detected, path.display(), e);
						}
					}
				}
			}
		}
	)
}

fn get_options() -> Result<(Regex, Vec<String>), String>
{	let mut opts = Options::new();
	opts.optflag("h", "help", "Print this help.");
	opts.optopt("r", "regex", "Regular expression. Files matching it will be nullified. Can contain named groups. If some named group matched, will print it's name.", "REGEX");
	let mut matches = match opts.parse(env::args().skip(1))
	{	Ok(matches) => matches,
		Err(err) => return Err(err.to_string())
	};
	if matches.opt_present("h")
	{	return Err(opts.usage("Usage: inullify [options] [DIR] [DIR]..."));
	}
	let re = matches.opt_str("r").unwrap_or_else(|| DEFAULT_REGEXP.to_string());
	let re = match Regex::new(&re)
	{	Ok(re) => re,
		Err(err) => return Err(err.to_string())
	};
	if matches.free.is_empty()
	{	matches.free.push(DEFAULT_DIR.to_string());
	}
	Ok((re, matches.free))
}

fn inotify_r<F>(dirs: Vec<String>, cb: F) where F: Fn(EventMask, &Path)
{	let mut inotify = Inotify::init().unwrap();
	let mut track = HashMap::new();
	// add watches
	for dir in dirs
	{	add_watch_dir(&mut inotify, &mut track, &dir).unwrap();
	}
	// read events
	let mut buffer = [0u8; 4096];
	loop
	{	for event in inotify.read_events_blocking(&mut buffer).unwrap()
		{	//println!("Event: {:?}", event);
			let wd = event.wd;
			if event.mask.contains(event_mask::CREATE)
			{	if let Some(path) = track.get(&wd).map(|dir| Path::new(dir).join(event.name))
				{	if event.mask.contains(event_mask::ISDIR)
					{	add_watch_dir(&mut inotify, &mut track, &path).unwrap();
					}
					cb(event.mask, &path);
				}
			}
			else if event.mask.contains(event_mask::DELETE_SELF)
			{	if let Some(path) = track.remove(&wd)
				{	println!("Unwatching: {:?}", path);
				}
			}
			else if event.mask.contains(event_mask::MODIFY) || event.mask.contains(event_mask::CLOSE_WRITE)
			{	if let Some(path) = track.get(&wd).map(|dir| Path::new(dir).join(event.name))
				{	cb(event.mask, &path);
				}
			}
		}
	}
}

fn add_watch_dir<P: AsRef<Path>>(inotify: &mut Inotify, track: &mut HashMap<WatchDescriptor, OsString>, dir: P) -> io::Result<()>
{	{	let path = dir.as_ref();
		println!("Watching: {}", path.display());
		match inotify.add_watch(path.to_owned(), watch_mask::CREATE|watch_mask::DELETE_SELF|watch_mask::MODIFY|watch_mask::CLOSE_WRITE)
		{	Ok(wd) =>
			{	track.insert(wd, path.to_path_buf().into_os_string());
			},
			Err(e) =>
			{	println!("Error: {:?}", e);
				return Ok(());
			}
		}
	}
	for item in read_dir(dir)?
	{	let item = item?;
		let path = item.path();
		if path.is_dir()
		{	if let Err(_) = path.read_link()
			{	add_watch_dir(inotify, track, &path)?;
			}
		}
    }
    Ok(())
}

fn detect(re: &Regex, file_path: &Path) -> Option<String>
{	if let Ok(file) = File::open(file_path)
	{	let mut buffer = [0; RE_STREAM_BUFFER];
		let mut reader = OverlapReader::new(file, &mut buffer, RE_STREAM_OVERLAP);
		while let Some(data) = reader.next()
		{	if let Some(caps) = re.captures(data)
			{	for name in re.capture_names()
				{	if let Some(name) = name
					{	if let Some(_cap) = caps.name(name)
						{	return Some(name.to_owned())
						}
					}
				}
				return Some(String::new())
			}
		}
	}
	None
}

pub struct OverlapReader<'a, T> where T: io::Read
{	input: T,
	buffer: &'a mut [u8],
	overlap: usize,
	len: usize,
	eof: bool,
}
impl<'a, T> OverlapReader<'a, T> where T: io::Read
{	fn new(input: T, buffer: &'a mut [u8], overlap: usize) -> Self
	{	Self
		{	input,
			buffer,
			overlap,
			len: 0,
			eof: false,
		}
	}

	fn next(&mut self) -> Option<&[u8]>
	{	if self.eof
		{	return None;
		}
		if self.len > self.overlap
		{	self.buffer.copy_within(self.overlap .. self.len, 0);
			self.len = self.overlap;
		}

		// read at least overlap
		while self.len < self.overlap
		{	match self.input.read(&mut self.buffer[self.len ..])
			{	Err(_) | Ok(0) =>
				{	self.eof = true;
					break;
				}
				Ok(n) =>
				{	self.len += n;
				}
			}
		}

		// yield
		if self.len == 0
		{	None
		}
		else
		{	Some(&self.buffer[.. self.len])
		}
	}
}
