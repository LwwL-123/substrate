/// Time and blocks.
pub mod time {
    use crate::{BlockNumber};

    /// This determines the average expected block time that we are targeting.
    /// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
    /// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
    /// up by `pallet_aura` to implement `fn slot_duration()`.
    ///
    /// Change this to adjust the block time.
    pub const MILLISECS_PER_BLOCK: u64 = 6000;

    // NOTE: Currently it is not possible to change the slot duration after the chain has started.
    // Attempting to do so will brick block production.
    pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

    // Time is measured by number of blocks.
    pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
    pub const HOURS: BlockNumber = MINUTES * 60;
    pub const DAYS: BlockNumber = HOURS * 24;

    pub const EPOCH_DURATION_IN_BLOCKS: BlockNumber = 1 * HOURS;

    pub const EPOCH_DURATION_IN_SLOTS: u64 = {
        const SLOT_FILL_RATE: f64 = MILLISECS_PER_BLOCK as f64 / SLOT_DURATION as f64;

        (EPOCH_DURATION_IN_BLOCKS as f64 * SLOT_FILL_RATE) as u64
    };
    pub const PRIMARY_PROBABILITY: (u64, u64) = (1, 4);
}

pub mod currency {
    use crate::Balance;


    pub const TTC: Balance = 1_000_000_000_000;  //10^12
    pub const DOLLARS: Balance = TTC;
    pub const CENTS: Balance = TTC / 100;        //10^10
    pub const MILLICENTS: Balance = CENTS / 1_000; //10^7


    pub const MILLI: Balance = TTC / 1000;       //10^9
    pub const MICRO: Balance = MILLI / 1000;     //10^6
    pub const NANO: Balance = MICRO / 1000;      //10^3
    pub const PICO: Balance = 1;                 //1

    // GPoS rewards in the first year
    pub const FIRST_YEAR_REWARDS: Balance = 5_000_000 * TTC;

}

pub mod staking {
    use crate::Balance;
    // The reward decrease ratio per year
    pub const REWARD_DECREASE_RATIO: (Balance, Balance) = (88, 100);
    // The minimal reward ratio
    pub const MIN_REWARD_RATIO: (Balance, Balance) = (28, 1000);
    // The start year for extra reward
    pub const EXTRA_REWARD_START_YEAR: u64 = 4;
}
