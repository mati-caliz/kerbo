use std::env;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use super::Workspace;

const SETTLE: Duration = Duration::from_millis(50);

fn same_directory_spelled_differently() -> (PathBuf, PathBuf) {
    let directory = env::current_dir().expect("hay directorio actual");
    let name = directory.file_name().expect("tiene nombre").to_owned();
    let respelled = directory.join("..").join(name);
    (directory, respelled)
}

#[test]
fn requests_for_the_same_directory_share_it_and_another_one_waits_its_turn() {
    let (directory, other) = same_directory_spelled_differently();
    let workspace = Workspace::default();
    let first = workspace.enter(&directory).expect("entra");
    let second = workspace
        .enter(&directory)
        .expect("entra al mismo directorio sin esperar");
    let entered = AtomicBool::new(false);
    thread::scope(|scope| {
        let waiting = scope.spawn(|| {
            let visit = workspace.enter(&other).expect("entra cuando se libera");
            entered.store(true, Ordering::SeqCst);
            drop(visit);
        });
        thread::sleep(SETTLE);
        assert!(!entered.load(Ordering::SeqCst));
        drop(first);
        thread::sleep(SETTLE);
        assert!(!entered.load(Ordering::SeqCst));
        drop(second);
        waiting.join().expect("el hilo termina");
    });
    assert!(entered.load(Ordering::SeqCst));
    assert_eq!(env::current_dir().expect("hay directorio actual"), directory);
}

#[test]
fn a_directory_that_does_not_exist_is_refused() {
    let workspace = Workspace::default();
    assert!(
        workspace
            .enter(&PathBuf::from("/kerbo-no-existe/en-ningun-lado"))
            .is_err()
    );
}
