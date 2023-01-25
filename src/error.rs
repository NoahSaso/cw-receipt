use cosmwasm_std::StdError;
use cw_denom::DenomError;
use cw_ownable::OwnershipError;
use cw_utils::PaymentError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error(transparent)]
    Ownable(#[from] OwnershipError),

    #[error("{0}")]
    PaymentError(#[from] PaymentError),

    #[error(transparent)]
    Denom(#[from] DenomError),

    #[error("Invalid denom")]
    InvalidDenom,

    #[error("Missing payment")]
    MissingPayment,

    #[error("Unauthorized payer")]
    UnauthorizedPayer,
}
