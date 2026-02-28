// tests/scenario_systems.rs

//! Ordex (OrdexArena) Practical Use Case: Systems Programming
//!
//! This scenario demonstrates how to utilize Ordex in real-world, system-level
//! architectures such as Compiler ASTs, Network Routing, and Task Scheduling.

use ordex::prelude::*;

// =====================================================================
// Scenario A: Compiler AST (Abstract Syntax Tree) Manipulation
// =====================================================================
#[derive(Debug)]
struct AstNode {
    op: &'static str,
    value: Option<i32>,
}

#[test]
fn scenario_ast_constant_folding() {
    let mut ast = OrdexArena::new();

    let left_node = ast.insert(AstNode {
        op: "Literal",
        value: Some(10),
    });
    let right_node = ast.insert(AstNode {
        op: "Literal",
        value: Some(20),
    });
    let parent_node = ast.insert(AstNode {
        op: "Add",
        value: None,
    });

    // Safely mutate the parent based on its children without Rc<RefCell<T>> overhead.
    align!(
        ast,
        parent_node,
        left_node,
        right_node,
        |parent, left, right| {
            if let (Some(l_val), Some(r_val)) = (left.value, right.value) {
                parent.value = Some(l_val + r_val);
                parent.op = "FoldedLiteral";
            }
        }
    );

    assert_eq!(ast.get(parent_node).unwrap().value, Some(30));
    assert_eq!(ast.get(parent_node).unwrap().op, "FoldedLiteral");
}

// =====================================================================
// Scenario B: WebSocket / Network Session Multiplexing
// =====================================================================
#[derive(Debug)]
struct Connection {
    client_id: &'static str,
    outbox: Vec<String>,
}

#[test]
fn scenario_network_multicast() {
    let mut router = OrdexArena::new();

    let conn1 = router.insert(Connection {
        client_id: "User_A",
        outbox: vec![],
    });
    let conn2 = router.insert(Connection {
        client_id: "User_B",
        outbox: vec![],
    });
    let conn3 = router.insert(Connection {
        client_id: "User_C",
        outbox: vec![],
    });

    // Dynamic subset targeting
    let target_connections = vec![conn3, conn1];
    let verified_route = VerifiedIndices::new(target_connections);

    // Multicast the payload safely to the exact subset using ordex.
    router.ordex(&verified_route, |mut iter| {
        while let Some(session) = iter.next() {
            session.outbox.push("Warning: System Rebooting".to_string());
        }
    });

    // Verify outbox length AND read client_id to satisfy the compiler
    let c1 = router.get(conn1).unwrap();
    assert_eq!(c1.outbox.len(), 1);
    assert_eq!(c1.client_id, "User_A");

    let c2 = router.get(conn2).unwrap();
    assert_eq!(c2.outbox.len(), 0); // Not targeted

    let c3 = router.get(conn3).unwrap();
    assert_eq!(c3.outbox.len(), 1);
    assert_eq!(c3.client_id, "User_C");
}

// =====================================================================
// Scenario C: Async Task Scheduler / Worker Pool
// =====================================================================
#[derive(Debug, PartialEq)]
enum TaskState {
    Pending,
    Running,
}

#[derive(Debug)]
struct Task {
    name: &'static str,
    state: TaskState,
}

#[test]
fn scenario_async_task_cancellation() {
    let mut task_pool = OrdexArena::new();

    let task1 = task_pool.insert(Task {
        name: "Data_Fetch",
        state: TaskState::Pending,
    });
    let task2 = task_pool.insert(Task {
        name: "Image_Render",
        state: TaskState::Running,
    });

    // Simulation: A worker thread completes a task and removes it.
    task_pool.remove(task2);

    let cancel_requests = vec![task1, task2];
    let mut successfully_canceled = 0;

    for target_idx in cancel_requests {
        // Use `get_mut()` as a safe diagnostic probe since existence is uncertain.
        if let Some(task) = task_pool.get_mut(target_idx) {
            task.state = TaskState::Pending;
            successfully_canceled += 1;
        }
    }

    assert_eq!(successfully_canceled, 1); // Only task1 was successfully accessed

    // Read state AND name to satisfy the compiler
    let alive_task = task_pool.get(task1).unwrap();
    assert_eq!(alive_task.state, TaskState::Pending);
    assert_eq!(alive_task.name, "Data_Fetch");
}
