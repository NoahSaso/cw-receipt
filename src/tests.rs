#![cfg(test)]
use cosmwasm_std::{coins, to_binary, Addr, Empty, Uint128};
use cw_denom::CheckedDenom;
use cw_multi_test::{App, BankSudo, Contract, ContractWrapper, Executor};

use crate::msg::{
    Cw20ReceiverMsg, ExecuteMsg, InstantiateMsg, ListIdsForPayerResponse, ListPaymentsResponse,
    ListPaymentsToIdResponse, ListTotalsPaidByPayerResponse, ListTotalsPaidToIdResponse,
    OutputResponse, QueryMsg, ReceiptPayment, ReceiptPaymentWithoutId, Total,
};
use crate::state::Payment;
use crate::ContractError;

const OUTPUT: &str = "output";
const OWNER: &str = "owner";
const PAYER: &str = "payer";
const OTHER_PAYER: &str = "other_payer";
const NATIVE_DENOM: &str = "uwasm";
const RECEIPT_ID: &str = "receipt_id";

fn setup_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

fn setup_cw20_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw20_base::contract::execute,
        cw20_base::contract::instantiate,
        cw20_base::contract::query,
    );
    Box::new(contract)
}

fn instantiate() -> (App, Addr, Addr) {
    let mut app = App::default();

    // Initialize payers native balances.
    app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
        to_address: PAYER.to_string(),
        amount: coins(10, NATIVE_DENOM),
    }))
    .unwrap();
    app.sudo(cw_multi_test::SudoMsg::Bank(BankSudo::Mint {
        to_address: OTHER_PAYER.to_string(),
        amount: coins(10, NATIVE_DENOM),
    }))
    .unwrap();

    // Instantiate contract.
    let code_id = app.store_code(setup_contract());
    let addr = app
        .instantiate_contract(
            code_id,
            Addr::unchecked(OWNER),
            &InstantiateMsg {
                owner: Some(OWNER.to_string()),
                output: OUTPUT.to_string(),
            },
            &[],
            "receipt",
            None,
        )
        .unwrap();

    // Instantiate cw20 contract.
    let cw20_code_id = app.store_code(setup_cw20_contract());
    let cw20_addr = app
        .instantiate_contract(
            cw20_code_id,
            Addr::unchecked(OWNER),
            &cw20_base::msg::InstantiateMsg {
                name: "Test".to_string(),
                symbol: "TEST".to_string(),
                decimals: 6,
                // Initialize payers cw20 balances.
                initial_balances: vec![
                    cw20::Cw20Coin {
                        address: PAYER.to_string(),
                        amount: Uint128::new(10),
                    },
                    cw20::Cw20Coin {
                        address: OTHER_PAYER.to_string(),
                        amount: Uint128::new(10),
                    },
                ],
                mint: None,
                marketing: None,
            },
            &[],
            "cw20",
            None,
        )
        .unwrap();

    (app, addr, cw20_addr)
}

#[test]
pub fn test_instantiate() {
    instantiate();
}

#[test]
pub fn test_updatable_owner() {
    let (mut app, addr, _) = instantiate();

    // Ensure owner is set.
    let res: cw_ownable::Ownership<String> = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Ownership {})
        .unwrap();
    assert_eq!(res.owner, Some(OWNER.to_string()));

    // Update owner.
    let new_owner = "new_owner";
    app.execute_contract(
        Addr::unchecked(OWNER),
        addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
            new_owner: new_owner.to_string(),
            expiry: None,
        }),
        &[],
    )
    .unwrap();

    // Accept ownership transfer.
    app.execute_contract(
        Addr::unchecked(new_owner),
        addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::AcceptOwnership),
        &[],
    )
    .unwrap();

    // Ensure owner is updated to new owner.
    let res: cw_ownable::Ownership<String> = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Ownership {})
        .unwrap();
    assert_eq!(res.owner, Some(new_owner.to_string()));

    // Ensure old owner can no longer update.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(OWNER),
            addr.clone(),
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
                new_owner: "new_new_owner".to_string(),
                expiry: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Ownable(cw_ownable::OwnershipError::NotOwner)
    );

    // Renounce ownership.
    app.execute_contract(
        Addr::unchecked(new_owner),
        addr.clone(),
        &ExecuteMsg::UpdateOwnership(cw_ownable::Action::RenounceOwnership),
        &[],
    )
    .unwrap();

    // Ensure new owner is removed.
    let res: cw_ownable::Ownership<String> = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Ownership {})
        .unwrap();
    assert_eq!(res.owner, None);

    // Ensure new owner can no longer update.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(new_owner),
            addr.clone(),
            &ExecuteMsg::UpdateOwnership(cw_ownable::Action::TransferOwnership {
                new_owner: new_owner.to_string(),
                expiry: None,
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Ownable(cw_ownable::OwnershipError::NoOwner)
    );

    // Ensure new owner is still removed.
    let res: cw_ownable::Ownership<String> = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::Ownership {})
        .unwrap();
    assert_eq!(res.owner, None);
}

#[test]
pub fn test_updatable_output() {
    let (mut app, addr, _) = instantiate();

    // Ensure output is set.
    let res: OutputResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Output {})
        .unwrap();
    assert_eq!(
        res,
        OutputResponse {
            output: Addr::unchecked(OUTPUT)
        }
    );

    // Update output.
    let new_output = "new_output";
    app.execute_contract(
        Addr::unchecked(OWNER),
        addr.clone(),
        &ExecuteMsg::UpdateOutput {
            output: new_output.to_string(),
        },
        &[],
    )
    .unwrap();

    // Ensure output is updated.
    let res: OutputResponse = app
        .wrap()
        .query_wasm_smart(addr.clone(), &QueryMsg::Output {})
        .unwrap();
    assert_eq!(
        res,
        OutputResponse {
            output: Addr::unchecked(new_output)
        }
    );

    // Ensure non-owner cannot update.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("non_owner"),
            addr.clone(),
            &ExecuteMsg::UpdateOutput {
                output: "non_owner_output".to_string(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::Ownable(cw_ownable::OwnershipError::NotOwner)
    );

    // Ensure output is the same as before.
    let res: OutputResponse = app
        .wrap()
        .query_wasm_smart(addr, &QueryMsg::Output {})
        .unwrap();
    assert_eq!(
        res,
        OutputResponse {
            output: Addr::unchecked(new_output)
        }
    );
}

#[test]
pub fn test_native_pay() {
    let (mut app, addr, _) = instantiate();
    let block = app.block_info();

    // Ensure output has no balance.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::zero());

    // Ensure cannot pay with no native tokens.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(PAYER),
            addr.clone(),
            &ExecuteMsg::Pay {
                id: RECEIPT_ID.to_string(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::MissingPayment);

    // Pay with native tokens.
    let amount: u128 = 2;
    app.execute_contract(
        Addr::unchecked(PAYER),
        addr.clone(),
        &ExecuteMsg::Pay {
            id: RECEIPT_ID.to_string(),
        },
        &coins(amount, NATIVE_DENOM),
    )
    .unwrap();

    // Ensure output has balance.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(amount));

    // Ensure contract still has no balance.
    let balance = app
        .wrap()
        .query_balance(addr.clone(), NATIVE_DENOM)
        .unwrap();
    assert_eq!(balance.amount, Uint128::zero());

    // Ensure payment #0 is stored for receipt.
    let res: ListPaymentsToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPaymentsToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsToIdResponse {
            payments: vec![ReceiptPaymentWithoutId {
                receipt_payment_id: 0,
                payment: Payment {
                    payer: Addr::unchecked(PAYER),
                    block: block.clone(),
                    denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                    amount: Uint128::from(amount),
                }
            }]
        }
    );

    // Ensure payment #0 returned in master list.
    let res: ListPaymentsResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPayments {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsResponse {
            payments: vec![ReceiptPayment {
                receipt_id: RECEIPT_ID.to_string(),
                receipt_payment_id: 0,
                payment: Payment {
                    payer: Addr::unchecked(PAYER),
                    block: block.clone(),
                    denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                    amount: Uint128::from(amount),
                }
            }]
        }
    );

    // Ensure receipt ID listed for payer.
    let res: ListIdsForPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListIdsForPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListIdsForPayerResponse {
            ids: vec![RECEIPT_ID.to_string()]
        }
    );

    // Try to pay with native tokens to same receipt ID from different payer.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(OTHER_PAYER),
            addr.clone(),
            &ExecuteMsg::Pay {
                id: RECEIPT_ID.to_string(),
            },
            &coins(amount, NATIVE_DENOM),
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::UnauthorizedPayer);

    // Ensure output has same balance.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(amount));

    // Ensure receipt ID not listed for other payer.
    let res: ListIdsForPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListIdsForPayer {
                payer: OTHER_PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListIdsForPayerResponse { ids: vec![] });

    // Pay with native tokens to same receipt ID.
    app.execute_contract(
        Addr::unchecked(PAYER),
        addr.clone(),
        &ExecuteMsg::Pay {
            id: RECEIPT_ID.to_string(),
        },
        &coins(amount * 2, NATIVE_DENOM),
    )
    .unwrap();

    // Ensure output balance increased.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(amount * 3));

    // Ensure two payments are stored for receipt.
    let res: ListPaymentsToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPaymentsToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsToIdResponse {
            payments: vec![
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(amount),
                    }
                },
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(amount * 2),
                    }
                }
            ]
        }
    );

    // Ensure two payments are stored in master list.
    let res: ListPaymentsResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPayments {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsResponse {
            payments: vec![
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(amount),
                    }
                },
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block,
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(amount * 2),
                    }
                }
            ]
        }
    );

    // Ensure total accumulated for receipt.
    let res: ListTotalsPaidToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidToIdResponse {
            totals: vec![Total {
                denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                amount: Uint128::from(amount * 3),
            }]
        }
    );

    // Ensure total accumulated for payer.
    let res: ListTotalsPaidByPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidByPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidByPayerResponse {
            totals: vec![Total {
                denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                amount: Uint128::from(amount * 3),
            }]
        }
    );

    // Ensure no total accumulated for other payer.
    let res: ListTotalsPaidByPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidByPayer {
                payer: OTHER_PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListTotalsPaidByPayerResponse { totals: vec![] });

    // Ensure no total accumulated for unused receipt.
    let res: ListTotalsPaidToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr,
            &QueryMsg::ListTotalsPaidToId {
                id: "unused_receipt".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListTotalsPaidToIdResponse { totals: vec![] });
}

#[test]
pub fn test_cw20_pay() {
    let (mut app, addr, cw20_addr) = instantiate();
    let block = app.block_info();

    // Ensure output has no balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::zero());

    // Pay with cw20 tokens.
    let amount: u128 = 2;
    app.execute_contract(
        Addr::unchecked(PAYER),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: addr.to_string(),
            amount: Uint128::from(amount),
            msg: to_binary(&Cw20ReceiverMsg::Pay {
                id: RECEIPT_ID.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Ensure output has balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::from(amount));

    // Ensure contract still has no balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: addr.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::zero());

    // Ensure payment #0 is stored for receipt.
    let res: ListPaymentsToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPaymentsToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsToIdResponse {
            payments: vec![ReceiptPaymentWithoutId {
                receipt_payment_id: 0,
                payment: Payment {
                    payer: Addr::unchecked(PAYER),
                    block: block.clone(),
                    denom: CheckedDenom::Cw20(cw20_addr.clone()),
                    amount: Uint128::from(amount),
                }
            }]
        }
    );

    // Ensure payment #0 is stored in master list.
    let res: ListPaymentsResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPayments {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsResponse {
            payments: vec![ReceiptPayment {
                receipt_id: RECEIPT_ID.to_string(),
                receipt_payment_id: 0,
                payment: Payment {
                    payer: Addr::unchecked(PAYER),
                    block: block.clone(),
                    denom: CheckedDenom::Cw20(cw20_addr.clone()),
                    amount: Uint128::from(amount),
                }
            }]
        }
    );

    // Ensure receipt ID listed for payer.
    let res: ListIdsForPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListIdsForPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListIdsForPayerResponse {
            ids: vec![RECEIPT_ID.to_string()]
        }
    );

    // Try to pay with cw20 tokens to same receipt ID from different payer.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked(OTHER_PAYER),
            cw20_addr.clone(),
            &cw20::Cw20ExecuteMsg::Send {
                contract: addr.to_string(),
                amount: Uint128::from(amount),
                msg: to_binary(&Cw20ReceiverMsg::Pay {
                    id: RECEIPT_ID.to_string(),
                })
                .unwrap(),
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::UnauthorizedPayer);

    // Ensure output has same balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::from(amount));

    // Ensure receipt ID not listed for other payer.
    let res: ListIdsForPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListIdsForPayer {
                payer: OTHER_PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListIdsForPayerResponse { ids: vec![] });

    // Pay with cw20 tokens to same receipt ID.
    app.execute_contract(
        Addr::unchecked(PAYER),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: addr.to_string(),
            amount: Uint128::from(amount * 2),
            msg: to_binary(&Cw20ReceiverMsg::Pay {
                id: RECEIPT_ID.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Ensure output balance increased.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::from(amount * 3));

    // Ensure two payments are stored for receipt.
    let res: ListPaymentsToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPaymentsToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsToIdResponse {
            payments: vec![
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(amount),
                    }
                },
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(amount * 2),
                    }
                }
            ]
        }
    );

    // Ensure two payments are stored in master list.
    let res: ListPaymentsResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPayments {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsResponse {
            payments: vec![
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(amount),
                    }
                },
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block,
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(amount * 2),
                    }
                }
            ]
        }
    );

    // Ensure total accumulated for receipt.
    let res: ListTotalsPaidToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidToIdResponse {
            totals: vec![Total {
                denom: CheckedDenom::Cw20(cw20_addr.clone()),
                amount: Uint128::from(amount * 3),
            }]
        }
    );

    // Ensure total accumulated for payer.
    let res: ListTotalsPaidByPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidByPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidByPayerResponse {
            totals: vec![Total {
                denom: CheckedDenom::Cw20(cw20_addr),
                amount: Uint128::from(amount * 3),
            }]
        }
    );

    // Ensure no total accumulated for other payer.
    let res: ListTotalsPaidByPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidByPayer {
                payer: OTHER_PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListTotalsPaidByPayerResponse { totals: vec![] });

    // Ensure no total accumulated for unused receipt.
    let res: ListTotalsPaidToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr,
            &QueryMsg::ListTotalsPaidToId {
                id: "unused_receipt".to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(res, ListTotalsPaidToIdResponse { totals: vec![] });
}

#[test]
pub fn test_both_pay() {
    let (mut app, addr, cw20_addr) = instantiate();
    let block = app.block_info();

    // Ensure output has no native balance.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::zero());

    // Ensure output has no cw20 balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::zero());

    // Pay with native tokens.
    let native_amount: u128 = 2;
    app.execute_contract(
        Addr::unchecked(PAYER),
        addr.clone(),
        &ExecuteMsg::Pay {
            id: RECEIPT_ID.to_string(),
        },
        &coins(native_amount, NATIVE_DENOM),
    )
    .unwrap();

    // Pay with cw20 tokens.
    let cw20_amount: u128 = 3;
    app.execute_contract(
        Addr::unchecked(PAYER),
        cw20_addr.clone(),
        &cw20::Cw20ExecuteMsg::Send {
            contract: addr.to_string(),
            amount: Uint128::from(cw20_amount),
            msg: to_binary(&Cw20ReceiverMsg::Pay {
                id: RECEIPT_ID.to_string(),
            })
            .unwrap(),
        },
        &[],
    )
    .unwrap();

    // Ensure output has native balance.
    let balance = app.wrap().query_balance(OUTPUT, NATIVE_DENOM).unwrap();
    assert_eq!(balance.amount, Uint128::from(native_amount));

    // Ensure output has cw20 balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: OUTPUT.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::from(cw20_amount));

    // Ensure contract still has no native balance.
    let balance = app
        .wrap()
        .query_balance(addr.clone(), NATIVE_DENOM)
        .unwrap();
    assert_eq!(balance.amount, Uint128::zero());

    // Ensure contract still has no cw20 balance.
    let res: cw20::BalanceResponse = app
        .wrap()
        .query_wasm_smart(
            cw20_addr.clone(),
            &cw20::Cw20QueryMsg::Balance {
                address: addr.to_string(),
            },
        )
        .unwrap();
    assert_eq!(res.balance, Uint128::zero());

    // Ensure two payments are stored for receipt. First native, then cw20.
    let res: ListPaymentsToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPaymentsToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsToIdResponse {
            payments: vec![
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(native_amount),
                    }
                },
                ReceiptPaymentWithoutId {
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(cw20_amount),
                    }
                }
            ]
        }
    );

    // Ensure two payments are stored in master list. First native, then cw20.
    let res: ListPaymentsResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListPayments {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListPaymentsResponse {
            payments: vec![
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 0,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block: block.clone(),
                        denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                        amount: Uint128::from(native_amount),
                    }
                },
                ReceiptPayment {
                    receipt_id: RECEIPT_ID.to_string(),
                    receipt_payment_id: 1,
                    payment: Payment {
                        payer: Addr::unchecked(PAYER),
                        block,
                        denom: CheckedDenom::Cw20(cw20_addr.clone()),
                        amount: Uint128::from(cw20_amount),
                    }
                }
            ]
        }
    );

    // Ensure receipt ID listed for payer.
    let res: ListIdsForPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListIdsForPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListIdsForPayerResponse {
            ids: vec![RECEIPT_ID.to_string()]
        }
    );

    // Ensure both totals accumulated for receipt.
    let res: ListTotalsPaidToIdResponse = app
        .wrap()
        .query_wasm_smart(
            addr.clone(),
            &QueryMsg::ListTotalsPaidToId {
                id: RECEIPT_ID.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidToIdResponse {
            totals: vec![
                Total {
                    denom: CheckedDenom::Cw20(cw20_addr.clone()),
                    amount: Uint128::from(cw20_amount),
                },
                Total {
                    denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                    amount: Uint128::from(native_amount),
                }
            ]
        }
    );

    // Ensure both totals accumulated for payer.
    let res: ListTotalsPaidByPayerResponse = app
        .wrap()
        .query_wasm_smart(
            addr,
            &QueryMsg::ListTotalsPaidByPayer {
                payer: PAYER.to_string(),
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(
        res,
        ListTotalsPaidByPayerResponse {
            totals: vec![
                Total {
                    denom: CheckedDenom::Cw20(cw20_addr),
                    amount: Uint128::from(cw20_amount),
                },
                Total {
                    denom: CheckedDenom::Native(NATIVE_DENOM.to_string()),
                    amount: Uint128::from(native_amount),
                }
            ]
        }
    );
}
