use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::io;

const INVALID_TRANSACTION_DATA_NO_AMOUNT: &str = "Invalid transaction data: missing amount";

#[derive(Debug, Copy, Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Debug, PartialEq)]
enum TransactionStatus {
    Processed,
    Disputed,
}

#[derive(Debug, Deserialize)]
pub struct Transaction {
    #[serde(rename(deserialize = "type"))]
    tx_type: TransactionType,
    #[serde(rename(deserialize = "client"))]
    client_id: u16,
    #[serde(rename(deserialize = "tx"))]
    id: u32,
    amount: Option<f32>,
}

type TransactionRecord = (Transaction, TransactionStatus);

#[derive(Debug, Serialize)]
struct Account {
    #[serde(rename(serialize = "client"))]
    client_id: u16,
    available: f32,
    held: f32,
    total: f32,
    locked: bool,
}

impl Account {
    fn new(client_id: u16) -> Self {
        Account {
            client_id: client_id,
            available: 0.0,
            held: 0.0,
            total: 0.0,
            locked: false,
        }
    }
}

pub struct Bank {
    accounts: RefCell<Vec<Account>>,
    transactions: RefCell<HashMap<u32, TransactionRecord>>,
}

/**
 * In this model 1 account = 1 Client
 */
impl Bank {
    pub fn new() -> Self {
        Self {
            accounts: RefCell::new(Vec::new()),
            transactions: RefCell::new(HashMap::new()),
        }
    }
    pub fn batch_process(&self, batch_tx: Vec<Transaction>) -> Result<(), String> {
        for tx in batch_tx {
            if let Err(e) = self.process_transaction(tx) {
                return Err(e);
            }
        }
        Ok(())
    }
    fn process_transaction(&self, tx: Transaction) -> Result<(), String> {
        let mut account = match self.get_account(tx.client_id) {
            Some(a) => a,
            None => Account::new(tx.client_id),
        };
        let tx_id = tx.id;

        let result = match tx.tx_type {
            TransactionType::Deposit => {
                let to_deposit = tx.amount.ok_or(INVALID_TRANSACTION_DATA_NO_AMOUNT)?;
                account.available += to_deposit;
                account.total += to_deposit;
                self.transactions
                    .borrow_mut()
                    .insert(tx_id, (tx, TransactionStatus::Processed));
            }
            TransactionType::Withdrawal => {
                let to_withdraw = tx.amount.ok_or(INVALID_TRANSACTION_DATA_NO_AMOUNT)?;

                if to_withdraw <= account.available {
                    account.available -= to_withdraw;
                    account.total -= to_withdraw;
                    self.transactions
                        .borrow_mut()
                        .insert(tx_id, (tx, TransactionStatus::Processed));
                }
            }
            TransactionType::Dispute => {
                match self.get_transaction_with_status(
                    &account,
                    &tx_id,
                    TransactionStatus::Processed,
                ) {
                    Ok(mut target_tx) => {
                        let tx_amount = target_tx
                            .0
                            .amount
                            .expect(INVALID_TRANSACTION_DATA_NO_AMOUNT);
                        account.held += tx_amount;
                        account.available -= tx_amount;
                        target_tx.1 = TransactionStatus::Disputed;
                        self.transactions.borrow_mut().insert(tx_id, target_tx);
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                };
            }
            TransactionType::Resolve => {
                match self.get_transaction_with_status(
                    &account,
                    &tx_id,
                    TransactionStatus::Disputed,
                ) {
                    Ok(mut target_tx) => {
                        let tx_amount = target_tx
                            .0
                            .amount
                            .expect(INVALID_TRANSACTION_DATA_NO_AMOUNT);
                        account.held -= tx_amount;
                        account.available += tx_amount;
                        target_tx.1 = TransactionStatus::Processed;
                        self.transactions.borrow_mut().insert(tx_id, target_tx);
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                };
            }
            TransactionType::Chargeback => {
                match self.get_transaction_with_status(
                    &account,
                    &tx_id,
                    TransactionStatus::Disputed,
                ) {
                    Ok(mut target_tx) => {
                        let tx_amount = target_tx
                            .0
                            .amount
                            .expect(INVALID_TRANSACTION_DATA_NO_AMOUNT);
                        account.held -= tx_amount;
                        account.total -= tx_amount;
                        account.locked = true;
                        target_tx.1 = TransactionStatus::Processed;
                        self.transactions.borrow_mut().insert(tx_id, target_tx);
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                };
            }
        };
        self.accounts.borrow_mut().push(account);
        Ok(result)
    }

    fn get_transaction_with_status(
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
            if desired_status != target_tx.1 {
                return Err(format!("Transaction #{} not in desired state", tx_id));
            }
            return Ok(target_tx);
        } else {
            return Err(format!("Transaction #{} not found", tx_id));
        }
    }

    fn get_account(&self, client_id: u16) -> Option<Account> {
        let index;
        {
            index = self
                .accounts
                .borrow()
                .iter()
                .position(|x| x.client_id == client_id);
        }
        match index {
            Some(i) => Some(self.accounts.borrow_mut().remove(i)),
            None => None,
        }
    }

    pub fn print_report(&self) -> Result<(), Box<dyn Error>> {
        let mut writer = csv::Writer::from_writer(io::stdout());
        for account in self.accounts.borrow().iter() {
            writer.serialize(account)?;
        }
        writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_process_deposit() {
        // GIVEN
        let deposit1 = Transaction {
            tx_type: TransactionType::Deposit,
            client_id: 1,
            id: 1,
            amount: Some(30.0),
        };
        let bank = Bank::new();

        // WHEN
        let result = bank.batch_process(vec![deposit1]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 1);
        assert_eq!(bank.accounts.borrow()[0].available, 30.0000);
        assert_eq!(bank.accounts.borrow()[0].total, 30.0000);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0000);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_deposit_no_amount_error() {
        // GIVEN
        let deposit1 = Transaction {
            tx_type: TransactionType::Deposit,
            client_id: 1,
            id: 1,
            amount: None,
        };
        let bank = Bank::new();

        // WHEN
        let result = bank.batch_process(vec![deposit1]);

        // THEN
        assert_eq!(
            result,
            Err(String::from("Invalid transaction data: missing amount"))
        );
        assert_eq!(bank.accounts.borrow().len(), 0);
    }

    #[test]
    fn test_batch_process_withdrawal() {
        // GIVEN
        let withdrawal = Transaction {
            tx_type: TransactionType::Withdrawal,
            client_id: 5,
            id: 2,
            amount: Some(15.0),
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 30.0,
            held: 0.0,
            total: 30.0,
            locked: false,
        }]);

        // WHEN
        let result = bank.batch_process(vec![withdrawal]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].available, 15.0);
        assert_eq!(bank.accounts.borrow()[0].total, 15.0);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_withdrawal_no_amount_error() {
        // GIVEN
        let withdrawal = Transaction {
            tx_type: TransactionType::Withdrawal,
            client_id: 1,
            id: 1,
            amount: None,
        };
        let bank = Bank::new();

        // WHEN
        let result = bank.batch_process(vec![withdrawal]);

        // THEN
        assert_eq!(
            result,
            Err(String::from("Invalid transaction data: missing amount"))
        );
        assert_eq!(bank.accounts.borrow().len(), 0);
    }

    #[test]
    fn test_batch_process_withdrawal_not_sufficient_fund_no_change() {
        // GIVEN
        let withdrawal = Transaction {
            tx_type: TransactionType::Withdrawal,
            client_id: 5,
            id: 2,
            amount: Some(45.0),
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 30.0,
            held: 0.0,
            total: 30.0,
            locked: false,
        }]);

        // WHEN
        let result = bank.batch_process(vec![withdrawal]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].available, 30.0);
        assert_eq!(bank.accounts.borrow()[0].total, 30.0);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_dispute() {
        // GIVEN
        let dispute = Transaction {
            tx_type: TransactionType::Dispute,
            client_id: 5,
            id: 2,
            amount: None,
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 15.0,
            held: 0.0,
            total: 15.0,
            locked: false,
        }]);

        bank.transactions = RefCell::new(HashMap::from([(
            2,
            (
                Transaction {
                    tx_type: TransactionType::Withdrawal,
                    client_id: 5,
                    id: 2,
                    amount: Some(10.0),
                },
                TransactionStatus::Processed,
            ),
        )]));

        // WHEN
        let result = bank.batch_process(vec![dispute]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].held, 10.0);
        assert_eq!(bank.accounts.borrow()[0].total, 15.0);
        assert_eq!(bank.accounts.borrow()[0].available, 5.0);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_dispute_valid_tx_but_not_matching_client_id() {
        // GIVEN
        let dispute = Transaction {
            tx_type: TransactionType::Dispute,
            client_id: 15,
            id: 2,
            amount: None,
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 15.0,
            held: 0.0,
            total: 15.0,
            locked: false,
        }]);

        bank.transactions = RefCell::new(HashMap::from([(
            2,
            (
                Transaction {
                    tx_type: TransactionType::Withdrawal,
                    client_id: 5,
                    id: 2,
                    amount: Some(10.0),
                },
                TransactionStatus::Processed,
            ),
        )]));

        // WHEN
        let result = bank.batch_process(vec![dispute]);

        // THEN
        assert_eq!(result, Ok(()));
        // No fund amount was changed
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0);
        assert_eq!(bank.accounts.borrow()[0].total, 15.0);
        assert_eq!(bank.accounts.borrow()[0].available, 15.0);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_resolve() {
        // GIVEN
        let resolve = Transaction {
            tx_type: TransactionType::Resolve,
            client_id: 5,
            id: 2,
            amount: None,
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 5.0,
            held: 10.0,
            total: 15.0,
            locked: false,
        }]);

        bank.transactions = RefCell::new(HashMap::from([(
            2,
            (
                Transaction {
                    tx_type: TransactionType::Withdrawal,
                    client_id: 5,
                    id: 2,
                    amount: Some(10.0),
                },
                TransactionStatus::Disputed,
            ),
        )]));

        // WHEN
        let result = bank.batch_process(vec![resolve]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0);
        assert_eq!(bank.accounts.borrow()[0].total, 15.0);
        assert_eq!(bank.accounts.borrow()[0].available, 15.0);
        assert_eq!(bank.accounts.borrow()[0].locked, false);
    }

    #[test]
    fn test_batch_process_chargeback() {
        // GIVEN
        let chargeback = Transaction {
            tx_type: TransactionType::Chargeback,
            client_id: 5,
            id: 2,
            amount: None,
        };

        let mut bank = Bank::new();

        bank.accounts = RefCell::new(vec![Account {
            client_id: 5,
            available: 5.0,
            held: 10.0,
            total: 15.0,
            locked: false,
        }]);

        bank.transactions = RefCell::new(HashMap::from([(
            2,
            (
                Transaction {
                    tx_type: TransactionType::Withdrawal,
                    client_id: 5,
                    id: 2,
                    amount: Some(10.0),
                },
                TransactionStatus::Disputed,
            ),
        )]));

        // WHEN
        let result = bank.batch_process(vec![chargeback]);

        // THEN
        assert_eq!(result, Ok(()));
        assert_eq!(bank.accounts.borrow()[0].client_id, 5);
        assert_eq!(bank.accounts.borrow()[0].held, 0.0);
        assert_eq!(bank.accounts.borrow()[0].total, 5.0);
        assert_eq!(bank.accounts.borrow()[0].available, 5.0);
        assert_eq!(bank.accounts.borrow()[0].locked, true);
    }
}
