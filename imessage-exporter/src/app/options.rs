use clap::{App, Arg, ArgMatches};

use imessage_database::util::dirs::default_db_path;

// CLI Arg Names
pub const OPTION_PATH: &str = "db-path";
pub const OPTION_COPY: &str = "no-copy";
pub const OPTION_DIAGNOSTIC: &str = "diagnostics";
pub const OPTION_EXPORT_TYPE: &str = "export";
pub const OPTION_EXPORT_PATH: &str = "export-path";
pub const ABOUT: &str = "";

pub struct Options<'a> {
    /// Path to database file
    pub db_path: String,
    /// If true, do not copy files from the Libary to the Archive
    pub no_copy: bool,
    /// If true, emit diagnostic information to stdout
    pub diagnostic: bool,
    /// The type of file we are exporting data to
    pub export_type: Option<&'a str>,
    /// Where the app will save exported data
    pub export_path: Option<&'a str>,
    /// Whether the options created are valid or not
    pub valid: bool,
}

impl<'a> Options<'a> {
    pub fn from_args(args: &'a ArgMatches) -> Self {
        let user_path = args.value_of(OPTION_PATH);
        let no_copy = args.is_present(OPTION_COPY);
        let diagnostic = args.is_present(OPTION_DIAGNOSTIC);
        let export_type = args.value_of(OPTION_EXPORT_TYPE);
        let export_path = args.value_of(OPTION_EXPORT_PATH);

        // Validation layer
        let mut valid = true;

        // Ensure an export type is speficied if other export options are selected
        if no_copy && export_type.is_none() {
            println!("No export type selected, required by {OPTION_COPY}");
            valid = false;
        }
        if export_path.is_some() && export_type.is_none() {
            println!("No export type selected, required by {OPTION_EXPORT_PATH}");
            valid = false;
        }

        // Ensure that if diagnostics are enabled, no other options are
        if diagnostic && no_copy {
            println!("Diagnostics are enabled; {OPTION_COPY} is disallowed");
            valid = false;
        }
        if diagnostic && export_path.is_some() {
            println!("Diagnostics are enabled; {OPTION_EXPORT_PATH} is disallowed");
            valid = false;
        }
        if diagnostic && export_type.is_some() {
            println!("Diagnostics are enabled; {OPTION_EXPORT_TYPE} is disallowed");
            valid = false;
        }

        Options {
            db_path: user_path.unwrap_or(&default_db_path()).to_string(),
            no_copy,
            diagnostic,
            export_type,
            export_path,
            valid,
        }
    }
}

pub fn from_command_line() -> ArgMatches {
    let matches = App::new("iMessage Exporter")
        .version("0.0.0")
        .about(ABOUT)
        .arg(
            Arg::new(OPTION_PATH)
                .short('p')
                .long(OPTION_PATH)
                .help("Specify a custom path for the iMessage database file")
                .takes_value(true)
                .value_name("path/to/chat.db"),
        )
        .arg(
            Arg::new(OPTION_COPY)
                .short('n')
                .long(OPTION_COPY)
                .help("Do not copy attachments, instead reference them in-place"),
        )
        .arg(
            Arg::new(OPTION_DIAGNOSTIC)
                .short('d')
                .long(OPTION_DIAGNOSTIC)
                .help("Print diagnostic information and exit"),
        )
        .arg(
            Arg::new(OPTION_EXPORT_TYPE)
                .short('e')
                .long(OPTION_EXPORT_TYPE)
                .help("Specify a single file format to export messages into")
                .takes_value(true)
                .value_name("txt, csv, pdf, html"),
        )
        .arg(
            Arg::new(OPTION_EXPORT_PATH)
                .short('o')
                .long(OPTION_EXPORT_PATH)
                .help("Specify a custom directory for outputting exported data")
                .takes_value(true)
                .value_name("path/to/save/files"),
        )
        .get_matches();
    matches
}