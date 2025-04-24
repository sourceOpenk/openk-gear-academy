use gtest::{constants, Program, System};

const USER: u64 = 3;

#[test]
fn test() {
    let system = System::new();
    system.init_logger();
    system.mint_to(USER, constants::EXISTENTIAL_DEPOSIT * 1000);

    let game_session_program = Program::current(&system);
    let message_id = game_session_program.send(USER, ());
    let block_run_result = system.run_next_block();
    assert!(block_run_result.succeed.contains(&message_id));
}
