#[cfg(feature = "clipboard")]
extern crate clipboard;
extern crate colored;
extern crate open;

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
use failure::{self, Fail};
use ffsend_api::url::Url;
use rpassword::prompt_password_stderr;

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

/// Quit the application with an error code,
/// and print the given error.
pub fn quit_error<E: Fail>(err: E) -> ! {
    // Print the error
    print_error(err);

    // Print some additional information
    eprintln!("\nFor detailed errors try '{}'", "--verbose".yellow());
    eprintln!("For more information try '{}'", "--help".yellow());

    // Quit
    exit(1);
}

/// Quit the application with an error code,
/// and print the given error message.
pub fn quit_error_msg<S>(err: S) -> !
    where
        S: AsRef<str> + Display + Debug + Sync + Send + 'static
{
    quit_error(failure::err_msg(err).compat());
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

/// Prompt the user to enter a password.
// TODO: do not prompt if no-interactive
// TODO: only allow emtpy password if forced
pub fn prompt_password() -> String {
    match prompt_password_stderr("Password: ") {
        Ok(password) => password,
        Err(err) => quit_error(err.context(
            "Failed to read password from password prompt"
        )),
    }
}

/// Get a password if required.
/// This method will ensure a password is set (or not) in the given `password`
/// parameter, as defined by `needs`.
///
/// This method will prompt the user for a password, if one is required but
/// wasn't set. An ignore message will be shown if it was not required while it
/// was set.
pub fn ensure_password(password: &mut Option<String>, needs: bool) {
    // Return if we're fine
    if password.is_some() == needs {
        return;
    }

    // Ask for a password, or reset it
    if needs {
        println!("This file is protected with a password.");
        *password = Some(prompt_password());
    } else {
        println!("Ignoring password, it is not required");
        *password = None;
    }
}

/// Prompt the user to enter some value.
/// The prompt that is shown should be passed to `msg`,
/// excluding the `:` suffix.
// TODO: do not prompt if no-interactive
pub fn prompt(msg: &str) -> String {
    // Show the prompt
    eprint!("{}: ", msg);
    let _ = stderr().flush();

    // Get the input
    let mut input = String::new();
    if let Err(err) = stdin().read_line(&mut input) {
        quit_error(err.context(
            "Failed to read input from prompt"
        ));
    }

    // Trim and return
    input.trim().to_owned()
}

/// Prompt the user to enter an owner token.
pub fn prompt_owner_token() -> String {
    prompt("Owner token")
}

/// Get the owner token.
/// This method will ensure an owner token is set in the given `token`
/// parameter.
///
/// This method will prompt the user for the token, if it wasn't set.
pub fn ensure_owner_token(token: &mut Option<String>) {
    // Notify that an owner token is required
    if token.is_none() {
        println!("The file owner token is required for authentication.");
    }

    loop {
        // Prompt for an owner token
        if token.is_none() {
            *token = Some(prompt_owner_token());
        }

        // The token must not be empty
        if token.as_ref().unwrap().is_empty() {
            eprintln!(
                "Empty owner token given, which is invalid. Use {} to cancel.",
                "[CTRL+C]".yellow(),
            );
            *token = None;
        } else {
            break;
        }
    }
}
