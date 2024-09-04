use std::collections::BTreeMap;

use candid::Principal;
use rand::Rng;

use crate::pocket_ic_tests::{deploy_dummy_scheduler_canister, DummyTask};

thread_local! {
    static CANISTER: std::cell::RefCell<Principal> = const { std::cell::RefCell::new(Principal::anonymous()) };
}

#[tokio::test]
async fn test_should_remove_panicking_task() {
    // Arrange
    let test_ctx = deploy_dummy_scheduler_canister().await.unwrap();
    CANISTER.with_borrow_mut(|principal| *principal = test_ctx.dummy_scheduler_canister);

    let mut tasks = vec![
        DummyTask::GoodTask,
        DummyTask::FailTask,
        DummyTask::Panicking,
        DummyTask::GoodTask,
        DummyTask::GoodTask,
        DummyTask::GoodTask,
        DummyTask::GoodTask,
        DummyTask::GoodTask,
        DummyTask::GoodTask,
        DummyTask::Panicking,
        DummyTask::Panicking,
        DummyTask::Panicking,
        DummyTask::FailTask,
        DummyTask::FailTask,
        DummyTask::FailTask,
    ];

    for _ in 0..1000 {
        // Append a randomly selected task to the tasks vector
        let task = match rand::thread_rng().gen_range(0..=2) {
            // rand 0.8
            0 => DummyTask::Panicking,
            1 => DummyTask::FailTask,
            _ => DummyTask::GoodTask,
        };
        tasks.push(task);
    }

    let task_ids = test_ctx.schedule_tasks(tasks.clone()).await;

    let tasks_map: BTreeMap<u64, DummyTask> = task_ids
        .into_iter()
        .enumerate()
        .map(|(id, key)| (key, tasks[id]))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(tasks.len(), tasks_map.len());

    // Act
    for _ in 0..10 {
        test_ctx.run_scheduler().await;
        println!("Get task 0: {:?}", test_ctx.get_task(0).await);
        println!("Get task 1: {:?}", test_ctx.get_task(1).await);
        println!("Get task 2: {:?}", test_ctx.get_task(2).await);
    }

    // Assert
    let panicked_tasks = test_ctx.panicked_tasks().await;
    let completed_tasks = test_ctx.completed_tasks().await;
    let failed_tasks = test_ctx.failed_tasks().await;

    assert_eq!(
        panicked_tasks.len() + completed_tasks.len() + failed_tasks.len(),
        tasks_map.len()
    );

    compare(panicked_tasks, &tasks_map, DummyTask::Panicking);
    compare(completed_tasks, &tasks_map, DummyTask::GoodTask);
    compare(failed_tasks, &tasks_map, DummyTask::FailTask);
}

fn compare(mut found: Vec<u64>, tasks_map: &BTreeMap<u64, DummyTask>, expected_task: DummyTask) {
    let mut expected = tasks_map
        .iter()
        .filter(|(_, task)| task == &&expected_task)
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();

    assert_eq!(
        expected.len(),
        found.len(),
        "Task: {:?}, Expected: {:?}, Found: {:?}",
        expected_task,
        expected,
        found
    );
    expected.sort();
    found.sort();
    assert_eq!(
        expected, found,
        "Task: {:?}, Expected: {:?}, Found: {:?}",
        expected_task, expected, found
    );
}
