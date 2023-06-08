use std::{collections::HashMap, sync::Arc};

use sp_core::H256;
use tokio::sync::RwLock;

use super::migration_transaction::types::{
    MigrationTransactionResultNotifier,
};

pub(crate) type AssetDatabaseId = i64;
pub(crate) type UserEmail = String;
pub(crate) type TransactionId = H256;
pub(crate) type MigrationTransactionMap =
    Arc<RwLock<HashMap<TransactionId, (UserEmail, AssetDatabaseId)>>>;

pub(crate) trait AssetManagerAttributes {

    fn txs(&self) -> &MigrationTransactionMap;
}

pub(crate) trait AssetManagerTrait: AssetManagerAttributes {}

pub trait Asset {
    fn contract_address(&self) -> &str;

    fn token_id(&self) -> i64;

    fn function_selector(&self) -> &str;
}