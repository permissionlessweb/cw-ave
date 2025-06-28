use cosmwasm_schema::{cw_serde, QueryResponses};
use cw20::Cw20ReceiveMsg;
use cw4::Member;

use crate::state::{CheckInDetails, Config, EventSegments, GuestDetails, RegisteringGuest};

#[cw_serde]
pub struct InstantiateMsg {
    /// if not set, sender
    pub event_curator: Option<String>,
    /// Dao one must be a member of to make use of shitstraps
    /// label for contract & front end
    pub title: String,
    /// description of shitstrap for recordkeeping
    pub description: String,
    /// list of admin keys able to manually modify event attendee contract
    pub usher_admins: Vec<Member>,
    pub guest_details: Vec<GuestDetails>,
    /// code-id of cw420 contract
    pub cw420: u64,
    pub event_timeline: Vec<EventSegments>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Cw20 Entry Point
    Receive(Cw20ReceiveMsg),
    PurchaseTickets {
        guests: Vec<RegisteringGuest>,
    },
    CheckInGuest {
        stage: u64,
        checkin: CheckInDetails,
    },
    RefundUnconfirmedTickets {
        guests: Vec<String>,
    },
}

#[cw_serde]
pub enum ReceiveMsg {
    PurchaseTickets { guests: Vec<RegisteringGuest> },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Returns max possible deposit value for a shit-strap instance
    #[returns(Config)]
    Config {},
    /// Returns max possible deposit value for a shit-strap instance
    #[returns(Vec<EventSegments>)]
    EventSegments {},
    /// Returns max possible deposit value for a shit-strap instance
    #[returns(bool)]
    GuestAttendanceStatus { guest: String, event_stage_id: u64 },
    #[returns(Vec<bool>)]
    GuestAttendanceStatusALL { guest: String },
}
