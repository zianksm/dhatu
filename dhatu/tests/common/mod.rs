use std::{any::Any, str::FromStr};

use dhatu::{
    self,
    registrar::{
        key_manager::keypair::PublicAddress,
        signer::{TxBuilder, WrappedExtrinsic},
    },
    tx::{
        self,
        extrinsics::{
            prelude::{extrinsics::Transaction, ExtrinsicSubmitter},
            transaction_constructor::calldata::Selector,
        }, dhatu_assets::traits::Asset,
    },
    types::MandalaClient,
};
use mandala_node_runner;
use parity_scale_codec::{Compact, Encode};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use sp_core::sr25519::Pair;
use subxt::{tx::PairSigner, utils::AccountId32, PolkadotConfig};

use crate::common::test_types::api::contracts::events::CodeStored;

use self::test_types::api::{
    contracts::{self, events::Instantiated},
    runtime_types::{pallet_contracts::wasm::Determinism, sp_weights::weight_v2::Weight},
};
mod test_types;

pub const DEFAULT_NFT_TOKEN_ID: u32 = 0;

pub const STATIC_GAS_LIMIT: Weight = Weight {
    ref_time: 500_000_000_000,
    proof_size: 11111111111,
};

const STATIC_MINT_STORAGE_DEPOSIT_LIMIT: Option<Compact<u128>> = Some(Compact(246_000_000_000_000));
const STATIC_CONRTACT_SALT_LENGTH: u32 = 32;
const CONSTRUCTOR_SELECTOR: &str = "0x9bae9d5e";
const MINT_FUNCTION_SELECTOR: &str = "cfdd9aa2";

pub async fn setup_node_and_client() -> dhatu::types::MandalaClient {
    let client = MandalaClient::dev()
        .await
        .expect("should create a new client instance!");

    client
}

fn generate_salt() -> Vec<u8> {
    let rng = thread_rng();

    let random_string: String = rng
        .sample_iter(&Alphanumeric)
        .take(STATIC_CONRTACT_SALT_LENGTH as usize)
        .map(char::from)
        .collect();

    let salt_string = hex::encode(random_string);
    let salt = hex::decode(salt_string).expect("static values are valid");

    salt
}

pub async fn setup_dummy_721_contract(client: &MandalaClient) -> subxt::utils::AccountId32 {
    let contract_code = get_code_hash(client).await;
    let mut constructor_selector = Selector::from_raw(CONSTRUCTOR_SELECTOR).unwrap();
    let calldata = encode_calldata(constructor_selector);
    let salt = generate_salt();

    let instantiate_payload = test_types::api::tx().contracts().instantiate(
        0,
        STATIC_GAS_LIMIT,
        Some(Compact(9000_000_000_000000)),
        contract_code.code_hash,
        calldata,
        salt,
    );

    let signer = sp_keyring::Sr25519Keyring::Bob.pair();
    let signer: PairSigner<PolkadotConfig, sp_core::sr25519::Pair> = PairSigner::new(signer);

    let instantiate = client
        .inner()
        .tx()
        .sign_and_submit_then_watch_default(&instantiate_payload, &signer)
        .await
        .expect("should instantiate a new dummy contract transaction successfuly! ")
        .wait_for_finalized_success()
        .await
        .expect("should instantiate contract successfully");

    let contract = instantiate
        .find_first::<Instantiated>()
        .expect("should emit instantiated event")
        .expect("should find instantiated event");

    contract.contract
}

fn encode_calldata(constructor_selector: Selector) -> Vec<u8> {
    let mut calldata = Vec::new();
    calldata.append(&mut constructor_selector.encoded());
    calldata
}

async fn get_code_hash(client: &MandalaClient) -> CodeStored {
    let contract_code = std::fs::read("tests/common/erc721.wasm").expect("should read wasm file");

    let tx_payload =
        test_types::api::tx()
            .contracts()
            .upload_code(contract_code, None, Determinism::Enforced);

    let signer = sp_keyring::Sr25519Keyring::Alice.pair();
    let signer: PairSigner<PolkadotConfig, sp_core::sr25519::Pair> = PairSigner::new(signer);

    let upload_code = client
        .inner()
        .tx()
        .sign_and_submit_then_watch_default(&tx_payload, &signer)
        .await
        .expect("should deploy a new dummy contract transaction successfuly! ")
        .wait_for_finalized_success()
        .await
        .expect("should upload contract successfully");

    let static_code_hash =
        hex::decode("7348c083c5fea839b2f9d1929cf0350d35840692f052ba58129890170a505588")
            .expect("static values are valid");

    println!("code hash size : {}", static_code_hash.len());

    let static_code_hash = subxt::utils::H256::from_slice(static_code_hash.as_ref());
    let static_code_hash_event = CodeStored {
        code_hash: static_code_hash,
    };

    let contract_code = upload_code
        .find_first::<contracts::events::CodeStored>()
        .unwrap()
        .unwrap_or(static_code_hash_event);

    println!("contract code hash: {:?}", contract_code.code_hash);

    contract_code
}

impl WrappedExtrinsic<contracts::calls::types::Call>
    for subxt::tx::Payload<contracts::calls::types::Call>
{
    fn into_inner(self) -> subxt::tx::Payload<contracts::calls::types::Call> {
        self
    }
}

pub async fn mint(client: &MandalaClient, address: PublicAddress, to: Pair, token_id: u32) {
    let mut mint_function_selector = Selector::from_raw(MINT_FUNCTION_SELECTOR).unwrap();

    let mut calldata = Vec::new();

    calldata.append(&mut mint_function_selector.encoded());
    calldata.append(&mut subxt::ext::codec::Encode::encode(&token_id));

    let payload = test_types::api::tx()
        .contracts()
        .call(
            subxt::utils::MultiAddress::Id(AccountId32::from(address)),
            0,                // default value to trf to contract
            STATIC_GAS_LIMIT, // static gas limit
            STATIC_MINT_STORAGE_DEPOSIT_LIMIT,
            calldata,
        )
        .unvalidated();
    // let signer: PairSigner<PolkadotConfig, Pair> = PairSigner::new(to);

    let tx = TxBuilder::signed(client, to, payload).await.unwrap();
    let tx = ExtrinsicSubmitter::submit(tx).await.unwrap();

    let tx = Transaction::wait(tx).await;
    let tx = match tx {
        tx::extrinsics::prelude::enums::ExtrinsicStatus::Success(v) => v.into_inner(),
        _ => panic!("should mint successfully!"),
    };
}

pub async fn batch_mint(
    client: &MandalaClient,
    contract_address: PublicAddress,
    to: Pair,
    amount: u32,
) -> Vec<DummyAsset> {
    let mut token_id = DEFAULT_NFT_TOKEN_ID;

    // we put the tx in a vector to be executed pararelly later
    let mut txs = vec![];

    // dummy assets minted
    let mut assets = vec![];

    for _ in 0..amount {
        let tx = mint(client, contract_address.clone(), to.clone(), token_id);
        txs.push(tx);

        let asset = DummyAsset::new(
            contract_address.clone(),
            token_id,
            Selector::from_raw(MINT_FUNCTION_SELECTOR).unwrap(),
        );

        assets.push(asset);

        token_id += 1;
    }

    // execute batch tx
    futures::future::join_all(txs).await;

    assets
}

pub struct DummyAsset {
    contract_address: PublicAddress,
    token_id: u32,
    function_selector: Selector,
}

impl DummyAsset {
    pub fn new(
        contract_address: PublicAddress,
        token_id: u32,
        function_selector: Selector,
    ) -> Self {
        Self {
            contract_address,
            token_id,
            function_selector,
        }
    }
}

impl Asset for DummyAsset {
    fn contract_address(&self) -> PublicAddress {
        self.contract_address.clone()
    }

    fn token_id(&self) -> u32 {
        self.token_id
    }

    fn function_selector(&self) -> Selector {
        self.function_selector.clone()
    }
}
