use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, BlockInfo, Empty, Uint128};

use cw_denom::CheckedDenom;
use cw_storage_plus::{Item, Map};

pub const OUTPUT: Item<Addr> = Item::new("output");

// A receipt ID can have multiple payments. Only one payer can pay for a given
// receipt.

// Map receipt ID and incrementing payment ID (starting from 0 for a given
// receipt ID) to the payment.
pub const RECEIPT_PAYMENTS: Map<(String, u64), Payment> = Map::new("receipt_payments");
// Map receipt ID to the number of payments for that receipt so far.
pub const RECEIPT_PAYMENT_COUNT: Map<String, u64> = Map::new("receipt_payment_count");
// Map receipt ID and serialized denom to total payment amount.
pub const RECEIPT_TOTALS: Map<(String, String), Uint128> = Map::new("receipt_totals");

// Map authorized payer and receipt ID to an empty value, making it easy to
// check if a payer is authorized for a given receipt and list receipts for a
// given payer.
pub const PAYER_RECEIPTS: Map<(Addr, String), Empty> = Map::new("payer_receipts");
// Map payer and serialized denom to total payment amount.
pub const PAYER_TOTALS: Map<(Addr, String), Uint128> = Map::new("payer_totals");

#[cw_serde]
pub struct Payment {
    pub payer: Addr,
    pub block: BlockInfo,
    pub denom: CheckedDenom,
    pub amount: Uint128,
}
