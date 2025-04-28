#![no_std]

use gmeta::{In, InOut, Metadata, Out};
use gstd::{string::String, vec::Vec, ActorId, Decode, Encode, MessageId, TypeInfo};
use wordle_io::Event as WordleEvent;

pub const MAX_ATTEMPTS: u8 = 6;
pub const TIMEOUT: u32 = 200;

pub struct GameSessionMetadata;

impl Metadata for GameSessionMetadata {
    type Init = In<ActorId>;
    type Handle = InOut<GameSessionAction, GameSessionEvent>;
    type Reply = ();
    type Others = ();
    type Signal = ();
    type State = Out<GameSession>;
}

#[derive(TypeInfo, Encode, Decode, Debug)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct GameSession {
    pub wordle_actor: ActorId,
    pub users_state: Vec<UserState>,
}

#[derive(TypeInfo, Encode, Decode, Debug)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub struct UserState {
    pub user: ActorId,
    pub status: Option<UserStatus>,
    pub wait_msg_id_user_to_game_session: Option<MessageId>,
    pub wait_msg_id_game_session_to_wordle: Option<MessageId>,
    pub wakeup_event: Option<WordleEvent>,
    pub check_word_msg_id: Option<MessageId>,
    pub attempts_remaining: u8,
}

#[derive(TypeInfo, Encode, Decode, Debug, PartialEq)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum UserStatus {
    Running,
    Win,
    Lose,
    Timeout,
}

#[derive(TypeInfo, Encode, Decode, Debug)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum GameSessionAction {
    StartGame,
    CheckWord { word: String },
    CheckGameStatus,
}

#[derive(TypeInfo, Encode, Decode, Debug)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum GameSessionEvent {
    GameStarted,
    GameTimeout,
    WordChecked {
        correct_positions: Vec<u8>,
        contained_in_word: Vec<u8>,
    },
    GameOver {
        status: GameOverStatus,
    },
}

#[derive(TypeInfo, Encode, Decode, Debug)]
#[codec(crate = gstd::codec)]
#[scale_info(crate = gstd::scale_info)]
pub enum GameOverStatus {
    Win,
    Lose,
    Timeout,
}
