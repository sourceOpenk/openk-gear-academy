use game_session_io::*;
use gear_core::ids::ProgramId;
use gtest::{constants, BlockRunResult, Log, Program, System};
use wordle::{BANK_OF_WORDS, WORD_LENGTH};

const USER: u64 = 3;

const WASM_TARGET_PATH: &str = "target/wasm32-unknown-unknown";
const BUILD_MODE: &str = if cfg!(debug_assertions) {
    "debug"
} else {
    "release"
};

fn wordle_program_path() -> std::path::PathBuf {
    std::path::PathBuf::from(WASM_TARGET_PATH)
        .join(BUILD_MODE)
        .join("wordle.opt.wasm")
}

fn setup_system() -> System {
    let system = System::new();
    system.init_logger();
    system.mint_to(USER, constants::EXISTENTIAL_DEPOSIT * 1000);
    system
}

fn setup_wordle_program(system: &System) -> Program {
    let wordle_program = Program::from_file(system, wordle_program_path());
    let message_id = wordle_program.send_bytes(USER, []);
    let block_run_result = system.run_next_block();
    assert!(block_run_result.succeed.contains(&message_id));
    wordle_program
}

fn setup_game_session_program(system: &System, wordle_program_id: ProgramId) -> Program {
    let game_session_program = Program::current(system);
    let message_id = game_session_program.send(USER, wordle_program_id);
    let block_run_result = system.run_next_block();
    assert!(block_run_result.succeed.contains(&message_id));
    game_session_program
}

fn start_game(system: &System, game_session_program: &Program) -> BlockRunResult {
    let message_id = game_session_program.send(USER, GameSessionAction::StartGame);
    let block_run_result = system.run_next_block();
    assert!(block_run_result.succeed.contains(&message_id));
    block_run_result
}

#[cfg(test)]
mod init {
    use crate::*;

    #[test]
    fn receives_and_store_the_wordle_programs_address() {
        let system = setup_system();
        let wordle_program = setup_wordle_program(&system);
        let game_session_program = setup_game_session_program(&system, wordle_program.id());

        let game_session: GameSession = game_session_program
            .read_state(())
            .expect("Read state failed");
        assert!(game_session.wordle_actor == wordle_program.id());
    }
}

#[cfg(test)]
mod handle {
    mod start_game {
        use crate::*;

        #[test]
        fn a_reply_is_sent_to_notify_the_user_that_the_game_has_beeen_successfully_started() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            let block_run_result = start_game(&system, &game_session_program);

            let logs = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameStarted);
            assert!(block_run_result.contains(&logs));
        }

        #[test]
        fn the_program_checks_if_a_game_already_exist_for_the_user() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let message_id = game_session_program.send(USER, GameSessionAction::StartGame);
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Game already started");
        }

        #[test]
        fn game_will_timeout_if_the_user_does_not_respond_in_time() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            system.run_to_block(system.block_height() + TIMEOUT);

            let mailbox = system.get_mailbox(USER);
            let log = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameTimeout);
            assert!(mailbox.contains(&log));
        }

        #[test]
        fn game_can_restart_after_timeout() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            system.run_to_block(system.block_height() + TIMEOUT);

            let mailbox = system.get_mailbox(USER);
            let log = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameTimeout);
            assert!(mailbox.contains(&log));

            let message_id = game_session_program.send(USER, GameSessionAction::StartGame);
            let block_run_result = system.run_next_block();
            assert!(block_run_result.succeed.contains(&message_id));

            let log = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameStarted);
            assert!(block_run_result.contains(&log));
        }
    }

    mod check_word {
        use crate::*;

        #[test]
        fn ensure_game_exist() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Game not exist");
        }

        #[test]
        fn ensure_game_not_in_win_status() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let mut game_over = false;

            for word in BANK_OF_WORDS.iter() {
                let message_id = game_session_program.send(
                    USER,
                    GameSessionAction::CheckWord {
                        word: word.to_string(),
                    },
                );
                let block_run_result = system.run_next_block();
                assert!(block_run_result.succeed.contains(&message_id));

                let log = block_run_result.decoded_log::<GameSessionEvent>();
                assert!(log.len() == 1);

                let event = log[0].payload();
                if let GameSessionEvent::GameOver {
                    status: GameOverStatus::Win,
                } = event
                {
                    game_over = true;
                    break;
                }
            }

            assert!(game_over);

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Game over");
        }

        #[test]
        fn ensure_game_not_in_lose_status() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let mut block_run_result = None;

            for _ in 0..MAX_ATTEMPTS {
                let message_id = game_session_program.send(
                    USER,
                    GameSessionAction::CheckWord {
                        word: "aaaaa".to_string(),
                    },
                );
                block_run_result = Some(system.run_next_block());
                assert!(block_run_result
                    .as_ref()
                    .unwrap()
                    .succeed
                    .contains(&message_id));
            }

            let log = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameOver {
                    status: GameOverStatus::Lose,
                });
            assert!(block_run_result.unwrap().contains(&log));

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Game over");
        }

        #[test]
        fn ensure_game_not_in_timeout_status() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            system.run_to_block(system.block_height() + TIMEOUT);

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Game over");
        }

        #[test]
        fn word_length_must_be_word_length() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result
                .assert_panicked_with(message_id, format!("Word length must be {}", WORD_LENGTH));
        }

        #[test]
        fn word_must_be_lowercase() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "AAAAA".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            block_run_result.assert_panicked_with(message_id, "Word must be lowercase");
        }

        #[test]
        fn check_word_return_workd_checked_event() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let message_id = game_session_program.send(
                USER,
                GameSessionAction::CheckWord {
                    word: "aaaaa".to_string(),
                },
            );
            let block_run_result = system.run_next_block();
            assert!(block_run_result.succeed.contains(&message_id));

            let log = block_run_result.decoded_log::<GameSessionEvent>();
            assert!(log.len() == 1);
            let event = log[0].payload();
            assert!(matches!(event, GameSessionEvent::WordChecked { .. }));
        }

        #[test]
        fn check_word_return_game_over_win_event() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let mut game_over = false;

            for word in BANK_OF_WORDS.iter() {
                let message_id = game_session_program.send(
                    USER,
                    GameSessionAction::CheckWord {
                        word: word.to_string(),
                    },
                );
                let block_run_result = system.run_next_block();
                assert!(block_run_result.succeed.contains(&message_id));

                let log = block_run_result.decoded_log::<GameSessionEvent>();
                assert!(log.len() == 1);

                let event = log[0].payload();
                if let GameSessionEvent::GameOver {
                    status: GameOverStatus::Win,
                } = event
                {
                    game_over = true;
                    break;
                }
            }

            assert!(game_over);
        }

        #[test]
        fn check_word_return_game_over_lose_event() {
            let system = setup_system();
            let wordle_program = setup_wordle_program(&system);
            let game_session_program = setup_game_session_program(&system, wordle_program.id());
            start_game(&system, &game_session_program);

            let mut block_run_result = None;

            for _ in 0..MAX_ATTEMPTS {
                let message_id = game_session_program.send(
                    USER,
                    GameSessionAction::CheckWord {
                        word: "aaaaa".to_string(),
                    },
                );
                block_run_result = Some(system.run_next_block());
                assert!(block_run_result
                    .as_ref()
                    .unwrap()
                    .succeed
                    .contains(&message_id));
            }

            let log = Log::builder()
                .source(game_session_program.id())
                .dest(USER)
                .payload(GameSessionEvent::GameOver {
                    status: GameOverStatus::Lose,
                });
            assert!(block_run_result.unwrap().contains(&log));
        }
    }
}
