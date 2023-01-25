use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, CosmosMsg, Deps, DepsMut, Empty, Env, MessageInfo,
    Response, StdError, StdResult, Storage, Uint128,
};
use cw20::Cw20ReceiveMsg;
use cw_denom::{CheckedDenom, DenomError, UncheckedDenom};
use cw_storage_plus::Bound;
use cw_utils::nonpayable;

use crate::error::ContractError;
use crate::msg::{
    Cw20ReceiverMsg, ExecuteMsg, InstantiateMsg, ListIdsForPayerResponse, ListPaymentsToIdResponse,
    ListTotalsPaidByPayerResponse, ListTotalsPaidToIdResponse, OutputResponse, PaymentWithId,
    QueryMsg, Total,
};
use crate::state::{
    Payment, OUTPUT, PAYER_RECEIPTS, PAYER_TOTALS, RECEIPT_PAYMENTS, RECEIPT_PAYMENT_COUNT,
    RECEIPT_TOTALS,
};
use cosmwasm_std::entry_point;
use cw2::set_contract_version;

// Version info for migration
const CONTRACT_NAME: &str = "crates.io:cw-receipt";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    cw_ownable::initialize_owner(deps.storage, deps.api, msg.owner.as_deref())?;

    let output = deps.api.addr_validate(&msg.output)?;
    OUTPUT.save(deps.storage, &output)?;

    Ok(Response::default()
        .add_attribute("method", "instantiate")
        .add_attribute("output", output.to_string())
        .add_attribute("owner", msg.owner.unwrap_or_default()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(msg) => execute_receive_cw20(deps, env, info, msg),
        ExecuteMsg::Pay { id } => execute_pay(deps, env, info, id),
        ExecuteMsg::UpdateOutput { output } => execute_update_output(deps, info, output),
        ExecuteMsg::UpdateOwnership(action) => execute_update_owner(deps, env, info, action),
    }
}

pub fn execute_receive_cw20(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    receive_msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    // Don't accept native tokens.
    nonpayable(&info)?;

    let msg: Cw20ReceiverMsg = from_binary(&receive_msg.msg)?;

    // Validate payer.
    let payer = deps.api.addr_validate(&receive_msg.sender)?;

    // Require cw20 tokens.
    let unchecked_denom = UncheckedDenom::Cw20(info.sender.to_string());
    let checked = unchecked_denom.into_checked(deps.as_ref())?;

    match msg {
        Cw20ReceiverMsg::Pay { id } => {
            let transfer_msg = record_payment_and_get_transfer_msg(
                deps.storage,
                &env,
                &id,
                &checked,
                payer,
                receive_msg.amount,
            )?;

            Ok(Response::new()
                .add_message(transfer_msg)
                .add_attribute("method", "receive_cw20")
                .add_attribute("id", id)
                .add_attribute("payer", receive_msg.sender))
        }
    }
}

pub fn execute_pay(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    id: String,
) -> Result<Response, ContractError> {
    // Require native tokens.
    if info.funds.is_empty() {
        return Err(ContractError::MissingPayment);
    }

    // Require native tokens.
    let checked_funds = info
        .funds
        .iter()
        .map(|fund| {
            UncheckedDenom::Native(fund.denom.clone())
                .into_checked(deps.as_ref())
                .map(|checked_denom| (checked_denom, fund.amount))
        })
        .collect::<Result<Vec<(CheckedDenom, Uint128)>, DenomError>>()?;

    // Record payments and get transfer messages.
    let transfer_msgs = checked_funds
        .into_iter()
        .map(|(checked_denom, amount)| {
            record_payment_and_get_transfer_msg(
                deps.storage,
                &env,
                &id,
                &checked_denom,
                info.sender.clone(),
                amount,
            )
        })
        .collect::<Result<Vec<CosmosMsg>, ContractError>>()?;

    Ok(Response::new()
        .add_messages(transfer_msgs)
        .add_attribute("method", "pay")
        .add_attribute("id", id)
        .add_attribute("payer", info.sender))
}

pub fn execute_update_output(
    deps: DepsMut,
    info: MessageInfo,
    output: String,
) -> Result<Response, ContractError> {
    cw_ownable::assert_owner(deps.storage, &info.sender)?;

    let output_addr = deps.api.addr_validate(&output)?;
    OUTPUT.save(deps.storage, &output_addr)?;

    Ok(Response::default()
        .add_attribute("action", "update_output")
        .add_attribute("output", output))
}

pub fn execute_update_owner(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    action: cw_ownable::Action,
) -> Result<Response, ContractError> {
    let ownership = cw_ownable::update_ownership(deps, &env.block, &info.sender, action)?;
    Ok(Response::default().add_attributes(ownership.into_attributes()))
}

fn record_payment_and_get_transfer_msg(
    storage: &mut dyn Storage,
    env: &Env,
    id: &String,
    denom: &CheckedDenom,
    payer: Addr,
    amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
    let output = OUTPUT.load(storage)?;

    // Get past payment count for receipt.
    let receipt_payment_count = RECEIPT_PAYMENT_COUNT
        .may_load(storage, id.to_string())?
        .unwrap_or(0);

    // If at least one payment, verify payer is authorized for this receipt.
    // Only one payer can pay for a receipt, determined by the first payment.
    if receipt_payment_count > 0 {
        let payer_authorized_for_receipt =
            PAYER_RECEIPTS.has(storage, (payer.clone(), id.to_string()));
        if !payer_authorized_for_receipt {
            return Err(ContractError::UnauthorizedPayer);
        }
    }
    // If no payments, set payer.
    else {
        PAYER_RECEIPTS.save(storage, (payer.clone(), id.to_string()), &Empty {})?;
    }

    // Record payment.
    RECEIPT_PAYMENTS.save(
        storage,
        (id.to_string(), receipt_payment_count),
        &Payment {
            block: env.block.clone(),
            denom: denom.clone(),
            amount,
        },
    )?;
    // Increment payment count.
    RECEIPT_PAYMENT_COUNT.update(storage, id.to_string(), |count| {
        Ok::<u64, StdError>(count.unwrap_or(0) + 1)
    })?;
    // Increase totals.
    RECEIPT_TOTALS.update(storage, (id.to_string(), denom_to_string(denom)), |total| {
        Ok::<Uint128, StdError>(total.unwrap_or(Uint128::zero()) + amount)
    })?;
    PAYER_TOTALS.update(storage, (payer, denom_to_string(denom)), |total| {
        Ok::<Uint128, StdError>(total.unwrap_or(Uint128::zero()) + amount)
    })?;

    Ok(denom.get_transfer_to_message(&output, amount)?)
}

fn denom_to_string(denom: &CheckedDenom) -> String {
    match denom {
        CheckedDenom::Native(denom) => format!("n{}", denom),
        CheckedDenom::Cw20(denom) => format!("c{}", denom),
    }
}

fn string_to_denom(s: String) -> Option<CheckedDenom> {
    let (prefix, denom) = s.split_at(1);
    match prefix {
        "n" => Some(CheckedDenom::Native(denom.to_string())),
        "c" => Some(CheckedDenom::Cw20(Addr::unchecked(denom))),
        _ => None,
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ListPaymentsToId {
            id,
            start_after,
            limit,
        } => query_list_payments_to_id(deps, id, start_after, limit),

        QueryMsg::ListTotalsPaidToId {
            id,
            start_after,
            limit,
        } => query_list_totals_paid_to_id(deps, id, start_after, limit),

        QueryMsg::ListIdsForPayer {
            payer,
            start_after,
            limit,
        } => query_list_ids_for_payer(deps, payer, start_after, limit),

        QueryMsg::ListTotalsPaidByPayer {
            payer,
            start_after,
            limit,
        } => query_list_totals_paid_by_payer(deps, payer, start_after, limit),

        QueryMsg::Output {} => to_binary(&OutputResponse {
            output: OUTPUT.load(deps.storage)?,
        }),

        QueryMsg::Ownership {} => to_binary(&cw_ownable::get_ownership(deps.storage)?),
    }
}

pub fn query_list_payments_to_id(
    deps: Deps,
    id: String,
    start_after: Option<u64>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let payments = cw_paginate::paginate_map_prefix(
        RECEIPT_PAYMENTS,
        deps.storage,
        id,
        start_after.map(Bound::exclusive),
        limit,
        |id, payment| Ok::<PaymentWithId, StdError>(PaymentWithId { id, payment }),
    )?;

    to_binary(&ListPaymentsToIdResponse { payments })
}

pub fn query_list_totals_paid_to_id(
    deps: Deps,
    id: String,
    start_after: Option<CheckedDenom>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let totals = cw_paginate::paginate_map_prefix(
        RECEIPT_TOTALS,
        deps.storage,
        id,
        start_after.map(|denom| Bound::exclusive(denom_to_string(&denom))),
        limit,
        |string_denom, amount| {
            Ok::<Option<Total>, StdError>(
                string_to_denom(string_denom).map(|denom| Total { denom, amount }),
            )
        },
    )?
    .into_iter()
    .flatten()
    .collect();

    to_binary(&ListTotalsPaidToIdResponse { totals })
}

pub fn query_list_ids_for_payer(
    deps: Deps,
    payer: String,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let payer = deps.api.addr_validate(&payer)?;

    let ids = cw_paginate::paginate_map_prefix(
        PAYER_RECEIPTS,
        deps.storage,
        payer,
        start_after.map(Bound::exclusive),
        limit,
        |id, _| Ok::<String, StdError>(id),
    )?;

    to_binary(&ListIdsForPayerResponse { ids })
}

pub fn query_list_totals_paid_by_payer(
    deps: Deps,
    payer: String,
    start_after: Option<CheckedDenom>,
    limit: Option<u32>,
) -> StdResult<Binary> {
    let payer = deps.api.addr_validate(&payer)?;

    let totals = cw_paginate::paginate_map_prefix(
        PAYER_TOTALS,
        deps.storage,
        payer,
        start_after.map(|denom| Bound::exclusive(denom_to_string(&denom))),
        limit,
        |string_denom, amount| {
            Ok::<Option<Total>, StdError>(
                string_to_denom(string_denom).map(|denom| Total { denom, amount }),
            )
        },
    )?
    .into_iter()
    .flatten()
    .collect();

    to_binary(&ListTotalsPaidByPayerResponse { totals })
}
