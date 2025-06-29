use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Checksum, Coin, Timestamp, Uint128};
use cw_storage_plus::{Item, Map};

pub const CONFIG: Item<Config> = Item::new("c");
pub const GUEST_DETAILS: Map<u64, GuestDetails> = Map::new("gd");
pub const GUEST_WEIGHT_BY_TYPE: Map<&String, u64> = Map::new("gwbt");
pub const RESERVED_TICKETS: Map<&String, TicketDetails> = Map::new("rt");
pub const EVENT_STAGES: Map<&String, EventSegments> = Map::new("es");

pub const ATTENDANCE_RECORD: Map<(&String, u64), bool> = Map::new("rt");
#[cw_serde]
pub struct Config {
    pub title: String,
    pub curator: String,
    pub event_usher_contract: Addr,
    pub event_guest_contract: Addr,
}

#[cw_serde]
pub struct RegisteringGuest {
    /// label specific to type of guest a user is purchasing a ticket for
    pub guest_type: String,
    /// the ephemeral wallet being used for this specific event
    pub ticket_wallet: String,
}

#[cw_serde]
pub struct CheckInDetails {
    /// label specific to type of guest a user is purchasing a ticket for
    pub signature: Binary,
    pub ticket_addr: String,
    /// cosmos_sdk_proto::Any of the pubkey that generated the signature
    pub pubkey: Binary,
}

/// Defines timelengths of a specific stage of an event.
/// For example, a private screening could have 2 shows, so we define the start and end for both.
#[cw_serde]
pub struct EventSegments {
    pub stage_description: String,
    pub start: Timestamp,
    pub end: Timestamp,
}

#[cw_serde]
pub struct TicketDetails {
    pub reserved: u128,
}

#[cw_serde]
pub struct TicketPaymentOptionResponse {
    pub guest_type: String,
    pub payment_options: Vec<Coin>,
}

#[cw_serde]
pub struct GuestDetails {
    /// label specific to type of guest
    pub guest_type: String,
    /// weight used in cw420 to distinguish guest types
    pub guest_weight: u64,
    /// limit to number of tickets a guest can purchase
    pub max_ticket_limit: u32,
    /// limit to number of this type of guests
    // pub max_guest_limit: u32,
    // pub overbooking_limit: u32,
    /// array of coins accepted for ticket
    pub ticket_cost: Vec<Coin>,
    pub event_segment_access: EventSegmentAccessType,
}

pub fn generate_instantiate_salt2(checksum: &Checksum, namespace: &[u8]) -> Binary {
    let mut hash = Vec::new();
    hash.extend_from_slice(checksum.as_slice());
    let checksum_hash = <sha2::Sha256 as sha2::Digest>::digest(hash);
    let mut result = checksum_hash.to_vec();
    result.extend_from_slice(namespace);
    Binary::new(result)
}

#[cosmwasm_schema::cw_serde]
pub enum EventSegmentAccessType {
    // guest once checked into one segment is checked into all segments
    AllSegments {},
    // 
    SpecificSegments { ids: Vec<String> },
}
