use cosmwasm_schema::{cw_serde, QueryResponses};
use cw4::Member;

use crate::state::{
    CheckInDetails, Config, EventSegment, GuestDetails, RegisteringGuest, TicketPaymentOption,
};

#[cw_serde]
pub struct InstantiateMsg {
    /// if not set, sender
    pub event_curator: String,
    /// label for contract & front end
    pub title: String,
    /// description of avEvent for recordkeeping
    pub description: String,
    /// list of admin keys able to checkin guests
    pub usher_admins: Vec<Member>,
    /// details of each type of guest attendees can participate as
    pub guest_details: Vec<GuestDetails>,
    /// code-id of cw420 contract
    pub cw420: u64,
    /// timeline of events segments
    pub event_timeline: Vec<EventSegment>,
}

#[cw_serde]
#[derive(cw_orch::ExecuteFns)]
pub enum ExecuteMsg {
    // /// Cw20 Entry Point
    // Receive(Cw20ReceiveMsg),
    PurchaseTickets { guests: Vec<RegisteringGuest> },
    CheckInGuest { checkin: CheckInDetails },
    RefundUnconfirmedTickets { guests: Vec<String> },
    ClaimTicketPayments {},
}

#[cw_serde]
pub enum ReceiveMsg {
    PurchaseTickets { guests: Vec<RegisteringGuest> },
}

#[cw_serde]
#[derive(cw_orch::QueryFns, QueryResponses)]
pub enum QueryMsg {
    /// returns basic details regarding this event
    #[returns(Config)]
    Config {},
    /// All segements for this event
    #[returns(Vec<EventSegmentRes>)]
    EventSegments {},
    /// Details of a specific type of guest able to participate in event as.
    #[returns(GuestDetails)]
    GuestTypeDetailsByWeight { guest_weight: u64 },
    /// All details of types of guest
    #[returns(Vec<GuestDetails>)]
    GuestTypeDetailsAll {},
    /// returns whether or not a guest has checked in for a specific segment of this event
    #[returns(bool)]
    GuestAttendanceStatus { guest: String, event_stage_id: u64 },
    #[returns(Vec<bool>)]
    /// Checkin status for a single guest, for all stages of this event
    GuestAttendanceStatusAll { guest: String },
    /// All payment options accepted for a given ticket type
    #[returns(TicketPaymentOption)]
    TicketPaymentOptionsByGuestWeight { guest_weight: u64 },
    /// All payment options available
    #[returns(Vec<TicketPaymentOption>)]
    AllTicketPaymentOptions {},
}

#[cw_serde]
pub struct EventSegmentRes {
    pub seg_id: u64,
    pub segment: EventSegment,
}

#[cw_serde]
pub struct MigrateMsg {}
