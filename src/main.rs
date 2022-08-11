use serde::{Deserialize, Serialize};
use std::env;
use std::io;
use std::{collections::HashMap, error::Error, ffi::OsString};

#[derive(Debug, Deserialize)]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

enum TransactionStatus {
    Processed,
    Disputed,
}

type TransactionRecord = (Transaction, TransactionStatus);
struct Bank {
    accounts: Vec<Account>,
    transactions: HashMap<u32, TransactionRecord>,
}

#[derive(Debug, Serialize)]
struct Account {
    client_id: u32,
    available: f32,
    held: f32,
    total: f32,
    locked: bool,
}

impl Account {
    fn new(client_id: u32) -> Self {
        Account {
            client_id: client_id,
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        }
    }
}

struct Transaction {
    tx_type: TransactionType,
    client_id: u32,
    id: u32,
    amount: Option<f32>,
}

/**
 * In this model 1 account = 1 Client
 */
impl Bank {
    fn new() -> Self {
        Self {
            accounts: Vec::new(),
            transactions: HashMap::new(),
        }
    }
    fn batch_process(self: &Self, batch_tx: &Vec<Transaction>) -> Result<(), String> {
        for tx in batch_tx {
            if let Err(e) = self.process_transaction(*tx) {
                return Err(e);
            }
        }
        Ok(())
    }
    fn process_transaction(self: &Self, tx: Transaction) -> Result<(), String> {
        let account = self.get_account(tx.client_id);
        let tx_id = tx.id;

        match tx.tx_type {
            Deposit => {
                let to_deposit = tx.amount.ok_or("Invalid transaction data")?;
                account.available += to_deposit;
                account.total += to_deposit;
                self.transactions
                    .insert(tx_id, (tx, TransactionStatus::Processed));

                Ok(())
            }
            Withdrawal => {
                let to_withdraw = tx.amount.ok_or("Invalid transaction data")?;

                if to_withdraw > account.available {
                    return Ok(());
                }

                account.available -= to_withdraw;
                account.total -= to_withdraw;
                self.transactions
                    .insert(tx_id, (tx, TransactionStatus::Processed));

                Ok(())
            }
            Dispute => {
                let target_tx = self.get_transaction(account, &tx_id, TransactionStatus::Processed);

                match target_tx {
                    Ok(target_tx) => {
                        account.held += target_tx.0.amount.expect("Invalid transaction data");
                        account.available -= target_tx.0.amount.expect("Invalid transaction data");
                        target_tx.1 = TransactionStatus::Disputed;
                        Ok(())
                    }
                    Err(e) => Ok(()),
                }
            }
            Resolve => {
                let target_tx = self.get_transaction(account, &tx_id, TransactionStatus::Disputed);

                match target_tx {
                    Ok(target_tx) => {
                        account.held -= target_tx.0.amount.expect("Invalid transaction data");
                        account.available += target_tx.0.amount.expect("Invalid transaction data");
                        target_tx.1 = TransactionStatus::Processed;
                        Ok(())
                    }
                    Err(e) => Ok(()),
                }
            }
            Chargeback => {
                let target_tx = self.get_transaction(account, &tx_id, TransactionStatus::Disputed);

                match target_tx {
                    Ok(target_tx) => {
                        account.held -= target_tx.0.amount.expect("Invalid transaction data");
                        account.total -= target_tx.0.amount.expect("Invalid transaction data");
                        account.locked = true;
                        target_tx.1 = TransactionStatus::Processed;
                        Ok(())
                    }
                    Err(e) => Ok(()),
                }
            }
        }
    }
    fn get_transaction(
        self: &Self,
        account: &mut Account,
        tx_id: &u32,
        desired_status: TransactionStatus,
    ) -> Result<&TransactionRecord, String> {
        let target_tx = self.transactions.get(&tx_id);
        if let Some(target_tx) = target_tx {
            if let desired_status = target_tx.1 {
                Ok(target_tx)
            } else {
                return Err(format!("Transaction #{} not in desired state", tx_id));
            }
        } else {
            return Err(format!("Transaction #{} not found", tx_id));
        }
    }
    fn get_account(self: &Self, client_id: u32) -> &mut Account {
        if let Some(account) = self.accounts.iter_mut().find(|x| x.client_id == client_id) {
            return account;
        } else {
            let new_account = Account::new(client_id);
            self.accounts.push(new_account);
            return &mut new_account;
        }
    }
    fn print_report(self: &Self) -> Result<(), Box<dyn Error>> {
        let mut writer = csv::Writer::from_writer(io::stdout());
        for account in self.accounts {
            writer.serialize(account)?;
        }
        writer.flush()?;
        Ok(())
    }
}

fn main() {
    match get_first_arg() {
        Ok(file_path) => {
            let transactions = parse_transactions(file_path).unwrap();

            let bank = Bank::new();
            if let Err(e) = bank.batch_process(&transactions) {
                eprintln!("{}", e);
                return;
            }
            if let Err(e) = bank.print_report() {
                eprintln!("{}", e);
            }
        }
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
        .from_path(file_path)?;

    let mut results = Vec::new();
    for record in reader.deserialize() {
        let transaction: Transaction = record?;
        results.push(transaction);
    }

    Ok(results)
}
