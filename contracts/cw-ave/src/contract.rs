use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg, ReceiveMsg};
use crate::state::{
    generate_instantiate_salt2, preamble_msg_arb_036, sha256, CheckInDetails, CheckInSignatureData,
    Config, EventSegments, GuestDetails, RegisteringGuest, TicketDetails, TicketPaymentOption,
    ATTENDANCE_RECORD, CONFIG, EVENT_STAGES, GUEST_DETAILS, RESERVED_TICKETS,
};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    from_json, instantiate2_address, to_json_binary, Addr, BankMsg, Binary, Coin, CosmosMsg, Deps,
    DepsMut, Env, MessageInfo, Order, Response, StdResult, Storage, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use cw20::Cw20ReceiveMsg;
use cw4::Member;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-shit-strap";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const NAMESPACE: &[u8] = b"aves";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // set owner
    let owner = match msg.event_curator.is_some() {
        true => deps
            .api
            .addr_validate(&msg.event_curator.clone().unwrap())?,
        false => info.sender,
    };

    // validate guest details
    for dt in msg.guest_details {
        // ensure no duplicate guest weights
        match GUEST_DETAILS.may_load(deps.storage, dt.guest_weight)? {
            Some(_) => return Err(ContractError::DuplicateGuestWeight {}),
            None => {
                GUEST_DETAILS.save(deps.storage, dt.guest_weight, &dt)?;
            }
        }
    }

    // validate event stages
    // ensure start & end dates are sequential
    // ensure the previous end date is before or at the next start date

    for (i, event) in msg.event_timeline.iter().enumerate() {
        // Validate that start date is before or equal to end date
        if event.start > event.end {
            return Err(ContractError::InvalidEventDates {});
        }

        // For events that are not the first, check that the previous end date is before or at the next start date
        if i > 0 {
            let prev_event = &msg.event_timeline[i - 1];
            if prev_event.end > event.start {
                return Err(ContractError::OverlappingEventDates {});
            }
        }

        // Save the event stage
        let event_stage_id = i + 1;
        EVENT_STAGES.save(deps.storage, event_stage_id as u64, &event)?;
    }

    // setup cw420 groups
    let cw721_checksum = deps.querier.query_wasm_code_info(msg.cw420)?;
    let usher_salt = generate_instantiate_salt2(&cw721_checksum.checksum, NAMESPACE);
    let mut guest_salt_data = usher_salt.to_vec();
    guest_salt_data[0] ^= 1; // Flip the first byte to ensure it's different
    let guest_salt = Binary::new(guest_salt_data);

    let contract_address = deps.api.addr_canonicalize(env.contract.address.as_str())?;

    let usher_cw420 = match instantiate2_address(
        cw721_checksum.checksum.as_slice(),
        &contract_address,
        usher_salt.as_slice(),
    ) {
        Ok(addr) => addr,
        Err(err) => return Err(ContractError::from(err)),
    };
    let guest_cw420 = match instantiate2_address(
        cw721_checksum.checksum.as_slice(),
        &contract_address,
        guest_salt.as_slice(),
    ) {
        Ok(addr) => addr,
        Err(err) => return Err(ContractError::from(err)),
    };

    let usher_msg = WasmMsg::Instantiate2 {
        admin: Some(owner.to_string()),
        code_id: msg.cw420,
        msg: to_json_binary(&cw420::msg::InstantiateMsg {
            admin: Some(env.contract.address.to_string()),
            members: msg.usher_admins,
        })?,
        funds: vec![],
        label: "cw-ave-ushers".to_string(),
        salt: usher_salt,
    };

    let guest_msg = WasmMsg::Instantiate2 {
        admin: Some(owner.to_string()),
        code_id: msg.cw420,
        msg: to_json_binary(&cw420::msg::InstantiateMsg {
            admin: Some(env.contract.address.to_string()),
            members: vec![],
        })?,
        funds: vec![],
        label: "cw-ave-guests".to_string(),
        salt: guest_salt,
    };

    CONFIG.save(
        deps.storage,
        &Config {
            title: msg.title,
            curator: msg
                .event_curator
                .unwrap_or(env.contract.address.to_string()),
            event_usher_contract: deps.api.addr_humanize(&usher_cw420)?,
            event_guest_contract: deps.api.addr_humanize(&guest_cw420)?,
        },
    )?;

    Ok(Response::new().add_messages(vec![usher_msg, guest_msg]))
}

// payment is made, address is recorded to state
// admin must confirm payment, releasing funds
// any non-confirmed payment can be returned
// limit of guest-type only met for confirmed guests

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Receive(cw20_msg) => receive_cw20_message(deps, info, cw20_msg),
        ExecuteMsg::PurchaseTickets { guests } => perform_ticket_purchase(deps, info, guests),
        ExecuteMsg::RefundUnconfirmedTickets { guests } => {
            refund_unconfirmed_ticket_purchase(deps, info, guests)
        }
        ExecuteMsg::CheckInGuest { checkin } => perform_checkin_guest(deps, info, checkin),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::EventSegments {} => to_json_binary(
            &EVENT_STAGES
                .range(deps.storage, None, None, cosmwasm_std::Order::Descending)
                .map(|item| item.map(|(_, v)| v.into()))
                .collect::<StdResult<Vec<EventSegments>>>()?
                .into_iter()
                .enumerate()
                .collect::<Vec<(usize, EventSegments)>>(),
        ),
        QueryMsg::GuestAttendanceStatus {
            guest,
            event_stage_id,
        } => to_json_binary(
            &ATTENDANCE_RECORD
                .may_load(deps.storage, (&guest, event_stage_id))?
                .unwrap_or_default(),
        ),
        QueryMsg::GuestAttendanceStatusAll { guest } => {
            let mut attendance_status = Vec::new();
            let prefix = ATTENDANCE_RECORD.prefix(&guest);
            for item in prefix.range(deps.storage, None, None, cosmwasm_std::Order::Ascending) {
                let (event_stage_id, status) = item?;
                attendance_status.push((event_stage_id, status));
            }
            to_json_binary(&attendance_status)
        }
        QueryMsg::TicketPaymentOptionsByGuestWeight { guest_weight } => {
            let gd = GUEST_DETAILS.load(deps.storage, guest_weight)?;
            Ok(to_json_binary(&TicketPaymentOption {
                guest_type: gd.guest_type,
                payment_options: gd.ticket_cost,
            })?)
        }
        QueryMsg::AllTicketPaymentOptions {} => Ok(to_json_binary(
            &GUEST_DETAILS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|res| {
                    res.map(|(_, guest_details)| TicketPaymentOption {
                        guest_type: guest_details.guest_type,
                        payment_options: guest_details.ticket_cost,
                    })
                })
                .collect::<StdResult<Vec<TicketPaymentOption>>>()?,
        )?),
        QueryMsg::GuestTypeDetailsByWeight { guest_weight } => Ok(to_json_binary(
            &GUEST_DETAILS.load(deps.storage, guest_weight)?,
        )?),
        QueryMsg::GuestTypeDetailsAll {} => Ok(to_json_binary(
            &GUEST_DETAILS
                .range(deps.storage, None, None, Order::Ascending)
                .map(|res| res.map(|(_, guest_details)| guest_details))
                .collect::<StdResult<Vec<GuestDetails>>>()?,
        )?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    Ok(Response::new())
}

/// Entry point to particpate in shitstrap
pub fn perform_checkin_guest(
    deps: DepsMut,
    info: MessageInfo,
    checkin: CheckInDetails,
) -> Result<Response, ContractError> {
    // sender must be one of event ushers
    let cfg = CONFIG.load(deps.storage)?;
    if check_if_cw420_member(deps.as_ref(), &cfg.event_usher_contract, &info.sender)?.is_none() {
        return Err(ContractError::NotAnEventUsher {});
    };

    // verify signature came from guest and is valid
    if !deps.api.secp256k1_verify(
        &sha256(preamble_msg_arb_036(&checkin.ticket_addr, &checkin.signed_data).as_bytes()),
        &checkin.signature,
        &checkin.pubkey,
    )? {
        return Err(ContractError::DuplicateGuestWeight {});
    };

    // parse signed_data to retrieve specific guest weight
    let signature_data: CheckInSignatureData = from_json(checkin.signed_data)?;

    if let Some(guest_weight) = check_if_cw420_member(
        deps.as_ref(),
        &cfg.event_guest_contract,
        &deps.api.addr_validate(&checkin.ticket_addr)?,
    )? {
        let guest_details = GUEST_DETAILS.load(deps.storage, guest_weight)?;

        // recurisvely update guest status for any
        match guest_details.event_segment_access {
            crate::state::EventSegmentAccessType::SingleSegment {} => {
                update_attendance_record(
                    deps.storage,
                    &checkin.ticket_addr,
                    signature_data.event_segment_id,
                )?;
            }
            crate::state::EventSegmentAccessType::SpecificSegments { ids } => {
                update_attendance_record(
                    deps.storage,
                    &checkin.ticket_addr,
                    signature_data.event_segment_id,
                )?;
                for id in ids {
                    update_attendance_record(deps.storage, &checkin.ticket_addr, id)?;
                }
            }
        }
    } else {
        return Err(ContractError::GuestTypeIncorrect {});
    };

    Ok(Response::new())
}

pub fn update_attendance_record(
    storage: &mut dyn Storage,
    ticket_addr: &String,
    event_segment_id: u64,
) -> Result<bool, ContractError> {
    ATTENDANCE_RECORD.update(storage, (ticket_addr, event_segment_id), |ci| {
        if let Some(status) = ci {
            match status {
                true => return Err(ContractError::GuestAlreadyCheckedIn {}),
                false => return Ok(true),
            }
        } else {
            return Err(ContractError::GuestTypeIncorrect {});
        }
    })
}
/// Entry point to purchase event tickets
pub fn perform_ticket_purchase(
    deps: DepsMut,
    info: MessageInfo,
    guests: Vec<RegisteringGuest>,
) -> Result<Response, ContractError> {
    let cfg = CONFIG.load(deps.storage)?;
    let mut msgs = Vec::new();

    for guest in guests {
        // check if guest type exists
        let gd = GUEST_DETAILS.load(deps.storage, guest.guest_weight)?;

        let sent = count_tickets_and_remainder(&info.funds, gd.ticket_cost);
        msgs.push(form_return_payment_overflow_msgs(&sent.1, &info.sender));

        // count how many tickets are being reserved from payment being made
        let reserved = sent.0;

        // save tickets reserved by the ticket wallet
        RESERVED_TICKETS.update(deps.storage, &guest.ticket_wallet, |a| match a {
            Some(mut td) => {
                td.reserved += reserved;
                if td.reserved > gd.max_ticket_limit.into() {
                    return Err(ContractError::CannotReserveTicketCount {});
                }
                return Ok::<TicketDetails, ContractError>(td);
            }
            None => {
                if reserved < gd.max_ticket_limit.into() {
                    return Ok(TicketDetails { reserved });
                } else {
                    return Err(ContractError::CannotReserveTicketCount {});
                }
            }
        })?;

        // add event guest to guest cw420 list
        msgs.push(
            form_update_guestlist_msg(
                &guest.ticket_wallet,
                gd.guest_weight,
                &cfg.event_guest_contract,
            )?
            .into(),
        );

        // if res.weight.is
    }
    Ok(Response::new())
}

fn refund_unconfirmed_ticket_purchase(
    deps: DepsMut,
    info: MessageInfo,
    guest: Vec<String>,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

fn receive_cw20_message(
    deps: DepsMut,
    info: MessageInfo,
    msg: Cw20ReceiveMsg,
) -> Result<Response, ContractError> {
    match from_json(&msg.msg)? {
        // only cw20 can call cw20 entry point.
        // we set the denom for the cw20 as info.sender,
        // which will result in error if any addr other than accepted cw20 makes call.
        ReceiveMsg::PurchaseTickets { guests } => {
            perform_ticket_purchase(deps, info.clone(), guests)
        }
    }
}

/// counts how many tickets are purchased, returning any overflow amounts sent.
fn count_tickets_and_remainder(
    info_funds: &Vec<Coin>,
    gd_ticket_cost: Vec<Coin>,
) -> (u128, Vec<Coin>) {
    let mut total_tickets = 0;
    let mut remaining_funds = info_funds.clone();
    for cost in gd_ticket_cost {
        if let Some(fund) = remaining_funds.iter_mut().find(|f| f.denom == cost.denom) {
            let tickets = fund.amount / cost.amount;
            total_tickets += tickets.u128();
            fund.amount -= tickets * cost.amount;
        } else {
            return (0, remaining_funds);
        }
    }

    // Filter out coins with zero amount
    remaining_funds.retain(|coin| coin.amount > Uint128::zero());

    (total_tickets, remaining_funds)
}

fn form_update_guestlist_msg(
    guest_addr: &String,
    guest_weight: u64,
    gust_cw420: &Addr,
) -> Result<WasmMsg, ContractError> {
    Ok(WasmMsg::Execute {
        contract_addr: gust_cw420.to_string(),
        msg: to_json_binary(&cw420::msg::ExecuteMsg::UpdateMembers {
            remove: vec![],
            add: vec![Member {
                addr: guest_addr.to_string(),
                weight: guest_weight,
            }],
        })?,
        funds: vec![],
    })
}

fn form_return_payment_overflow_msgs(overflow: &Vec<Coin>, sender: &Addr) -> CosmosMsg {
    CosmosMsg::Bank(BankMsg::Send {
        to_address: sender.to_string(),
        amount: overflow.clone(),
    })
}

fn check_if_cw420_member(
    deps: Deps,
    cw420: &Addr,
    wallet: &Addr,
) -> Result<Option<u64>, ContractError> {
    /// check if guest already is member
    let res: cw4::MemberResponse = deps.querier.query_wasm_smart(
        cw420,
        &cw420::msg::QueryMsg::Member {
            addr: wallet.to_string(),
            at_height: None,
        },
    )?;

    Ok(res.weight)
}
