use std::io;
use std::process::Command;
use std::{thread, time};

#[derive(PartialEq, Clone)]
struct Sample {
    window_title: String,
    pid: u32,
    process_name: String,
    screensaver_active: bool,
    idle: bool,
}

#[derive(Debug)]
enum SampleError {
    Io(io::Error),
    FromUtf8(std::string::FromUtf8Error),
    ParseInt(std::num::ParseIntError),
}

impl From<io::Error> for SampleError {
    fn from(err: io::Error) -> SampleError {
        return SampleError::Io(err);
    }
}

impl From<std::string::FromUtf8Error> for SampleError {
    fn from(err: std::string::FromUtf8Error) -> SampleError {
        return SampleError::FromUtf8(err);
    }
}

impl From<std::num::ParseIntError> for SampleError {
    fn from(err: std::num::ParseIntError) -> SampleError {
        return SampleError::ParseInt(err);
    }
}

fn get_command_output(command: &str, args: &[&str]) -> Result<String, SampleError> {
    return Ok(String::from_utf8(
        Command::new(command)
            .args(args)
            .env("LC_ALL", "C") // I will never understand localization of CLI tool output
            .output()?
            .stdout,
    )?
    .trim()
    .to_string());
}

fn get_sample(sample_interval: &std::time::Duration) -> Result<Sample, SampleError> {
    let window_title = get_command_output("xdotool", &["getactivewindow", "getwindowname"])?;
    let pid_str = get_command_output("xdotool", &["getactivewindow", "getwindowpid"])?;
    let pid: u32 = pid_str.parse()?;
    let process_name = get_command_output("ps", &["-p", &pid_str, "-o", "comm="])?;
    let screensaver_active =
        !get_command_output("gnome-screensaver-command", &["-q"])?.contains("inactive");
    let idle_time: u128 = get_command_output("xprintidle", &[])?.parse()?;
    let idle = idle_time > sample_interval.as_millis();
    return Ok(Sample {
        window_title,
        pid,
        process_name,
        screensaver_active,
        idle,
    });
}

fn main() {
    let sample_interval = time::Duration::from_secs(1);
    let mut last_sample: Option<Sample> = None;
    loop {
        match get_sample(&sample_interval) {
            Ok(sample) => {
                if last_sample != Some(sample.clone()) {
                    if sample.screensaver_active {
                        println!("screensaver");
                    } else {
                        print!(
                            "'{}' ([{}] {})",
                            sample.window_title, sample.pid, sample.process_name
                        );
                        if sample.idle {
                            println!(" (idle)");
                        } else {
                            println!("");
                        }
                    }
                    last_sample = Some(sample);
                }
            }
            Err(err) => {
                println!("Error fetching data: {:?}", err);
                last_sample = None;
            }
        }
        thread::sleep(sample_interval);
    }
}
