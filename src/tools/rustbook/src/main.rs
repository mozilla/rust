extern crate mdbook;
#[macro_use]
extern crate clap;

use std::env;
use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use clap::{App, ArgMatches, SubCommand, AppSettings};

use mdbook::MDBook;

const NAME: &'static str = "rustbook";

fn main() {
    // Create a list of valid arguments and sub-commands
    let matches = App::new(NAME)
                    .about("Build a book with mdBook")
                    .author("Steve Klabnik <steve@steveklabnik.com>")
                    .version(&*format!("v{}", crate_version!()))
                    .setting(AppSettings::SubcommandRequired)
                    .subcommand(SubCommand::with_name("build")
                        .about("Build the book from the markdown files")
                        .arg_from_usage("-d, --dest-dir=[dest-dir] 'The output directory for your book{n}(Defaults to ./book when omitted)'")
                        .arg_from_usage("[dir] 'A directory for your book{n}(Defaults to Current Directory when omitted)'"))
                    .get_matches();

    // Check which subcomamnd the user ran...
    let res = match matches.subcommand() {
        ("build", Some(sub_matches)) => build(sub_matches),
        ("test", Some(sub_matches)) => test(sub_matches),
        (_, _) => unreachable!(),
    };

    if let Err(e) = res {
        writeln!(&mut io::stderr(), "An error occured:\n{}", e).ok();
        ::std::process::exit(101);
    }
}

// Build command implementation
fn build(args: &ArgMatches) -> Result<(), Box<Error>> {
    let book_dir = get_book_dir(args);
    let book = MDBook::new(&book_dir).read_config();

    let mut book = match args.value_of("dest-dir") {
        Some(dest_dir) => book.set_dest(Path::new(dest_dir)),
        None => book
    };

    try!(book.build());

    Ok(())
}

fn test(args: &ArgMatches) -> Result<(), Box<Error>> {
    let book_dir = get_book_dir(args);
    let mut book = MDBook::new(&book_dir).read_config();

    try!(book.test());

    Ok(())
}

fn get_book_dir(args: &ArgMatches) -> PathBuf {
    if let Some(dir) = args.value_of("dir") {
        // Check if path is relative from current dir, or absolute...
        let p = Path::new(dir);
        if p.is_relative() {
            env::current_dir().unwrap().join(dir)
        } else {
            p.to_path_buf()
        }
    } else {
        env::current_dir().unwrap()
    }
}

