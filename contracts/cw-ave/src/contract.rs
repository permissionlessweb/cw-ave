use crate::error::ContractError;
use crate::msg::{EventSegmentRes, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};
use crate::state::{
    generate_instantiate_salt2, preamble_msg_arb_036, sha256, CheckInDetails, CheckInSignatureData,
    Config, GuestDetails, RegisteringEventAddressAndPayment, RegisteringGuest, TicketPaymentOption,
    ATTENDANCE_RECORD, CONFIG, EVENT_STAGES, GUEST_DETAILS, LICENSE_ADDR, RESERVED_TICKETS,
    TOTAL_RESERVED_BY_GUEST,
};
use av_event_helpers::{get_license_addr, LICENSE_CANONICAL_ADDR};
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    coin, from_json, instantiate2_address, to_json_binary, Addr, BankMsg, Binary, CanonicalAddr,
    Coin, CosmosMsg, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult, Storage, WasmMsg,
};
use cw2::set_contract_version;
use cw4::Member;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw-ave";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const NAMESPACE: &[u8] = b"aves";
pub const CHARACTER_LIMIT: usize = 128;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    LICENSE_ADDR.save(deps.storage, &get_license_addr(&env.block.chain_id)?)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    // set owner
    let curator = deps.api.addr_validate(&msg.event_curator.clone())?;

    // generic validation
    if msg.title.len() > CHARACTER_LIMIT || msg.description.len() > CHARACTER_LIMIT {
        return Err(ContractError::BadEventTitleOrDescription {});
    }

    // validate guest details
    for dt in msg.guest_details {
        // ensure no duplicate guest weights
        match GUEST_DETAILS.may_load(deps.storage, dt.guest_weight)? {
            Some(_) => return Err(ContractError::DuplicateGuestWeight {}),
            None => {
                if dt.guest_type.len() > CHARACTER_LIMIT
                    || dt.max_ticket_limit > dt.total_ticket_limit
                {
                    return Err(ContractError::BadGuestDetailParams {});
                }

                // cannot set duplicate accepted tokens
                let mut unique = Vec::new();
                for fee in &dt.ticket_cost {
                    if unique.contains(&fee.denom) {
                        return Err(ContractError::DuplicateFeeDenom {});
                    }
                    unique.push(fee.denom.to_string());
                }

                GUEST_DETAILS.save(deps.storage, dt.guest_weight, &dt)?;
                TOTAL_RESERVED_BY_GUEST.save(deps.storage, dt.guest_weight, &0)?;
            }
        }
    }

    // validate event stages
    for (i, event) in msg.event_timeline.iter().enumerate() {
        // Validate that start date is before or equal to end date
        if event.start > event.end {
            return Err(ContractError::InvalidEventDates {});
        }
        if event.stage_description.len() > 128usize {
            return Err(ContractError::BadEventDescriptionLength {});
        }

        // For events that are not the first, check that the previous end date is before or at the next start date
        if i > 0 {
            let prev_event = &msg.event_timeline[i - 1];
            if prev_event.end > event.start {
                return Err(ContractError::OverlappingEventDates {});
            }
        }

        // Save the event stage
        EVENT_STAGES.save(deps.storage, i as u64, event)?;
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
        admin: Some(env.contract.address.to_string()),
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
        admin: Some(env.contract.address.to_string()),
        code_id: msg.cw420,
        msg: to_json_binary(&cw420::msg::InstantiateMsg {
            admin: Some(env.contract.address.to_string()),
            members: vec![],
        })?,
        funds: vec![],
        label: "cw-ave-guests".to_string(),
        salt: guest_salt,
    };
    let event_usher_contract = deps.api.addr_humanize(&usher_cw420)?;
    let event_guest_contract = deps.api.addr_humanize(&guest_cw420)?;

    CONFIG.save(
        deps.storage,
        &Config {
            title: msg.title,
            curator,
            event_usher_contract,
            event_guest_contract,
        },
    )?;

    Ok(Response::new().add_messages(vec![usher_msg, guest_msg]))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::PurchaseTickets { guests } => perform_ticket_purchase(deps, info, guests),
        ExecuteMsg::RefundUnconfirmedTickets { guests } => {
            refund_unconfirmed_ticket_purchase(deps, info, guests)
        }
        ExecuteMsg::CheckInGuest { checkin } => perform_checkin_guest(deps, info, checkin),
        ExecuteMsg::ClaimTicketPayments {} => perform_claim_ticket_payments(deps, env, info),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&CONFIG.load(deps.storage)?),
        QueryMsg::EventSegments {} => {
            let segments: Vec<EventSegmentRes> = EVENT_STAGES
                .range(deps.storage, None, None, cosmwasm_std::Order::Descending)
                .map(|item| item.map(|(seg_id, segment)| EventSegmentRes { seg_id, segment }))
                .collect::<StdResult<Vec<EventSegmentRes>>>()?;

            to_json_binary(&segments)
        }
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
        let count = TOTAL_RESERVED_BY_GUEST.load(deps.storage, gd.guest_weight)?;

        // Calculate how many tickets we can actually process (respecting the limit)
        let max_possible = gd.total_ticket_limit.saturating_sub(count) as usize;
        let process_count = guest.reap.len().min(max_possible);

        // Split the guest list - prioritize first entries in the array
        let to_process = &guest.reap[..process_count];
        // todo: implmeent overbooking feature where we can still accept these payments if neccessary
        // let overflow = &guest.reap[process_count..];
        // if !overflow.is_empty() {
        // }

        // if len of guest.reap is greater than gd.limit, strip the # of entries in guest.reap from the object so that we will reach the limit and not error.
        let (reserved, remaining_funds, dev_fee_msg) = count_tickets_and_remainder(
            LICENSE_ADDR.load(deps.storage)?.to_string(),
            &info.funds,
            gd.ticket_cost,
            to_process,
        );

        msgs.extend([
            form_return_payment_overflow_msgs(&remaining_funds, &info.sender),
            dev_fee_msg,
        ]);

        let count = guest.reap.len();
        // save tickets reserved by the ticket wallet
        RESERVED_TICKETS.update(deps.storage, &gd.guest_weight, |a| match a {
            Some(mut td) => {
                td += count as u128;
                if td > gd.max_ticket_limit.into() {
                    return Err(ContractError::CannotReserveTicketCount {});
                }
                Ok::<u128, ContractError>(td)
            }
            None => {
                if reserved <= gd.max_ticket_limit.into() {
                    Ok(reserved)
                } else {
                    Err(ContractError::CannotReserveTicketCount {})
                }
            }
        })?;

        msgs.push(
            form_update_guestlist_msg(&guest.reap, gd.guest_weight, &cfg.event_guest_contract)?
                .into(),
        );
    }
    Ok(Response::new())
}

/// Entry point to checkin guests as event usher
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

        // if giving ticket to another external wallet,
        // create composite storage key to prevent unauthorized checkin for another wallet address

        // consume ticket
        // RESERVED_TICKETS.update(deps.storage, &checkin.ticket_addr, |a| match a {
        //     Some(mut td) => {
        //         if td == 0u128 {
        //             return Err(ContractError::GuestAlreadyCheckedIn {});
        //         }
        //         td -= 1;
        //         Ok::<u128, ContractError>(td)
        //     }
        //     None => Err(ContractError::NoReservedTicketsForGuest {}),
        // })?;

        // recurisvely update guest status for specific segment
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
                true => Err(ContractError::GuestAlreadyCheckedIn {}),
                false => Ok(true),
            }
        } else {
            Err(ContractError::GuestTypeIncorrect {})
        }
    })
}

fn refund_unconfirmed_ticket_purchase(
    _deps: DepsMut,
    _info: MessageInfo,
    _guest: Vec<String>,
) -> Result<Response, ContractError> {
    Ok(Response::new())
}

/// counts how many tickets are purchased, returning any overflow amounts sent.
fn count_tickets_and_remainder(
    dev_addr: String,
    funds_sent: &[Coin],
    gd_ticket_cost: Vec<Coin>,
    reap: &[RegisteringEventAddressAndPayment],
) -> (u128, Vec<Coin>, CosmosMsg) {
    let mut remaining_funds = funds_sent.to_vec();
    let mut total_tickets = 0;

    let mut dev_fee_coins = Vec::new();

    for guest in reap {
        let denom = guest.payment_asset.clone();

        // Find required payment amount for this denom
        if let Some(cost) = gd_ticket_cost.iter().find(|c| c.denom == denom) {
            // Find matching coin in remaining funds
            if let Some(fund) = remaining_funds.iter_mut().find(|c| c.denom == denom) {
                // Check if sufficient funds are available
                if fund.amount >= cost.amount {
                    // 3% flat fee
                    dev_fee_coins.push(coin(
                        cost.amount.multiply_ratio(3u128, 100u128).u128(),
                        fund.denom.to_string(),
                    ));
                    // Deduct payment and count ticket
                    fund.amount = fund.amount.checked_sub(cost.amount).unwrap();
                    total_tickets += 1;
                }
            }
        }
    }

    // Filter out zero-amount coins
    remaining_funds.retain(|coin| !coin.amount.is_zero());
    (
        total_tickets,
        remaining_funds,
        CosmosMsg::Bank(BankMsg::Send {
            to_address: dev_addr,
            amount: dev_fee_coins,
        }),
    )
}

fn form_update_guestlist_msg(
    guest_addrs: &[RegisteringEventAddressAndPayment],
    guest_weight: u64,
    gust_cw420: &Addr,
) -> Result<WasmMsg, ContractError> {
    Ok(WasmMsg::Execute {
        contract_addr: gust_cw420.to_string(),
        msg: to_json_binary(&cw420::msg::ExecuteMsg::UpdateMembers {
            remove: vec![],
            add: guest_addrs
                .iter()
                .map(|a| Member {
                    addr: a.ticket_addr.to_string(),
                    weight: guest_weight,
                })
                .collect(),
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
    // check if guest already is member
    let res: cw4::MemberResponse = deps.querier.query_wasm_smart(
        cw420,
        &cw420::msg::QueryMsg::Member {
            addr: wallet.to_string(),
            at_height: None,
        },
    )?;

    Ok(res.weight)
}

/// Entry point to claim funds sent for ticket payments
pub fn perform_claim_ticket_payments(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if info.sender != config.curator {
        return Err(ContractError::NotAnEventUsher {});
    }

    // retrive all tokens to query
    let tokens: Vec<String> = GUEST_DETAILS
        .range(deps.storage, None, None, Order::Ascending)
        .filter_map(|res| res.ok())
        .flat_map(|(_, guest_details)| guest_details.ticket_cost.into_iter().map(|c| c.denom))
        .fold(vec![], |mut acc, denom| {
            if !acc.contains(&denom) {
                acc.push(denom);
            }
            acc
        });

    let balances: Vec<Coin> = tokens
        .iter()
        .map(|denom| deps.querier.query_balance(&env.contract.address, denom))
        .collect::<StdResult<Vec<Coin>>>()?;

    Ok(Response::new().add_message(BankMsg::Send {
        to_address: config.curator.to_string(),
        amount: balances,
    }))
}
