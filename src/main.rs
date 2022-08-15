use csv::Trim;
use std::env;
use std::{error::Error, ffi::OsString};

mod bank;
use crate::bank::{Bank as RustBank, Transaction};

fn main() {
    match get_first_arg() {
        Ok(file_path) => match parse_transactions(file_path) {
            Ok(transactions) => {
                let bank = RustBank::new();
                if let Err(e) = bank.batch_process(transactions) {
                    eprintln!("{}", e);
                    return;
                }
                if let Err(e) = bank.print_report() {
                    eprintln!("{}", e);
                }
            }
            Err(e) => {
                eprintln!("{}", e);
                return;
            }
        },
        Err(e) => eprintln!("{}", e),
    }
}

fn get_first_arg() -> Result<OsString, String> {
    match env::args_os().nth(1) {
        None => Err(From::from("Expected 1 argument, but got none")),
        Some(file_path) => Ok(file_path),
    }
}

fn parse_transactions(file_path: OsString) -> Result<Vec<Transaction>, Box<dyn Error>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .trim(Trim::All)
        .from_path(file_path)?;

    let mut results = Vec::new();
    for record in reader.deserialize() {
        let transaction: Transaction = record?;
        results.push(transaction);
    }

    Ok(results)
}
