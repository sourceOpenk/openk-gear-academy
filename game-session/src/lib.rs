#![no_std]

use game_session_io::*;
use gstd::{exec, msg, prelude::*, ActorId};
use wordle_io::{Action as WordleAction, Event as WordleEvent};

static mut GAME_SESSION: Option<GameSession> = None;

#[no_mangle]
extern "C" fn init() {
    let wordle_actor: ActorId = msg::load().expect("Unable to decode");

    unsafe {
        GAME_SESSION = Some(GameSession {
            wordle_actor,
            users_state: Vec::new(),
        });
    }
}

fn process_start_game_first_half(game_session: &mut GameSession) {
    let msg_id = msg::send(
        game_session.wordle_actor,
        WordleAction::StartGame {
            user: msg::source(),
        },
        0,
    )
    .expect("Error in sending message");

    let existing_user_index = game_session
        .users_state
        .iter()
        .position(|x| x.user == msg::source());

    if let Some(index) = existing_user_index {
        game_session.users_state[index] = UserState {
            user: msg::source(),
            status: None,
            wait_msg_id_user_to_game_session: Some(msg::id()),
            wait_msg_id_game_session_to_wordle: Some(msg_id),
            wakeup_event: None,
            check_word_msg_id: None,
            attempts_remaining: MAX_ATTEMPTS,
        };
    } else {
        game_session.users_state.push(UserState {
            user: msg::source(),
            status: None,
            wait_msg_id_user_to_game_session: Some(msg::id()),
            wait_msg_id_game_session_to_wordle: Some(msg_id),
            wakeup_event: None,
            check_word_msg_id: None,
            attempts_remaining: MAX_ATTEMPTS,
        });
    }

    exec::wait();
}

fn process_start_game_second_half(user_state: &mut UserState) {
    user_state.wait_msg_id_user_to_game_session = None;
    user_state.wait_msg_id_game_session_to_wordle = None;
    user_state.status = Some(UserStatus::Running);
    msg::reply(GameSessionEvent::GameStarted, 0).expect("Error in sending reply");
    let message_id = msg::send_delayed(
        exec::program_id(),
        GameSessionAction::CheckGameStatus,
        0,
        TIMEOUT,
    )
    .expect("Error in sending delayed");
    user_state.check_word_msg_id = Some(message_id);
}

fn process_game_already_started() {
    panic!("Game already started");
}

fn start_game_action(game_session: &mut GameSession) {
    let user_index = game_session
        .users_state
        .iter()
        .position(|x| x.user == msg::source());

    if user_index.is_none() {
        process_start_game_first_half(game_session);
    } else {
        let user_state = &mut game_session.users_state[user_index.unwrap()];

        if user_state.wait_msg_id_game_session_to_wordle.is_some() {
            process_start_game_second_half(user_state);
        } else if [UserStatus::Win, UserStatus::Lose, UserStatus::Timeout]
            .contains(user_state.status.as_ref().unwrap())
        {
            process_start_game_first_half(game_session);
        } else {
            process_game_already_started();
        }
    }
}

fn check_word_action(game_session: &mut GameSession, word: String) {
    let user_state = game_session
        .users_state
        .iter_mut()
        .find(|x| x.user == msg::source());

    if user_state.is_none() {
        panic!("Game not exist");
    }
    let user_state = user_state.unwrap();
    let status = user_state.status.as_ref().unwrap();

    if [UserStatus::Win, UserStatus::Lose, UserStatus::Timeout].contains(status) {
        panic!("Game over");
    }
    if word.len() != 5 {
        panic!("Word length must be 5");
    }
    if word != word.to_lowercase() {
        panic!("Word must be lowercase");
    }

    if user_state.wait_msg_id_game_session_to_wordle.is_none() {
        let message_id = msg::send(
            game_session.wordle_actor,
            WordleAction::CheckWord {
                user: msg::source(),
                word,
            },
            0,
        )
        .expect("Error in sending message");
        user_state.wait_msg_id_user_to_game_session = Some(msg::id());
        user_state.wait_msg_id_game_session_to_wordle = Some(message_id);
        exec::wait();
    } else {
        user_state.wait_msg_id_user_to_game_session = None;
        user_state.wait_msg_id_game_session_to_wordle = None;

        if let Some(WordleEvent::WordChecked {
            user,
            correct_positions,
            contained_in_word,
        }) = user_state.wakeup_event.clone()
        {
            assert!(user == msg::source());

            if correct_positions.len() == 5 {
                user_state.status = Some(UserStatus::Win);
                msg::reply(
                    GameSessionEvent::GameOver {
                        status: GameOverStatus::Win,
                    },
                    0,
                )
                .expect("Error in sending reply");
            } else {
                user_state.attempts_remaining -= 1;

                if user_state.attempts_remaining == 0 {
                    user_state.status = Some(UserStatus::Lose);
                    msg::reply(
                        GameSessionEvent::GameOver {
                            status: GameOverStatus::Lose,
                        },
                        0,
                    )
                    .expect("Error in sending reply");
                } else {
                    msg::reply(
                        GameSessionEvent::WordChecked {
                            correct_positions,
                            contained_in_word,
                        },
                        0,
                    )
                    .expect("Error in sending reply");
                }
            }
        } else {
            panic!("Invalid wakeup event");
        }
    }
}

fn check_game_status_action(game_session: &mut GameSession) {
    if exec::program_id() == msg::source() {
        let user_state = game_session
            .users_state
            .iter_mut()
            .find(|x| x.check_word_msg_id.unwrap() == msg::id())
            .unwrap();
        user_state.status = Some(UserStatus::Timeout);
        user_state.check_word_msg_id = None;
        msg::send(user_state.user, GameSessionEvent::GameTimeout, 0)
            .expect("Error in sending message");
    }
}

#[no_mangle]
extern "C" fn handle() {
    let action: GameSessionAction = msg::load().expect("Error in loading action");
    #[allow(static_mut_refs)]
    let game_session = unsafe { GAME_SESSION.as_mut().unwrap() };

    match action {
        GameSessionAction::StartGame => start_game_action(game_session),
        GameSessionAction::CheckWord { word } => check_word_action(game_session, word),
        GameSessionAction::CheckGameStatus => check_game_status_action(game_session),
    }
}

#[no_mangle]
extern "C" fn handle_reply() {
    let event: WordleEvent = msg::load().expect("Error in loading event");
    #[allow(static_mut_refs)]
    let game_session = unsafe { GAME_SESSION.as_mut().unwrap() };
    let reply_to = msg::reply_to().unwrap();
    let user_state = game_session
        .users_state
        .iter_mut()
        .find(|x| x.wait_msg_id_game_session_to_wordle == Some(reply_to));

    if user_state.is_some() {
        let user_state = user_state.unwrap();
        user_state.wakeup_event = Some(event);
        exec::wake(user_state.wait_msg_id_user_to_game_session.unwrap()).expect("Error in waking");
    }
}

#[no_mangle]
extern "C" fn state() {
    #[allow(static_mut_refs)]
    let game_session = unsafe { GAME_SESSION.as_ref().unwrap() };
    msg::reply(game_session, 0).expect("Error in sending reply");
}
