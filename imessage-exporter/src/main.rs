#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]
mod app;
mod exporters;

pub use exporters::{exporter::Exporter, html::HTML, txt::TXT, ndjson::NDJSON};

use app::{
    options::{from_command_line, Options},
    runtime::Config,
};

fn main() {
    // Get args from command line
    let args = from_command_line();
    // Create application options
    let options = Options::from_args(&args);

    // Create app state and start
    if let Err(why) = &options {
        eprintln!("{why}");
    } else {
        match options {
            Ok(options) => match Config::new(options) {
                Ok(app) => {
                    if let Err(why) = app.start() {
                        eprintln!("Unable to export: {why}");
                    }
                }
                Err(why) => {
                    eprintln!("Invalid configuration: {why}");
                }
            },
            Err(why) => eprintln!("Invalid command line options: {why}"),
        }
    }
}
