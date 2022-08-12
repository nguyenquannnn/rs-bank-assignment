use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::env;
use std::io;
use std::{collections::HashMap, error::Error, ffi::OsString};

#[derive(Debug, Copy, Clone, Deserialize)]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(PartialEq)]
enum TransactionStatus {
    Processed,
    Disputed,
}

type TransactionRecord = (Transaction, TransactionStatus);
struct Bank {
    accounts: RefCell<Vec<Account>>,
    transactions: RefCell<HashMap<u32, TransactionRecord>>,
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
#[derive(Debug, Deserialize)]
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
            accounts: RefCell::new(Vec::new()),
            transactions: RefCell::new(HashMap::new()),
        }
    }
    fn batch_process(&self, batch_tx: Vec<Transaction>) -> Result<(), String> {
        for tx in batch_tx {
            if let Err(e) = self.process_transaction(tx) {
                return Err(e);
            }
        }
        Ok(())
    }
    fn process_transaction(&self, tx: Transaction) -> Result<(), String> {
        let mut account = self.get_account(tx.client_id);
        let tx_id = tx.id;

        match tx.tx_type {
            TransactionType::Deposit => {
                let to_deposit = tx.amount.ok_or("Invalid transaction data")?;
                account.available += to_deposit;
                account.total += to_deposit;
                self.transactions
                    .borrow_mut()
                    .insert(tx_id, (tx, TransactionStatus::Processed));

                Ok(())
            }
            TransactionType::Withdrawal => {
                let to_withdraw = tx.amount.ok_or("Invalid transaction data")?;

                if to_withdraw > account.available {
                    return Ok(());
                }

                account.available -= to_withdraw;
                account.total -= to_withdraw;
                self.transactions
                    .borrow_mut()
                    .insert(tx_id, (tx, TransactionStatus::Processed));

                Ok(())
            }
            TransactionType::Dispute => {
                let target_tx =
                    self.get_transaction(&account, &tx_id, TransactionStatus::Processed);

                match target_tx {
                    Ok(mut target_tx) => {
                        account.held += target_tx.0.amount.expect("Invalid transaction data");
                        account.available -= target_tx.0.amount.expect("Invalid transaction data");
                        target_tx.1 = TransactionStatus::Disputed;
                        Ok(())
                    }
                    Err(_) => Ok(()),
                }
            }
            TransactionType::Resolve => {
                let target_tx = self.get_transaction(&account, &tx_id, TransactionStatus::Disputed);

                match target_tx {
                    Ok(mut target_tx) => {
                        account.held -= target_tx.0.amount.expect("Invalid transaction data");
                        account.available += target_tx.0.amount.expect("Invalid transaction data");
                        target_tx.1 = TransactionStatus::Processed;
                        Ok(())
                    }
                    Err(_) => Ok(()),
                }
            }
            TransactionType::Chargeback => {
                let target_tx = self.get_transaction(&account, &tx_id, TransactionStatus::Disputed);

                match target_tx {
                    Ok(mut target_tx) => {
                        account.held -= target_tx.0.amount.expect("Invalid transaction data");
                        account.total -= target_tx.0.amount.expect("Invalid transaction data");
                        account.locked = true;
                        target_tx.1 = TransactionStatus::Processed;
                        Ok(())
                    }
                    Err(_) => Ok(()),
                }
            }
        }
    }
    fn get_transaction(
        &self,
        account: &Account,
        tx_id: &u32,
        desired_status: TransactionStatus,
    ) -> Result<TransactionRecord, String> {
        if let Some(target_tx) = self.transactions.borrow_mut().remove(&tx_id) {
            if target_tx.0.client_id != account.client_id {
                return Err(format!(
                    "Transaction #{} does not have matching client id",
                    tx_id
                ));
            }
            if desired_status == target_tx.1 {
                return Err(format!("Transaction #{} not in desired state", tx_id));
            }
            return Ok(target_tx);
        } else {
            return Err(format!("Transaction #{} not found", tx_id));
        }
    }

    fn get_account(&self, client_id: u32) -> Account {
        match self
            .accounts
            .borrow()
            .iter()
            .position(|x| x.client_id == client_id)
        {
            Some(index) => self.accounts.borrow_mut().remove(index),
            // None => todo!(),
            // Some(_) => todo!(),
            None => {
                let new_account = Account::new(client_id);
                self.accounts.borrow_mut().push(new_account);
                return self.accounts.borrow_mut().pop().unwrap();
            }
        }
    }

    fn print_report(&self) -> Result<(), Box<dyn Error>> {
        let mut writer = csv::Writer::from_writer(io::stdout());
        for account in self.accounts.borrow().iter() {
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
            if let Err(e) = bank.batch_process(transactions) {
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
