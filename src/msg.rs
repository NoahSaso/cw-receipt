use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};
use cw20::Cw20ReceiveMsg;
use cw_denom::CheckedDenom;
use cw_ownable::{cw_ownable_execute, cw_ownable_query};

use crate::state::Payment;

#[cw_serde]
pub struct InstantiateMsg {
    /// The owner can change the owner and output address.
    pub owner: Option<String>,
    /// The output address is where all funds are sent.
    pub output: String,
}

#[cw_ownable_execute]
#[cw_serde]
pub enum ExecuteMsg {
    /// Receive a cw20 token payment.
    Receive(Cw20ReceiveMsg),
    /// Pay a native token payment.
    Pay { id: String },
    /// Update output. Only the owner can call this.
    UpdateOutput { output: String },
}

// Cw20 receiver message
#[cw_serde]
pub enum Cw20ReceiverMsg {
    Pay { id: String },
}

#[cw_ownable_query]
#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns the output address.
    #[returns(OutputResponse)]
    Output {},

    /// Returns list of payments for all receipts and payers.
    #[returns(ListPaymentsResponse)]
    ListPayments {
        start_after: Option<(String, u64)>,
        limit: Option<u32>,
    },

    /// Returns list of payments for receipt ID.
    #[returns(ListPaymentsToIdResponse)]
    ListPaymentsToId {
        id: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },

    /// Returns total paid per-denom to receipt ID.
    #[returns(ListTotalsPaidToIdResponse)]
    ListTotalsPaidToId {
        id: String,
        start_after: Option<CheckedDenom>,
        limit: Option<u32>,
    },

    /// Returns list of receipt IDs for payer.
    #[returns(ListIdsForPayerResponse)]
    ListIdsForPayer {
        payer: String,
        start_after: Option<String>,
        limit: Option<u32>,
    },

    /// Returns total paid per-denom by payer across all receipt IDs.
    #[returns(ListTotalsPaidByPayerResponse)]
    ListTotalsPaidByPayer {
        payer: String,
        start_after: Option<CheckedDenom>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct OutputResponse {
    pub output: Addr,
}

#[cw_serde]
pub struct ReceiptPaymentWithoutId {
    pub receipt_payment_id: u64,
    pub payment: Payment,
}

#[cw_serde]
pub struct ReceiptPayment {
    pub receipt_id: String,
    pub receipt_payment_id: u64,
    pub payment: Payment,
}

#[cw_serde]
pub struct Total {
    pub denom: CheckedDenom,
    pub amount: Uint128,
}

#[cw_serde]
pub struct ListPaymentsResponse {
    pub payments: Vec<ReceiptPayment>,
}

#[cw_serde]
pub struct ListPaymentsToIdResponse {
    pub payments: Vec<ReceiptPaymentWithoutId>,
}

#[cw_serde]
pub struct ListTotalsPaidToIdResponse {
    pub totals: Vec<Total>,
}

#[cw_serde]
pub struct ListIdsForPayerResponse {
    pub ids: Vec<String>,
}

#[cw_serde]
pub struct ListTotalsPaidByPayerResponse {
    pub totals: Vec<Total>,
}
