#![feature(exit_status_error)]

mod common;
mod trim;

use crate::common::*;
use crate::trim::trim_lw;

use std::path::Path;

use clap::{App, Arg, SubCommand};

use sqlx::{Connection, SqliteConnection};

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("streamwatch tool")
        .version("1.0")
        .author("Lieuwe Rooijakkers <lieuwerooijakkers@gmail.com>")
        .about("Tools for working with streams")
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Print more"),
        )
        .arg(
            Arg::with_name("dry_run")
                .long("dry-run")
                .help("Don't actually execute filesystem operations"),
        )
        .subcommand(
            SubCommand::with_name("trimlw")
                .about("Trim lekker wachten")
                .arg(
                    Arg::with_name("database")
                        .help("Set the database file")
                        .required(true)
                        .index(1),
                )
                .arg(
                    Arg::with_name("streams")
                        .help("Set the streams dir")
                        .required(true)
                        .index(2),
                ),
        )
        .get_matches();

    let settings = Settings {
        verbose: matches.is_present("verbose"),
        dry_run: matches.is_present("dry_run"),
    };

    match matches.subcommand() {
        ("trimlw", Some(subcmd)) => {
            let database_path = subcmd.value_of("database").unwrap();
            let mut conn = SqliteConnection::connect(&format!("sqlite:{}", database_path))
                .await
                .unwrap();

            let folder = subcmd.value_of("streams").unwrap();
            let folder = Path::new(folder);

            trim_lw(&mut conn, &settings, folder).await?;
        }

        _ => {
            eprintln!("{}", matches.usage());
        }
    }

    Ok(())
}
