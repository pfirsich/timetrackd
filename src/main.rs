use std::io;
use std::fs;
use std::path;
use std::process::Command;
use std::{thread, time};

extern crate dirs;
extern crate toml;

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

#[derive(Debug)]
enum LoadConfigError {
    Io(io::Error),
    ConfigDirError,
    ParseError(String),
    TomlError(toml::de::Error)
}

impl From<io::Error> for LoadConfigError {
    fn from(err: io::Error) -> LoadConfigError {
        return LoadConfigError::Io(err);
    }
}

impl From<toml::de::Error> for LoadConfigError {
    fn from(err: toml::de::Error) -> LoadConfigError {
        return LoadConfigError::TomlError(err);
    }
}

#[derive(Debug)]
enum DatabaseType {
    Sqlite,
}

#[derive(Debug)]
struct Config {
    database_path: path::PathBuf,
    database_type: DatabaseType,
    sample_interval: u64,
}

impl Default for Config {
    fn default() -> Config {
        return Config {
            database_path: match dirs::home_dir() {
                Some(path) => path,
                None => panic!("Could not get home directory!")
            }.join(".timetrackd.db"),
            database_type: DatabaseType::Sqlite,
            sample_interval: 5,
        }
    }
}

fn parse_u64(value: &toml::Value) -> Option<u64> {
    if !value.is_integer() || value.as_integer().unwrap() < 0 {
        return None;
    }
    return Some(value.as_integer().unwrap() as u64);
}

fn parse_path(value: &toml::Value) -> Option<path::PathBuf> {
    if !value.is_str() {
        return None;
    }
    return Some(path::PathBuf::from(value.as_str().unwrap()));
}

fn parse_database_type(value: &toml::Value) -> Option<DatabaseType> {
    if !value.is_str() {
        return None;
    }
    return match value.as_str().unwrap() {
        "sqlite" => Some(DatabaseType::Sqlite),
        _ => None
    }
}

fn load_config() -> Result<Config, LoadConfigError> {
    let config_path = match dirs::config_dir() {
        Some(path) => path,
        None => return Err(LoadConfigError::ConfigDirError)
    }.join("timetrackd.toml");

    let mut config = Config::default();
    if config_path.is_file() {
        let file_str = fs::read_to_string(config_path)?;
        let config_data: toml::Value = toml::from_str(&file_str)?;

        if config_data.get("sample_interval").is_some() {
            config.sample_interval = match parse_u64(&config_data["sample_interval"]) {
                Some(val) => val,
                None => return Err(LoadConfigError::ParseError {0: "sample_interval has to be a positive integer".to_string()})
            }
        }

        if config_data.get("database_path").is_some() {
            config.database_path = match parse_path(&config_data["database_path"]) {
                Some(path) => path,
                None => return Err(LoadConfigError::ParseError {0: "database_path must be a filesystem path".to_string()})
            }
        }

        if config_data.get("database_type").is_some() {
            config.database_type = match parse_database_type(&config_data["database_type"]) {
                Some(db_type) => db_type,
                None => return Err(LoadConfigError::ParseError {0: "database_type must be 'sqlite'".to_string()})
            }
        }
    } else {
        eprintln!("Could not load config file '{}'", config_path.to_string_lossy());
    }
    return Ok(config);
}

fn main() {
    let config: Config = match load_config() {
        Ok(config) => config,
        Err(err) => panic!("Could not load config: {:?}", err)
    };
    println!("Config: {:?}", config);

    let sample_interval = time::Duration::from_secs(config.sample_interval);
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
                eprintln!("Error fetching data: {:?}", err);
                last_sample = None;
            }
        }
        thread::sleep(sample_interval);
    }
}
