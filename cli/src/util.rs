#[cfg(feature = "clipboard")]
extern crate clipboard;
extern crate colored;
extern crate open;

use std::env::current_exe;
#[cfg(feature = "clipboard")]
use std::error::Error as StdError;
use std::fmt::{Debug, Display};
use std::io::{
    Error as IoError,
    stdin,
    stderr,
    Write,
};
use std::process::{exit, ExitStatus};

#[cfg(feature = "clipboard")]
use self::clipboard::{ClipboardContext, ClipboardProvider};
use self::colored::*;
use failure::{err_msg, Fail};
use ffsend_api::url::Url;
use rpassword::prompt_password_stderr;

use cmd::matcher::MainMatcher;

/// Print a success message.
pub fn print_success(msg: &str) {
    println!("{}", msg.green());
}

/// Print the given error in a proper format for the user,
/// with it's causes.
pub fn print_error<E: Fail>(err: E) {
    // Report each printable error, count them
    let count = err.causes() .map(|err| format!("{}", err))
        .filter(|err| !err.is_empty())
        .enumerate()
        .map(|(i, err)| if i == 0 {
            eprintln!("{} {}", "error:".red().bold(), err);
        } else {
            eprintln!("{} {}", "caused by:".red().bold(), err);
        })
        .count();

    // Fall back to a basic message
    if count == 0 {
        eprintln!("{} {}", "error:".red().bold(), "An undefined error occurred");
    }
}

/// Print the given error message in a proper format for the user,
/// with it's causes.
pub fn print_error_msg<S>(err: S)
    where
        S: AsRef<str> + Display + Debug + Sync + Send + 'static
{
    print_error(err_msg(err).compat());
}

/// Quit the application regularly.
pub fn quit() -> ! {
    exit(0);
}

/// Quit the application with an error code,
/// and print the given error.
pub fn quit_error<E: Fail>(err: E, hints: ErrorHints) -> ! {
    // Print the error
    print_error(err);

    // Print error hints
    hints.print();

    // Quit
    exit(1);
}

/// Quit the application with an error code,
/// and print the given error message.
pub fn quit_error_msg<S>(err: S, hints: ErrorHints) -> !
    where
        S: AsRef<str> + Display + Debug + Sync + Send + 'static
{
    quit_error(err_msg(err).compat(), hints);
}

/// The error hint configuration.
#[derive(Copy, Clone, Builder)]
#[builder(default)]
pub struct ErrorHints {
    /// Show about the password option.
    password: bool,

    /// Show about the owner option.
    owner: bool,

    /// Show about the history flag.
    history: bool,

    /// Show about the force flag.
    force: bool,

    /// Show about the verbose flag.
    verbose: bool,

    /// Show about the help flag.
    help: bool,
}

impl ErrorHints {
    /// Check whether any hint should be printed.
    pub fn any(&self) -> bool {
        self.password
            || self.owner
            || self.history
            || self.force
            || self.verbose
            || self.help
    }

    /// Print the error hints.
    pub fn print(&self) {
        // Stop if nothing should be printed
        if !self.any() {
            return;
        }

        eprint!("\n");

        // Print hints
        if self.password {
            eprintln!("Use '{}' to specify a password", highlight("--password <PASSWORD>"));
        }
        if self.owner {
            eprintln!("Use '{}' to specify an owner token", highlight("--owner <TOKEN>"));
        }
        if self.history {
            eprintln!("Use '{}' to specify a history file", highlight("--history <FILE>"));
        }
        if self.force {
            eprintln!("Use '{}' to force", highlight("--force"));
        }
        if self.verbose {
            eprintln!("For detailed errors try '{}'", highlight("--verbose"));
        }
        if self.help {
            eprintln!("For more information try '{}'", highlight("--help"));
        }

        // Flush
        let _ = stderr().flush();
    }
}

impl Default for ErrorHints {
    fn default() -> Self {
        ErrorHints {
            password: false,
            owner: false,
            history: false,
            force: false,
            verbose: true,
            help: true,
        }
    }
}

/// Highlight the given text with a color.
pub fn highlight(msg: &str) -> ColoredString {
    msg.yellow()
}

/// Open the given URL in the users default browser.
/// The browsers exit statis is returned.
pub fn open_url(url: Url) -> Result<ExitStatus, IoError> {
    open_path(url.as_str())
}

/// Open the given path or URL using the program configured on the system.
/// The program exit statis is returned.
pub fn open_path(path: &str) -> Result<ExitStatus, IoError> {
    open::that(path)
}

/// Set the clipboard of the user to the given `content` string.
#[cfg(feature = "clipboard")]
pub fn set_clipboard(content: String) -> Result<(), Box<StdError>> {
    let mut context: ClipboardContext = ClipboardProvider::new()?;
    context.set_contents(content)
}

/// Check for an emtpy password in the given `password`.
/// If the password is emtpy the program will quit with an error unless
/// forced.
// TODO: move this to a better module
pub fn check_empty_password(password: &str, matcher_main: &MainMatcher) {
    if !matcher_main.force() && password.is_empty() {
        quit_error_msg(
            "An empty password is not supported by the web interface",
            ErrorHintsBuilder::default()
                .force(true)
                .verbose(false)
                .build()
                .unwrap(),
        )
    }
}

/// Prompt the user to enter a password.
///
/// If `empty` is `false`, emtpy passwords aren't allowed unless forced.
pub fn prompt_password(main_matcher: &MainMatcher) -> String {
    // Quit with an error if we may not interact
    if main_matcher.no_interact() {
        quit_error_msg(
            "Missing password, must be specified in no-interact mode",
            ErrorHintsBuilder::default()
                .password(true)
                .verbose(false)
                .build()
                .unwrap(),
        );
    }

    // Prompt for the password
    match prompt_password_stderr("Password: ") {
        Ok(password) => password,
        Err(err) => quit_error(err.context(
            "Failed to read password from password prompt"
        ), ErrorHints::default()),
    }
}

/// Get a password if required.
/// This method will ensure a password is set (or not) in the given `password`
/// parameter, as defined by `needs`.
///
/// This method will prompt the user for a password, if one is required but
/// wasn't set. An ignore message will be shown if it was not required while it
/// was set.
pub fn ensure_password(
    password: &mut Option<String>,
    needs: bool,
    main_matcher: &MainMatcher,
) {
    // Return if we're fine
    if password.is_some() == needs {
        return;
    }

    // Prompt for the password, or clear it if not required
    if needs {
        println!("This file is protected with a password.");
        *password = Some(prompt_password(main_matcher));
    } else {
        println!("Ignoring password, it is not required");
        *password = None;
    }
}

/// Prompt the user to enter some value.
/// The prompt that is shown should be passed to `msg`,
/// excluding the `:` suffix.
pub fn prompt(msg: &str, main_matcher: &MainMatcher) -> String {
    // Quit with an error if we may not interact
    if main_matcher.no_interact() {
        quit_error_msg(format!(
            "Could not prompt for '{}' in no-interact mode, maybe specify it",
            msg,
        ), ErrorHints::default());
    }

    // Show the prompt
    eprint!("{}: ", msg);
    let _ = stderr().flush();

    // Get the input
    let mut input = String::new();
    if let Err(err) = stdin().read_line(&mut input) {
        quit_error(err.context(
            "Failed to read input from prompt"
        ), ErrorHints::default());
    }

    // Trim and return
    input.trim().to_owned()
}

/// Prompt the user for a question, allowing a yes or now answer.
/// True is returned if yes was answered, false if no.
///
/// A default may be given, which is chosen if no-interact mode is
/// enabled, or if enter was pressed by the user without entering anything.
pub fn prompt_yes(
    msg: &str,
    def: Option<bool>,
    main_matcher: &MainMatcher,
) -> bool {
    // Define the available options string
    let options = format!("[{}/{}]", match def {
        Some(def) if def => "Y",
        _ => "y",
    }, match def {
        Some(def) if !def => "N",
        _ => "n",
    });

    // Assume yes
    if main_matcher.assume_yes() {
        eprintln!("{} {}: yes", msg, options);
        return true;
    }

    // Autoselect if in no-interact mode
    if main_matcher.no_interact() {
        if let Some(def) = def {
            eprintln!("{} {}: {}", msg, options, if def {
                "yes"
            } else {
                "no"
            });
            return def;
        } else {
            quit_error_msg(format!(
                "Could not prompt question '{}' in no-interact mode, maybe specify it",
                msg,
            ), ErrorHints::default());
        }
    }

    // Get the user input
    let answer = prompt(&format!("{} {}", msg, options), main_matcher);

    // Assume the default if the answer is empty
    if answer.is_empty() && def.is_some() {
        return def.unwrap();
    }

    // Derive a boolean and return
    match derive_bool(&answer) {
        Some(answer) => answer,
        None => prompt_yes(msg, def, main_matcher),
    }
}

/// Try to derive true or false (yes or no) from the given input.
/// None is returned if no boolean could be derived accurately.
fn derive_bool(input: &str) -> Option<bool> {
    // Process the input
    let input = input.trim().to_lowercase();

    // Handle short or incomplete answers
    match input.as_str() {
        "y" | "ye" | "t" | "1" => return Some(true),
        "n" | "f" | "0" => return Some(false),
        _ => {},
    }

    // Handle complete answers with any suffix
    if input.starts_with("yes") || input.starts_with("true") {
        return Some(true);
    }
    if input.starts_with("no") || input.starts_with("false") {
        return Some(false);
    }

    // The answer could not be determined, return none
    None
}

/// Prompt the user to enter an owner token.
pub fn prompt_owner_token(main_matcher: &MainMatcher) -> String {
    prompt("Owner token", main_matcher)
}

/// Get the owner token.
/// This method will ensure an owner token is set in the given `token`
/// parameter.
///
/// This method will prompt the user for the token, if it wasn't set.
pub fn ensure_owner_token(
    token: &mut Option<String>,
    main_matcher: &MainMatcher,
) {
    // Check whehter we allow interaction
    let interact = !main_matcher.no_interact();

    // Notify that an owner token is required
    if interact && token.is_none() {
        println!("The file owner token is required for authentication.");
    }

    loop {
        // Prompt for an owner token
        if token.is_none() {
            if interact {
                *token = Some(prompt_owner_token(main_matcher));
            } else {
                quit_error_msg(
                    "Missing owner token, must be specified in no-interact mode",
                    ErrorHintsBuilder::default()
                        .owner(true)
                        .verbose(false)
                        .build()
                        .unwrap(),
                );
            }
        }

        // The token must not be empty
        if token.as_ref().unwrap().is_empty() {
            eprintln!(
                "Empty owner token given, which is invalid. Use {} to cancel.",
                highlight("[CTRL+C]"),
            );
            *token = None;
        } else {
            break;
        }
    }
}

/// Format the given number of bytes readable for humans.
pub fn format_bytes(bytes: u64) -> String {
    let bytes = bytes as f64;
    let kb = 1024f64;
    match bytes {
        bytes if bytes >= kb.powf(4_f64) => format!("{:.*} TiB", 2, bytes / kb.powf(4_f64)),
        bytes if bytes >= kb.powf(3_f64) => format!("{:.*} GiB", 2, bytes / kb.powf(3_f64)),
        bytes if bytes >= kb.powf(2_f64) => format!("{:.*} MiB", 2, bytes / kb.powf(2_f64)),
        bytes if bytes >= kb => format!("{:.*} KiB", 2, bytes / kb),
        _ => format!("{:.*} B", 0, bytes),
    }
}

/// Get the name of the executable that was invoked.
pub fn exe_name() -> String {
    current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|n| n.to_owned()))
        .and_then(|n| n.into_string().ok())
        .unwrap_or(crate_name!().into())
}
