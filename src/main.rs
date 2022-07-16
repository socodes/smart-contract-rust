[dependencies]
// A library for developing Casper network smart contracts.
casper-contract = "1.4.4"
// Types shared by many Casper crates for use on a Casper Network.
casper-types = "1.4.6"

// This code imports necessary aspects of external crates that we will use in our contract code.

extern crate alloc;

// Importing Rust types.
use alloc::string::{String, ToString};
use alloc::vec;
// Importing aspects of the Casper platform.
use casper_contract::contract_api::storage::dictionary_get;
use casper_contract::contract_api::{runtime, storage, system};
use casper_contract::unwrap_or_revert::UnwrapOrRevert;
// Importing specific Casper types.
use casper_types::account::AccountHash;
use casper_types::contracts::NamedKeys;
use casper_types::{runtime_args, CLType, CLValue, EntryPoint, EntryPointAccess, EntryPointType, EntryPoints, Key, Parameter, ApiError, RuntimeArgs};


// Creating constants for the various contract entry points.
const ENTRY_POINT_INIT: &str = "init";
const ENTRY_POINT_DONATE: &str = "donate";
const ENTRY_POINT_GET_DONATION_COUNT: &str = "get_donation_count";
const ENTRY_POINT_GET_FUNDS_RAISED: &str = "get_funds_raised";

// Creating constants for values within the contract.
const DONATING_ACCOUNT_KEY: &str = "donating_account_key";
const LEDGER: &str = "ledger";
const FUNDRAISING_PURSE: &str = "fundraising_purse";


// This entry point initializes the donation system, setting up the fundraising purse
// and creating a dictionary to track the account hashes and the number of donations
// made.
#[no_mangle]
pub extern "C" fn init() {
    let fundraising_purse = system::create_purse();
    runtime::put_key(FUNDRAISING_PURSE, fundraising_purse.into());
    // Create a dictionary to track the mapping of account hashes to number of donations made.
    storage::new_dictionary(LEDGER).unwrap_or_revert();
}

// This is the donation entry point. When called, it records the caller's account
// hash and returns the donation purse, with add access, to the immediate caller.
#[no_mangle]
pub extern "C" fn donate() {
    let donating_account_key: Key = runtime::get_named_arg(DONATING_ACCOUNT_KEY);
    if let Key::Account(donating_account_hash) = donating_account_key {
        update_ledger_record(donating_account_hash.to_string())
    } else {
        runtime::revert(FundRaisingError::InvalidKeyVariant)
    }
    let donation_purse = *runtime::get_key(FUNDRAISING_PURSE)
        .unwrap_or_revert_with(FundRaisingError::MissingFundRaisingPurseURef)
        .as_uref()
        .unwrap_or_revert();
    // The return value is the donation_purse URef with `add` access only. As a result
    // the entity receiving this purse URef may only add to the purse, and cannot remove
    // funds.
    let value = CLValue::from_t(donation_purse.into_add()).unwrap_or_revert();
    runtime::ret(value)
}

// This entry point returns the amount of donations from the caller.
#[no_mangle]
pub extern "C" fn get_donation_count() {
    let donating_account_key: Key = runtime::get_named_arg(DONATING_ACCOUNT_KEY);
    if let Key::Account(donating_account_hash) = donating_account_key {
        let ledger_seed_uref = *runtime::get_key(LEDGER)
            .unwrap_or_revert_with(FundRaisingError::MissingLedgerSeedURef)
            .as_uref()
            .unwrap_or_revert();
        let donation_count = if let Some(donation_count) =
            storage::dictionary_get::<u64>(ledger_seed_uref, &donating_account_hash.to_string())
                .unwrap_or_revert()
        {
            donation_count
        } else {
            0u64
        };
        runtime::ret(CLValue::from_t(donation_count).unwrap_or_revert())
    } else {
        runtime::revert(FundRaisingError::InvalidKeyVariant)
    }
}

// This entry point returns the total funds raised.
#[no_mangle]
pub extern "C" fn get_funds_raised() {
    let donation_purse = *runtime::get_key(FUNDRAISING_PURSE)
        .unwrap_or_revert_with(FundRaisingError::MissingFundRaisingPurseURef)
        .as_uref()
        .unwrap_or_revert();
    let funds_raised = system::get_purse_balance(donation_purse)
        .unwrap_or_revert();
    runtime::ret(CLValue::from_t(funds_raised).unwrap_or_revert())
}

//This is the full `call` function as defined within the donation contract.
#[no_mangle]
pub extern "C" fn call() {
    // This establishes the `init` entry point for initializing the contract's infrastructure.
    let init_entry_point = EntryPoint::new(
        ENTRY_POINT_INIT,
        vec![],
        CLType::Unit,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    );

    // This establishes the `donate` entry point for callers looking to donate.
    let donate_entry_point = EntryPoint::new(
        ENTRY_POINT_DONATE,
        vec![Parameter::new(DONATING_ACCOUNT_KEY, CLType::Key)],
        CLType::URef,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    );

    // This establishes an entry point called `donation_count` that returns the amount of
    // donations from a specific account.
    let get_donation_count_entry_point = EntryPoint::new(
        ENTRY_POINT_GET_DONATION_COUNT,
        vec![Parameter::new(DONATING_ACCOUNT_KEY, CLType::Key)],
        CLType::U64,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    );

    // This establishes an entry point called `funds_raised` that returns the total amount
    // donated by all participants.
    let funds_raised_entry_point = EntryPoint::new(
        ENTRY_POINT_GET_FUNDS_RAISED,
        vec![],
        CLType::U512,
        EntryPointAccess::Public,
        EntryPointType::Contract,
    );
}

let mut entry_points = EntryPoints::new();
entry_points.add_entry_point(init_entry_point);
entry_points.add_entry_point(donate_entry_point);
entry_points.add_entry_point(get_donation_count_entry_point);
entry_points.add_entry_point(funds_raised_entry_point);


let (contract_hash, _contract_version) = storage::new_contract(
    entry_points,
    None,
    Some("fundraiser_package_hash".to_string()),
    Some("fundraiser_access_uref".to_string()),
);

runtime::put_key("fundraiser_contract_hash", contract_hash.into());
// Call the init entry point to setup and create the fundraising purse
// and the ledger to track donations made.
runtime::call_contract::<()>(contract_hash, ENTRY_POINT_INIT, runtime_args! {})


pub fn new_locked_contract(
    entry_points: EntryPoints,
    named_keys: Option<NamedKeys>,
    hash_name: Option<String>,
    uref_name: Option<String>,
) -> (ContractHash, ContractVersion) {
    create_contract(entry_points, named_keys, hash_name, uref_name, true)
}
