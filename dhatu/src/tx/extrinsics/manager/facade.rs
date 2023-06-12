use serde::Serialize;

use sp_core::H256;

use crate::{
    tx::extrinsics::{
        extrinsics_tracker::extrinsics::TransactionMessage,
        prelude::{enums::Hash, NotificationMessage, TransactionId},
    },
    types::{MandalaClient, MandalaExtrinsics, ReceiverChannel, SenderChannel},
};

use super::super::{
    callback_executor::Executor,
    extrinsics_tracker::tracker::ExtrinsicWatcher,
    prelude::{ExtrinsicSubmitter, GenericError},
    types::{BlockchainClient, Extrinsic},
};

#[cfg(feature = "tokio")]
#[cfg(feature = "serde")]
pub struct ExtrinsicFacade {
    transaction_watcher: ExtrinsicWatcher,
    transaction_sender_channel: SenderChannel<TransactionMessage>,
}

impl ExtrinsicFacade {
    pub fn new(client: MandalaClient) -> Self {
        let (tx_sender_channel, tx_receiver_channel) = Self::create_channel();

        let callback_executor = Executor::new();
        let tx_watcher = ExtrinsicWatcher::new(client);

        Self::initialize_receive_task(
            tx_watcher.clone(),
            callback_executor.clone(),
            tx_receiver_channel,
        );

        Self {
            transaction_watcher: tx_watcher,
            transaction_sender_channel: tx_sender_channel,
        }
    }

    fn initialize_receive_task(
        tx_watcher: ExtrinsicWatcher,
        callback_executor: Executor,
        mut tx_receiver_channel: ReceiverChannel<TransactionMessage>,
    ) {
        let recv = async move {
            loop {
                let msg = tx_receiver_channel.recv().await.unwrap();

                tx_watcher.stop_watching(msg.id()).await;

                if let Some(callback) = msg.callback() {
                    // will fail silently if if there's an error when executing the callback
                    callback_executor.execute(msg.status.clone(), &callback);
                }
            }
        };

        tokio::task::spawn(recv);
    }

    pub async fn submit(
        &self,
        tx: MandalaExtrinsics,
        callback: Option<String>,
    ) -> Result<Hash, GenericError> {
        let progress = ExtrinsicSubmitter::submit(tx).await?;
        let tx = self
            .transaction_watcher
            .watch(
                progress,
                Some(self.transaction_sender_channel.clone()),
                callback,
            )
            .await;

        Ok(tx)
    }

    pub fn create_channel() -> (
        SenderChannel<TransactionMessage>,
        ReceiverChannel<TransactionMessage>,
    ) {
        tokio::sync::mpsc::unbounded_channel::<TransactionMessage>()
    }
}
