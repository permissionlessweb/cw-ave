use cosmwasm_std::{DecimalRangeExceeded, Instantiate2AddressError, StdError};
use cw_denom::DenomError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    ShitStd(#[from] StdError),

    #[error("{0}")]
    ShitDenomError(#[from] DenomError),

    #[error("{0}")]
    Instantiate2AddressError(#[from] Instantiate2AddressError),

    #[error("Cannot set duplicate guest weights.")]
    DuplicateGuestWeight {},

    #[error("You are trying to reserve more than the limit for a single wallet.")]
    CannotReserveTicketCount {},

    #[error("end of the previous event stage must come before the start of the next event stage.")]
    OverlappingEventDates {},

    #[error("event stage start must come before event stage end.")]
    InvalidEventDates {},

    #[error("No guest type exists.")]
    GuestTypeIncorrect {},

    #[error("you cannot register, this wallet is not an usher for this event.")]
    NotAnEventUsher {},

    #[error("guest has already checked in.")]
    GuestAlreadyCheckedIn {},
}
